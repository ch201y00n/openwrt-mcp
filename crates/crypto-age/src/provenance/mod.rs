//! Algorithm only, independent of key custody and age recipient/identity roles.
use hmac::{Hmac, Mac};
use openwrt_mcp_runtime::{
    backups::{BackupError, MacPurpose, RecordAuthenticator},
    protection::KeyMaterial,
};
use sha2::Sha256;
use zeroize::Zeroizing;

pub struct HmacSha256 {
    key: Zeroizing<[u8; 32]>,
}
impl HmacSha256 {
    pub fn new(material: KeyMaterial) -> Result<Self, BackupError> {
        if material.expose_bytes().len() != 32 {
            return Err(BackupError::Invalid);
        }
        let mut key = Zeroizing::new([0; 32]);
        key.copy_from_slice(material.expose_bytes());
        Ok(Self { key })
    }
    fn mac(&self, purpose: MacPurpose, parts: &[&[u8]]) -> Result<Hmac<Sha256>, BackupError> {
        if parts.len() > 4 || parts.iter().any(|p| p.len() > 1048576) {
            return Err(BackupError::Limit);
        }
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&*self.key).map_err(|_| BackupError::Invalid)?;
        mac.update(match purpose {
            MacPurpose::StoreHeader => b"openwrt-mcp/store-header/v1\0",
            MacPurpose::ArtifactRecord => b"openwrt-mcp/artifact-record/v1\0",
        });
        for part in parts {
            mac.update(&(part.len() as u64).to_be_bytes());
            mac.update(part);
        }
        Ok(mac)
    }
}
impl RecordAuthenticator for HmacSha256 {
    fn tag(&self, purpose: MacPurpose, parts: &[&[u8]]) -> Result<[u8; 32], BackupError> {
        Ok(self.mac(purpose, parts)?.finalize().into_bytes().into())
    }
    fn verify(&self, purpose: MacPurpose, parts: &[&[u8]], tag: &[u8]) -> Result<(), BackupError> {
        self.mac(purpose, parts)?
            .verify_slice(tag)
            .map_err(|_| BackupError::Integrity)
    }
}
