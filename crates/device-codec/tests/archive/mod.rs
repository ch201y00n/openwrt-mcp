//! Synthetic in-memory archive fixtures. No router paths or plaintext backups.
mod boundaries;
mod malformed;

use openwrt_mcp_device_codec::archive::{
    ArchiveError, ArchiveSummary, ArchiveValidator, ExpectedArchive, ExpectedFile, MAX_CHUNK_BYTES,
};

fn octal(field: &mut [u8], value: u64) {
    field.fill(0);
    let text = format!("{value:0width$o}", width = field.len() - 1);
    assert!(text.len() < field.len());
    field[..text.len()].copy_from_slice(text.as_bytes());
}
fn checksum(header: &mut [u8; 512]) {
    header[148..156].fill(b' ');
    let value = header.iter().map(|b| u64::from(*b)).sum();
    octal(&mut header[148..156], value);
}
fn header(name: &str, size: u64, gnu: bool) -> [u8; 512] {
    assert!(name.len() <= 100);
    let mut h = [0; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    for (from, to, value) in [
        (100, 108, 0o600),
        (108, 116, 0),
        (116, 124, 0),
        (124, 136, size),
        (136, 148, 1234),
    ] {
        octal(&mut h[from..to], value);
    }
    h[156] = b'0';
    h[257..265].copy_from_slice(if gnu { b"ustar  \0" } else { b"ustar\x0000" });
    h[265..269].copy_from_slice(b"root");
    h[297..301].copy_from_slice(b"root");
    checksum(&mut h);
    h
}
fn member(name: &str, body: &[u8], gnu: bool) -> Vec<u8> {
    let mut data = header(name, body.len() as u64, gnu).to_vec();
    data.extend_from_slice(body);
    data.resize(data.len().next_multiple_of(512), 0);
    data
}
fn terminated(mut members: Vec<u8>) -> Vec<u8> {
    members.resize(members.len() + 1024, 0);
    members
}
fn validator(files: &[(&str, u64)]) -> ArchiveValidator {
    let files: Vec<_> = files
        .iter()
        .map(|(name, size)| ExpectedFile::new(name, *size).unwrap())
        .collect();
    ArchiveValidator::new(ExpectedArchive::new(&files).unwrap())
}
fn check(
    files: &[(&str, u64)],
    bytes: &[u8],
    chunk: usize,
) -> Result<ArchiveSummary, ArchiveError> {
    let mut v = validator(files);
    for part in bytes.chunks(chunk) {
        v.feed(part)?;
    }
    v.finish()
}

#[test]
fn exact_posix_gnu_and_mixed_regular_records_return_only_counts() {
    for gnu in [false, true] {
        let mut bytes = member("fixture/a", b"public", gnu);
        bytes.extend(member("fixture/empty", b"", !gnu));
        let bytes = terminated(bytes);
        assert_eq!(
            check(&[("fixture/empty", 0), ("fixture/a", 6)], &bytes, 512),
            Ok(ArchiveSummary {
                files: 2,
                payload_bytes: 6,
                archive_bytes: 2560
            })
        );
        let summary = check(&[("fixture/a", 6), ("fixture/empty", 0)], &bytes, 1).unwrap();
        let rendered = format!("{summary:?}");
        assert!(!rendered.contains("fixture") && !rendered.contains("public"));
    }
}

#[test]
fn arbitrary_chunks_and_all_two_piece_splits_have_identical_results() {
    let payload: Vec<u8> = (0..513).map(|i| (i % 256) as u8).collect();
    let bytes = terminated(member("fixture/data", &payload, false));
    let files = [("fixture/data", 513)];
    let expected = check(&files, &bytes, MAX_CHUNK_BYTES).unwrap();
    for split in 0..=bytes.len() {
        let mut v = validator(&files);
        v.feed(&bytes[..split]).unwrap();
        v.feed(&[]).unwrap();
        v.feed(&bytes[split..]).unwrap();
        assert_eq!(v.finish(), Ok(expected));
    }
    for chunk in [1, 7, 31, 127, 511, 512, 513, 1023, MAX_CHUNK_BYTES] {
        assert_eq!(check(&files, &bytes, chunk), Ok(expected));
    }
}

#[test]
fn every_incomplete_prefix_fails_even_after_all_expected_headers_arrive() {
    for size in [0, 1, 511, 512, 513] {
        let bytes = terminated(member("fixture/file", &vec![0x91; size], false));
        for cut in 0..bytes.len() {
            assert_eq!(
                check(&[("fixture/file", size as u64)], &bytes[..cut], 512),
                Err(ArchiveError::IncompleteArchive),
                "size={size} cut={cut}"
            );
        }
        assert!(check(&[("fixture/file", size as u64)], &bytes, 512).is_ok());
    }
}

#[test]
fn manifest_requires_all_members_once_exact_sizes_and_no_unexpected_names() {
    let a = member("a", b"abc", false);
    let b = member("b", b"", true);
    for bytes in [
        terminated(a.clone()),
        terminated([a.clone(), a.clone()].concat()),
        terminated([a.clone(), member("other", b"", false)].concat()),
    ] {
        assert_eq!(
            check(&[("a", 3), ("b", 0)], &bytes, 71),
            Err(ArchiveError::ManifestMismatch)
        );
    }
    assert_eq!(
        check(&[("a", 2)], &terminated(a.clone()), 512),
        Err(ArchiveError::ManifestMismatch)
    );
    assert_eq!(
        check(&[("A", 3)], &terminated(a.clone()), 512),
        Err(ArchiveError::ManifestMismatch)
    );
    let expected = check(
        &[("a", 3), ("b", 0)],
        &terminated([a.clone(), b.clone()].concat()),
        512,
    )
    .unwrap();
    assert_eq!(
        check(&[("b", 0), ("a", 3)], &terminated([b, a].concat()), 513),
        Ok(expected)
    );
    assert_eq!(
        check(&[("a", 0)], &[0; 1024], 512),
        Err(ArchiveError::ManifestMismatch)
    );
}

#[test]
fn terminal_padding_is_fully_consumed_aligned_bounded_and_never_concatenated() {
    let body = member("a", b"x", false);
    for tail in [1024, 1536, 32768] {
        let mut bytes = body.clone();
        bytes.resize(bytes.len() + tail, 0);
        assert_eq!(
            check(&[("a", 1)], &bytes, 127).unwrap().archive_bytes,
            bytes.len() as u64
        );
    }
    for tail in [0, 1, 511, 512, 513, 1023, 1025, 32767] {
        let mut bytes = body.clone();
        bytes.resize(bytes.len() + tail, 0);
        assert_eq!(
            check(&[("a", 1)], &bytes, 513),
            Err(ArchiveError::IncompleteArchive)
        );
    }
    let mut excessive = body.clone();
    excessive.resize(excessive.len() + 32769, 0);
    assert_eq!(
        check(&[("a", 1)], &excessive, 512),
        Err(ArchiveError::LimitExceeded)
    );
    let mut concatenated = terminated(body.clone());
    concatenated.extend(terminated(member("b", b"", false)));
    assert_eq!(
        check(&[("a", 1), ("b", 0)], &concatenated, 1),
        Err(ArchiveError::InvalidPadding)
    );
    for offset in [body.len() + 512, body.len() + 1023] {
        let mut corrupted = terminated(body.clone());
        corrupted[offset] = 1;
        assert_eq!(
            check(&[("a", 1)], &corrupted, 513),
            Err(ArchiveError::InvalidPadding)
        );
    }
    let mut bad_member = terminated(body);
    bad_member[513] = 1;
    assert_eq!(
        check(&[("a", 1)], &bad_member, MAX_CHUNK_BYTES),
        Err(ArchiveError::InvalidPadding)
    );
}

#[test]
fn failed_feeds_latch_the_first_error_and_never_resume_or_finish_successfully() {
    let mut damaged = header("a", 0, false);
    damaged[4] ^= 1;
    for (data, error) in [
        (damaged.to_vec(), ArchiveError::InvalidHeader),
        (vec![0; MAX_CHUNK_BYTES + 1], ArchiveError::LimitExceeded),
    ] {
        let mut v = validator(&[("a", 0)]);
        assert_eq!(v.feed(&data), Err(error));
        assert_eq!(v.feed(&[]), Err(error));
        assert_eq!(v.feed(&terminated(member("a", b"", false))), Err(error));
        assert_eq!(v.finish(), Err(error));
    }
}

#[test]
fn errors_are_fixed_codes_without_untrusted_names_or_header_values() {
    for (error, code) in [
        (ArchiveError::InvalidPath, "invalid_archive_path"),
        (ArchiveError::InvalidManifest, "invalid_archive_manifest"),
        (ArchiveError::InvalidHeader, "invalid_archive_header"),
        (ArchiveError::UnsupportedEntry, "unsupported_archive_entry"),
        (ArchiveError::ManifestMismatch, "archive_manifest_mismatch"),
        (ArchiveError::LimitExceeded, "archive_limit_exceeded"),
        (ArchiveError::IncompleteArchive, "incomplete_archive"),
        (ArchiveError::InvalidPadding, "invalid_archive_padding"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
    }
    let marker = "synthetic-private-name";
    let failure = check(&[("a", 0)], &terminated(member(marker, b"", false)), 512).unwrap_err();
    assert_eq!(failure, ArchiveError::ManifestMismatch);
    assert!(!format!("{failure:?} {failure}").contains(marker));
}
