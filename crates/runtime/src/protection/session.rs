use super::{
    CryptoLimits, CryptoReport, DecryptionProvider, EncryptionProvider, IdentityMaterial,
    KeyLimits, KeySource, ProtectionError, RecipientMaterial,
};
use std::{io::Read, io::Write, sync::Arc};

/// An encryption-only deployment never needs an identity source.
pub struct EncryptionSession {
    recipients: Arc<dyn KeySource>,
    provider: Arc<dyn EncryptionProvider>,
    keys: KeyLimits,
    crypto: CryptoLimits,
}

impl EncryptionSession {
    pub fn new(
        recipients: Arc<dyn KeySource>,
        provider: Arc<dyn EncryptionProvider>,
        keys: KeyLimits,
        crypto: CryptoLimits,
    ) -> Result<Self, ProtectionError> {
        keys.validate()?;
        crypto.validate()?;
        Ok(Self {
            recipients,
            provider,
            keys,
            crypto,
        })
    }

    pub fn format_id(&self) -> &'static str {
        self.provider.format_id()
    }

    pub fn encrypt(
        &self,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<CryptoReport, ProtectionError> {
        let material = self.recipients.read(self.keys.max_key_bytes)?;
        if material.expose_bytes().len() > self.keys.max_key_bytes {
            return Err(ProtectionError::ResourceLimit);
        }
        self.provider.encrypt(
            &RecipientMaterial::new(material),
            input,
            output,
            &self.crypto,
        )
    }
}

pub struct DecryptionSession {
    identities: Arc<dyn KeySource>,
    provider: Arc<dyn DecryptionProvider>,
    keys: KeyLimits,
    crypto: CryptoLimits,
}

impl DecryptionSession {
    pub fn new(
        identities: Arc<dyn KeySource>,
        provider: Arc<dyn DecryptionProvider>,
        keys: KeyLimits,
        crypto: CryptoLimits,
    ) -> Result<Self, ProtectionError> {
        keys.validate()?;
        crypto.validate()?;
        Ok(Self {
            identities,
            provider,
            keys,
            crypto,
        })
    }

    pub fn format_id(&self) -> &'static str {
        self.provider.format_id()
    }

    /// Discard all staging on any failure, even when it already contains output.
    pub fn decrypt_to_staging(
        &self,
        input: &mut dyn Read,
        staging: &mut dyn Write,
    ) -> Result<CryptoReport, ProtectionError> {
        let material = self.identities.read(self.keys.max_key_bytes)?;
        if material.expose_bytes().len() > self.keys.max_key_bytes {
            return Err(ProtectionError::ResourceLimit);
        }
        self.provider.decrypt_to_staging(
            &IdentityMaterial::new(material),
            input,
            staging,
            &self.crypto,
        )
    }
}
