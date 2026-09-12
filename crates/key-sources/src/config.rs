use openwrt_mcp_runtime::protection::ProtectionError;
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerFormat {
    Zip,
}

/// Locations only: there is intentionally no inline key or fallback option.
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceConfig {
    RestrictedFile {
        path: PathBuf,
    },
    PersonalVaultFile {
        path: PathBuf,
    },
    Environment {
        variable: String,
    },
    ArchiveEntry {
        source: String,
        format: ContainerFormat,
        entry: String,
    },
}

pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(crate) fn environment_name(value: &str) -> bool {
    identifier(value)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && value.as_bytes()[0].is_ascii_alphabetic()
}

pub(crate) fn file_path(path: &Path) -> Result<(), ProtectionError> {
    if !path.is_absolute()
        || path.as_os_str().len() > 4096
        || path.as_os_str().as_encoded_bytes().contains(&0)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || path.file_name().is_none()
    {
        return Err(ProtectionError::InvalidConfig);
    }
    Ok(())
}

pub(crate) fn entry_name(value: &str, directory: bool) -> bool {
    if value.is_empty() || value.len() > 1024 || value.contains(['\\', ':', '\0']) {
        return false;
    }
    let name = if directory {
        value.strip_suffix('/').unwrap_or(value)
    } else {
        value
    };
    !name.is_empty()
        && name
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}
