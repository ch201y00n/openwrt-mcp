use super::{
    CheckCounts, CipherCounts, CleanupOutcome, PublishOutcome, SealPortError, SealedCounts,
    StageCapabilities,
};
use std::io::{Read, Write};

/// EOF and successful producer completion are separate obligations.
pub trait SealSource: Read {
    fn complete(&mut self) -> Result<u64, SealPortError>;
    /// Cleanup must not panic; failure to prove cleanup returns Unknown.
    fn cancel(&mut self) -> CleanupOutcome;
}

pub trait SealCheck {
    fn feed(&mut self, bytes: &[u8]) -> Result<(), SealPortError>;
    fn finish(self: Box<Self>) -> Result<CheckCounts, SealPortError>;
}

/// A trusted compiled encryption implementation, not an untrusted plugin.
pub trait SealCipher {
    fn encrypt(
        &self,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<CipherCounts, SealPortError>;
}

/// A pre-bound private ciphertext destination. No path or artifact handle crosses
/// this port; adapters retain their own correlation handle for reconciliation.
pub trait SealStage: Write {
    fn capabilities(&self) -> StageCapabilities;
    fn publish(&mut self, counts: SealedCounts) -> PublishOutcome;
    /// Abort only private, definitely unpublished staging. Must not panic.
    fn abort(&mut self) -> CleanupOutcome;
}

/// Cooperative deadline source. Implementations must be monotonic, non-panicking
/// and quick; this API cannot cancel a blocked provider call.
pub trait SealClock {
    fn now_ms(&self) -> u64;
}
