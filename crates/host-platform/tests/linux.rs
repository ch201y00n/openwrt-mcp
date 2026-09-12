#![cfg(target_os = "linux")]

use openwrt_mcp_host_platform::{HostError, PrivateLog, read_config, read_secret};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openwrt-mcp-platform-fixture-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
    fn file(&self, name: &str, mode: u32) -> PathBuf {
        let path = self.0.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&path)
            .unwrap();
        file.write_all(b"synthetic-only").unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.0.parent(), Some(std::env::temp_dir().as_path()));
        assert!(
            self.0
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("openwrt-mcp-platform-fixture-")
        );
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn config_integrity_and_secret_confidentiality_have_distinct_requirements() {
    let fixture = Fixture::new();
    let file = fixture.file("config", 0o644);
    assert_eq!(read_config(&file, 1024).unwrap(), b"synthetic-only");
    assert_eq!(read_secret(&file, 1024).err(), Some(HostError::Insecure));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        read_secret(&file, 1024).unwrap().as_slice(),
        b"synthetic-only"
    );
    assert_eq!(read_config(&file, 1).err(), Some(HostError::Limit));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o620)).unwrap();
    assert_eq!(read_config(&file, 1024).err(), Some(HostError::Insecure));
}

#[test]
fn config_and_secret_reject_links_and_untrusted_parent_writes() {
    let fixture = Fixture::new();
    let file = fixture.file("key", 0o600);
    let linked = fixture.0.join("linked");
    symlink(&file, &linked).unwrap();
    assert!(read_config(&linked, 1024).is_err());
    assert!(read_secret(&linked, 1024).is_err());
    std::fs::hard_link(&file, fixture.0.join("hardlink")).unwrap();
    assert_eq!(read_config(&file, 1024).err(), Some(HostError::Insecure));
    std::fs::remove_file(fixture.0.join("hardlink")).unwrap();
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(read_config(&file, 1024).err(), Some(HostError::Insecure));
    assert_eq!(read_secret(&file, 1024).err(), Some(HostError::Insecure));
}

#[test]
fn path_and_kind_validation_prevents_unintended_reads() {
    let fixture = Fixture::new();
    assert_eq!(
        read_secret(std::path::Path::new("relative"), 1024).err(),
        Some(HostError::InvalidPath)
    );
    assert_eq!(
        read_config(&fixture.0.join("../invalid"), 1024).err(),
        Some(HostError::InvalidPath)
    );
    assert_eq!(
        read_config(&fixture.0, 1024).err(),
        Some(HostError::Insecure)
    );
    assert_eq!(
        read_config(&fixture.0.join("missing"), 1024).err(),
        Some(HostError::Unavailable)
    );
}

#[test]
fn log_rotation_retains_only_private_bounded_generations() {
    let fixture = Fixture::new();
    let path = fixture.0.join("audit.log");
    let mut log = PrivateLog::open(&path, 16, 2).unwrap();
    for _ in 0..20 {
        log.write(b"record\n").unwrap();
    }
    drop(log);
    let entries: Vec<_> = std::fs::read_dir(&fixture.0).unwrap().collect();
    assert_eq!(entries.len(), 3);
    for entry in entries {
        let metadata = entry.unwrap().metadata().unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert!(metadata.len() <= 16);
    }
}

#[test]
fn owned_readable_log_may_tighten_but_writable_log_is_rejected() {
    let fixture = Fixture::new();
    let readable = fixture.file("readable", 0o644);
    let mut log = PrivateLog::open(&readable, 1024, 2).unwrap();
    log.write(b"record\n").unwrap();
    assert_eq!(
        std::fs::metadata(&readable).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let writable = fixture.file("writable", 0o600);
    std::fs::set_permissions(&writable, std::fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(
        PrivateLog::open(&writable, 1024, 2).err(),
        Some(HostError::Insecure)
    );
    assert_eq!(
        std::fs::metadata(&writable).unwrap().permissions().mode() & 0o777,
        0o666
    );
}

#[test]
fn log_rotation_rejects_poisoned_slots_and_latches_failure() {
    let fixture = Fixture::new();
    let path = fixture.0.join("audit");
    let target = fixture.file("untouched", 0o600);
    let mut log = PrivateLog::open(&path, 8, 2).unwrap();
    log.write(b"record\n").unwrap();
    symlink(&target, fixture.0.join("audit.1")).unwrap();
    assert_eq!(log.write(b"record\n").err(), Some(HostError::Insecure));
    std::fs::remove_file(fixture.0.join("audit.1")).unwrap();
    assert_eq!(log.write(b"x").err(), Some(HostError::Unavailable));
    assert_eq!(std::fs::read(target).unwrap(), b"synthetic-only");
}

#[test]
fn log_parent_must_not_be_untrusted_writable() {
    let fixture = Fixture::new();
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(
        PrivateLog::open(&fixture.0.join("audit"), 1024, 2).err(),
        Some(HostError::Insecure)
    );
    assert!(!fixture.0.join("audit").exists());
}

#[test]
fn log_uses_original_parent_handle_after_path_rename() {
    let fixture = Fixture::new();
    let original = fixture.0.join("original");
    let moved = fixture.0.join("moved");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&original)
        .unwrap();
    let mut log = PrivateLog::open(&original.join("audit"), 8, 2).unwrap();
    log.write(b"record\n").unwrap();
    std::fs::rename(&original, &moved).unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&original)
        .unwrap();
    log.write(b"record\n").unwrap();
    assert!(!original.join("audit").exists());
    assert!(moved.join("audit").exists());
    assert!(moved.join("audit.1").exists());
}

#[test]
fn external_log_replacement_and_size_changes_are_rejected() {
    let fixture = Fixture::new();
    let path = fixture.0.join("audit");
    let mut log = PrivateLog::open(&path, 1024, 2).unwrap();
    std::fs::rename(&path, fixture.0.join("old")).unwrap();
    fixture.file("audit", 0o600);
    assert_eq!(log.write(b"record\n").err(), Some(HostError::Insecure));
    let mut log = PrivateLog::open(&path, 1024, 2).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"external")
        .unwrap();
    assert_eq!(log.write(b"record\n").err(), Some(HostError::Insecure));
}
