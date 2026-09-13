use crate::HostError;
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, Stat, fchmod, fstat, open, openat, renameat, statat, unlinkat,
};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixDatagram;
use std::path::{Component, Path, PathBuf};
use zeroize::Zeroizing;

pub(crate) fn native_file_protection_supported() -> bool {
    true
}
pub(crate) fn private_log_supported() -> bool {
    true
}
pub(crate) fn system_log_supported() -> bool {
    true
}

#[derive(Clone, Copy)]
enum Ownership {
    Operator,
    Root,
}

fn trusted_owner(stat: &Stat, owner: Ownership) -> bool {
    stat.st_uid == 0
        || matches!(owner, Ownership::Operator)
            && stat.st_uid == rustix::process::geteuid().as_raw()
}

fn validate_path(path: &Path, relative: bool) -> Result<PathBuf, HostError> {
    if path.as_os_str().is_empty()
        || path.as_os_str().len() > 4096
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err(HostError::InvalidPath);
    }
    let joined = if path.is_absolute() {
        path.to_owned()
    } else if relative {
        std::env::current_dir()
            .map_err(|_| HostError::Unavailable)?
            .join(path)
    } else {
        return Err(HostError::InvalidPath);
    };
    // Lexical components may omit '.', never resolve a symlink or '..'.
    let normalized: PathBuf = joined
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .collect();
    if !normalized.is_absolute()
        || normalized.file_name().is_none()
        || normalized.as_os_str().len() > 4096
    {
        return Err(HostError::InvalidPath);
    }
    Ok(normalized)
}

fn validate_directory(
    fd: &OwnedFd,
    owner: Ownership,
    sticky_allowed: bool,
) -> Result<(), HostError> {
    let stat = fstat(fd).map_err(|_| HostError::Unavailable)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory || !trusted_owner(&stat, owner)
    {
        return Err(HostError::Insecure);
    }
    let trusted_sticky = sticky_allowed && stat.st_uid == 0 && stat.st_mode & 0o1000 != 0;
    if stat.st_mode & 0o022 != 0 && !trusted_sticky {
        return Err(HostError::Insecure);
    }
    Ok(())
}

fn parent_handle(
    path: &Path,
    owner: Ownership,
    private_parent: bool,
) -> Result<(OwnedFd, OsString), HostError> {
    let mut parent = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| HostError::Unavailable)?;
    validate_directory(&parent, owner, false)?;
    let components: Vec<_> = path.components().collect();
    for component in components
        .iter()
        .skip(1)
        .take(components.len().saturating_sub(2))
    {
        let Component::Normal(name) = component else {
            return Err(HostError::InvalidPath);
        };
        parent = openat(
            &parent,
            *name,
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| HostError::Unavailable)?;
        validate_directory(&parent, owner, true)?;
    }
    if private_parent {
        validate_directory(&parent, owner, false)?;
    }
    let name = path.file_name().ok_or(HostError::InvalidPath)?.to_owned();
    Ok((parent, name))
}

fn validate_file(stat: &Stat, owner: Ownership, private: bool) -> Result<(), HostError> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 1
        || !trusted_owner(stat, owner)
        || stat.st_mode & (if private { 0o077 } else { 0o022 }) != 0
        || stat.st_size < 0
    {
        return Err(HostError::Insecure);
    }
    Ok(())
}

fn read_protected(
    path: &Path,
    max_bytes: usize,
    private: bool,
    owner: Ownership,
) -> Result<Zeroizing<Vec<u8>>, HostError> {
    let (parent, name) = parent_handle(path, owner, false)?;
    let fd = openat(
        &parent,
        &name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| HostError::Unavailable)?;
    let stat = fstat(&fd).map_err(|_| HostError::Unavailable)?;
    validate_file(&stat, owner, private)?;
    if stat.st_size as u64 > max_bytes as u64 {
        return Err(HostError::Limit);
    }
    let file = File::from(fd);
    let mut buffer = Zeroizing::new(Vec::with_capacity(max_bytes + 1));
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut buffer)
        .map_err(|_| HostError::Unavailable)?;
    if buffer.len() > max_bytes {
        return Err(HostError::Limit);
    }
    Ok(buffer)
}

pub(crate) fn read_config(path: &Path, max_bytes: usize) -> Result<Vec<u8>, HostError> {
    let path = validate_path(path, true)?;
    let mut bytes = read_protected(&path, max_bytes, false, Ownership::Operator)?;
    Ok(std::mem::take(&mut *bytes))
}

pub(crate) fn read_secret(path: &Path, max_bytes: usize) -> Result<Zeroizing<Vec<u8>>, HostError> {
    read_protected(
        &validate_path(path, false)?,
        max_bytes,
        true,
        Ownership::Operator,
    )
}

pub(crate) fn verify_openwrt_local() -> Result<(), HostError> {
    let bytes = read_protected(
        Path::new("/etc/openwrt_release"),
        16 * 1024,
        false,
        Ownership::Root,
    )?;
    if openwrt_marker(&bytes) {
        Ok(())
    } else {
        Err(HostError::Unsupported)
    }
}

fn openwrt_marker(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("DISTRIB_ID="));
    lines.next() == Some("DISTRIB_ID='OpenWrt'") && lines.next().is_none()
}

fn file_at(parent: &OwnedFd, name: &OsStr) -> Result<Option<Stat>, HostError> {
    match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            validate_file(&stat, Ownership::Operator, true)?;
            Ok(Some(stat))
        }
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(_) => Err(HostError::Unavailable),
    }
}

fn append_file(parent: &OwnedFd, name: &OsStr) -> Result<File, HostError> {
    let fd = openat(
        parent,
        name,
        OFlags::WRONLY
            | OFlags::APPEND
            | OFlags::CREATE
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| HostError::Unavailable)?;
    let stat = fstat(&fd).map_err(|_| HostError::Unavailable)?;
    // Existing operator-owned 0644 is tightened only after ownership and handle validation.
    // Existing untrusted writers are not made safe by chmod: reject those instead.
    validate_file(&stat, Ownership::Operator, false)?;
    fchmod(&fd, Mode::RUSR | Mode::WUSR).map_err(|_| HostError::Unavailable)?;
    validate_file(
        &fstat(&fd).map_err(|_| HostError::Unavailable)?,
        Ownership::Operator,
        true,
    )?;
    Ok(File::from(fd))
}

pub(crate) struct PrivateLog {
    parent: OwnedFd,
    name: OsString,
    file: Option<File>,
    size: u64,
    max_bytes: u64,
    retained: usize,
}

impl PrivateLog {
    pub(crate) fn open(path: &Path, max_bytes: u64, retained: usize) -> Result<Self, HostError> {
        let path = validate_path(path, false)?;
        let (parent, name) = parent_handle(&path, Ownership::Operator, true)?;
        let file = append_file(&parent, &name)?;
        let size = file.metadata().map_err(|_| HostError::Unavailable)?.len();
        Ok(Self {
            parent,
            name,
            file: Some(file),
            size,
            max_bytes,
            retained,
        })
    }

    pub(crate) fn write(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        let result = self.write_inner(bytes);
        if result.is_err() {
            self.file.take();
        }
        result
    }

    fn write_inner(&mut self, bytes: &[u8]) -> Result<(), HostError> {
        if self.file.is_none() {
            return Err(HostError::Unavailable);
        }
        if bytes.len() as u64 > self.max_bytes {
            return Err(HostError::Limit);
        }
        validate_directory(&self.parent, Ownership::Operator, false)?;
        self.validate_active()?;
        if self.size.saturating_add(bytes.len() as u64) > self.max_bytes {
            self.rotate()?;
        }
        self.file
            .as_mut()
            .ok_or(HostError::Unavailable)?
            .write_all(bytes)
            .map_err(|_| HostError::Unavailable)?;
        self.size = self.size.saturating_add(bytes.len() as u64);
        Ok(())
    }

    fn validate_active(&self) -> Result<(), HostError> {
        let file = self.file.as_ref().ok_or(HostError::Unavailable)?;
        let opened = fstat(file).map_err(|_| HostError::Unavailable)?;
        validate_file(&opened, Ownership::Operator, true)?;
        let named = file_at(&self.parent, &self.name)?.ok_or(HostError::Insecure)?;
        if named.st_ino != opened.st_ino
            || named.st_dev != opened.st_dev
            || opened.st_size as u64 != self.size
        {
            return Err(HostError::Insecure);
        }
        Ok(())
    }

    fn generation(&self, index: usize) -> OsString {
        let mut name = self.name.clone();
        name.push(format!(".{index}"));
        name
    }

    fn rotate(&mut self) -> Result<(), HostError> {
        // All metadata and mutations are relative to the originally verified directory.
        // Another process/external rotator sharing these names is not supported.
        for index in 1..=self.retained {
            file_at(&self.parent, &self.generation(index))?;
        }
        self.file.take();
        let last = self.generation(self.retained);
        if file_at(&self.parent, &last)?.is_some() {
            unlinkat(&self.parent, &last, AtFlags::empty()).map_err(|_| HostError::Unavailable)?;
        }
        for index in (1..self.retained).rev() {
            let source = self.generation(index);
            if file_at(&self.parent, &source)?.is_some() {
                renameat(
                    &self.parent,
                    &source,
                    &self.parent,
                    self.generation(index + 1),
                )
                .map_err(|_| HostError::Unavailable)?;
            }
        }
        renameat(&self.parent, &self.name, &self.parent, self.generation(1))
            .map_err(|_| HostError::Unavailable)?;
        self.file = Some(append_file(&self.parent, &self.name)?);
        self.size = 0;
        Ok(())
    }
}

pub(crate) struct SystemLog {
    socket: UnixDatagram,
}
impl SystemLog {
    pub(crate) fn open() -> Result<Self, HostError> {
        let socket = UnixDatagram::unbound().map_err(|_| HostError::Unavailable)?;
        socket
            .connect("/dev/log")
            .map_err(|_| HostError::Unavailable)?;
        socket
            .set_nonblocking(true)
            .map_err(|_| HostError::Unavailable)?;
        Ok(Self { socket })
    }
    pub(crate) fn send(&self, bytes: &[u8]) -> Result<(), HostError> {
        match self.socket.send(bytes) {
            Ok(count) if count == bytes.len() => Ok(()),
            _ => Err(HostError::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostError, Ownership, openwrt_marker, validate_file};
    #[test]
    fn only_explicit_unique_upstream_marker_is_accepted_without_executing_text() {
        assert!(openwrt_marker(
            b"DISTRIB_ID='OpenWrt'\nDISTRIB_RELEASE='fixture'\n"
        ));
        for data in [
            b"DISTRIB_ID='ImmortalWrt'".as_slice(),
            b"DISTRIB_ID=$(echo OpenWrt)",
            b"DISTRIB_ID='OpenWrt'\nDISTRIB_ID='Other'",
            b"export DISTRIB_ID='OpenWrt'",
            b"# DISTRIB_ID='OpenWrt'",
        ] {
            assert!(!openwrt_marker(data));
        }
    }

    #[test]
    fn integrity_rejects_another_owner_even_when_mode_is_0644() {
        use rustix::fs::{Mode, OFlags, fstat, open};
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("openwrt-mcp-owner-metadata-fixture-{suffix}"));
        let file = open(
            &path,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        let mut stat = fstat(&file).unwrap();
        std::fs::remove_file(&path).unwrap();
        stat.st_mode = (stat.st_mode & !0o777) | 0o644;
        let owner = rustix::process::geteuid().as_raw();
        stat.st_uid = if owner == 1 { 2 } else { 1 };
        assert_eq!(
            validate_file(&stat, Ownership::Operator, false),
            Err(HostError::Insecure)
        );
        stat.st_uid = owner;
        assert_eq!(validate_file(&stat, Ownership::Operator, false), Ok(()));
        if owner != 0 {
            assert_eq!(
                validate_file(&stat, Ownership::Root, false),
                Err(HostError::Insecure)
            );
        }
    }
}
