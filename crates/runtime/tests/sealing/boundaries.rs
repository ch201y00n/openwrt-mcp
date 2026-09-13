use super::*;

#[test]
fn limits_validate_exact_defaults_hard_caps_and_invalid_boundaries() {
    assert_eq!(
        SealLimits::default(),
        SealLimits {
            source_bytes: 67_108_864,
            ciphertext_bytes: 68_157_440,
            timeout_ms: 30_000
        }
    );
    for valid in [
        SealLimits::default(),
        SealLimits {
            source_bytes: 1,
            ciphertext_bytes: 1,
            timeout_ms: 1,
        },
        SealLimits {
            source_bytes: 75_497_472,
            ciphertext_bytes: 76_546_048,
            timeout_ms: 300_000,
        },
    ] {
        assert!(ArchiveSealer::new(valid).is_ok());
    }
    for invalid in [
        SealLimits {
            source_bytes: 0,
            ..SealLimits::default()
        },
        SealLimits {
            source_bytes: 75_497_473,
            ..SealLimits::default()
        },
        SealLimits {
            source_bytes: u64::MAX,
            ..SealLimits::default()
        },
        SealLimits {
            ciphertext_bytes: 0,
            ..SealLimits::default()
        },
        SealLimits {
            ciphertext_bytes: 76_546_049,
            ..SealLimits::default()
        },
        SealLimits {
            ciphertext_bytes: u64::MAX,
            ..SealLimits::default()
        },
        SealLimits {
            timeout_ms: 0,
            ..SealLimits::default()
        },
        SealLimits {
            timeout_ms: 300_001,
            ..SealLimits::default()
        },
        SealLimits {
            timeout_ms: u64::MAX,
            ..SealLimits::default()
        },
    ] {
        assert_eq!(
            ArchiveSealer::new(invalid).err(),
            Some(SealFailure::InvalidLimits)
        );
    }
}

#[test]
fn exact_input_cap_uses_one_byte_probe_and_never_delivers_overflow() {
    for size in [1, 7, 65_536, 65_537] {
        let mut exact = Case::new(size);
        exact.limits.source_bytes = size;
        exact.cipher.probe_again = true;
        assert_eq!(exact.run().unwrap().source_bytes, size);
        assert_eq!(exact.source.requests.last(), Some(&1));
        assert_eq!(
            calls(&exact.trace, "read"),
            size.div_ceil(65_536) as usize + 1
        );
        let mut overflow = Case::new(size + 1);
        overflow.limits.source_bytes = size;
        overflow.pre_failure(SealFailure::SourceLimit);
        assert_eq!(overflow.source.requests.last(), Some(&1));
        assert_eq!(overflow.stage.written, size);
        assert_eq!(
            calls(&overflow.trace, "feed"),
            size.div_ceil(65_536) as usize
        );
    }
}

#[test]
fn exact_ciphertext_limit_includes_final_cipher_bytes_without_extra_delegation() {
    for size in [1, 7, 65_536, 65_537] {
        let mut exact = Case::new(size);
        exact.limits.ciphertext_bytes = size + 1;
        exact.cipher.finish_write = true;
        assert_eq!(exact.run().unwrap().ciphertext_bytes, size + 1);
        let mut overflow = Case::new(size);
        overflow.limits.ciphertext_bytes = size;
        overflow.cipher.finish_write = true;
        overflow.pre_failure(SealFailure::CiphertextLimit);
        assert_eq!(overflow.stage.written, size);
        assert_eq!(
            calls(&overflow.trace, "write"),
            size.div_ceil(65_536) as usize
        );
    }
}

#[test]
fn short_io_and_large_requests_are_bounded_without_losing_bytes() {
    for (read_chunk, write_chunk) in [(1, 1), (7, 3), (65_536, 13), (usize::MAX, usize::MAX)] {
        let mut case = Case::new(131_077);
        case.source.chunks = read_chunk;
        case.stage.chunks = write_chunk;
        assert_eq!(case.run().unwrap().ciphertext_bytes, 131_077);
        assert!(
            case.source
                .requests
                .iter()
                .chain(&case.stage.requests)
                .all(|n| *n > 0 && *n <= 65_536)
        );
        assert_eq!(case.stage.written, case.source.read);
    }
}

#[test]
fn maximum_source_is_counted_with_bounded_fixture_memory() {
    let mut case = Case::new(75_497_472);
    case.limits.source_bytes = 75_497_472;
    case.limits.ciphertext_bytes = 76_546_048;
    let report = case.run().unwrap();
    assert_eq!(report.source_bytes, 75_497_472);
    assert_eq!(case.source.requests.len(), 1153);
    assert!(case.source.requests.iter().all(|n| *n <= 65_536));
}

#[test]
fn checker_counts_are_finite_and_bound_to_independent_input() {
    let base = CheckCounts {
        source_bytes: 7,
        expanded_bytes: 7,
        payload_bytes: 0,
        files: 1,
    };
    for summary in [
        CheckCounts {
            source_bytes: 6,
            ..base
        },
        CheckCounts {
            source_bytes: u64::MAX,
            ..base
        },
        CheckCounts { files: 0, ..base },
        CheckCounts {
            files: 4097,
            ..base
        },
        CheckCounts {
            expanded_bytes: 0,
            ..base
        },
        CheckCounts {
            expanded_bytes: 75_497_473,
            ..base
        },
        CheckCounts {
            payload_bytes: 8,
            ..base
        },
        CheckCounts {
            expanded_bytes: 75_497_472,
            payload_bytes: 67_108_865,
            ..base
        },
    ] {
        let mut case = Case::new(7);
        case.checker.summary = Some(summary);
        case.pre_failure(if summary.source_bytes != 7 {
            SealFailure::CountMismatch
        } else {
            SealFailure::InvalidSummary
        });
        assert_eq!(calls(&case.trace, "complete"), 0);
    }
    let mut case = Case::new(7);
    case.checker.summary = Some(CheckCounts {
        expanded_bytes: 75_497_472,
        payload_bytes: 67_108_864,
        files: 4096,
        ..base
    });
    assert_eq!(case.run().unwrap().files, 4096);
}

#[test]
fn expiry_at_each_callback_stops_later_work_with_truthful_cleanup() {
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
        case.trace.borrow_mut().change = Some((point, 100));
        let result = case.run().unwrap_err();
        assert_eq!(result.cause, SealFailure::Deadline, "{point}");
        if point == "publish" {
            assert_eq!(result.publication, Publication::Published);
            assert_eq!(calls(&case.trace, "abort"), 0);
        } else {
            assert_eq!(result.publication, Publication::NotAttempted);
            assert_eq!(calls(&case.trace, "abort"), 1);
            assert_eq!(calls(&case.trace, "publish"), 0);
        }
        assert_eq!(
            calls(&case.trace, "cancel"),
            usize::from(!["complete", "flush", "publish"].contains(&point))
        );
    }
}

#[test]
fn every_monotonic_clock_boundary_and_before_after_threshold_is_checked() {
    let mut baseline = Case::new(7);
    baseline.run().unwrap();
    let clock_calls = baseline.trace.borrow().clock_calls;
    assert!(clock_calls >= 20);
    for jump in 2..=clock_calls {
        let mut case = Case::new(7);
        case.trace.borrow_mut().clock_jump = Some(jump);
        assert_eq!(
            case.run().unwrap_err().cause,
            SealFailure::Deadline,
            "clock read {jump}"
        );
    }
    for point in [
        "caps",
        "read",
        "feed",
        "write",
        "cipher_done",
        "finish",
        "complete",
        "flush",
        "publish",
    ] {
        let mut before = Case::new(7);
        before.trace.borrow_mut().change = Some((point, 99));
        assert!(before.run().is_ok());
        let mut reverse = Case::new(7);
        reverse.trace.borrow_mut().time = 10;
        reverse.trace.borrow_mut().change = Some((point, 9));
        assert_eq!(reverse.run().unwrap_err().cause, SealFailure::ClockReversed);
    }
}

#[test]
fn clock_arithmetic_near_maximum_does_not_overflow() {
    let mut case = Case::new(7);
    case.trace.borrow_mut().time = u64::MAX - 1;
    case.trace.borrow_mut().change = Some(("read", u64::MAX));
    assert!(case.run().is_ok());
}
