//! Test-only private directory setup, following the existing native custody fixture.
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) struct Fixture {
    pub(super) path: PathBuf,
    parent: PathBuf,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let parent = PathBuf::from(std::env::var_os("USERPROFILE").unwrap());
        let path = parent.join(format!(
            "openwrt-mcp-audit-fixture-{}-{}",
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
                .starts_with("openwrt-mcp-audit-fixture-")
        );
        fs::remove_dir_all(&self.path).unwrap();
    }
}
