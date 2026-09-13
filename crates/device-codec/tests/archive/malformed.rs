use super::*;

fn check_header(mut h: [u8; 512]) -> Result<ArchiveSummary, ArchiveError> {
    checksum(&mut h);
    check(&[("a", 0)], &terminated(h.to_vec()), 512)
}

#[test]
fn unsafe_archive_paths_and_hidden_text_after_terminators_fail() {
    for path in [
        "", "/a", "a/", "a//b", "./a", "a/../b", "../a", "a/./b", "a\\b", "C:a", "a b", "a\tb",
        "é", "a\n", "a:b", "a?b", "a*b",
    ] {
        assert_eq!(
            ExpectedFile::new(path, 0).err(),
            Some(ArchiveError::InvalidPath)
        );
        assert_eq!(
            check_header(header(path, 0, false)),
            Err(ArchiveError::InvalidPath)
        );
    }
    for range in [0..100, 345..500, 265..297, 297..329] {
        let mut h = header("a", 0, false);
        h[range.clone()].fill(0);
        h[range.start] = b'a';
        h[range.start + 2] = b'x';
        assert_eq!(check_header(h), Err(ArchiveError::InvalidHeader));
    }
    for i in [265, 297] {
        let mut h = header("a", 0, false);
        h[i] = b' ';
        assert_eq!(check_header(h), Err(ArchiveError::InvalidHeader));
    }
}

#[test]
fn all_nonregular_types_links_extensions_and_ambiguous_header_forms_are_rejected() {
    for kind in 0_u8..=255 {
        if matches!(kind, 0 | b'0') {
            continue;
        }
        let mut h = header("a", 0, false);
        h[156] = kind;
        assert_eq!(check_header(h), Err(ArchiveError::UnsupportedEntry));
    }
    let mut h = header("a", 0, false);
    h[156] = 0;
    assert!(check_header(h).is_ok());
    for (gnu, offset) in [
        (false, 157),
        (false, 256),
        (false, 500),
        (false, 511),
        (true, 345),
        (true, 386),
        (true, 482),
        (true, 511),
    ] {
        let mut h = header("a", 0, gnu);
        h[offset] = 1;
        assert_eq!(check_header(h), Err(ArchiveError::InvalidHeader));
    }
    for magic in [
        b"\0\0\0\0\0\0\0\0",
        b"ustar\x0001",
        b"ustar\0\0\0",
        b"ustar  X",
    ] {
        let mut h = header("a", 0, false);
        h[257..265].copy_from_slice(magic);
        assert_eq!(check_header(h), Err(ArchiveError::InvalidHeader));
    }
}

#[test]
fn numeric_fields_reject_padding_only_signs_base256_and_digits_after_terminators() {
    for range in [100..108, 108..116, 116..124, 124..136, 136..148] {
        for value in [
            b"".as_slice(),
            b" ",
            b"-1",
            b"+1",
            b"8",
            b"9",
            b"1\0".as_slice(),
            b"1 2",
            b"\x80\0\0\0",
        ] {
            let mut h = header("a", 0, false);
            h[range.clone()].fill(0);
            h[range.start..range.start + value.len()].copy_from_slice(value);
            if value == b"1\0" {
                h[range.start + 2] = b'2';
            }
            assert_eq!(check_header(h), Err(ArchiveError::InvalidHeader));
        }
    }
    for range in [329..337, 337..345] {
        let mut zero = header("a", 0, false);
        octal(&mut zero[range.clone()], 0);
        assert!(check_header(zero).is_ok());
        let mut nonzero = header("a", 0, false);
        octal(&mut nonzero[range], 1);
        assert_eq!(check_header(nonzero), Err(ArchiveError::InvalidHeader));
    }
    let mut privileged = header("a", 0, false);
    octal(&mut privileged[100..108], 0o4600);
    assert_eq!(check_header(privileged), Err(ArchiveError::InvalidHeader));
    let mut too_large = header("a", 0, false);
    octal(&mut too_large[124..136], 8 * 1024 * 1024 + 1);
    assert_eq!(check_header(too_large), Err(ArchiveError::LimitExceeded));
    let mut numeric = header("a", 0, false);
    numeric[136..148].copy_from_slice(b"777777777777");
    assert!(check_header(numeric).is_ok());
}

#[test]
fn checksum_is_unsigned_exact_and_requires_an_unambiguous_octal_field() {
    let original = header("a", 0, false);
    for offset in 0..512 {
        let mut h = original;
        h[offset] ^= 0x80;
        assert_eq!(
            check(&[("a", 0)], &terminated(h.to_vec()), 512),
            Err(ArchiveError::InvalidHeader)
        );
    }
    for value in [
        b"        ",
        b"\0\0\0\0\0\0\0\0",
        b"0001\0".as_slice(),
        b"-0000001",
        b"\x800000000",
    ] {
        let mut h = original;
        h[148..156].fill(0);
        h[148..148 + value.len()].copy_from_slice(value);
        assert_eq!(
            check(&[("a", 0)], &terminated(h.to_vec()), 512),
            Err(ArchiveError::InvalidHeader)
        );
    }
    // Space/NUL padding and full-width octal are separately valid forms.
    let mut h = header("a", 0, false);
    h[100..108].copy_from_slice(b"  600 \0\0");
    h[108..116].copy_from_slice(b"00000000");
    h[136..148].copy_from_slice(b" 1234   \0\0\0\0");
    checksum(&mut h);
    let sum: u64 = h
        .iter()
        .enumerate()
        .map(|(i, b)| {
            if (148..156).contains(&i) {
                32
            } else {
                u64::from(*b)
            }
        })
        .sum();
    let field = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(field.as_bytes());
    assert!(check(&[("a", 0)], &terminated(h.to_vec()), 512).is_ok());
}
