use super::*;

#[test]
fn fixed_header_requires_exact_magic_method_and_reserved_bits() {
    let base = fixed_fixture();
    for offset in [0, 1, 2] {
        for byte in 0..=255 {
            if byte == base[offset] {
                continue;
            }
            let mut bytes = base.clone();
            bytes[offset] = byte;
            assert_eq!(check(&bytes, 7, 1), Err(GzipError::InvalidHeader));
        }
    }
    for byte in 32..=255 {
        let mut bytes = base.clone();
        bytes[3] = byte;
        assert_eq!(
            check(&bytes, 7, MAX_CHUNK_BYTES),
            Err(GzipError::InvalidHeader)
        );
    }
}

#[test]
fn suffixes_second_members_and_later_feeds_are_never_discarded() {
    let base = fixed_fixture();
    for suffix in [vec![0], vec![255], base.clone(), vec![0; 512]] {
        let mut bytes = base.clone();
        bytes.extend_from_slice(&suffix);
        for chunk in [1, 17, MAX_CHUNK_BYTES] {
            assert_eq!(check(&bytes, 7, chunk), Err(GzipError::TrailingData));
        }
        let mut v = validator(7);
        v.feed(&base).unwrap();
        v.feed(&[]).unwrap();
        assert_eq!(v.feed(&suffix), Err(GzipError::TrailingData));
        assert_eq!(v.feed(&[]), Err(GzipError::TrailingData));
        assert_eq!(v.finish(), Err(GzipError::TrailingData));
    }
}

#[test]
fn valid_gzip_cannot_hide_invalid_incomplete_or_mismatched_tar() {
    let plain = terminated(member("fixture/a", b"public\n", false));
    let mut bad_padding = plain.clone();
    bad_padding[519] = 1;
    let mut bad_header = plain.clone();
    bad_header[0] ^= 1;
    for (bytes, error) in [
        (bad_padding, ArchiveError::InvalidPadding),
        (bad_header, ArchiveError::InvalidHeader),
        (plain[..1024].to_vec(), ArchiveError::IncompleteArchive),
        (
            terminated(member("unknown", b"public\n", false)),
            ArchiveError::ManifestMismatch,
        ),
        (
            terminated(member("fixture/a", b"wrong size", false)),
            ArchiveError::ManifestMismatch,
        ),
        (vec![0; 1024], ArchiveError::ManifestMismatch),
        (
            terminated(
                [
                    member("fixture/a", b"public\n", false),
                    member("fixture/a", b"public\n", false),
                ]
                .concat(),
            ),
            ArchiveError::ManifestMismatch,
        ),
    ] {
        assert_eq!(
            check(&compressed(&bytes), 7, 1),
            Err(GzipError::from(error))
        );
    }
}

#[test]
fn first_error_is_permanent_and_diagnostics_never_contain_input() {
    let base = fixed_fixture();
    let mut bad_crc = base.clone();
    let last = bad_crc.len() - 8;
    bad_crc[last] ^= 1;
    for bytes in [
        b"secret fixture header".to_vec(),
        bad_crc,
        vec![0; MAX_CHUNK_BYTES + 1],
        compressed(&[42; 512]),
    ] {
        let mut v = validator(7);
        let error = v.feed(&bytes).unwrap_err();
        assert_eq!(v.feed(&[]), Err(error));
        assert_eq!(v.feed(&base), Err(error));
        assert_eq!(v.finish(), Err(error));
        let text = format!("{error:?}: {error}");
        assert!(!text.contains("secret") && !text.contains("fixture"));
    }
    for (error, code) in [
        (GzipError::InvalidHeader, "invalid_gzip_header"),
        (GzipError::InvalidData, "invalid_gzip_data"),
        (GzipError::LimitExceeded, "gzip_limit_exceeded"),
        (GzipError::IncompleteStream, "incomplete_gzip_stream"),
        (GzipError::TrailingData, "trailing_gzip_data"),
        (GzipError::InvalidArchive, "invalid_gzip_archive"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
    }
}
