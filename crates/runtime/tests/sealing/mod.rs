//! Public synthetic byte/count models, NOT cipher or real storage implementations.
mod boundaries;
mod failures;

use openwrt_mcp_runtime::sealing::*;
use std::{
    cell::RefCell,
    io::{self, Read, Write},
    rc::Rc,
};

#[derive(Default)]
struct Trace {
    events: Vec<&'static str>,
    time: u64,
    clock_calls: usize,
    clock_jump: Option<usize>,
    change: Option<(&'static str, u64)>,
    panic_on: Option<&'static str>,
}
type Shared = Rc<RefCell<Trace>>;
fn event(shared: &Shared, name: &'static str) {
    let mut trace = shared.borrow_mut();
    trace.events.push(name);
    if let Some((trigger, time)) = trace.change
        && trigger == name
    {
        trace.time = time;
    }
    assert_ne!(trace.panic_on, Some(name), "synthetic port unwind");
}
fn calls(shared: &Shared, name: &str) -> usize {
    shared
        .borrow()
        .events
        .iter()
        .filter(|e| **e == name)
        .count()
}
struct FakeClock(Shared);
impl SealClock for FakeClock {
    fn now_ms(&self) -> u64 {
        let mut trace = self.0.borrow_mut();
        trace.clock_calls += 1;
        if trace.clock_jump == Some(trace.clock_calls) {
            trace.time = 100;
        }
        trace.time
    }
}

struct Source {
    trace: Shared,
    size: u64,
    read: u64,
    chunks: usize,
    requests: Vec<usize>,
    read_error: Option<io::ErrorKind>,
    impossible: bool,
    impossible_after: Option<u64>,
    completion: Option<Result<u64, SealPortError>>,
    cleanup: CleanupOutcome,
}
impl Source {
    fn new(trace: Shared, size: u64) -> Self {
        Self {
            trace,
            size,
            read: 0,
            chunks: usize::MAX,
            requests: Vec::new(),
            read_error: None,
            impossible: false,
            impossible_after: None,
            completion: None,
            cleanup: CleanupOutcome::Cleaned,
        }
    }
}
impl Read for Source {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        event(&self.trace, "read");
        self.requests.push(bytes.len());
        bytes.fill(0x71);
        if let Some(kind) = self.read_error {
            return Err(io::Error::new(kind, "synthetic source diagnostic"));
        }
        if self.impossible || self.impossible_after == Some(self.read) {
            return Ok(bytes.len() + 1);
        }
        let count = bytes
            .len()
            .min(self.chunks)
            .min((self.size - self.read) as usize);
        self.read += count as u64;
        Ok(count)
    }
}
impl SealSource for Source {
    fn complete(&mut self) -> Result<u64, SealPortError> {
        event(&self.trace, "complete");
        self.completion.unwrap_or(Ok(self.read))
    }
    fn cancel(&mut self) -> CleanupOutcome {
        event(&self.trace, "cancel");
        self.cleanup
    }
}

struct Check {
    trace: Shared,
    count: u64,
    fail_feed: bool,
    fail_finish: bool,
    summary: Option<CheckCounts>,
}
impl Check {
    fn new(trace: Shared) -> Self {
        Self {
            trace,
            count: 0,
            fail_feed: false,
            fail_finish: false,
            summary: None,
        }
    }
}
impl SealCheck for Check {
    fn feed(&mut self, bytes: &[u8]) -> Result<(), SealPortError> {
        event(&self.trace, "feed");
        self.count += bytes.len() as u64;
        assert!(bytes.len() <= 65_536);
        if self.fail_feed {
            Err(SealPortError)
        } else {
            Ok(())
        }
    }
    fn finish(self: Box<Self>) -> Result<CheckCounts, SealPortError> {
        event(&self.trace, "finish");
        if self.fail_finish {
            return Err(SealPortError);
        }
        Ok(self.summary.unwrap_or(CheckCounts {
            source_bytes: self.count,
            expanded_bytes: self.count,
            payload_bytes: 0,
            files: 1,
        }))
    }
}

struct Cipher {
    trace: Shared,
    early: bool,
    empty_reads_only: bool,
    fail: bool,
    ignore_error: bool,
    wrong_input: bool,
    wrong_output: bool,
    omit_output: bool,
    finish_write: bool,
    flush: bool,
    probe_again: bool,
    stop_after_read: bool,
}
impl Cipher {
    fn new(trace: Shared) -> Self {
        Self {
            trace,
            early: false,
            empty_reads_only: false,
            fail: false,
            ignore_error: false,
            wrong_input: false,
            wrong_output: false,
            omit_output: false,
            finish_write: false,
            flush: false,
            probe_again: false,
            stop_after_read: false,
        }
    }
}
impl SealCipher for Cipher {
    fn encrypt(
        &self,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<CipherCounts, SealPortError> {
        event(&self.trace, "cipher");
        let mut count = 0;
        let mut written = 0;
        if self.empty_reads_only {
            assert_eq!(input.read(&mut []).unwrap(), 0);
        }
        if !self.early && !self.empty_reads_only {
            let mut bytes = [0_u8; 131_072];
            loop {
                let n = match input.read(&mut bytes) {
                    Ok(n) => n,
                    Err(_) if self.ignore_error => {
                        assert!(input.read(&mut bytes).is_err());
                        assert!(output.write(b"x").is_err());
                        break;
                    }
                    Err(_) => return Err(SealPortError),
                };
                if n == 0 {
                    break;
                }
                count += n as u64;
                if !self.omit_output {
                    if output.write_all(&bytes[..n]).is_err() {
                        if self.ignore_error {
                            assert!(input.read(&mut bytes).is_err());
                            assert!(output.flush().is_err());
                            break;
                        }
                        return Err(SealPortError);
                    }
                    written += n as u64;
                }
                if self.stop_after_read {
                    break;
                }
            }
            if self.probe_again {
                assert_eq!(input.read(&mut bytes).unwrap(), 0);
            }
        }
        if self.finish_write {
            output.write_all(b"x").map_err(|_| SealPortError)?;
            written += 1;
        }
        if self.flush && output.flush().is_err() {
            if !self.ignore_error {
                return Err(SealPortError);
            }
            assert!(output.write(b"x").is_err());
            assert!(input.read(&mut [0; 1]).is_err());
        }
        event(&self.trace, "cipher_done");
        if self.fail {
            return Err(SealPortError);
        }
        Ok(CipherCounts {
            input_bytes: count + u64::from(self.wrong_input),
            output_bytes: written + u64::from(self.wrong_output),
        })
    }
}

struct Stage {
    trace: Shared,
    caps: StageCapabilities,
    written: u64,
    chunks: usize,
    requests: Vec<usize>,
    write_error: Option<io::ErrorKind>,
    impossible: bool,
    fail_flush: bool,
    outcome: PublishOutcome,
    cleanup: CleanupOutcome,
    published: Option<SealedCounts>,
}
impl Stage {
    fn new(trace: Shared) -> Self {
        Self {
            trace,
            caps: StageCapabilities {
                private_staging: true,
                atomic_publication: true,
                durability: true,
            },
            written: 0,
            chunks: usize::MAX,
            requests: Vec::new(),
            write_error: None,
            impossible: false,
            fail_flush: false,
            outcome: PublishOutcome::Published,
            cleanup: CleanupOutcome::Cleaned,
            published: None,
        }
    }
}
impl Write for Stage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        event(&self.trace, "write");
        self.requests.push(bytes.len());
        if let Some(kind) = self.write_error {
            return Err(io::Error::new(kind, "synthetic stage diagnostic"));
        }
        if self.impossible {
            return Ok(bytes.len() + 1);
        }
        let count = bytes.len().min(self.chunks);
        self.written += count as u64;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        event(&self.trace, "flush");
        if self.fail_flush {
            Err(io::Error::other("synthetic flush diagnostic"))
        } else {
            Ok(())
        }
    }
}
impl SealStage for Stage {
    fn capabilities(&self) -> StageCapabilities {
        event(&self.trace, "caps");
        self.caps
    }
    fn publish(&mut self, counts: SealedCounts) -> PublishOutcome {
        event(&self.trace, "publish");
        self.published = Some(counts);
        self.outcome
    }
    fn abort(&mut self) -> CleanupOutcome {
        event(&self.trace, "abort");
        self.cleanup
    }
}

struct Case {
    trace: Shared,
    source: Source,
    checker: Check,
    cipher: Cipher,
    stage: Stage,
    limits: SealLimits,
}
impl Case {
    fn new(size: u64) -> Self {
        let trace = Shared::default();
        Self {
            source: Source::new(trace.clone(), size),
            checker: Check::new(trace.clone()),
            cipher: Cipher::new(trace.clone()),
            stage: Stage::new(trace.clone()),
            trace,
            limits: SealLimits {
                timeout_ms: 100,
                ..SealLimits::default()
            },
        }
    }
    fn run(&mut self) -> Result<SealedCounts, SealFailureReport> {
        let checker = std::mem::replace(&mut self.checker, Check::new(self.trace.clone()));
        ArchiveSealer::new(self.limits).unwrap().seal_with_clock(
            &mut self.source,
            Box::new(checker),
            &self.cipher,
            &mut self.stage,
            &FakeClock(self.trace.clone()),
        )
    }
    fn pre_failure(&mut self, cause: SealFailure) -> SealFailureReport {
        let report = self.run().unwrap_err();
        assert_eq!(report.cause, cause);
        assert_eq!(report.publication, Publication::NotAttempted);
        assert_eq!(calls(&self.trace, "publish"), 0);
        assert_eq!(calls(&self.trace, "abort"), 1);
        assert!(calls(&self.trace, "cancel") <= 1);
        report
    }
}

#[test]
fn full_order_requires_real_eof_then_checker_producer_flush_and_publication() {
    let mut case = Case::new(7);
    let counts = case.run().unwrap();
    assert_eq!(
        counts,
        SealedCounts {
            source_bytes: 7,
            ciphertext_bytes: 7,
            expanded_bytes: 7,
            payload_bytes: 0,
            files: 1
        }
    );
    assert_eq!(case.stage.published, Some(counts));
    assert_eq!(
        case.trace.borrow().events,
        [
            "caps",
            "cipher",
            "read",
            "feed",
            "write",
            "read",
            "cipher_done",
            "finish",
            "complete",
            "flush",
            "publish"
        ]
    );
}

#[test]
fn empty_early_and_empty_buffer_reads_do_not_fabricate_completion() {
    for empty_buffer in [false, true] {
        let mut case = Case::new(7);
        case.cipher.early = !empty_buffer;
        case.cipher.empty_reads_only = empty_buffer;
        case.pre_failure(SealFailure::IncompleteInput);
        assert_eq!(case.source.read, 0);
        assert_eq!(calls(&case.trace, "read"), 0);
        assert_eq!(calls(&case.trace, "finish"), 0);
        assert_eq!(calls(&case.trace, "cancel"), 1);
    }
    Case::new(0).pre_failure(SealFailure::EmptyStream);
    let mut case = Case::new(7);
    case.cipher.omit_output = true;
    case.pre_failure(SealFailure::EmptyStream);
}

#[test]
fn cipher_counts_are_independent_and_cannot_override_actual_io() {
    for wrong_input in [false, true] {
        let mut case = Case::new(7);
        case.cipher.wrong_input = wrong_input;
        case.cipher.wrong_output = !wrong_input;
        case.pre_failure(SealFailure::CountMismatch);
        assert_eq!(calls(&case.trace, "finish"), 0);
    }
}

#[test]
fn every_missing_stage_guarantee_stops_before_cipher_source_or_checker() {
    for flags in 0..7 {
        let mut case = Case::new(7);
        case.stage.caps = StageCapabilities {
            private_staging: flags & 1 != 0,
            atomic_publication: flags & 2 != 0,
            durability: flags & 4 != 0,
        };
        case.pre_failure(SealFailure::StageUnsupported);
        assert_eq!(case.trace.borrow().events, ["caps", "cancel", "abort"]);
    }
}
