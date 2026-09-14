use super::{MutationError, SecretPurpose};
use zeroize::Zeroizing;

pub struct SecretReference(String);
impl SecretReference {
    /// Operator-owned alias, not a path or client-selected provider.
    pub fn new(alias: String) -> Result<Self, MutationError> {
        if alias.is_empty()
            || alias.len() > 64
            || !alias
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        {
            return Err(MutationError::Invalid);
        }
        Ok(Self(alias))
    }
    pub fn alias(&self) -> &str {
        &self.0
    }
}
pub struct SecretValue {
    bytes: Zeroizing<Vec<u8>>,
    purpose: SecretPurpose,
}
impl SecretValue {
    pub fn new(bytes: Zeroizing<Vec<u8>>, purpose: SecretPurpose) -> Result<Self, MutationError> {
        if bytes.is_empty() || bytes.len() > 65536 {
            return Err(MutationError::Limit);
        }
        Ok(Self { bytes, purpose })
    }
    pub fn expose_for(&self, purpose: SecretPurpose) -> Result<&[u8], MutationError> {
        if self.purpose != purpose {
            return Err(MutationError::Invalid);
        }
        Ok(&self.bytes)
    }
}
