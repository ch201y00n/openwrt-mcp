pub(super) const CHUNK: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealLimits {
    pub source_bytes: u64,
    pub ciphertext_bytes: u64,
    pub timeout_ms: u64,
}

impl Default for SealLimits {
    fn default() -> Self {
        Self {
            source_bytes: 67_108_864,
            ciphertext_bytes: 68_157_440,
            timeout_ms: 30_000,
        }
    }
}

impl SealLimits {
    pub(super) fn validate(self) -> Result<(), SealFailure> {
        if self.source_bytes == 0
            || self.source_bytes > 75_497_472
            || self.ciphertext_bytes == 0
            || self.ciphertext_bytes > 76_546_048
            || self.timeout_ms == 0
            || self.timeout_ms > 300_000
        {
            return Err(SealFailure::InvalidLimits);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckCounts {
    pub source_bytes: u64,
    pub expanded_bytes: u64,
    pub payload_bytes: u64,
    pub files: u64,
}

impl CheckCounts {
    pub(super) fn validate(self, source: u64) -> Result<(), SealFailure> {
        if self.source_bytes != source {
            return Err(SealFailure::CountMismatch);
        }
        if self.files == 0
            || self.files > 4096
            || self.expanded_bytes == 0
            || self.expanded_bytes > 75_497_472
            || self.payload_bytes > 67_108_864
            || self.payload_bytes > self.expanded_bytes
        {
            return Err(SealFailure::InvalidSummary);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CipherCounts {
    pub input_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealedCounts {
    pub source_bytes: u64,
    pub ciphertext_bytes: u64,
    pub expanded_bytes: u64,
    pub payload_bytes: u64,
    pub files: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageCapabilities {
    pub private_staging: bool,
    pub atomic_publication: bool,
    pub durability: bool,
}

/// The port must report uncertainty if publication may have happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    Published,
    NotPublished,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publication {
    NotAttempted,
    NotPublished,
    Published,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupOutcome {
    Cleaned,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    NotRequired,
    Cleaned,
    Unknown,
}

impl From<CleanupOutcome> for Cleanup {
    fn from(value: CleanupOutcome) -> Self {
        match value {
            CleanupOutcome::Cleaned => Self::Cleaned,
            CleanupOutcome::Unknown => Self::Unknown,
        }
    }
}

/// Intentionally carries no provider diagnostics or nested I/O error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealPortError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealFailure {
    InvalidLimits,
    StageUnsupported,
    SourceRead,
    SourceCompletion,
    Check,
    Cipher,
    StageWrite,
    StageFlush,
    SourceLimit,
    CiphertextLimit,
    InvalidIoCount,
    ZeroWrite,
    IncompleteInput,
    EmptyStream,
    CountMismatch,
    InvalidSummary,
    Deadline,
    ClockReversed,
    PublicationNotConfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SealFailureReport {
    pub cause: SealFailure,
    pub publication: Publication,
    pub source_cleanup: Cleanup,
    pub stage_cleanup: Cleanup,
}
