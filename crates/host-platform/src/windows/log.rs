//! Safe facade; concrete handles and native operations stay in the SDK bridge.
use crate::HostError;
use std::path::Path;

pub(super) trait LogWriter: Send {
    fn write(&mut self, bytes: &[u8]) -> Result<(), HostError>;
}

pub(crate) struct PrivateLog {
    writer: Option<Box<dyn LogWriter>>,
    max_bytes: u64,
}

impl PrivateLog {
    pub(crate) fn open(path: &Path, max_bytes: u64, retained: usize) -> Result<Self, HostError> {
        if max_bytes == 0 || max_bytes > 1_073_741_824 || !(1..=100).contains(&retained) {
            return Err(HostError::Limit);
        }
        Ok(Self {
            writer: Some(super::native::open_log(path, max_bytes, retained)?),
            max_bytes,
        })
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        let writer = self.writer.as_mut().ok_or(HostError::Unavailable)?;
        let result = if bytes.len() as u64 > self.max_bytes {
            Err(HostError::Limit)
        } else {
            writer.write(bytes)
        };
        if result.is_err() {
            self.writer.take();
        }
        result
    }
}
