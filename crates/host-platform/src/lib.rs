//! Purpose-specific host security. Unsupported OS protection never silently downgrades.

mod error;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "linux"))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
use unsupported as platform;
#[cfg(target_os = "windows")]
use windows as platform;

pub use error::HostError;
use std::path::Path;
use zeroize::Zeroizing;

pub fn native_file_protection_supported() -> bool {
    platform::native_file_protection_supported()
}
pub fn system_log_supported() -> bool {
    platform::system_log_supported()
}
pub fn private_log_supported() -> bool {
    platform::private_log_supported()
}

pub fn read_config(path: &Path, max_bytes: usize) -> Result<Vec<u8>, HostError> {
    if max_bytes == 0 || max_bytes > 1024 * 1024 {
        return Err(HostError::Limit);
    }
    platform::read_config(path, max_bytes)
}

pub fn read_secret(path: &Path, max_bytes: usize) -> Result<Zeroizing<Vec<u8>>, HostError> {
    if max_bytes == 0 || max_bytes > 16 * 1024 * 1024 {
        return Err(HostError::Limit);
    }
    platform::read_secret(path, max_bytes)
}

pub fn verify_openwrt_local() -> Result<(), HostError> {
    platform::verify_openwrt_local()
}

pub struct PrivateLog {
    inner: platform::PrivateLog,
}

impl PrivateLog {
    pub fn open(path: &Path, max_bytes: u64, retained: usize) -> Result<Self, HostError> {
        if max_bytes == 0 || max_bytes > 1_073_741_824 || !(1..=100).contains(&retained) {
            return Err(HostError::Limit);
        }
        Ok(Self {
            inner: platform::PrivateLog::open(path, max_bytes, retained)?,
        })
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        self.inner.write(bytes)
    }
}

pub struct SystemLog {
    inner: platform::SystemLog,
}

impl SystemLog {
    pub fn open() -> Result<Self, HostError> {
        Ok(Self {
            inner: platform::SystemLog::open()?,
        })
    }
    pub fn send(&self, bytes: &[u8]) -> Result<(), HostError> {
        self.inner.send(bytes)
    }
}
