use super::{
    EXPANSION_SLACK_BYTES, GzipArchiveSummary, GzipError, MAX_CHUNK_BYTES, MAX_EXPANSION_RATIO,
    MAX_SOURCE_BYTES, OUTPUT_BUFFER_BYTES, header::Header,
};
use crate::archive::{ArchiveValidator, ExpectedArchive};
use flate2::{Decompress, FlushDecompress, Status};
use zeroize::{Zeroize, Zeroizing};

/// No I/O, extraction, format fallback, premature summary or reset operation.
pub struct GzipArchiveValidator {
    header: Option<Header>,
    decoder: Option<Decompress>,
    inner: Option<ArchiveValidator>,
    output: Zeroizing<[u8; OUTPUT_BUFFER_BYTES]>,
    source_bytes: u64,
    header_bytes: u64,
    deflate_bytes: u64,
    complete: bool,
    failure: Option<GzipError>,
}
impl GzipArchiveValidator {
    pub fn new(expected: ExpectedArchive) -> Self {
        Self {
            header: Some(Header::new()),
            decoder: None,
            inner: Some(ArchiveValidator::new(expected)),
            output: Zeroizing::new([0; OUTPUT_BUFFER_BYTES]),
            source_bytes: 0,
            header_bytes: 0,
            deflate_bytes: 0,
            complete: false,
            failure: None,
        }
    }

    pub fn feed(&mut self, input: &[u8]) -> Result<(), GzipError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let result = self.feed_inner(input);
        if let Err(error) = result {
            self.failure = Some(error);
            self.header = None;
            self.decoder = None; // Release is not a guarantee of backend scrubbing.
            self.inner = None;
            self.output.zeroize();
        }
        result
    }

    fn feed_inner(&mut self, mut input: &[u8]) -> Result<(), GzipError> {
        if input.len() > MAX_CHUNK_BYTES {
            return Err(GzipError::LimitExceeded);
        }
        self.source_bytes = self
            .source_bytes
            .checked_add(input.len() as u64)
            .filter(|n| *n <= MAX_SOURCE_BYTES)
            .ok_or(GzipError::LimitExceeded)?;
        if self.complete {
            return if input.is_empty() {
                Ok(())
            } else {
                Err(GzipError::TrailingData)
            };
        }
        if let Some(header) = self.header.as_mut() {
            let consumed = header.feed(input)?;
            input = &input[consumed..];
            if !header.complete() {
                return Ok(());
            }
            // Move the private header out so no copy or self-referential borrow
            // outlives this call. Its owned bytes zeroize on every return path.
            let header = self.header.take().ok_or(GzipError::InvalidHeader)?;
            self.header_bytes = header.bytes().len() as u64;
            self.decoder = Some(Decompress::new_gzip(15));
            self.decode(header.bytes())?;
        }
        self.decode(input)
    }

    fn decode(&mut self, mut input: &[u8]) -> Result<(), GzipError> {
        loop {
            let decoder = self.decoder.as_mut().ok_or(GzipError::InvalidData)?;
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let status = decoder
                .decompress(input, &mut self.output[..], FlushDecompress::None)
                .map_err(|_| GzipError::InvalidData)?;
            let used = decoder
                .total_in()
                .checked_sub(before_in)
                .filter(|n| *n <= input.len() as u64)
                .ok_or(GzipError::InvalidData)? as usize;
            let written = decoder
                .total_out()
                .checked_sub(before_out)
                .filter(|n| *n <= OUTPUT_BUFFER_BYTES as u64)
                .ok_or(GzipError::InvalidData)? as usize;
            self.inner
                .as_mut()
                .ok_or(GzipError::InvalidData)?
                .feed(&self.output[..written])
                .map_err(GzipError::from)?;
            self.output.zeroize();
            input = &input[used..];
            if status == Status::StreamEnd {
                if !input.is_empty() {
                    return Err(GzipError::TrailingData);
                }
                if decoder.total_in() != self.source_bytes {
                    return Err(GzipError::InvalidData);
                }
                self.deflate_bytes = decoder
                    .total_in()
                    .checked_sub(self.header_bytes)
                    .and_then(|n| n.checked_sub(8))
                    .filter(|n| *n > 0)
                    .ok_or(GzipError::InvalidData)?;
                self.complete = true;
                self.decoder = None;
                return Ok(());
            }
            if used == 0 && written == 0 {
                return if input.is_empty() {
                    Ok(())
                } else {
                    Err(GzipError::InvalidData)
                };
            }
            // Drain a full output buffer even when input was exhausted. Stop on
            // no progress; a future nonempty feed supplies any missing bits.
            if input.is_empty() && written < OUTPUT_BUFFER_BYTES {
                return Ok(());
            }
        }
    }

    pub fn finish(mut self) -> Result<GzipArchiveSummary, GzipError> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        if !self.complete {
            return Err(GzipError::IncompleteStream);
        }
        let summary = self
            .inner
            .take()
            .ok_or(GzipError::InvalidData)?
            .finish()
            .map_err(GzipError::from)?;
        let allowed = self
            .deflate_bytes
            .checked_mul(MAX_EXPANSION_RATIO)
            .and_then(|n| n.checked_add(EXPANSION_SLACK_BYTES))
            .ok_or(GzipError::LimitExceeded)?;
        if summary.archive_bytes > allowed {
            return Err(GzipError::LimitExceeded);
        }
        Ok(GzipArchiveSummary {
            compressed_bytes: self.source_bytes,
            files: summary.files,
            payload_bytes: summary.payload_bytes,
            archive_bytes: summary.archive_bytes,
        })
    }
}
