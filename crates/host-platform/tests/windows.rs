#![cfg(target_os = "windows")]
#![allow(unsafe_code)]
//! Native Windows synthetic files only; never real identities or Vault data.
mod windows_logs;
use openwrt_mcp_host_platform::{
    HostError, native_file_protection_supported, private_log_supported, read_config, read_secret,
};
use std::{
    fs,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    ptr::null_mut,
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::{
    Foundation::HANDLE,
    Security::{
        DACL_SECURITY_INFORMATION, GetLengthSid, GetTokenInformation,
        PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, SetFileSecurityW, TOKEN_QUERY,
        TOKEN_USER, TokenUser,
    },
    Storage::FileSystem::CreateDirectoryW,
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

fn user() -> Vec<u8> {
    let mut raw: HANDLE = null_mut();
    // Test-owned read-only process token, closed exactly once by OwnedHandle.
    assert_ne!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) },
        0
    );
    let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut words = [0usize; 512];
    let mut needed = 0;
    assert_ne!(
        unsafe {
            GetTokenInformation(
                handle.as_raw_handle(),
                TokenUser,
                words.as_mut_ptr().cast(),
                size_of_val(&words) as u32,
                &mut needed,
            )
        },
        0
    );
    let token = unsafe { &*words.as_ptr().cast::<TOKEN_USER>() };
    let length = unsafe { GetLengthSid(token.User.Sid) } as usize;
    assert!(length <= 68);
    unsafe { std::slice::from_raw_parts(token.User.Sid.cast::<u8>(), length) }.to_vec()
}
fn sd(user: &[u8], other_mask: Option<u32>, inherited: bool) -> Vec<u32> {
    // Self-relative owner + protected DACL; all structures DWORD aligned.
    let world = [1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    let mut bytes = vec![0u8; 20];
    bytes[0] = 1;
    bytes[2..4].copy_from_slice(&0x9004u16.to_le_bytes());
    bytes[4..8].copy_from_slice(&20u32.to_le_bytes());
    bytes.extend_from_slice(user);
    let acl_at = bytes.len();
    bytes[16..20].copy_from_slice(&(acl_at as u32).to_le_bytes());
    bytes.extend_from_slice(&[2, 0, 0, 0, 1 + u8::from(other_mask.is_some()), 0, 0, 0]);
    for (trustee, mask) in [(user, 0x001f01ff)]
        .into_iter()
        .chain(other_mask.map(|m| (world.as_slice(), m)))
    {
        bytes.extend_from_slice(&[0, if inherited { 0x03 } else { 0 }, 0, 0]);
        let start = bytes.len() - 4;
        bytes[start + 2..start + 4].copy_from_slice(&((trustee.len() + 8) as u16).to_le_bytes());
        bytes.extend_from_slice(&mask.to_le_bytes());
        bytes.extend_from_slice(trustee);
    }
    let length = bytes.len() - acl_at;
    bytes[acl_at + 2..acl_at + 4].copy_from_slice(&(length as u16).to_le_bytes());
    bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
fn wide(path: &Path) -> Vec<u16> {
    path.to_str()
        .unwrap()
        .encode_utf16()
        .chain(Some(0))
        .collect()
}
struct Fixture {
    root: PathBuf,
    parent: PathBuf,
    user: Vec<u8>,
}
impl Fixture {
    fn new() -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        // The normal temp directory may have intentionally shared ancestors.
        // This direct user-profile child is non-synced; no parent ACL is modified.
        let parent = PathBuf::from(
            std::env::var_os("USERPROFILE").expect("native fixture requires USERPROFILE"),
        );
        let root = parent.join(format!(
            "openwrt-mcp-private-fixture-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let user = user();
        let mut descriptor = sd(&user, None, true);
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.as_mut_ptr().cast(),
            bInheritHandle: 0,
        };
        // Atomic new directory with an explicit private DACL; never overwrite/reuse a name.
        assert_ne!(
            unsafe { CreateDirectoryW(wide(&root).as_ptr(), &attributes) },
            0,
            "cannot create private synthetic fixture"
        );
        Self { root, parent, user }
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, b"synthetic-test-material").unwrap();
        path
    }
    fn grant(&self, path: &Path, mask: u32) {
        assert!(path.starts_with(&self.root));
        let mut descriptor = sd(&self.user, Some(mask), false);
        assert_ne!(
            unsafe {
                SetFileSecurityW(
                    wide(path).as_ptr(),
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    descriptor.as_mut_ptr().cast(),
                )
            },
            0
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Exact exclusively created child only; never the profile/workspace root.
        assert_eq!(self.root.parent(), Some(self.parent.as_path()));
        assert!(
            self.root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("openwrt-mcp-private-fixture-")
        );
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn private_native_file_reads_and_bounds_are_enforced() {
    assert!(native_file_protection_supported());
    assert!(private_log_supported());
    let f = Fixture::new();
    let path = f.file("synthetic.key");
    assert_eq!(
        read_config(&path, 1024).unwrap(),
        b"synthetic-test-material"
    );
    assert_eq!(
        &*read_secret(&path, 23).unwrap(),
        b"synthetic-test-material"
    );
    assert_eq!(read_secret(&path, 22).err(), Some(HostError::Limit));
    fs::write(&path, b"").unwrap();
    assert!(read_secret(&path, 1).unwrap().is_empty());
}

#[test]
fn config_readability_does_not_authorize_secret_disclosure_or_writes() {
    let f = Fixture::new();
    for mask in [1, 8, 0x20, 0x80000000, 0x20000000] {
        let path = f.file(&format!("read-{mask}.key"));
        f.grant(&path, mask);
        assert!(read_config(&path, 1024).is_ok());
        assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Insecure));
    }
    for mask in [
        2, 4, 16, 0x100, 0x10000, 0x40000, 0x80000, 0x40000000, 0x10000000,
    ] {
        let path = f.file(&format!("write-{mask}.key"));
        f.grant(&path, mask);
        assert_eq!(read_config(&path, 1024).err(), Some(HostError::Insecure));
        assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Insecure));
    }
}

#[test]
fn hardlinks_and_existing_writers_cannot_be_read_as_protected_files() {
    use std::os::windows::fs::OpenOptionsExt;
    let f = Fixture::new();
    let path = f.file("single.key");
    let writer = fs::OpenOptions::new()
        .write(true)
        .share_mode(7)
        .open(&path)
        .unwrap();
    assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Unavailable));
    drop(writer);
    assert!(read_secret(&path, 1024).is_ok());
    fs::hard_link(&path, f.root.join("hard.key")).unwrap();
    assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Insecure));
}

#[test]
fn lexical_path_rejections_happen_without_opening_a_target() {
    for path in [
        "",
        "file",
        "C:file",
        "\\file",
        "\\\\server\\share\\file",
        "\\\\?\\C:\\file",
        "C:\\",
        "C:\\a\\..\\b",
        "C:\\a\\.\\b",
        "C:\\a:stream",
        "C:\\a\\\\b",
        "C:\\a ",
        "C:\\a.",
        "C:\\CON.txt",
        "C:\\LPT¹.txt",
        "C:/a",
        "C:\\a?",
        "C:\\a\0",
    ] {
        assert_eq!(
            read_secret(Path::new(path), 1024).err(),
            Some(HostError::InvalidPath)
        );
    }
}

#[test]
fn effective_inheritance_and_untrusted_ancestor_writes_are_rejected() {
    let f = Fixture::new();
    let directory = f.root.join("inherited");
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
    let path = directory.join("child.key");
    fs::write(&path, b"synthetic").unwrap();
    assert!(read_config(&path, 1024).is_ok());
    assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Insecure));
    f.grant(&directory, 2);
    assert_eq!(read_config(&path, 1024).err(), Some(HostError::Insecure));
}

#[test]
fn null_dacl_and_reparse_junctions_are_not_protected_files() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::{
        Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT},
        System::IO::DeviceIoControl,
    };
    let f = Fixture::new();
    let path = f.file("null.key");
    let mut descriptor = sd(&f.user, None, false);
    descriptor[4] = 0; // DACL_PRESENT with null offset.
    assert_ne!(
        unsafe {
            SetFileSecurityW(
                wide(&path).as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor.as_mut_ptr().cast(),
            )
        },
        0
    );
    assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Insecure));
    let destination = f.root.join("destination");
    fs::create_dir(&destination).unwrap();
    fs::write(destination.join("child.key"), b"synthetic").unwrap();
    let junction = f.root.join("junction");
    fs::create_dir(&junction).unwrap();
    let handle = fs::OpenOptions::new()
        .write(true)
        .share_mode(7)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&junction)
        .unwrap();
    let target: Vec<u16> = format!("\\??\\{}", destination.to_str().unwrap())
        .encode_utf16()
        .collect();
    let mut data = Vec::new();
    data.extend_from_slice(&0xA0000003u32.to_le_bytes());
    data.extend_from_slice(&((target.len() * 2 + 12) as u16).to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    for n in [0, target.len() * 2, target.len() * 2 + 2, 0] {
        data.extend_from_slice(&(n as u16).to_le_bytes());
    }
    for n in target {
        data.extend_from_slice(&n.to_le_bytes());
    }
    data.extend_from_slice(&[0, 0, 0, 0]);
    let mut returned = 0;
    // FSCTL_SET_REPARSE_POINT affects only this exclusively created fixture junction.
    assert_ne!(
        unsafe {
            DeviceIoControl(
                handle.as_raw_handle(),
                0x000900A4,
                data.as_ptr().cast(),
                data.len() as u32,
                null_mut(),
                0,
                &mut returned,
                null_mut(),
            )
        },
        0
    );
    drop(handle);
    assert!(read_config(&junction, 1024).is_err());
    assert!(read_config(&junction.join("child.key"), 1024).is_err());
    assert!(read_secret(&junction.join("child.key"), 1024).is_err());
    assert!(openwrt_mcp_host_platform::PrivateLog::open(&junction, 1024, 2).is_err());
    assert!(
        openwrt_mcp_host_platform::PrivateLog::open(&junction.join("child.key"), 1024, 2).is_err()
    );
    assert!(
        openwrt_mcp_host_platform::PrivateLog::open(&junction.join("new.log"), 1024, 2).is_err()
    );
    assert!(!destination.join("new.log").exists());
    fs::remove_dir(&junction).unwrap();
    assert!(read_secret(&destination.join("child.key"), 1024).is_ok());
}

#[test]
fn normalized_names_preserve_unicode_and_reject_existing_short_aliases() {
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;
    let f = Fixture::new();
    let unicode = f.file("합성 키 파일.key");
    assert!(read_secret(&unicode, 1024).is_ok());
    let literal = f.file("LITERAL~1.KEY");
    assert!(read_secret(&literal, 1024).is_ok());
    let path = f.file("long-synthetic-identity-name.key");
    let mut buffer = [0u16; 4096];
    let count = unsafe {
        GetShortPathNameW(
            wide(&path).as_ptr(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    assert!(count > 0 && count < buffer.len());
    let short = String::from_utf16(&buffer[..count]).unwrap();
    // NTFS can disable 8.3 creation. In that case there is no alias to reject;
    // the deterministic normalized-name policy regressions still run separately.
    if !short.eq_ignore_ascii_case(path.to_str().unwrap()) {
        assert_eq!(
            read_secret(Path::new(&short), 1024).err(),
            Some(HostError::Insecure)
        );
        assert_eq!(
            openwrt_mcp_host_platform::PrivateLog::open(Path::new(&short), 1024, 2).err(),
            Some(HostError::Insecure)
        );
    } else {
        assert!(read_secret(Path::new(&short), 1024).is_ok());
    }
}
