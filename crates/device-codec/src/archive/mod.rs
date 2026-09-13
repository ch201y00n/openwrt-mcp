//! Supplied tar structure only: no source attestation, extraction or permission.
//! No production consumer is admitted before a separate integration checkpoint.
mod header;
mod manifest;
mod stream;

pub use manifest::{ExpectedArchive, ExpectedFile};
pub use stream::ArchiveValidator;

pub const MAX_CHUNK_BYTES: usize = 65536;
pub const MAX_ARCHIVE_BYTES: u64 = 72 * 1024 * 1024;
pub const MAX_PAYLOAD_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_FILES: usize = 4096;
pub const MAX_PATH_BYTES: usize = 256;
pub const MAX_PATH_COMPONENTS: usize = 32;
pub const MAX_COMPONENT_BYTES: usize = 100;
pub const MAX_TAIL_BYTES: usize = 32768;
const BLOCK_BYTES: usize = 512;

/// Counts about the supplied manifest/stream, never a completed-backup receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveSummary {
    pub files: usize,
    pub payload_bytes: u64,
    pub archive_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    InvalidPath,
    InvalidManifest,
    InvalidHeader,
    UnsupportedEntry,
    ManifestMismatch,
    LimitExceeded,
    IncompleteArchive,
    InvalidPadding,
}
impl ArchiveError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_archive_path",
            Self::InvalidManifest => "invalid_archive_manifest",
            Self::InvalidHeader => "invalid_archive_header",
            Self::UnsupportedEntry => "unsupported_archive_entry",
            Self::ManifestMismatch => "archive_manifest_mismatch",
            Self::LimitExceeded => "archive_limit_exceeded",
            Self::IncompleteArchive => "incomplete_archive",
            Self::InvalidPadding => "invalid_archive_padding",
        }
    }
}
impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for ArchiveError {}
