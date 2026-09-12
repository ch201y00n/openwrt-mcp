use super::ProtectionError;
use std::fmt::{Debug, Formatter};
use zeroize::Zeroizing;

const MAX_MATERIAL_BYTES: usize = 16 * 1024 * 1024;

/// Owned secret bytes: no Clone, serialization, deref, or implicit text conversion.
pub struct KeyMaterial(Zeroizing<Vec<u8>>);

impl KeyMaterial {
    pub fn new(bytes: Vec<u8>, max_bytes: usize) -> Result<Self, ProtectionError> {
        Self::from_zeroizing(Zeroizing::new(bytes), max_bytes)
    }

    pub fn from_zeroizing(
        bytes: Zeroizing<Vec<u8>>,
        max_bytes: usize,
    ) -> Result<Self, ProtectionError> {
        if max_bytes == 0 || max_bytes > MAX_MATERIAL_BYTES {
            return Err(ProtectionError::InvalidConfig);
        }
        if bytes.len() > max_bytes {
            return Err(ProtectionError::ResourceLimit);
        }
        if bytes.is_empty() {
            return Err(ProtectionError::InvalidMaterial);
        }
        Ok(Self(bytes))
    }

    /// Explicit sensitive access for the selected source/container/cipher adapter.
    pub fn expose_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Debug for KeyMaterial {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("KeyMaterial([REDACTED])")
    }
}

#[derive(Debug)]
pub struct RecipientMaterial(KeyMaterial);

impl RecipientMaterial {
    pub fn new(material: KeyMaterial) -> Self {
        Self(material)
    }

    pub fn expose_bytes(&self) -> &[u8] {
        self.0.expose_bytes()
    }
}

#[derive(Debug)]
pub struct IdentityMaterial(KeyMaterial);

impl IdentityMaterial {
    pub fn new(material: KeyMaterial) -> Self {
        Self(material)
    }

    pub fn expose_bytes(&self) -> &[u8] {
        self.0.expose_bytes()
    }
}
