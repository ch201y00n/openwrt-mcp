//! Ciphertext-only pre-provisioned local-file profile, not native ACL protection.
use crate::HostError;
use std::{
    fs::{File, OpenOptions, symlink_metadata},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path},
};
const MAX_BYTES: u64 = 33562688;
const CHUNK: usize = 65536;

pub struct LockedCiphertextFile {
    file: File,
    failed: bool,
}
impl LockedCiphertextFile {
    /// Operator must provision a stable, durable filename on a controlled local
    /// filesystem. No namespace is created, repaired or removed by this API.
    pub fn open(path: &Path) -> Result<Self, HostError> {
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.components().any(|c| matches!(c, Component::ParentDir))
        {
            return Err(HostError::InvalidPath);
        }
        for ancestor in path.ancestors() {
            let meta = symlink_metadata(ancestor).map_err(|_| HostError::Unavailable)?;
            if meta.file_type().is_symlink() || (ancestor != path && !meta.is_dir()) {
                return Err(HostError::Insecure);
            }
        }
        if !symlink_metadata(path)
            .map_err(|_| HostError::Unavailable)?
            .is_file()
        {
            return Err(HostError::Insecure);
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|_| HostError::Unavailable)?;
        file.try_lock().map_err(|_| HostError::Unavailable)?;
        let meta = file.metadata().map_err(|_| HostError::Unavailable)?;
        if !meta.is_file() || meta.len() > MAX_BYTES {
            return Err(HostError::Insecure);
        }
        Ok(Self {
            file,
            failed: false,
        })
    }
    pub fn size(&self) -> Result<u64, HostError> {
        if self.failed {
            return Err(HostError::Unavailable);
        }
        let size = self
            .file
            .metadata()
            .map_err(|_| HostError::Unavailable)?
            .len();
        if size > MAX_BYTES {
            Err(HostError::Limit)
        } else {
            Ok(size)
        }
    }
    pub fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> Result<(), HostError> {
        if bytes.len() > CHUNK
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > self.size().unwrap_or(0))
        {
            return Err(HostError::Limit);
        }
        self.file
            .seek(SeekFrom::Start(offset))
            .and_then(|_| self.file.read_exact(bytes))
            .map_err(|_| HostError::Unavailable)
    }
    pub fn append(&mut self, expected_size: u64, bytes: &[u8]) -> Result<(), HostError> {
        if bytes.is_empty()
            || bytes.len() > CHUNK
            || self.size()? != expected_size
            || expected_size
                .checked_add(bytes.len() as u64)
                .is_none_or(|n| n > MAX_BYTES)
        {
            return Err(HostError::Limit);
        }
        // Failure latches before any potentially partial write.
        self.failed = true;
        self.file
            .seek(SeekFrom::End(0))
            .and_then(|_| self.file.write_all(bytes))
            .map_err(|_| HostError::Unavailable)?;
        if self
            .file
            .metadata()
            .map_err(|_| HostError::Unavailable)?
            .len()
            != expected_size + bytes.len() as u64
        {
            return Err(HostError::Unavailable);
        }
        self.failed = false;
        Ok(())
    }
    pub fn synchronize(&mut self) -> Result<(), HostError> {
        if self.failed {
            return Err(HostError::Unavailable);
        }
        self.failed = true;
        self.file.sync_all().map_err(|_| HostError::Unavailable)?;
        self.failed = false;
        Ok(())
    }
}
