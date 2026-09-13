use super::*;
use openwrt_mcp_device_codec::archive::{
    MAX_ARCHIVE_BYTES, MAX_COMPONENT_BYTES, MAX_FILE_BYTES, MAX_FILES, MAX_PATH_BYTES,
    MAX_PATH_COMPONENTS, MAX_PAYLOAD_BYTES, MAX_TAIL_BYTES,
};

#[test]
fn path_components_width_and_exact_posix_prefix_limits_are_bounded() {
    assert_eq!(
        (MAX_PATH_BYTES, MAX_PATH_COMPONENTS, MAX_COMPONENT_BYTES),
        (256, 32, 100)
    );
    for count in [32, 33] {
        let path = vec!["a"; count].join("/");
        assert_eq!(ExpectedFile::new(&path, 0).is_ok(), count == 32);
    }
    let name = "n".repeat(100);
    assert!(ExpectedFile::new(&name, 0).is_ok());
    assert_eq!(
        ExpectedFile::new(&"n".repeat(101), 0).err(),
        Some(ArchiveError::InvalidPath)
    );
    let prefix = format!("{}/{}", "p".repeat(100), "q".repeat(54));
    let path = format!("{prefix}/{name}");
    assert_eq!(path.len(), 256);
    let mut h = header(&name, 0, false);
    h[345..500].copy_from_slice(prefix.as_bytes());
    checksum(&mut h);
    assert!(check(&[(&path, 0)], &terminated(h.to_vec()), 1).is_ok());
    assert_eq!(
        ExpectedFile::new(&format!("{path}/x"), 0).err(),
        Some(ArchiveError::InvalidPath)
    );
    for gnu in [false, true] {
        let h = header(&name, 0, gnu);
        assert!(check(&[(&name, 0)], &terminated(h.to_vec()), 512).is_ok());
    }
}

#[test]
fn manifest_rejects_empty_duplicates_and_file_ancestors_in_every_order() {
    assert_eq!(
        ExpectedArchive::new(&[]).err(),
        Some(ArchiveError::InvalidManifest)
    );
    for names in [
        vec!["a", "a"],
        vec!["a", "a/b"],
        vec!["a/b", "a"],
        vec!["a", "a-", "a/b"],
        vec!["a/b/c", "a", "a-"],
    ] {
        let files: Vec<_> = names
            .iter()
            .map(|p| ExpectedFile::new(p, 0).unwrap())
            .collect();
        assert_eq!(
            ExpectedArchive::new(&files).err(),
            Some(ArchiveError::InvalidManifest)
        );
    }
    let files: Vec<_> = ["a-", "a.b", "a@b", "a+b", "a/b", "A/B"]
        .iter()
        .map(|p| ExpectedFile::new(p, 0).unwrap())
        .collect();
    assert!(ExpectedArchive::new(&files).is_ok());
}

#[test]
fn file_count_exact_boundary_has_complete_unique_members_not_silent_truncation() {
    assert_eq!(MAX_FILES, 4096);
    let names: Vec<_> = (0..MAX_FILES).map(|i| format!("fixture/{i:04}")).collect();
    let files: Vec<_> = names
        .iter()
        .map(|p| ExpectedFile::new(p, 0).unwrap())
        .collect();
    let mut v = ArchiveValidator::new(ExpectedArchive::new(&files).unwrap());
    for name in names.iter().rev() {
        v.feed(&header(name, 0, false)).unwrap();
    }
    v.feed(&[0; 1024]).unwrap();
    assert_eq!(
        v.finish(),
        Ok(ArchiveSummary {
            files: 4096,
            payload_bytes: 0,
            archive_bytes: 4096 * 512 + 1024
        })
    );
    let mut excessive = files;
    excessive.push(ExpectedFile::new("extra", 0).unwrap());
    assert_eq!(
        ExpectedArchive::new(&excessive).err(),
        Some(ArchiveError::LimitExceeded)
    );
}

#[test]
fn payload_and_feed_limits_are_checked_without_retaining_whole_file_bodies() {
    assert_eq!(
        (MAX_ARCHIVE_BYTES, MAX_PAYLOAD_BYTES, MAX_FILE_BYTES),
        (75_497_472, 67_108_864, 8_388_608)
    );
    assert_eq!((MAX_CHUNK_BYTES, MAX_TAIL_BYTES), (65_536, 32_768));
    assert!(ExpectedFile::new("a", MAX_FILE_BYTES).is_ok());
    assert_eq!(
        ExpectedFile::new("a", MAX_FILE_BYTES + 1).err(),
        Some(ArchiveError::LimitExceeded)
    );
    assert_eq!(
        ExpectedFile::new("a", u64::MAX).err(),
        Some(ArchiveError::LimitExceeded)
    );
    let names: Vec<_> = (0..8).map(|i| format!("fixture/{i}")).collect();
    let files: Vec<_> = names
        .iter()
        .map(|p| ExpectedFile::new(p, MAX_FILE_BYTES).unwrap())
        .collect();
    let mut v = ArchiveValidator::new(ExpectedArchive::new(&files).unwrap());
    let chunk = [0xa5; MAX_CHUNK_BYTES];
    for name in &names {
        v.feed(&header(name, MAX_FILE_BYTES, false)).unwrap();
        for _ in 0..MAX_FILE_BYTES / MAX_CHUNK_BYTES as u64 {
            v.feed(&chunk).unwrap();
        }
    }
    v.feed(&[0; 1024]).unwrap();
    assert_eq!(
        v.finish(),
        Ok(ArchiveSummary {
            files: 8,
            payload_bytes: MAX_PAYLOAD_BYTES,
            archive_bytes: MAX_PAYLOAD_BYTES + 8 * 512 + 1024
        })
    );
    let mut excessive = files;
    excessive.push(ExpectedFile::new("extra", 1).unwrap());
    assert_eq!(
        ExpectedArchive::new(&excessive).err(),
        Some(ArchiveError::LimitExceeded)
    );
    let mut v = validator(&[("a", 0)]);
    assert_eq!(
        v.feed(&vec![0; MAX_CHUNK_BYTES + 1]),
        Err(ArchiveError::LimitExceeded)
    );
    // 72 MiB archive cap is conservative: all valid member overhead and the
    // maximum aligned tail already fit below it. It is not a reachable valid size.
    let maximum_framed_bytes = MAX_PAYLOAD_BYTES + MAX_FILES as u64 * 1023 + MAX_TAIL_BYTES as u64;
    assert!(maximum_framed_bytes < MAX_ARCHIVE_BYTES);
}
