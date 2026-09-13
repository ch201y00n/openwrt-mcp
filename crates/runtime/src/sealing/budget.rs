use super::{SealClock, SealFailure};
use std::cell::Cell;

pub(super) struct Budget<'a> {
    clock: &'a dyn SealClock,
    start: u64,
    last: Cell<u64>,
    timeout: u64,
    error: Cell<Option<SealFailure>>,
}

impl<'a> Budget<'a> {
    pub fn new(clock: &'a dyn SealClock, timeout: u64) -> Self {
        let start = clock.now_ms();
        Self {
            clock,
            start,
            last: Cell::new(start),
            timeout,
            error: Cell::new(None),
        }
    }

    pub fn fail(&self, error: SealFailure) -> SealFailure {
        let first = self.error.get().unwrap_or(error);
        self.error.set(Some(first));
        first
    }

    pub fn check(&self) -> Result<(), SealFailure> {
        if let Some(error) = self.error.get() {
            return Err(error);
        }
        let now = self.clock.now_ms();
        if now < self.last.get() {
            return Err(self.fail(SealFailure::ClockReversed));
        }
        self.last.set(now);
        if now - self.start >= self.timeout {
            return Err(self.fail(SealFailure::Deadline));
        }
        Ok(())
    }

    pub fn after<T>(&self, result: Result<T, SealFailure>) -> Result<T, SealFailure> {
        let value = result.map_err(|error| self.fail(error))?;
        self.check()?;
        Ok(value)
    }
}
