use openwrt_mcp_runtime::protection::{KeyMaterial, ProtectionError};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileProtection {
    Restricted,
    PersonalVault,
}

/// A platform integration must actually enforce its advertised protection.
pub trait ProtectedFileAccess: Send + Sync {
    fn read(
        &self,
        path: &Path,
        protection: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError>;
}

pub struct NativeFileAccess;

impl ProtectedFileAccess for NativeFileAccess {
    fn read(
        &self,
        path: &Path,
        protection: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError> {
        match protection {
            FileProtection::PersonalVault => Err(ProtectionError::UnsupportedProtection),
            FileProtection::Restricted => {
                #[cfg(unix)]
                {
                    restricted_unix(path, max_bytes)
                }
                #[cfg(not(unix))]
                {
                    let _ = (path, max_bytes);
                    Err(ProtectionError::UnsupportedProtection)
                }
            }
        }
    }
}

#[cfg(unix)]
fn restricted_unix(path: &Path, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
    use rustix::fs::{Mode, OFlags, fstat, open, openat};
    use std::io::Read;
    use std::path::Component;
    use zeroize::Zeroizing;

    super::config::file_path(path)?;
    if max_bytes == 0 || max_bytes > 16 * 1024 * 1024 {
        return Err(ProtectionError::ResourceLimit);
    }
    let owner = rustix::process::geteuid().as_raw();
    let components: Vec<_> = path.components().collect();
    let mut parent = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| ProtectionError::SourceUnavailable)?;
    for (index, component) in components.iter().enumerate().skip(1) {
        let Component::Normal(name) = component else {
            return Err(ProtectionError::InvalidConfig);
        };
        let directory = index + 1 < components.len();
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
        let opened = openat(
            &parent,
            *name,
            if directory {
                flags | OFlags::DIRECTORY
            } else {
                flags
            },
            Mode::empty(),
        )
        .map_err(|_| ProtectionError::SourceUnavailable)?;
        let stat = fstat(&opened).map_err(|_| ProtectionError::SourceUnavailable)?;
        if stat.st_uid != owner && stat.st_uid != 0 {
            return Err(ProtectionError::UnsupportedProtection);
        }
        if directory {
            // A root-owned sticky directory (e.g. /tmp) prevents other users replacing
            // our owned child. Every following component is opened relative to its handle.
            let trusted_sticky = stat.st_uid == 0 && stat.st_mode & 0o1000 != 0;
            if stat.st_mode & 0o022 != 0 && !trusted_sticky {
                return Err(ProtectionError::UnsupportedProtection);
            }
            parent = opened;
            continue;
        }
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
            || stat.st_mode & 0o077 != 0
            || stat.st_nlink != 1
            || stat.st_size < 0
        {
            return Err(ProtectionError::UnsupportedProtection);
        }
        if stat.st_size as u64 > max_bytes as u64 {
            return Err(ProtectionError::ResourceLimit);
        }
        let file = std::fs::File::from(opened);
        let mut bytes = Zeroizing::new(Vec::with_capacity(max_bytes.saturating_add(1)));
        file.take(max_bytes.saturating_add(1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| ProtectionError::SourceUnavailable)?;
        return KeyMaterial::from_zeroizing(bytes, max_bytes);
    }
    Err(ProtectionError::InvalidConfig)
}
