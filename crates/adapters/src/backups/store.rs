use openwrt_mcp_host_platform::ciphertext::LockedCiphertextFile;
use openwrt_mcp_runtime::{
    backups::{
        ArtifactBinding, BINDING_BYTES, BackupError, CiphertextRecordIo, MAX_CIPHERTEXT,
        MAX_RECORDS, MAX_STORE_BYTES, MacPurpose, RecordAuthenticator, RecordPublication,
    },
    mutation_ports::WorkBudget,
};
use std::{collections::BTreeSet, path::Path, sync::Arc};

const HEADER: usize = 64;
const RECORD_HEADER: usize = 8 + BINDING_BYTES + 8;
const TAG: usize = 32;
const CHUNK: usize = 65536;

pub struct NativeRecordIo(LockedCiphertextFile);
impl NativeRecordIo {
    pub fn open(path: &Path) -> Result<Self, BackupError> {
        LockedCiphertextFile::open(path)
            .map(Self)
            .map_err(|_| BackupError::Unavailable)
    }
}
impl CiphertextRecordIo for NativeRecordIo {
    fn size(&self) -> Result<u64, BackupError> {
        self.0.size().map_err(|_| BackupError::Unavailable)
    }
    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> Result<(), BackupError> {
        self.0
            .read_at(offset, bytes)
            .map_err(|_| BackupError::Unavailable)
    }
    fn append(&mut self, expected_size: u64, bytes: &[u8]) -> Result<(), BackupError> {
        self.0
            .append(expected_size, bytes)
            .map_err(|_| BackupError::Unknown)
    }
    fn synchronize(&mut self) -> Result<(), BackupError> {
        self.0.synchronize().map_err(|_| BackupError::Unknown)
    }
}

/// Header bytes for explicit offline operator provisioning, not store creation.
/// Contains only public random ID/version/tag; never embeds its authentication key.
pub fn provisioning_header(
    store: [u8; 16],
    authenticator: &dyn RecordAuthenticator,
) -> Result<[u8; HEADER], BackupError> {
    if store == [0; 16] {
        return Err(BackupError::Invalid);
    }
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(b"OMCPST01");
    header[8..24].copy_from_slice(&store);
    header[24..32].copy_from_slice(b"recd0001");
    let tag = authenticator.tag(MacPurpose::StoreHeader, &[&header[..32]])?;
    header[32..].copy_from_slice(&tag);
    Ok(header)
}

pub struct RecordStore {
    io: Box<dyn CiphertextRecordIo>,
    auth: Arc<dyn RecordAuthenticator>,
    store: [u8; 16],
    uncertain: bool,
}
struct Scan {
    size: u64,
    count: usize,
    matched: Option<Vec<u8>>,
}
impl RecordStore {
    pub fn open(
        path: &Path,
        store: [u8; 16],
        auth: Arc<dyn RecordAuthenticator>,
        budget: &WorkBudget,
    ) -> Result<Self, BackupError> {
        budget.check()?;
        Self::from_io(Box::new(NativeRecordIo::open(path)?), store, auth, budget)
    }
    /// Trusted pre-bound I/O injection; useful for precise failure tests.
    pub fn from_io(
        io: Box<dyn CiphertextRecordIo>,
        store: [u8; 16],
        auth: Arc<dyn RecordAuthenticator>,
        budget: &WorkBudget,
    ) -> Result<Self, BackupError> {
        let mut result = Self {
            io,
            auth,
            store,
            uncertain: false,
        };
        result.scan(None, budget)?;
        Ok(result)
    }
    fn read(
        &mut self,
        mut offset: u64,
        bytes: &mut [u8],
        budget: &WorkBudget,
    ) -> Result<(), BackupError> {
        for chunk in bytes.chunks_mut(CHUNK) {
            budget.check()?;
            self.io.read_at(offset, chunk)?;
            budget.check()?;
            offset += chunk.len() as u64;
        }
        Ok(())
    }
    fn scan(
        &mut self,
        expected: Option<&ArtifactBinding>,
        budget: &WorkBudget,
    ) -> Result<Scan, BackupError> {
        budget.check()?;
        let size = self.io.size()?;
        if !(HEADER as u64..=MAX_STORE_BYTES).contains(&size) {
            return Err(BackupError::Integrity);
        }
        if expected.is_some_and(|binding| binding.store() != self.store) {
            return Err(BackupError::Integrity);
        }
        let mut header = [0; HEADER];
        self.read(0, &mut header, budget)?;
        if &header[..8] != b"OMCPST01"
            || header[8..24] != self.store
            || &header[24..32] != b"recd0001"
            || self.store == [0; 16]
        {
            return Err(BackupError::Integrity);
        }
        self.auth
            .verify(MacPurpose::StoreHeader, &[&header[..32]], &header[32..])?;
        budget.check()?;
        let mut position = HEADER as u64;
        let mut jobs = BTreeSet::new();
        let mut matched = None;
        while position < size {
            if jobs.len() >= MAX_RECORDS || size - position < (RECORD_HEADER + TAG) as u64 {
                return Err(BackupError::Integrity);
            }
            let mut header = [0; RECORD_HEADER];
            self.read(position, &mut header, budget)?;
            if &header[..8] != b"OMCPBK01" {
                return Err(BackupError::Integrity);
            }
            let binding = ArtifactBinding::decode(&header[8..8 + BINDING_BYTES])?;
            let count = u64::from_be_bytes(
                header[8 + BINDING_BYTES..]
                    .try_into()
                    .map_err(|_| BackupError::Integrity)?,
            );
            if binding.store() != self.store
                || !jobs.insert(
                    <[u8; 16]>::try_from(binding.job()).map_err(|_| BackupError::Integrity)?,
                )
                || !(1..=MAX_CIPHERTEXT as u64).contains(&count)
                || count + (RECORD_HEADER + TAG) as u64 > size - position
            {
                return Err(BackupError::Integrity);
            }
            let mut ciphertext = vec![0; count as usize];
            self.read(position + RECORD_HEADER as u64, &mut ciphertext, budget)?;
            let mut tag = [0; TAG];
            self.read(position + RECORD_HEADER as u64 + count, &mut tag, budget)?;
            self.auth
                .verify(MacPurpose::ArtifactRecord, &[&header, &ciphertext], &tag)?;
            budget.check()?;
            if let Some(expected) = expected
                && expected.job() == binding.job()
            {
                if expected != &binding {
                    return Err(BackupError::Integrity);
                }
                matched = Some(ciphertext);
            }
            position += (RECORD_HEADER + TAG) as u64 + count;
        }
        if self.io.size()? != size {
            return Err(BackupError::Integrity);
        }
        budget.check()?;
        Ok(Scan {
            size,
            count: jobs.len(),
            matched,
        })
    }
    pub(crate) fn publish(
        &mut self,
        binding: &ArtifactBinding,
        ciphertext: &[u8],
        budget: &WorkBudget,
    ) -> RecordPublication {
        if self.uncertain {
            return RecordPublication::Unknown;
        }
        let scan = match self.scan(Some(binding), budget) {
            Ok(scan) => scan,
            Err(_) => {
                self.uncertain = true;
                return RecordPublication::Unknown;
            }
        };
        if scan.matched.is_some()
            || scan.count >= MAX_RECORDS
            || ciphertext.is_empty()
            || ciphertext.len() > MAX_CIPHERTEXT
            || !ciphertext.starts_with(b"age-encryption.org/v1\n")
        {
            return RecordPublication::NotPublished;
        }
        let mut header = [0; RECORD_HEADER];
        header[..8].copy_from_slice(b"OMCPBK01");
        header[8..8 + BINDING_BYTES].copy_from_slice(binding.encoded());
        header[8 + BINDING_BYTES..].copy_from_slice(&(ciphertext.len() as u64).to_be_bytes());
        let tag = match self
            .auth
            .tag(MacPurpose::ArtifactRecord, &[&header, ciphertext])
        {
            Ok(tag) => tag,
            Err(_) => return RecordPublication::NotPublished,
        };
        if budget.check().is_err() {
            return RecordPublication::NotPublished;
        }
        self.uncertain = true;
        let mut position = scan.size;
        for part in [header.as_slice(), ciphertext, &tag] {
            for chunk in part.chunks(CHUNK) {
                if budget.check().is_err()
                    || self.io.append(position, chunk).is_err()
                    || budget.check().is_err()
                {
                    return RecordPublication::Unknown;
                }
                position += chunk.len() as u64;
            }
        }
        if self.io.size().ok() != Some(position)
            || self.io.synchronize().is_err()
            || budget.check().is_err()
        {
            return RecordPublication::Unknown;
        }
        self.uncertain = false;
        RecordPublication::Durable
    }
    pub fn reconcile(
        &mut self,
        binding: &ArtifactBinding,
        budget: &WorkBudget,
    ) -> RecordPublication {
        let Ok(scan) = self.scan(Some(binding), budget) else {
            return RecordPublication::Unknown;
        };
        if scan.matched.is_none()
            || budget.check().is_err()
            || self.io.synchronize().is_err()
            || budget.check().is_err()
        {
            return RecordPublication::Unknown;
        }
        self.uncertain = false;
        RecordPublication::Durable
    }
    pub(crate) fn authenticated_bytes(
        &mut self,
        binding: &ArtifactBinding,
        budget: &WorkBudget,
    ) -> Result<Vec<u8>, BackupError> {
        self.scan(Some(binding), budget)?
            .matched
            .ok_or(BackupError::Unknown)
    }
}
