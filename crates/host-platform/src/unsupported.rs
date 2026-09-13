use crate::HostError;
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use zeroize::Zeroizing;

#[cfg(not(target_os = "windows"))]
pub(crate) fn native_file_protection_supported() -> bool {
    false
}
pub(crate) fn private_log_supported() -> bool {
    false
}
pub(crate) fn system_log_supported() -> bool {
    false
}
#[cfg(not(target_os = "windows"))]
pub(crate) fn read_config(_: &Path, _: usize) -> Result<Vec<u8>, HostError> {
    Err(HostError::Unsupported)
}
#[cfg(not(target_os = "windows"))]
pub(crate) fn read_secret(_: &Path, _: usize) -> Result<Zeroizing<Vec<u8>>, HostError> {
    Err(HostError::Unsupported)
}
pub(crate) fn verify_openwrt_local() -> Result<(), HostError> {
    Err(HostError::Unsupported)
}

pub(crate) struct PrivateLog;
impl PrivateLog {
    pub(crate) fn open(_: &Path, _: u64, _: usize) -> Result<Self, HostError> {
        Err(HostError::Unsupported)
    }
    pub(crate) fn write(&mut self, _: &[u8]) -> Result<(), HostError> {
        Err(HostError::Unsupported)
    }
}
pub(crate) struct SystemLog;
impl SystemLog {
    pub(crate) fn open() -> Result<Self, HostError> {
        Err(HostError::Unsupported)
    }
    pub(crate) fn send(&self, _: &[u8]) -> Result<(), HostError> {
        Err(HostError::Unsupported)
    }
}
