use super::{SealCheck, SealFailure, SealSource, SealStage, budget::Budget, model::CHUNK};
use std::io::{Error, Read, Result, Write};
use zeroize::{Zeroize, Zeroizing};

fn io_error(_: SealFailure) -> Error {
    Error::other("seal_stream_failed")
}

pub(super) struct CheckedRead<'a, 'clock> {
    pub source: &'a mut dyn SealSource,
    pub checker: &'a mut dyn SealCheck,
    pub budget: &'a Budget<'clock>,
    pub limit: u64,
    pub count: u64,
    pub eof: bool,
}

impl Read for CheckedRead<'_, '_> {
    fn read(&mut self, output: &mut [u8]) -> Result<usize> {
        self.budget.check().map_err(io_error)?;
        if output.is_empty() || self.eof {
            return Ok(0);
        }
        if self.count == self.limit {
            let mut probe = Zeroizing::new([0_u8; 1]);
            let result = self
                .source
                .read(&mut *probe)
                .map_err(|_| SealFailure::SourceRead);
            let count = self.budget.after(result).map_err(io_error)?;
            if count > 1 {
                return Err(io_error(self.budget.fail(SealFailure::InvalidIoCount)));
            }
            if count != 0 {
                return Err(io_error(self.budget.fail(SealFailure::SourceLimit)));
            }
            self.eof = true;
            return Ok(0);
        }
        let size = output
            .len()
            .min(CHUNK)
            .min((self.limit - self.count) as usize);
        let result = self
            .source
            .read(&mut output[..size])
            .map_err(|_| SealFailure::SourceRead);
        let count = match self.budget.after(result) {
            Ok(count) if count <= size => count,
            result => {
                output[..size].zeroize();
                let failure = result.err().unwrap_or(SealFailure::InvalidIoCount);
                return Err(io_error(self.budget.fail(failure)));
            }
        };
        if count == 0 {
            self.eof = true;
            return Ok(0);
        }
        let Some(total) = self.count.checked_add(count as u64) else {
            output[..size].zeroize();
            return Err(io_error(self.budget.fail(SealFailure::SourceLimit)));
        };
        // Validation must succeed before any of these bytes reach the cipher.
        let checked = self
            .checker
            .feed(&output[..count])
            .map_err(|_| SealFailure::Check);
        if let Err(error) = self.budget.after(checked) {
            output[..size].zeroize();
            return Err(io_error(error));
        }
        self.count = total;
        Ok(count)
    }
}

pub(super) struct CountedWrite<'a, 'clock> {
    pub stage: &'a mut dyn SealStage,
    pub budget: &'a Budget<'clock>,
    pub limit: u64,
    pub count: u64,
}

impl Write for CountedWrite<'_, '_> {
    fn write(&mut self, input: &[u8]) -> Result<usize> {
        self.budget.check().map_err(io_error)?;
        if input.is_empty() {
            return Ok(0);
        }
        if self.count == self.limit {
            return Err(io_error(self.budget.fail(SealFailure::CiphertextLimit)));
        }
        let size = input
            .len()
            .min(CHUNK)
            .min((self.limit - self.count) as usize);
        let result = self
            .stage
            .write(&input[..size])
            .map_err(|_| SealFailure::StageWrite);
        let count = self.budget.after(result).map_err(io_error)?;
        if count > size {
            return Err(io_error(self.budget.fail(SealFailure::InvalidIoCount)));
        }
        if count == 0 {
            return Err(io_error(self.budget.fail(SealFailure::ZeroWrite)));
        }
        self.count = self
            .count
            .checked_add(count as u64)
            .ok_or_else(|| io_error(self.budget.fail(SealFailure::CiphertextLimit)))?;
        Ok(count)
    }

    fn flush(&mut self) -> Result<()> {
        self.budget.check().map_err(io_error)?;
        let result = self.stage.flush().map_err(|_| SealFailure::StageFlush);
        self.budget.after(result).map_err(io_error)
    }
}
