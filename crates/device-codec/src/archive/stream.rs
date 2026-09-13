use super::{
    ArchiveError, ArchiveSummary, BLOCK_BYTES, ExpectedArchive, MAX_ARCHIVE_BYTES, MAX_CHUNK_BYTES,
    MAX_TAIL_BYTES, header,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy)]
enum Stage {
    Header,
    Body { remaining: u64, padding: usize },
    Padding { remaining: usize },
    End { bytes: usize },
}

/// Bounded supplied-byte validator. Feed success is not complete-backup success.
pub struct ArchiveValidator {
    expected: ExpectedArchive,
    seen: Vec<bool>,
    header: Zeroizing<[u8; BLOCK_BYTES]>,
    filled: usize,
    stage: Stage,
    total: u64,
    failure: Option<ArchiveError>,
}
impl ArchiveValidator {
    pub fn new(expected: ExpectedArchive) -> Self {
        Self {
            seen: vec![false; expected.files.len()],
            expected,
            header: Zeroizing::new([0; BLOCK_BYTES]),
            filled: 0,
            stage: Stage::Header,
            total: 0,
            failure: None,
        }
    }

    pub fn feed(&mut self, input: &[u8]) -> Result<(), ArchiveError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.feed_inner(input);
        if let Err(error) = result {
            self.failure = Some(error);
            self.header.zeroize();
            self.filled = 0;
        }
        result
    }

    fn feed_inner(&mut self, mut input: &[u8]) -> Result<(), ArchiveError> {
        if input.len() > MAX_CHUNK_BYTES {
            return Err(ArchiveError::LimitExceeded);
        }
        self.total = self
            .total
            .checked_add(input.len() as u64)
            .filter(|n| *n <= MAX_ARCHIVE_BYTES)
            .ok_or(ArchiveError::LimitExceeded)?;
        while !input.is_empty() {
            match self.stage {
                Stage::Header => {
                    let take = input.len().min(BLOCK_BYTES - self.filled);
                    self.header[self.filled..self.filled + take].copy_from_slice(&input[..take]);
                    self.filled += take;
                    input = &input[take..];
                    if self.filled == BLOCK_BYTES {
                        self.accept_header()?;
                    }
                }
                Stage::Body { remaining, padding } => {
                    let take = remaining.min(input.len() as u64) as usize;
                    input = &input[take..];
                    let remaining = remaining - take as u64;
                    self.stage = if remaining > 0 {
                        Stage::Body { remaining, padding }
                    } else if padding > 0 {
                        Stage::Padding { remaining: padding }
                    } else {
                        Stage::Header
                    };
                }
                Stage::Padding { remaining } => {
                    let take = remaining.min(input.len());
                    if input[..take].iter().any(|b| *b != 0) {
                        return Err(ArchiveError::InvalidPadding);
                    }
                    input = &input[take..];
                    self.stage = if remaining == take {
                        Stage::Header
                    } else {
                        Stage::Padding {
                            remaining: remaining - take,
                        }
                    };
                }
                Stage::End { bytes } => {
                    let bytes = bytes
                        .checked_add(input.len())
                        .filter(|n| *n <= MAX_TAIL_BYTES)
                        .ok_or(ArchiveError::LimitExceeded)?;
                    if input.iter().any(|b| *b != 0) {
                        return Err(ArchiveError::InvalidPadding);
                    }
                    self.stage = Stage::End { bytes };
                    input = &[];
                }
            }
        }
        Ok(())
    }

    fn accept_header(&mut self) -> Result<(), ArchiveError> {
        if self.header.iter().all(|b| *b == 0) {
            self.stage = Stage::End { bytes: BLOCK_BYTES };
        } else {
            let entry = header::parse(&self.header)?;
            let index = self
                .expected
                .files
                .binary_search_by(|f| f.path.cmp(&entry.path))
                .map_err(|_| ArchiveError::ManifestMismatch)?;
            if self.seen[index] || self.expected.files[index].size != entry.size {
                return Err(ArchiveError::ManifestMismatch);
            }
            self.seen[index] = true;
            if entry.size > 0 {
                self.stage = Stage::Body {
                    remaining: entry.size,
                    padding: (BLOCK_BYTES - entry.size as usize % BLOCK_BYTES) % BLOCK_BYTES,
                };
            }
        }
        self.header.zeroize();
        self.filled = 0;
        Ok(())
    }

    pub fn finish(self) -> Result<ArchiveSummary, ArchiveError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if !matches!(self.stage, Stage::End { bytes }
            if bytes >= 2 * BLOCK_BYTES && bytes % BLOCK_BYTES == 0)
        {
            return Err(ArchiveError::IncompleteArchive);
        }
        if self.seen.iter().any(|seen| !seen) {
            return Err(ArchiveError::ManifestMismatch);
        }
        Ok(ArchiveSummary {
            files: self.expected.files.len(),
            payload_bytes: self.expected.payload_bytes,
            archive_bytes: self.total,
        })
    }
}
