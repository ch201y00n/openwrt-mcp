use openwrt_mcp_runtime::protection::ProtectionError;
use std::{
    cell::Cell,
    io::{Error, ErrorKind, Read, Result as IoResult, Write},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
enum Fault {
    Limit,
    Stream,
    Deadline,
}

pub(crate) struct Budget {
    started: Instant,
    timeout: Duration,
    failure: Cell<Option<Fault>>,
    header_maximum: Cell<Option<u64>>,
}

impl Budget {
    pub(crate) fn new(timeout_ms: u64) -> Self {
        Self {
            started: Instant::now(),
            timeout: Duration::from_millis(timeout_ms),
            failure: Cell::new(None),
            header_maximum: Cell::new(None),
        }
    }

    pub(crate) fn begin_header(&self, maximum: u64) {
        self.header_maximum.set(Some(maximum));
    }

    pub(crate) fn finish_header(&self) {
        self.header_maximum.set(None);
    }

    pub(crate) fn check(&self) -> Result<(), ProtectionError> {
        if self.started.elapsed() >= self.timeout && self.failure.get().is_none() {
            self.failure.set(Some(Fault::Deadline));
        }
        if self.failure.get().is_some() {
            Err(self.error(ProtectionError::StreamFailed))
        } else {
            Ok(())
        }
    }

    pub(crate) fn error(&self, fallback: ProtectionError) -> ProtectionError {
        match self.failure.get() {
            Some(Fault::Limit) => ProtectionError::ResourceLimit,
            Some(Fault::Stream) => ProtectionError::StreamFailed,
            Some(Fault::Deadline) => ProtectionError::DeadlineExceeded,
            None => fallback,
        }
    }

    fn fail(&self, failure: Fault) -> Error {
        if self.failure.get().is_none() {
            self.failure.set(Some(failure));
        }
        safe_error()
    }

    fn check_io(&self) -> IoResult<()> {
        self.check().map_err(|_| safe_error())
    }
}

fn safe_error() -> Error {
    Error::other("protected_stream_failed")
}

pub(crate) struct BoundedRead<'a> {
    inner: &'a mut dyn Read,
    maximum: u64,
    count: u64,
    budget: &'a Budget,
}

impl<'a> BoundedRead<'a> {
    pub(crate) fn new(inner: &'a mut dyn Read, maximum: u64, budget: &'a Budget) -> Self {
        Self {
            inner,
            maximum,
            count: 0,
            budget,
        }
    }

    pub(crate) fn count(&self) -> u64 {
        self.count
    }

    fn read_inner(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        loop {
            self.budget.check_io()?;
            let result = self.inner.read(buffer);
            self.budget.check_io()?;
            match result {
                Ok(count) if count <= buffer.len() => return Ok(count),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                _ => return Err(self.budget.fail(Fault::Stream)),
            }
        }
    }
}

impl Read for BoundedRead<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        self.budget.check_io()?;
        if buffer.is_empty() {
            return Ok(0);
        }
        let maximum = self
            .budget
            .header_maximum
            .get()
            .map_or(self.maximum, |header| header.min(self.maximum));
        let remaining = maximum - self.count;
        if remaining == 0 {
            // A one-byte probe distinguishes an exact-limit EOF from truncation.
            // The probe is never passed to age and is wiped on every exit path.
            let mut probe = Zeroizing::new([0_u8; 1]);
            return match self.read_inner(&mut *probe)? {
                0 => Ok(0),
                _ => Err(self.budget.fail(Fault::Limit)),
            };
        }
        let capacity = remaining.min(buffer.len() as u64) as usize;
        let count = self.read_inner(&mut buffer[..capacity])?;
        self.count += count as u64;
        Ok(count)
    }
}

pub(crate) struct BoundedWrite<'a> {
    inner: &'a mut dyn Write,
    maximum: u64,
    count: u64,
    budget: &'a Budget,
}

impl<'a> BoundedWrite<'a> {
    pub(crate) fn new(inner: &'a mut dyn Write, maximum: u64, budget: &'a Budget) -> Self {
        Self {
            inner,
            maximum,
            count: 0,
            budget,
        }
    }

    pub(crate) fn count(&self) -> u64 {
        self.count
    }
}

impl Write for BoundedWrite<'_> {
    fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
        self.budget.check_io()?;
        if buffer.len() as u64 > self.maximum - self.count {
            return Err(self.budget.fail(Fault::Limit));
        }
        // age's header serializer requires complete writes. Normalize short
        // caller writes here without buffering an unbounded output document.
        let mut offset = 0;
        while offset < buffer.len() {
            self.budget.check_io()?;
            let result = self.inner.write(&buffer[offset..]);
            self.budget.check_io()?;
            match result {
                Ok(count) if count > 0 && count <= buffer.len() - offset => {
                    self.count += count as u64;
                    offset += count;
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                _ => return Err(self.budget.fail(Fault::Stream)),
            }
        }
        Ok(offset)
    }

    fn flush(&mut self) -> IoResult<()> {
        self.budget.check_io()?;
        let result = self.inner.flush();
        self.budget.check_io()?;
        result.map_err(|_| self.budget.fail(Fault::Stream))
    }
}

pub(crate) fn copy(
    input: &mut dyn Read,
    output: &mut dyn Write,
    budget: &Budget,
    fallback: ProtectionError,
) -> Result<(), ProtectionError> {
    let mut buffer = Zeroizing::new([0_u8; 8192]);
    loop {
        budget.check()?;
        let count = input
            .read(&mut *buffer)
            .map_err(|_| budget.error(fallback))?;
        budget.check()?;
        if count == 0 {
            return Ok(());
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| budget.error(fallback))?;
    }
}
