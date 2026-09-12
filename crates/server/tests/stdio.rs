//! Optional Linux protected configuration tests. Common binary lifecycle is portable_stdio.
#![cfg(target_os = "linux")]
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;

struct PrivateConfig(PathBuf);
impl PrivateConfig {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("openwrt-mcp-stdio-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("fixture.toml");
        fs::write(
            &path,
            include_str!("../../../config/example-extension.toml"),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        Self(path)
    }
}
impl Drop for PrivateConfig {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

#[tokio::test]
async fn check_is_offline_and_rejects_invalid_audit_config_without_echoing_contents() {
    let config = PrivateConfig::new();
    let valid = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
        .arg("check")
        .arg("--config")
        .arg(&config.0)
        .output()
        .await
        .unwrap();
    assert!(valid.status.success());
    fs::write(
        &config.0,
        "[audit]\ndestination = 'file'\npath = 'synthetic-private-relative-path'\n",
    )
    .unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
        .arg("check")
        .arg("--config")
        .arg(&config.0)
        .output()
        .await
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(
        !String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("synthetic-private")
    );
}
