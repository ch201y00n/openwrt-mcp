use super::*;
use openwrt_mcp_device_codec::gzip::{
    EXPANSION_SLACK_BYTES, MAX_COMMENT_BYTES, MAX_EXPANSION_RATIO, MAX_EXTRA_BYTES,
    MAX_HEADER_BYTES, MAX_NAME_BYTES, MAX_SOURCE_BYTES, OUTPUT_BUFFER_BYTES,
};

#[test]
fn each_optional_bound_and_combined_header_bound_is_exact() {
    let base = fixed_fixture();
    let extra = vec![42; MAX_EXTRA_BYTES];
    assert!(check(&optional(&base, 4, &extra, b"", b""), 7, 1).is_ok());
    assert_eq!(
        check(
            &optional(&base, 4, &vec![42; MAX_EXTRA_BYTES + 1], b"", b""),
            7,
            1
        ),
        Err(GzipError::LimitExceeded)
    );
    for (flag, limit) in [(8, MAX_NAME_BYTES), (16, MAX_COMMENT_BYTES)] {
        let mut field = vec![42; limit];
        field[limit - 1] = 0;
        assert!(check(&optional(&base, flag, b"", &field, &field), 7, 1).is_ok());
        field[limit - 1] = 42;
        field.push(0);
        assert_eq!(
            check(&optional(&base, flag, b"", &field, &field), 7, 1),
            Err(GzipError::LimitExceeded)
        );
        let mut unterminated = base[..10].to_vec();
        unterminated[3] = flag;
        unterminated.extend_from_slice(&[42; 1024]);
        assert_eq!(
            check(&unterminated, 7, 17),
            Err(GzipError::IncompleteStream)
        );
    }
    let name = [42; 1023].into_iter().chain([0]).collect::<Vec<_>>();
    let extra = vec![42; MAX_HEADER_BYTES - 12 - 2 * 1024 - 2];
    assert!(check(&optional(&base, 30, &extra, &name, &name), 7, 1).is_ok());
    assert_eq!(
        check(
            &optional(&base, 30, &[extra, vec![42]].concat(), &name, &name),
            7,
            1
        ),
        Err(GzipError::LimitExceeded)
    );
}

#[test]
fn output_drains_across_many_small_buffers_and_large_input_chunks() {
    assert_eq!(OUTPUT_BUFFER_BYTES, 4096);
    let payload: Vec<_> = (0..200_000).map(|i| (i % 251) as u8).collect();
    let plain = terminated(member("fixture/a", &payload, false));
    for bytes in [compressed(&plain), stored(&plain)] {
        for chunk in [1, 7, 4096, MAX_CHUNK_BYTES] {
            let result = check(&bytes, payload.len() as u64, chunk).unwrap();
            assert_eq!(result.archive_bytes, plain.len() as u64);
        }
    }
    assert_eq!(
        validator(0).feed(&vec![0; MAX_CHUNK_BYTES + 1]),
        Err(GzipError::LimitExceeded)
    );
}

#[test]
fn final_ratio_is_chunk_invariant_and_excludes_header_metadata() {
    assert_eq!((EXPANSION_SLACK_BYTES, MAX_EXPANSION_RATIO), (1048576, 128));
    for (size, accepted) in [(900_000, true), (3_000_000, false)] {
        let plain = terminated(member("fixture/a", &vec![0; size], false));
        let base = compressed(&plain);
        for bytes in [
            base.clone(),
            optional(&base, 4, &[42; MAX_EXTRA_BYTES], b"", b""),
        ] {
            for chunk in [1, 17, 4096, MAX_CHUNK_BYTES] {
                let result = check(&bytes, size as u64, chunk);
                if accepted {
                    assert!(result.is_ok());
                } else {
                    assert_eq!(result, Err(GzipError::LimitExceeded));
                }
            }
        }
    }
}

#[test]
fn inner_file_and_tail_limits_remain_active_during_inflate() {
    let oversized = header("fixture/a", 8 * 1024 * 1024 + 1, false);
    assert_eq!(
        check(&compressed(&oversized), 0, 1),
        Err(GzipError::LimitExceeded)
    );
    let mut tail = member("fixture/a", b"", false);
    tail.resize(tail.len() + 32769, 0);
    assert_eq!(
        check(&compressed(&tail), 0, 1),
        Err(GzipError::LimitExceeded)
    );
}

#[test]
fn expansion_equality_accepts_but_five_fewer_deflate_bytes_reject_even_with_metadata() {
    // Exactly 2 MiB tar, with 8192 DEFLATE bytes: 1 MiB + 128 * 8192.
    // Prefix stored bytes plus non-final empty blocks tune the denominator
    // without changing the expanded archive or relying on compressor ratios.
    let payload_size = 2 * 1024 * 1024 - 1536;
    let plain = terminated(member("fixture/a", &vec![0; payload_size], false));
    assert_eq!(plain.len(), 2 * 1024 * 1024);
    let mut tuned = None;
    for prefix in 0..64 {
        let rest = compressed(&plain[prefix..]);
        let base = 5 + prefix + rest.len() - 18;
        if base < 8192 && (8192 - base).is_multiple_of(5) {
            let padding = (8192 - base) / 5;
            let mut bytes = rest[..10].to_vec();
            for _ in 0..padding {
                bytes.extend_from_slice(&[0, 0, 0, 255, 255]);
            }
            bytes.push(0);
            bytes.extend_from_slice(&(prefix as u16).to_le_bytes());
            bytes.extend_from_slice(&(!(prefix as u16)).to_le_bytes());
            bytes.extend_from_slice(&plain[..prefix]);
            bytes.extend_from_slice(&rest[10..rest.len() - 8]);
            bytes.extend_from_slice(&crc(&plain).to_le_bytes());
            bytes.extend_from_slice(&(plain.len() as u32).to_le_bytes());
            tuned = Some(bytes);
            break;
        }
    }
    let bytes = tuned.expect("fixture compressor must permit finite denominator tuning");
    assert_eq!(bytes.len() - 18, 8192);
    for chunk in [1, MAX_CHUNK_BYTES] {
        assert!(check(&bytes, payload_size as u64, chunk).is_ok());
    }
    let mut short = bytes;
    short.drain(10..15);
    for bytes in [
        short.clone(),
        optional(&short, 4, &[42; MAX_EXTRA_BYTES], b"", b""),
    ] {
        for chunk in [1, MAX_CHUNK_BYTES] {
            assert_eq!(
                check(&bytes, payload_size as u64, chunk),
                Err(GzipError::LimitExceeded)
            );
        }
    }
}

#[test]
fn compressed_source_budget_is_enforced_even_without_expanded_output() {
    assert_eq!(MAX_SOURCE_BYTES, 75497472);
    let mut v = validator(0);
    v.feed(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255]).unwrap();
    // Repeated non-final empty stored blocks. A fixed 64 KiB buffer prevents
    // this 72 MiB source-limit fixture from allocating the source stream.
    let block = [0, 0, 0, 255, 255];
    let mut buffer = [0; MAX_CHUNK_BYTES];
    let mut sent = 0_u64;
    let remaining = MAX_SOURCE_BYTES - 10;
    while sent < remaining {
        let count = (remaining - sent).min(MAX_CHUNK_BYTES as u64) as usize;
        for (i, byte) in buffer[..count].iter_mut().enumerate() {
            *byte = block[((sent + i as u64) % 5) as usize];
        }
        v.feed(&buffer[..count]).unwrap();
        sent += count as u64;
    }
    v.feed(&[]).unwrap();
    assert_eq!(v.feed(&[0]), Err(GzipError::LimitExceeded));
    assert_eq!(v.feed(&[]), Err(GzipError::LimitExceeded));
    assert_eq!(v.finish(), Err(GzipError::LimitExceeded));
}
