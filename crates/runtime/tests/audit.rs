#![cfg(unix)]

use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use openwrt_mcp_runtime::{
    AuditConfig, AuditDestination, AuditEvent, AuditFormat, AuditOutcome, AuditPhase, AuditSink,
    AuditWriter,
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "openwrt-mcp-audit-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn log(&self) -> PathBuf {
        self.0.join("audit.log")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config(dir: &TempDir) -> AuditConfig {
    AuditConfig {
        destination: AuditDestination::File,
        path: Some(dir.log()),
        max_bytes: 1024,
        retained_files: 2,
        ..AuditConfig::default()
    }
}

fn event(sequence: u64) -> AuditEvent {
    AuditEvent::new(
        sequence,
        AuditPhase::Start,
        "system_board",
        AuditOutcome::Attempt,
        None,
    )
}

#[test]
fn rotation_retains_bounded_private_files_and_valid_json_lines() {
    let dir = TempDir::new();
    let writer = AuditWriter::new(config(&dir)).unwrap();
    for sequence in 0..80 {
        writer.record(&event(sequence)).unwrap();
    }
    drop(writer);
    let mut count = 0;
    for entry in fs::read_dir(&dir.0).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert!(metadata.len() <= 1024);
        for line in fs::read_to_string(path).unwrap().lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(value["operation"], "system_board");
        }
        count += 1;
    }
    assert_eq!(count, 3); // active file plus exactly two retained generations
}

#[test]
fn reopening_tightens_permissions_and_appends() {
    let dir = TempDir::new();
    fs::write(dir.log(), "existing\n").unwrap();
    fs::set_permissions(dir.log(), fs::Permissions::from_mode(0o644)).unwrap();
    let writer = AuditWriter::new(config(&dir)).unwrap();
    writer.record(&event(1)).unwrap();
    assert!(
        fs::read_to_string(dir.log())
            .unwrap()
            .starts_with("existing\n")
    );
    assert_eq!(
        fs::metadata(dir.log()).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn symlink_and_hardlink_log_targets_are_rejected() {
    let dir = TempDir::new();
    let target = dir.0.join("target");
    fs::write(&target, "fixture-secret").unwrap();
    symlink(&target, dir.log()).unwrap();
    assert!(AuditWriter::new(config(&dir)).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "fixture-secret");
    fs::remove_file(dir.log()).unwrap();
    fs::hard_link(&target, dir.log()).unwrap();
    assert!(AuditWriter::new(config(&dir)).is_err());
}

#[test]
fn symlink_parents_and_rotation_slots_are_rejected() {
    let dir = TempDir::new();
    let link = dir.0.join("linked-directory");
    symlink(&dir.0, &link).unwrap();
    let mut linked = config(&dir);
    linked.path = Some(link.join("nested.log"));
    assert!(AuditWriter::new(linked).is_err());
    let writer = AuditWriter::new(config(&dir)).unwrap();
    let target = dir.0.join("target");
    fs::write(&target, "fixture-secret").unwrap();
    symlink(&target, dir.0.join("audit.log.1")).unwrap();
    assert!((0..20).any(|index| writer.record(&event(index)).is_err()));
    assert_eq!(fs::read_to_string(target).unwrap(), "fixture-secret");
}

#[test]
fn text_records_cannot_inject_lines_through_operation_name() {
    let dir = TempDir::new();
    let mut settings = config(&dir);
    settings.format = AuditFormat::Text;
    let writer = AuditWriter::new(settings).unwrap();
    let mut attempt = event(1);
    attempt.operation = "fixture-secret\nforged-record".to_owned();
    writer.record(&attempt).unwrap();
    let content = fs::read_to_string(dir.log()).unwrap();
    assert_eq!(content.lines().count(), 1);
    assert!(!content.contains("fixture-secret"));
    assert!(content.contains("operation=unknown"));
}

#[test]
fn disabled_audit_creates_no_files() {
    let dir = TempDir::new();
    let mut settings = config(&dir);
    settings.enabled = false;
    AuditWriter::new(settings)
        .unwrap()
        .record(&event(1))
        .unwrap();
    assert!(!dir.log().exists());
}
