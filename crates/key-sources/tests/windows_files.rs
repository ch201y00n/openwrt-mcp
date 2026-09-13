#![cfg(target_os = "windows")]
//! Native file/ZIP custody composition with synthetic material, not Vault acceptance.
use openwrt_mcp_key_sources::{ContainerFormat, SourceConfig, SourceRegistry};
use openwrt_mcp_runtime::protection::{KeyLimits, ProtectionError};
use std::os::windows::process::CommandExt;
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

struct Fixture {
    path: PathBuf,
    parent: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let parent = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        let path = parent.join(format!(
            "openwrt-mcp-custody-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!path.exists());
        let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        // Fixture-only .NET ACL setup. Production never executes a host command.
        // The exact new directory is supplied as data, never interpolated into code.
        let script = r#"$ErrorActionPreference='Stop'
$account=[System.Security.Principal.WindowsIdentity]::GetCurrent().User
$acl=New-Object System.Security.AccessControl.DirectorySecurity
$acl.SetOwner($account)
$acl.SetAccessRuleProtection($true,$false)
$rule=New-Object System.Security.AccessControl.FileSystemAccessRule($account,'FullControl','ContainerInherit,ObjectInherit','None','Allow')
$acl.AddAccessRule($rule)
if ([System.IO.Directory]::Exists($env:OPENWRT_MCP_FIXTURE_DIR)) { exit 2 }
[System.IO.Directory]::CreateDirectory($env:OPENWRT_MCP_FIXTURE_DIR,$acl) | Out-Null
"#;
        let mut child = Command::new(powershell)
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("OPENWRT_MCP_FIXTURE_DIR", &path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "synthetic private directory setup failed");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("synthetic ACL setup exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Self { path, parent }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        assert_eq!(self.path.parent(), Some(self.parent.as_path()));
        assert!(
            self.path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("openwrt-mcp-custody-fixture-")
        );
        fs::remove_dir_all(&self.path).unwrap();
    }
}

#[test]
fn native_restricted_file_and_exact_zip_entry_keep_purpose_limits_and_vault_separation() {
    let f = Fixture::new();
    let direct = f.path.join("synthetic.key");
    let archive = f.path.join("synthetic.zip");
    fs::write(&direct, b"synthetic-direct").unwrap();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, value) in [
        ("keys/selected", b"synthetic-selected".as_slice()),
        ("keys/other", b"synthetic-other".as_slice()),
    ] {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(value).unwrap();
    }
    fs::write(&archive, zip.finish().unwrap().into_inner()).unwrap();
    let registry = SourceRegistry::new(
        BTreeMap::from([
            (
                "direct".into(),
                SourceConfig::RestrictedFile {
                    path: direct.clone(),
                },
            ),
            (
                "archive".into(),
                SourceConfig::RestrictedFile { path: archive },
            ),
            (
                "selected".into(),
                SourceConfig::ArchiveEntry {
                    source: "archive".into(),
                    format: ContainerFormat::Zip,
                    entry: "keys/selected".into(),
                },
            ),
            (
                "vault".into(),
                SourceConfig::PersonalVaultFile {
                    path: direct.clone(),
                },
            ),
        ]),
        KeyLimits {
            max_key_bytes: 64,
            ..KeyLimits::default()
        },
    )
    .unwrap();
    let source = registry.resolve_key("direct").unwrap();
    assert_eq!(
        source.read(1024).unwrap().expose_bytes(),
        b"synthetic-direct"
    );
    assert_eq!(source.read(1).unwrap_err(), ProtectionError::ResourceLimit);
    assert_eq!(
        registry
            .resolve_key("selected")
            .unwrap()
            .read(64)
            .unwrap()
            .expose_bytes(),
        b"synthetic-selected"
    );
    assert_eq!(
        registry
            .resolve_key("selected")
            .unwrap()
            .read(1)
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
    assert_eq!(
        registry
            .resolve_key("vault")
            .unwrap()
            .read(1024)
            .unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
    fs::hard_link(&direct, f.path.join("alias.key")).unwrap();
    assert_eq!(
        source.read(1024).unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
}
