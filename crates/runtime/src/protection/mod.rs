//! Internal key custody and stream protection ports, deliberately absent from MCP.
//! No filesystem/environment/OS handles are opened by this module.

mod error;
mod limits;
mod material;
mod session;

pub use error::ProtectionError;
pub use limits::{CryptoLimits, KeyLimits};
pub use material::{IdentityMaterial, KeyMaterial, RecipientMaterial};
pub use session::{DecryptionSession, EncryptionSession};

use std::io::{Read, Write};

pub trait KeySource: Send + Sync {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError>;
}

pub trait KeyContainer: Send + Sync {
    fn read_entry(
        &self,
        container: &KeyMaterial,
        entry: &str,
        limits: &KeyLimits,
    ) -> Result<KeyMaterial, ProtectionError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CryptoReport {
    pub input_bytes: u64,
    pub output_bytes: u64,
}

pub trait EncryptionProvider: Send + Sync {
    fn format_id(&self) -> &'static str;
    fn encrypt(
        &self,
        recipients: &RecipientMaterial,
        input: &mut dyn Read,
        output: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError>;
}

pub trait DecryptionProvider: Send + Sync {
    fn format_id(&self) -> &'static str;
    /// Output is untrusted staging until the entire stream succeeds. On error the
    /// caller must discard staging, including plaintext already written there.
    fn decrypt_to_staging(
        &self,
        identity: &IdentityMaterial,
        input: &mut dyn Read,
        staging: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError>;
}
