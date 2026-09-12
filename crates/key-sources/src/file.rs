use openwrt_mcp_host_platform::{HostError, read_secret};
use openwrt_mcp_runtime::protection::{KeyMaterial, ProtectionError};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileProtection {
    Restricted,
    PersonalVault,
}

/// A platform integration must actually enforce its advertised protection.
pub trait ProtectedFileAccess: Send + Sync {
    fn read(
        &self,
        path: &Path,
        protection: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError>;
}

pub struct NativeFileAccess;

impl ProtectedFileAccess for NativeFileAccess {
    fn read(
        &self,
        path: &Path,
        protection: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError> {
        match protection {
            FileProtection::PersonalVault => Err(ProtectionError::UnsupportedProtection),
            FileProtection::Restricted => {
                let bytes = read_secret(path, max_bytes).map_err(|error| match error {
                    HostError::InvalidPath => ProtectionError::InvalidConfig,
                    HostError::Unsupported | HostError::Insecure => {
                        ProtectionError::UnsupportedProtection
                    }
                    HostError::Unavailable => ProtectionError::SourceUnavailable,
                    HostError::Limit => ProtectionError::ResourceLimit,
                })?;
                KeyMaterial::from_zeroizing(bytes, max_bytes)
            }
        }
    }
}
