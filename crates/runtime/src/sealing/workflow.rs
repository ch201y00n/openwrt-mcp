use super::{
    Cleanup, Publication, PublishOutcome, SealCheck, SealCipher, SealClock, SealFailure,
    SealFailureReport, SealLimits, SealSource, SealStage, SealedCounts,
    budget::Budget,
    stream::{CheckedRead, CountedWrite},
};
use std::time::Instant;

pub struct ArchiveSealer {
    limits: SealLimits,
}

impl ArchiveSealer {
    pub fn new(limits: SealLimits) -> Result<Self, SealFailure> {
        limits.validate()?;
        Ok(Self { limits })
    }

    pub fn seal(
        &self,
        source: &mut dyn SealSource,
        checker: Box<dyn SealCheck>,
        cipher: &dyn SealCipher,
        stage: &mut dyn SealStage,
    ) -> Result<SealedCounts, SealFailureReport> {
        self.seal_with_clock(source, checker, cipher, stage, &Elapsed(Instant::now()))
    }

    pub fn seal_with_clock(
        &self,
        source: &mut dyn SealSource,
        checker: Box<dyn SealCheck>,
        cipher: &dyn SealCipher,
        stage: &mut dyn SealStage,
        clock: &dyn SealClock,
    ) -> Result<SealedCounts, SealFailureReport> {
        // Establish cleanup responsibility before the first injected callback.
        let mut guard = Guard::new(source, stage);
        let budget = Budget::new(clock, self.limits.timeout_ms);
        match self.run(&mut guard, checker, cipher, &budget) {
            Ok(counts) => Ok(counts),
            Err(cause) => {
                guard.cleanup();
                Err(SealFailureReport {
                    cause,
                    publication: guard.publication,
                    source_cleanup: guard.source_cleanup,
                    stage_cleanup: guard.stage_cleanup,
                })
            }
        }
    }

    fn run(
        &self,
        guard: &mut Guard<'_>,
        mut checker: Box<dyn SealCheck>,
        cipher: &dyn SealCipher,
        budget: &Budget<'_>,
    ) -> Result<SealedCounts, SealFailure> {
        budget.check()?;
        let caps = guard.stage.capabilities();
        budget.check()?;
        if !caps.private_staging || !caps.atomic_publication || !caps.durability {
            return Err(SealFailure::StageUnsupported);
        }
        budget.check()?;
        let mut input = CheckedRead {
            source: guard.source,
            checker: checker.as_mut(),
            budget,
            limit: self.limits.source_bytes,
            count: 0,
            eof: false,
        };
        let mut output = CountedWrite {
            stage: guard.stage,
            budget,
            limit: self.limits.ciphertext_bytes,
            count: 0,
        };
        let encrypted = cipher
            .encrypt(&mut input, &mut output)
            .map_err(|_| SealFailure::Cipher);
        let reported = budget.after(encrypted)?;
        if !input.eof {
            return Err(SealFailure::IncompleteInput);
        }
        if input.count == 0 || output.count == 0 {
            return Err(SealFailure::EmptyStream);
        }
        if reported.input_bytes != input.count || reported.output_bytes != output.count {
            return Err(SealFailure::CountMismatch);
        }
        let input_count = input.count;
        let output_count = output.count;
        budget.check()?;
        let checked = checker.finish().map_err(|_| SealFailure::Check);
        let summary = budget.after(checked)?;
        summary.validate(input_count)?;
        budget.check()?;
        let completed = guard
            .source
            .complete()
            .map_err(|_| SealFailure::SourceCompletion);
        // A confirmed producer exit needs no cancellation, even if its count or
        // late acknowledgement fails validation. Failed completion stays pending.
        guard.source_complete = completed.is_ok();
        let completed = budget.after(completed)?;
        if completed != input_count {
            return Err(SealFailure::CountMismatch);
        }
        budget.check()?;
        let flushed = guard.stage.flush().map_err(|_| SealFailure::StageFlush);
        budget.after(flushed)?;
        let counts = SealedCounts {
            source_bytes: input_count,
            ciphertext_bytes: output_count,
            expanded_bytes: summary.expanded_bytes,
            payload_bytes: summary.payload_bytes,
            files: summary.files,
        };
        budget.check()?;
        // An unwind or lost acknowledgement during publish must never cause abort.
        guard.publication = Publication::Unknown;
        let outcome = guard.stage.publish(counts);
        guard.publication = match outcome {
            PublishOutcome::Published => Publication::Published,
            PublishOutcome::NotPublished => Publication::NotPublished,
            PublishOutcome::Unknown => Publication::Unknown,
        };
        budget.check()?;
        if outcome != PublishOutcome::Published {
            return Err(SealFailure::PublicationNotConfirmed);
        }
        Ok(counts)
    }
}

struct Elapsed(Instant);

impl SealClock for Elapsed {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

struct Guard<'a> {
    source: &'a mut dyn SealSource,
    stage: &'a mut dyn SealStage,
    source_complete: bool,
    source_cancelled: bool,
    stage_aborted: bool,
    publication: Publication,
    source_cleanup: Cleanup,
    stage_cleanup: Cleanup,
}

impl<'a> Guard<'a> {
    fn new(source: &'a mut dyn SealSource, stage: &'a mut dyn SealStage) -> Self {
        Self {
            source,
            stage,
            source_complete: false,
            source_cancelled: false,
            stage_aborted: false,
            publication: Publication::NotAttempted,
            source_cleanup: Cleanup::NotRequired,
            stage_cleanup: Cleanup::NotRequired,
        }
    }

    fn cleanup(&mut self) {
        if !self.source_complete && !self.source_cancelled {
            self.source_cancelled = true;
            self.source_cleanup = Cleanup::Unknown;
            self.source_cleanup = self.source.cancel().into();
        }
        if !self.stage_aborted
            && matches!(
                self.publication,
                Publication::NotAttempted | Publication::NotPublished
            )
        {
            self.stage_aborted = true;
            self.stage_cleanup = Cleanup::Unknown;
            self.stage_cleanup = self.stage.abort().into();
        }
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.cleanup();
    }
}
