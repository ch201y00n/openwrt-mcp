use super::*;

#[test]
fn eof_does_not_replace_producer_success_and_failed_completion_is_cancelled() {
    let mut failed = Case::new(7);
    failed.source.completion = Some(Err(SealPortError));
    let report = failed.pre_failure(SealFailure::SourceCompletion);
    assert_eq!(report.source_cleanup, Cleanup::Cleaned);
    assert_eq!(calls(&failed.trace, "cancel"), 1);
    for wrong in [0, 6, 8, u64::MAX] {
        let mut case = Case::new(7);
        case.source.completion = Some(Ok(wrong));
        let report = case.pre_failure(SealFailure::CountMismatch);
        assert_eq!(report.source_cleanup, Cleanup::NotRequired);
        assert_eq!(calls(&case.trace, "cancel"), 0);
    }
}

#[test]
fn feed_failure_never_delivers_rejected_bytes_or_continues_when_ignored() {
    for ignored in [false, true] {
        let mut case = Case::new(7);
        case.checker.fail_feed = true;
        case.cipher.ignore_error = ignored;
        case.pre_failure(SealFailure::Check);
        assert_eq!(calls(&case.trace, "read"), 1);
        assert_eq!(calls(&case.trace, "write"), 0);
        assert_eq!(calls(&case.trace, "finish"), 0);
    }
}

#[test]
fn ignored_io_failures_remain_latched_in_both_directions_including_interrupted() {
    for kind in [
        io::ErrorKind::Other,
        io::ErrorKind::Interrupted,
        io::ErrorKind::WouldBlock,
        io::ErrorKind::UnexpectedEof,
    ] {
        for ignored in [false, true] {
            let mut read = Case::new(7);
            read.source.read_error = Some(kind);
            read.cipher.ignore_error = ignored;
            let error = read.pre_failure(SealFailure::SourceRead);
            assert!(!format!("{error:?}").contains("synthetic"));
            assert_eq!(calls(&read.trace, "read"), 1);
            let mut write = Case::new(7);
            write.stage.write_error = Some(kind);
            write.cipher.ignore_error = ignored;
            write.pre_failure(SealFailure::StageWrite);
            assert_eq!(calls(&write.trace, "write"), 1);
        }
    }
}

#[test]
fn impossible_counts_and_nonempty_zero_writes_fail_closed() {
    let mut read = Case::new(7);
    read.source.impossible = true;
    read.pre_failure(SealFailure::InvalidIoCount);
    let mut write = Case::new(7);
    write.stage.impossible = true;
    write.pre_failure(SealFailure::InvalidIoCount);
    let mut zero = Case::new(7);
    zero.stage.chunks = 0;
    zero.pre_failure(SealFailure::ZeroWrite);
    let mut probe = Case::new(7);
    probe.limits.source_bytes = 7;
    probe.source.impossible_after = Some(7);
    probe.pre_failure(SealFailure::InvalidIoCount);
    assert_eq!(probe.source.requests, [7, 1]);
}

#[test]
fn checker_cipher_and_flush_failures_cannot_publish_or_retry() {
    let mut finish = Case::new(7);
    finish.checker.fail_finish = true;
    finish.pre_failure(SealFailure::Check);
    assert_eq!(calls(&finish.trace, "complete"), 0);
    let mut cipher = Case::new(7);
    cipher.cipher.fail = true;
    cipher.pre_failure(SealFailure::Cipher);
    assert_eq!(calls(&cipher.trace, "cipher"), 1);
    assert_eq!(calls(&cipher.trace, "finish"), 0);
    for cipher_flush in [false, true] {
        let mut flush = Case::new(7);
        flush.stage.fail_flush = true;
        flush.cipher.flush = cipher_flush;
        let report = flush.pre_failure(SealFailure::StageFlush);
        assert_eq!(calls(&flush.trace, "flush"), 1);
        assert_eq!(
            report.source_cleanup,
            if cipher_flush {
                Cleanup::Cleaned
            } else {
                Cleanup::NotRequired
            }
        );
    }
    let mut ignored_flush = Case::new(7);
    ignored_flush.cipher.flush = true;
    ignored_flush.cipher.ignore_error = true;
    ignored_flush.stage.fail_flush = true;
    ignored_flush.pre_failure(SealFailure::StageFlush);
    assert_eq!(calls(&ignored_flush.trace, "flush"), 1);
}

#[test]
fn consuming_a_prefix_or_all_bytes_without_observing_eof_is_not_completion() {
    for chunk in [3, 7] {
        let mut case = Case::new(7);
        case.source.chunks = chunk;
        case.cipher.stop_after_read = true;
        case.pre_failure(SealFailure::IncompleteInput);
        assert_eq!(case.source.read, chunk as u64);
        assert_eq!(calls(&case.trace, "read"), 1);
        assert_eq!(calls(&case.trace, "finish"), 0);
    }
}

#[test]
fn first_stream_error_survives_later_cipher_failure_or_deadline() {
    let mut case = Case::new(7);
    case.source.read_error = Some(io::ErrorKind::Interrupted);
    case.cipher.ignore_error = true;
    case.cipher.fail = true;
    case.trace.borrow_mut().change = Some(("cipher_done", 100));
    case.pre_failure(SealFailure::SourceRead);
    assert_eq!(calls(&case.trace, "read"), 1);
}

#[test]
fn failed_reads_scrub_the_delegated_buffer_and_return_only_fixed_io_text() {
    struct Inspect;
    impl SealCipher for Inspect {
        fn encrypt(
            &self,
            input: &mut dyn Read,
            _: &mut dyn Write,
        ) -> Result<CipherCounts, SealPortError> {
            let mut buffer = [0xaa_u8; 131_072];
            let error = input.read(&mut buffer).unwrap_err();
            assert_eq!(error.to_string(), "seal_stream_failed");
            assert!(buffer[..65_536].iter().all(|b| *b == 0));
            assert!(buffer[65_536..].iter().all(|b| *b == 0xaa));
            Err(SealPortError)
        }
    }
    for read_error in [false, true] {
        let mut case = Case::new(7);
        if read_error {
            case.source.read_error = Some(io::ErrorKind::Other);
        } else {
            case.checker.fail_feed = true;
        }
        let report = ArchiveSealer::new(case.limits)
            .unwrap()
            .seal_with_clock(
                &mut case.source,
                Box::new(case.checker),
                &Inspect,
                &mut case.stage,
                &FakeClock(case.trace.clone()),
            )
            .unwrap_err();
        assert_eq!(
            report.cause,
            if read_error {
                SealFailure::SourceRead
            } else {
                SealFailure::Check
            }
        );
        assert_eq!(calls(&case.trace, "abort"), 1);
    }
}

#[test]
fn sealing_containers_and_reports_do_not_implicitly_gain_raw_serialization() {
    macro_rules! assert_not_impl {
        ($type:ty, $trait:path) => {{
            trait AmbiguousIfImpl<A> {
                fn marker() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            struct Forbidden;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Forbidden> for T {}
            let _ = <$type as AmbiguousIfImpl<_>>::marker;
        }};
    }
    assert_not_impl!(ArchiveSealer, std::fmt::Debug);
    assert_not_impl!(ArchiveSealer, serde::Serialize);
    assert_not_impl!(SealedCounts, serde::Serialize);
    assert_not_impl!(SealFailureReport, serde::Serialize);
    assert_not_impl!(SealPortError, std::error::Error);
}

#[test]
fn cleanup_uncertainty_preserves_primary_cause_and_attempts_each_only_once() {
    for source in [CleanupOutcome::Cleaned, CleanupOutcome::Unknown] {
        for stage in [CleanupOutcome::Cleaned, CleanupOutcome::Unknown] {
            let mut case = Case::new(7);
            case.source.cleanup = source;
            case.stage.cleanup = stage;
            case.cipher.fail = true;
            let report = case.pre_failure(SealFailure::Cipher);
            assert_eq!(report.source_cleanup, Cleanup::from(source));
            assert_eq!(report.stage_cleanup, Cleanup::from(stage));
            assert_eq!(calls(&case.trace, "cancel"), 1);
        }
    }
}

#[test]
fn publication_absent_unknown_and_late_ack_never_trigger_unsafe_abort_or_retry() {
    for outcome in [
        PublishOutcome::Published,
        PublishOutcome::NotPublished,
        PublishOutcome::Unknown,
    ] {
        for late in [false, true] {
            let mut case = Case::new(7);
            case.stage.outcome = outcome;
            if late {
                case.trace.borrow_mut().change = Some(("publish", 100));
            }
            let result = case.run();
            if outcome == PublishOutcome::Published && !late {
                assert!(result.is_ok());
            } else {
                let report = result.unwrap_err();
                assert_eq!(
                    report.cause,
                    if late {
                        SealFailure::Deadline
                    } else {
                        SealFailure::PublicationNotConfirmed
                    }
                );
                assert_eq!(
                    report.publication,
                    match outcome {
                        PublishOutcome::Published => Publication::Published,
                        PublishOutcome::NotPublished => Publication::NotPublished,
                        PublishOutcome::Unknown => Publication::Unknown,
                    }
                );
                assert_eq!(report.source_cleanup, Cleanup::NotRequired);
                assert_eq!(
                    report.stage_cleanup,
                    if outcome == PublishOutcome::NotPublished {
                        Cleanup::Cleaned
                    } else {
                        Cleanup::NotRequired
                    }
                );
            }
            assert_eq!(calls(&case.trace, "publish"), 1);
            assert_eq!(calls(&case.trace, "cancel"), 0);
            assert_eq!(
                calls(&case.trace, "abort"),
                usize::from(outcome == PublishOutcome::NotPublished)
            );
        }
    }
}

#[test]
fn unwinding_cleans_only_unfinished_and_definitely_unpublished_resources() {
    for point in [
        "caps",
        "cipher",
        "read",
        "feed",
        "write",
        "cipher_done",
        "finish",
        "complete",
        "flush",
        "publish",
    ] {
        let mut case = Case::new(7);
        case.trace.borrow_mut().panic_on = Some(point);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| case.run()));
        assert!(result.is_err());
        assert_eq!(
            calls(&case.trace, "cancel"),
            usize::from(!["flush", "publish"].contains(&point)),
            "{point}"
        );
        assert_eq!(
            calls(&case.trace, "abort"),
            usize::from(point != "publish"),
            "{point}"
        );
        assert!(calls(&case.trace, "publish") <= 1);
    }
}

#[test]
fn public_recipient_session_failure_cleans_without_reading_source_or_writing_stage() {
    use openwrt_mcp_runtime::protection::{
        CryptoLimits, EncryptionSession, KeyLimits, ProtectionError,
    };
    use std::sync::Arc;
    let keys = Arc::new(super::super::Source {
        error: Some(ProtectionError::SourceUnavailable),
        ..super::super::Source::new(b"synthetic")
    });
    let provider = Arc::new(super::super::EncryptOnly::new("synthetic"));
    let cipher = EncryptionSession::new(
        keys.clone(),
        provider.clone(),
        KeyLimits::default(),
        CryptoLimits::default(),
    )
    .unwrap();
    let mut case = Case::new(7);
    let report = ArchiveSealer::new(case.limits)
        .unwrap()
        .seal_with_clock(
            &mut case.source,
            Box::new(case.checker),
            &cipher,
            &mut case.stage,
            &FakeClock(case.trace.clone()),
        )
        .unwrap_err();
    assert_eq!(report.cause, SealFailure::Cipher);
    assert_eq!(case.trace.borrow().events, ["caps", "cancel", "abort"]);
    assert_eq!(keys.requests().len(), 1);
    assert_eq!(provider.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}
