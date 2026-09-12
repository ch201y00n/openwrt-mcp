#![cfg(target_os = "linux")]

use openwrt_mcp_key_sources::{FileProtection, NativeFileAccess, ProtectedFileAccess};
use openwrt_mcp_runtime::protection::ProtectionError;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openwrt-mcp-key-source-fixture-{}-{}-{}",
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
    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
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
                .starts_with("openwrt-mcp-key-source-fixture-")
        );
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn restricted_file_read_and_bound() {
    let fixture = Fixture::new();
    let file = fixture.file("key");
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, 1024)
            .unwrap()
            .expose_bytes(),
        b"synthetic-only"
    );
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, 1)
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, usize::MAX)
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
}

#[test]
fn broad_file_permissions_and_hardlinks_are_rejected() {
    let fixture = Fixture::new();
    let file = fixture.file("key");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, 1024)
            .unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::hard_link(&file, fixture.0.join("second-name")).unwrap();
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, 1024)
            .unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
}

#[test]
fn symlink_components_and_broad_parent_are_rejected() {
    let fixture = Fixture::new();
    let file = fixture.file("key");
    let link = fixture.0.join("link");
    symlink(&file, &link).unwrap();
    assert!(
        NativeFileAccess
            .read(&link, FileProtection::Restricted, 1024)
            .is_err()
    );
    let linked_directory = fixture.0.join("directory-link");
    symlink(&fixture.0, &linked_directory).unwrap();
    assert!(
        NativeFileAccess
            .read(
                &linked_directory.join("key"),
                FileProtection::Restricted,
                1024
            )
            .is_err()
    );
    std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o777)).unwrap();
    assert_eq!(
        NativeFileAccess
            .read(&file, FileProtection::Restricted, 1024)
            .unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
}

#[test]
fn directories_missing_files_and_parent_traversal_are_rejected() {
    let fixture = Fixture::new();
    assert!(
        NativeFileAccess
            .read(&fixture.0, FileProtection::Restricted, 1024)
            .is_err()
    );
    assert_eq!(
        NativeFileAccess
            .read(&fixture.0.join("absent"), FileProtection::Restricted, 1024)
            .unwrap_err(),
        ProtectionError::SourceUnavailable
    );
    assert_eq!(
        NativeFileAccess
            .read(
                &fixture.0.join("../invalid"),
                FileProtection::Restricted,
                1024
            )
            .unwrap_err(),
        ProtectionError::InvalidConfig
    );
}
