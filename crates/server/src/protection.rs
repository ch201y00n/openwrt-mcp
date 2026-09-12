//! Operator-only protection wiring. Validation and construction do not read keys.
//! These primitives are not backup/restore MCP tools or a publication transaction.

use openwrt_mcp_crypto_age::AgeX25519;
use openwrt_mcp_key_sources::{SourceConfig, SourceRegistry};
use openwrt_mcp_runtime::protection::{
    CryptoLimits, DecryptionSession, EncryptionSession, KeyLimits, ProtectionError,
};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cipher {
    #[default]
    Age,
}

/// Locations are trusted local configuration; there is no inline key field.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectionConfig {
    #[serde(default)]
    pub cipher: Cipher,
    pub recipient_source: String,
    pub identity_source: Option<String>,
    pub sources: BTreeMap<String, SourceConfig>,
    #[serde(default)]
    pub key_limits: KeyLimits,
    #[serde(default)]
    pub crypto_limits: CryptoLimits,
}

impl ProtectionConfig {
    fn registry(&self) -> Result<SourceRegistry, ProtectionError> {
        self.crypto_limits.validate()?;
        let registry = SourceRegistry::new(self.sources.clone(), self.key_limits)?;
        registry.resolve(&self.recipient_source)?;
        if let Some(identity) = &self.identity_source {
            // Disallow accidental reuse of the exact binding for both purposes.
            if identity == &self.recipient_source {
                return Err(ProtectionError::InvalidConfig);
            }
            registry.resolve(identity)?;
        }
        Ok(registry)
    }

    /// Offline: check references and bounds, without opening any source.
    pub fn validate(&self) -> Result<(), ProtectionError> {
        self.registry().map(|_| ())
    }

    /// Creates only public-recipient authority; private material is never loaded.
    pub fn encryption(&self) -> Result<EncryptionSession, ProtectionError> {
        let registry = self.registry()?;
        let source = registry.resolve(&self.recipient_source)?;
        match self.cipher {
            Cipher::Age => EncryptionSession::new(
                source,
                Arc::new(AgeX25519),
                self.key_limits,
                self.crypto_limits,
            ),
        }
    }

    /// Separate opt-in identity authority. The caller must protect and discard
    /// failed staging and must not apply plaintext before full authentication.
    pub fn decryption(&self) -> Result<Option<DecryptionSession>, ProtectionError> {
        let registry = self.registry()?;
        let Some(identity) = &self.identity_source else {
            return Ok(None);
        };
        let source = registry.resolve(identity)?;
        match self.cipher {
            Cipher::Age => DecryptionSession::new(
                source,
                Arc::new(AgeX25519),
                self.key_limits,
                self.crypto_limits,
            )
            .map(Some),
        }
    }
}
