use super::{
    ArchiveError, MAX_COMPONENT_BYTES, MAX_FILE_BYTES, MAX_FILES, MAX_PATH_BYTES,
    MAX_PATH_COMPONENTS, MAX_PAYLOAD_BYTES,
};
use zeroize::Zeroizing;

pub(super) fn validate_path(path: &[u8]) -> Result<(), ArchiveError> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES {
        return Err(ArchiveError::InvalidPath);
    }
    if !path
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'+' | b'@' | b'/'))
    {
        return Err(ArchiveError::InvalidPath);
    }
    let mut count = 0;
    for component in path.split(|b| *b == b'/') {
        count += 1;
        if component.is_empty()
            || component.len() > MAX_COMPONENT_BYTES
            || component == b"."
            || component == b".."
            || count > MAX_PATH_COMPONENTS
        {
            return Err(ArchiveError::InvalidPath);
        }
    }
    Ok(())
}

/// Expected logical router-relative name and size, with private zeroizing storage.
#[derive(Clone)]
pub struct ExpectedFile {
    pub(super) path: Zeroizing<Vec<u8>>,
    pub(super) size: u64,
}
impl ExpectedFile {
    pub fn new(path: &str, size: u64) -> Result<Self, ArchiveError> {
        validate_path(path.as_bytes())?;
        if size > MAX_FILE_BYTES {
            return Err(ArchiveError::LimitExceeded);
        }
        Ok(Self {
            path: Zeroizing::new(path.as_bytes().to_vec()),
            size,
        })
    }
}

/// Supplied expectations, not a trusted live inventory or authenticated manifest.
pub struct ExpectedArchive {
    pub(super) files: Box<[ExpectedFile]>,
    pub(super) payload_bytes: u64,
}
impl ExpectedArchive {
    pub fn new(files: &[ExpectedFile]) -> Result<Self, ArchiveError> {
        if files.is_empty() {
            return Err(ArchiveError::InvalidManifest);
        }
        if files.len() > MAX_FILES {
            return Err(ArchiveError::LimitExceeded);
        }
        let mut payload_bytes = 0_u64;
        for file in files {
            payload_bytes = payload_bytes
                .checked_add(file.size)
                .filter(|n| *n <= MAX_PAYLOAD_BYTES)
                .ok_or(ArchiveError::LimitExceeded)?;
        }
        let mut files = files.to_vec();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if files.windows(2).any(|p| p[0].path == p[1].path) {
            return Err(ArchiveError::InvalidManifest);
        }
        // Adjacent-name checks alone miss a, a-, a/b. Test every separator's
        // exact prefix against the complete bounded sorted set.
        for file in &files {
            for (i, byte) in file.path.iter().enumerate() {
                if *byte == b'/'
                    && files
                        .binary_search_by(|f| f.path.as_slice().cmp(&file.path[..i]))
                        .is_ok()
                {
                    return Err(ArchiveError::InvalidManifest);
                }
            }
        }
        Ok(Self {
            files: files.into_boxed_slice(),
            payload_bytes,
        })
    }
}
