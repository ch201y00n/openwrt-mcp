//! Closed local-NTFS protected read and private audit-log profiles.
mod log;
mod native;
mod policy;
pub(crate) use super::unsupported::{SystemLog, system_log_supported, verify_openwrt_local};
use crate::HostError;
pub(crate) use log::PrivateLog;
use std::path::Path;
use zeroize::Zeroizing;
pub(crate) fn native_file_protection_supported() -> bool {
    true
}
pub(crate) fn private_log_supported() -> bool {
    true
}
pub(crate) fn read_config(path: &Path, max_bytes: usize) -> Result<Vec<u8>, HostError> {
    let mut bytes = native::read(path, max_bytes, false)?;
    Ok(std::mem::take(&mut *bytes))
}
pub(crate) fn read_secret(path: &Path, max_bytes: usize) -> Result<Zeroizing<Vec<u8>>, HostError> {
    native::read(path, max_bytes, true)
}
