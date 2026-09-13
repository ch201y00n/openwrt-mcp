//! Windows-only public synthetic audit records in the parent suite's private fixtures.
use super::{Fixture, HostError, fs, read_secret, sd, wide};
use openwrt_mcp_host_platform::{PrivateLog, private_log_supported, system_log_supported};
use std::os::windows::fs::OpenOptionsExt;

#[test]
fn unicode_names_and_multichunk_records_survive_append_and_rotation() {
    let f = Fixture::new();
    let path = f.root.join("합성 감사 기록.log");
    let bytes: Vec<u8> = (0..131_073).map(|index| (index % 251) as u8).collect();
    let mut log = PrivateLog::open(&path, bytes.len() as u64, 1).unwrap();
    log.write(&bytes).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    log.write(b"next").unwrap();
    drop(log);
    assert_eq!(fs::read(&path).unwrap(), b"next");
    assert_eq!(
        &*read_secret(&f.root.join("합성 감사 기록.log.1"), bytes.len()).unwrap(),
        &bytes
    );
}

#[test]
fn held_ancestors_cannot_be_deleted_and_unsafe_ancestor_drift_latches() {
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_FLAG_BACKUP_SEMANTICS};
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    let mut log = PrivateLog::open(&path, 8, 2).unwrap();
    log.write(b"safe").unwrap();
    assert!(
        fs::OpenOptions::new()
            .access_mode(DELETE)
            .share_mode(7)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(&f.root)
            .is_err()
    );
    f.grant(&f.root, 2);
    assert_eq!(log.write(b"next"), Err(HostError::Insecure));
    assert_eq!(log.write(b""), Err(HostError::Unavailable));
    assert_eq!(fs::read(&path).unwrap(), b"safe");
    assert!(!f.root.join("audit.log.1").exists());
}

#[test]
fn sharing_failure_in_older_generation_is_not_treated_as_an_absent_file() {
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    fs::write(&path, b"active").unwrap();
    fs::write(f.root.join("audit.log.1"), b"recent").unwrap();
    let oldest = f.root.join("audit.log.2");
    fs::write(&oldest, b"oldest").unwrap();
    let writer = fs::OpenOptions::new()
        .write(true)
        .share_mode(7)
        .open(&oldest)
        .unwrap();
    let mut log = PrivateLog::open(&path, 6, 2).unwrap();
    assert_eq!(log.write(b"next"), Err(HostError::Unavailable));
    drop(writer);
    assert_eq!(log.write(b"x"), Err(HostError::Unavailable));
    assert_eq!(fs::read(&path).unwrap(), b"active");
    assert_eq!(fs::read(f.root.join("audit.log.1")).unwrap(), b"recent");
    assert_eq!(fs::read(&oldest).unwrap(), b"oldest");
}

#[test]
fn conflicting_ancestor_delete_handle_prevents_creation_and_protected_reads() {
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_FLAG_BACKUP_SEMANTICS};
    let f = Fixture::new();
    let existing = f.file("existing.log");
    let deleting = fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(7)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&f.root)
        .unwrap();
    assert_eq!(
        read_secret(&existing, 64).err(),
        Some(HostError::Unavailable)
    );
    let path = f.root.join("new.log");
    assert_eq!(
        PrivateLog::open(&path, 64, 2).err(),
        Some(HostError::Unavailable)
    );
    assert!(!path.exists());
    drop(deleting);
    assert!(read_secret(&existing, 64).is_ok());
    assert!(PrivateLog::open(&path, 64, 2).is_ok());
}

#[test]
fn append_reopen_exact_limit_and_rotation_keep_only_configured_generations() {
    assert!(private_log_supported());
    assert!(!system_log_supported());
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    let mut log = PrivateLog::open(&path, 7, 2).unwrap();
    log.write(b"aaa").unwrap();
    log.write(b"bbb").unwrap();
    log.write(b"cc").unwrap();
    log.write(b"dddddd").unwrap();
    log.write(b"eee").unwrap();
    drop(log);
    assert_eq!(&*read_secret(&path, 7).unwrap(), b"eee");
    assert_eq!(fs::read(f.root.join("audit.log.1")).unwrap(), b"dddddd");
    assert_eq!(fs::read(f.root.join("audit.log.2")).unwrap(), b"cc");
    assert!(!f.root.join("audit.log.3").exists());
    let mut log = PrivateLog::open(&path, 7, 2).unwrap();
    log.write(b"ffff").unwrap();
    log.write(b"").unwrap();
    drop(log);
    assert_eq!(fs::read(&path).unwrap(), b"eeeffff");
    assert_eq!(fs::read(f.root.join("audit.log.1")).unwrap(), b"dddddd");
}

#[test]
fn creation_has_private_dacl_even_under_readable_inheritable_parent() {
    use windows_sys::Win32::Security::{
        DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW,
    };
    let f = Fixture::new();
    let directory = f.root.join("readable-parent");
    fs::create_dir(&directory).unwrap();
    let mut descriptor = sd(&f.user, Some(0x80000000), true);
    assert_ne!(
        unsafe {
            SetFileSecurityW(
                wide(&directory).as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor.as_mut_ptr().cast(),
            )
        },
        0
    );
    let path = directory.join("audit.log");
    let mut log = PrivateLog::open(&path, 4, 1).unwrap();
    log.write(b"safe").unwrap();
    log.write(b"next").unwrap();
    drop(log);
    assert_eq!(&*read_secret(&path, 4).unwrap(), b"next");
    assert_eq!(
        &*read_secret(&directory.join("audit.log.1"), 4).unwrap(),
        b"safe"
    );
}

#[test]
fn insecure_active_files_are_rejected_without_acl_repair_or_content_change() {
    let f = Fixture::new();
    for mask in [1, 2, 4, 0x20, 0x10000, 0x40000, 0x80000000] {
        let path = f.file(&format!("unsafe-{mask}.log"));
        f.grant(&path, mask);
        assert_eq!(
            PrivateLog::open(&path, 64, 2).err(),
            Some(HostError::Insecure)
        );
        assert_eq!(fs::read(&path).unwrap(), b"synthetic-test-material");
        assert_eq!(read_secret(&path, 64).err(), Some(HostError::Insecure));
    }
}

#[test]
fn unsafe_retained_generations_fail_before_rotation_and_latch() {
    for hardlink in [false, true] {
        let f = Fixture::new();
        let path = f.root.join("audit.log");
        fs::write(&path, b"active").unwrap();
        fs::write(f.root.join("audit.log.1"), b"recent").unwrap();
        let oldest = f.root.join("audit.log.2");
        fs::write(&oldest, b"oldest").unwrap();
        if hardlink {
            fs::hard_link(&oldest, f.root.join("other.log")).unwrap();
        } else {
            f.grant(&oldest, 1);
        }
        let mut log = PrivateLog::open(&path, 6, 2).unwrap();
        assert_eq!(log.write(b"next"), Err(HostError::Insecure));
        assert_eq!(log.write(b"x"), Err(HostError::Unavailable));
        assert_eq!(fs::read(&path).unwrap(), b"active");
        assert_eq!(fs::read(f.root.join("audit.log.1")).unwrap(), b"recent");
        assert_eq!(fs::read(&oldest).unwrap(), b"oldest");
    }
}

#[test]
fn active_object_excludes_other_writers_deletion_and_log_instances() {
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    let mut log = PrivateLog::open(&path, 64, 2).unwrap();
    assert!(
        fs::OpenOptions::new()
            .append(true)
            .share_mode(7)
            .open(&path)
            .is_err()
    );
    assert!(fs::remove_file(&path).is_err());
    assert_eq!(
        PrivateLog::open(&path, 64, 2).err(),
        Some(HostError::Unavailable)
    );
    log.write(b"public event").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"public event");
    drop(log);
    let writer = fs::OpenOptions::new()
        .write(true)
        .share_mode(7)
        .open(&path)
        .unwrap();
    assert_eq!(
        PrivateLog::open(&path, 64, 2).err(),
        Some(HostError::Unavailable)
    );
    drop(writer);
    assert!(PrivateLog::open(&path, 64, 2).is_ok());
}

#[test]
fn security_changes_and_oversized_records_permanently_disable_writer() {
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    let mut log = PrivateLog::open(&path, 4, 2).unwrap();
    log.write(b"safe").unwrap();
    assert_eq!(log.write(b"large"), Err(HostError::Limit));
    assert_eq!(log.write(b"x"), Err(HostError::Unavailable));
    assert_eq!(fs::read(&path).unwrap(), b"safe");
    let mut log = PrivateLog::open(&path, 4, 2).unwrap();
    f.grant(&path, 1);
    assert_eq!(log.write(b"x"), Err(HostError::Insecure));
    assert_eq!(log.write(b""), Err(HostError::Unavailable));
    assert_eq!(fs::read(&path).unwrap(), b"safe");
    assert!(!f.root.join("audit.log.1").exists());
}

#[test]
fn hardlinks_directories_and_generated_name_overflow_are_not_log_targets() {
    let f = Fixture::new();
    let linked = f.file("linked.log");
    fs::hard_link(&linked, f.root.join("second.log")).unwrap();
    assert_eq!(
        PrivateLog::open(&linked, 64, 2).err(),
        Some(HostError::Insecure)
    );
    let directory = f.root.join("directory.log");
    fs::create_dir(&directory).unwrap();
    assert!(PrivateLog::open(&directory, 64, 2).is_err());
    let invalid = f.root.join("x".repeat(252));
    assert_eq!(
        PrivateLog::open(&invalid, 4, 100).err(),
        Some(HostError::InvalidPath)
    );
    assert!(!invalid.exists());
    let valid = f.root.join("x".repeat(251));
    let mut log = PrivateLog::open(&valid, 4, 100).unwrap();
    log.write(b"safe").unwrap();
    drop(log);
    assert_eq!(&*read_secret(&valid, 4).unwrap(), b"safe");
}

#[test]
fn retained_reader_cannot_force_replacement_after_delete_pending() {
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    fs::write(&path, b"active").unwrap();
    fs::write(f.root.join("audit.log.1"), b"recent").unwrap();
    let oldest = f.root.join("audit.log.2");
    fs::write(&oldest, b"oldest").unwrap();
    // Read sharing permits preflight's DELETE handle, but deletion stays pending
    // while this ordinary reader holds the object. No-replacement rename must fail.
    let reader = fs::OpenOptions::new()
        .read(true)
        .share_mode(7)
        .open(&oldest)
        .unwrap();
    let mut log = PrivateLog::open(&path, 6, 2).unwrap();
    assert_eq!(log.write(b"next"), Err(HostError::Unavailable));
    assert_eq!(fs::read(&path).unwrap(), b"active");
    assert_eq!(fs::read(f.root.join("audit.log.1")).unwrap(), b"recent");
    drop(reader);
    assert_eq!(log.write(b"x"), Err(HostError::Unavailable));
}

#[test]
fn maximum_retention_and_smaller_reopened_limit_have_bounded_generation_scope() {
    let f = Fixture::new();
    let path = f.root.join("audit.log");
    let mut log = PrivateLog::open(&path, 1, 100).unwrap();
    for value in 0..=102 {
        log.write(&[value]).unwrap();
    }
    drop(log);
    assert_eq!(fs::read(&path).unwrap(), [102]);
    for index in 1..=100 {
        assert_eq!(
            fs::read(f.root.join(format!("audit.log.{index}"))).unwrap(),
            [102 - index as u8]
        );
    }
    assert!(!f.root.join("audit.log.101").exists());
    let large = f.root.join("older-large.log");
    fs::write(&large, b"older").unwrap();
    let mut log = PrivateLog::open(&large, 1, 1).unwrap();
    log.write(b"").unwrap();
    assert!(!f.root.join("older-large.log.1").exists());
    log.write(b"n").unwrap();
    drop(log);
    assert_eq!(
        fs::read(f.root.join("older-large.log.1")).unwrap(),
        b"older"
    );
}
