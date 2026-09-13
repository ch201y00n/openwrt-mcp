//! One supplied gzip member containing the strict regular-tar profile.
//! Integrity means corruption detection, not authenticity or backup authority.
//! Owned buffers zeroize; the backend history is NOT guaranteed scrubbed.
mod header;
mod stream;

use crate::archive::ArchiveError;
pub use stream::GzipArchiveValidator;

pub const MAX_CHUNK_BYTES: usize = 65536;
pub const MAX_SOURCE_BYTES: u64 = 72 * 1024 * 1024;
pub const MAX_HEADER_BYTES: usize = 4096;
pub const MAX_EXTRA_BYTES: usize = 4084;
pub const MAX_NAME_BYTES: usize = 1024;
pub const MAX_COMMENT_BYTES: usize = 1024;
pub const OUTPUT_BUFFER_BYTES: usize = 4096;
pub const MAX_EXPANSION_RATIO: u64 = 128;
pub const EXPANSION_SLACK_BYTES: u64 = 1024 * 1024;

/// Complete structure/corruption checks only; never a backup receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GzipArchiveSummary {
    pub compressed_bytes: u64,
    pub files: usize,
    pub payload_bytes: u64,
    pub archive_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GzipError {
    InvalidHeader,
    InvalidData,
    LimitExceeded,
    IncompleteStream,
    TrailingData,
    InvalidArchive,
}
impl GzipError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidHeader => "invalid_gzip_header",
            Self::InvalidData => "invalid_gzip_data",
            Self::LimitExceeded => "gzip_limit_exceeded",
            Self::IncompleteStream => "incomplete_gzip_stream",
            Self::TrailingData => "trailing_gzip_data",
            Self::InvalidArchive => "invalid_gzip_archive",
        }
    }
}
impl std::fmt::Display for GzipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for GzipError {}

impl From<ArchiveError> for GzipError {
    fn from(error: ArchiveError) -> Self {
        match error {
            ArchiveError::LimitExceeded => Self::LimitExceeded,
            _ => Self::InvalidArchive,
        }
    }
}
