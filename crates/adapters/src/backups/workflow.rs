use super::{RecordStore, archive::Check};
use openwrt_mcp_runtime::{
    backups::{
        ArtifactBinding, BackupError, BackupInspection, CapturedArchive, MAX_CIPHERTEXT,
        MAX_PLAINTEXT, RecordPublication,
    },
    mutation_ports::WorkBudget,
    protection::DecryptionSession,
    sealing::{
        ArchiveSealer, CleanupOutcome, Publication, PublishOutcome, SealCheck, SealCipher,
        SealLimits, SealPortError, SealSource, SealStage, SealedCounts, StageCapabilities,
    },
};
use std::io::{Cursor, Error, Read, Write};
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupFailure {
    pub cause: BackupError,
    pub publication: RecordPublication,
}
fn stream_error() -> Error {
    Error::other("backup_stream_failed")
}
struct Source {
    data: Cursor<Zeroizing<Vec<u8>>>,
    budget: WorkBudget,
}
impl Read for Source {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        self.budget.check().map_err(|_| stream_error())?;
        self.data.read(output)
    }
}
impl SealSource for Source {
    fn complete(&mut self) -> Result<u64, SealPortError> {
        self.budget.check().map_err(|_| SealPortError)?;
        Ok(self.data.get_ref().len() as u64)
    }
    fn cancel(&mut self) -> CleanupOutcome {
        self.data.get_mut().zeroize();
        CleanupOutcome::Cleaned
    }
}
struct Memory {
    bytes: Zeroizing<Vec<u8>>,
    maximum: usize,
    budget: WorkBudget,
}
impl Write for Memory {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.budget.check().map_err(|_| stream_error())?;
        if bytes.len() > 65536 || bytes.len() > self.maximum - self.bytes.len() {
            return Err(stream_error());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.budget.check().map_err(|_| stream_error())
    }
}
struct Stage<'a> {
    memory: Memory,
    store: &'a mut RecordStore,
    binding: ArtifactBinding,
}
impl Write for Stage<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.memory.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.memory.flush()
    }
}
impl SealStage for Stage<'_> {
    fn capabilities(&self) -> StageCapabilities {
        StageCapabilities {
            private_staging: true,
            atomic_publication: true,
            durability: true,
        }
    }
    fn publish(&mut self, counts: SealedCounts) -> PublishOutcome {
        if counts.source_bytes != self.binding.source_bytes()
            || counts.payload_bytes != self.binding.file_bytes()
            || counts.files != 1
            || counts.ciphertext_bytes != self.memory.bytes.len() as u64
        {
            return PublishOutcome::NotPublished;
        }
        match self
            .store
            .publish(&self.binding, &self.memory.bytes, &self.memory.budget)
        {
            RecordPublication::Durable => PublishOutcome::Published,
            RecordPublication::NotPublished => PublishOutcome::NotPublished,
            RecordPublication::Unknown => PublishOutcome::Unknown,
        }
    }
    fn abort(&mut self) -> CleanupOutcome {
        self.memory.bytes.zeroize();
        CleanupOutcome::Cleaned
    }
}

pub fn seal_capture(
    capture: CapturedArchive,
    cipher: &dyn SealCipher,
    store: &mut RecordStore,
    budget: WorkBudget,
) -> Result<BackupInspection, BackupFailure> {
    let before = |cause| BackupFailure {
        cause,
        publication: RecordPublication::NotPublished,
    };
    budget.check().map_err(BackupError::from).map_err(before)?;
    let (binding, bytes) = capture.into_parts();
    let check = Box::new(Check::new(&binding, budget.clone()).map_err(before)?);
    let mut source = Source {
        data: Cursor::new(bytes),
        budget: budget.clone(),
    };
    let mut stage = Stage {
        memory: Memory {
            bytes: Zeroizing::new(Vec::with_capacity(MAX_CIPHERTEXT)),
            maximum: MAX_CIPHERTEXT,
            budget: budget.clone(),
        },
        store,
        binding,
    };
    let sealer = ArchiveSealer::new(SealLimits {
        source_bytes: MAX_PLAINTEXT as u64,
        ciphertext_bytes: MAX_CIPHERTEXT as u64,
        timeout_ms: 30000,
    })
    .map_err(|_| before(BackupError::Invalid))?;
    let counts = sealer
        .seal(&mut source, check, cipher, &mut stage)
        .map_err(|report| BackupFailure {
            cause: budget
                .check()
                .err()
                .map(BackupError::from)
                .unwrap_or(BackupError::Integrity),
            publication: match report.publication {
                Publication::Published => RecordPublication::Durable,
                Publication::Unknown => RecordPublication::Unknown,
                Publication::NotAttempted | Publication::NotPublished => {
                    RecordPublication::NotPublished
                }
            },
        })?;
    Ok(BackupInspection {
        source_bytes: counts.source_bytes,
        payload_bytes: counts.payload_bytes,
        files: counts.files,
    })
}

pub fn restore_inspect(
    store: &mut RecordStore,
    binding: &ArtifactBinding,
    decryptor: &DecryptionSession,
    budget: WorkBudget,
) -> Result<BackupInspection, BackupError> {
    budget.check()?;
    let ciphertext = store.authenticated_bytes(binding, &budget)?;
    let expected_ciphertext = ciphertext.len() as u64;
    let mut input = Cursor::new(ciphertext);
    let mut staging = Memory {
        bytes: Zeroizing::new(Vec::with_capacity(MAX_PLAINTEXT)),
        maximum: binding.source_bytes() as usize,
        budget: budget.clone(),
    };
    let report = decryptor
        .decrypt_to_staging(&mut input, &mut staging)
        .map_err(|_| BackupError::Integrity)?;
    budget.check()?;
    if input.position() != expected_ciphertext
        || report.input_bytes != expected_ciphertext
        || report.output_bytes != binding.source_bytes()
        || staging.bytes.len() as u64 != binding.source_bytes()
    {
        return Err(BackupError::Integrity);
    }
    // No parser or caller receives staging until complete cryptographic success.
    let mut check = Box::new(Check::new(binding, budget.clone())?);
    for chunk in staging.bytes.chunks(65536) {
        check.feed(chunk).map_err(|_| BackupError::Integrity)?;
    }
    let counts = check.finish().map_err(|_| BackupError::Integrity)?;
    budget.check()?;
    Ok(BackupInspection {
        source_bytes: counts.source_bytes,
        payload_bytes: counts.payload_bytes,
        files: counts.files,
    })
}
