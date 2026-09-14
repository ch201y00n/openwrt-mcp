//! Non-authorizing infrastructure evidence, never a client capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupError {
    Invalid,
    Limit,
    Cancelled,
    Deadline,
    Busy,
    Unavailable,
    Integrity,
    Unknown,
}
impl From<crate::mutation_ports::MutationError> for BackupError {
    fn from(error: crate::mutation_ports::MutationError) -> Self {
        use crate::mutation_ports::MutationError;
        match error {
            MutationError::Cancelled => Self::Cancelled,
            MutationError::Deadline => Self::Deadline,
            MutationError::Busy => Self::Busy,
            MutationError::Limit => Self::Limit,
            MutationError::Invalid => Self::Invalid,
            MutationError::Integrity => Self::Integrity,
            MutationError::Unknown => Self::Unknown,
            MutationError::Unavailable => Self::Unavailable,
        }
    }
}
use zeroize::Zeroizing;

pub const MAX_PLAINTEXT: usize = 131072;
pub const MAX_CIPHERTEXT: usize = 1048576;
pub const MAX_STORE_BYTES: u64 = 33562688;
pub const MAX_RECORDS: usize = 32;
pub const BINDING_BYTES: usize = 88;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Tar,
    Gzip,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactBinding {
    encoded: [u8; BINDING_BYTES],
}
impl ArtifactBinding {
    /// Compiled trusted wiring only. This is data, not admission/authorization.
    pub fn system(
        store: [u8; 16],
        target: [u8; 16],
        boot: [u8; 16],
        job: [u8; 16],
        file_bytes: u64,
        source_bytes: u64,
        format: ArchiveFormat,
    ) -> Result<Self, BackupError> {
        if [store, target, boot, job].contains(&[0; 16])
            || file_bytes > 65536
            || source_bytes == 0
            || source_bytes > MAX_PLAINTEXT as u64
        {
            return Err(BackupError::Invalid);
        }
        let mut encoded = [0; BINDING_BYTES];
        for (index, id) in [store, target, boot, job].iter().enumerate() {
            encoded[index * 16..index * 16 + 16].copy_from_slice(id);
        }
        encoded[64..72].copy_from_slice(&file_bytes.to_be_bytes());
        encoded[72..80].copy_from_slice(&source_bytes.to_be_bytes());
        encoded[80] = match format {
            ArchiveFormat::Tar => 0,
            ArchiveFormat::Gzip => 1,
        };
        encoded[81..].copy_from_slice(b"SYSv001");
        Ok(Self { encoded })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, BackupError> {
        if bytes.len() != BINDING_BYTES || &bytes[81..] != b"SYSv001" {
            return Err(BackupError::Integrity);
        }
        let id = |start| {
            <[u8; 16]>::try_from(&bytes[start..start + 16]).map_err(|_| BackupError::Integrity)
        };
        let number = |start| {
            <[u8; 8]>::try_from(&bytes[start..start + 8])
                .map(u64::from_be_bytes)
                .map_err(|_| BackupError::Integrity)
        };
        let format = match bytes[80] {
            0 => ArchiveFormat::Tar,
            1 => ArchiveFormat::Gzip,
            _ => return Err(BackupError::Integrity),
        };
        Self::system(
            id(0)?,
            id(16)?,
            id(32)?,
            id(48)?,
            number(64)?,
            number(72)?,
            format,
        )
    }
    pub fn encoded(&self) -> &[u8; BINDING_BYTES] {
        &self.encoded
    }
    pub fn store(&self) -> &[u8] {
        &self.encoded[..16]
    }
    pub fn target(&self) -> &[u8] {
        &self.encoded[16..32]
    }
    pub fn job(&self) -> &[u8] {
        &self.encoded[48..64]
    }
    pub fn file_bytes(&self) -> u64 {
        u64::from_be_bytes(self.encoded[64..72].try_into().expect("fixed binding"))
    }
    pub fn source_bytes(&self) -> u64 {
        u64::from_be_bytes(self.encoded[72..80].try_into().expect("fixed binding"))
    }
    pub fn format(&self) -> ArchiveFormat {
        if self.encoded[80] == 0 {
            ArchiveFormat::Tar
        } else {
            ArchiveFormat::Gzip
        }
    }
}

pub struct CapturedArchive {
    binding: ArtifactBinding,
    bytes: Zeroizing<Vec<u8>>,
}
impl CapturedArchive {
    /// Trusted transport must independently prove EOF, zero exit and close before
    /// constructing this value. Structure is still untrusted until sealing.
    pub fn completed(
        binding: ArtifactBinding,
        bytes: Zeroizing<Vec<u8>>,
        producer_bytes: u64,
    ) -> Result<Self, BackupError> {
        if bytes.len() as u64 != binding.source_bytes()
            || producer_bytes != binding.source_bytes()
            || bytes.len() > MAX_PLAINTEXT
        {
            return Err(BackupError::Integrity);
        }
        Ok(Self { binding, bytes })
    }
    pub fn binding(&self) -> &ArtifactBinding {
        &self.binding
    }
    pub fn into_parts(self) -> (ArtifactBinding, Zeroizing<Vec<u8>>) {
        (self.binding, self.bytes)
    }
}

#[derive(Clone, Copy)]
pub enum MacPurpose {
    StoreHeader,
    ArtifactRecord,
}
pub trait RecordAuthenticator: Send + Sync {
    fn tag(&self, purpose: MacPurpose, parts: &[&[u8]]) -> Result<[u8; 32], BackupError>;
    fn verify(&self, purpose: MacPurpose, parts: &[&[u8]], tag: &[u8]) -> Result<(), BackupError>;
}

/// Pre-bound ciphertext-only I/O; no path, key or arbitrary command crosses it.
pub trait CiphertextRecordIo: Send {
    fn size(&self) -> Result<u64, BackupError>;
    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> Result<(), BackupError>;
    fn append(&mut self, expected_size: u64, bytes: &[u8]) -> Result<(), BackupError>;
    fn synchronize(&mut self) -> Result<(), BackupError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupInspection {
    pub source_bytes: u64,
    pub payload_bytes: u64,
    pub files: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordPublication {
    Durable,
    NotPublished,
    Unknown,
}
