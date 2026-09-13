//! Handle-free sealing over trusted pre-bound ports, not a backup receipt or permit.
//! No production workflow consumes this module until a separate integration review.

mod budget;
mod cipher;
mod model;
mod ports;
mod stream;
mod workflow;

pub use model::{
    CheckCounts, CipherCounts, Cleanup, CleanupOutcome, Publication, PublishOutcome, SealFailure,
    SealFailureReport, SealLimits, SealPortError, SealedCounts, StageCapabilities,
};
pub use ports::{SealCheck, SealCipher, SealClock, SealSource, SealStage};
pub use workflow::ArchiveSealer;
