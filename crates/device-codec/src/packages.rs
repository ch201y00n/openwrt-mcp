//! Closed package profiles. No paths/options are supplied by a client.
mod opkg;
use crate::{CodecError, CommandSpec, parse_action_response};
use openwrt_mcp_core::packages::{
    MAX_PACKAGE_RECORDS, MAX_PACKAGE_SOURCE_BYTES, PackageObservation,
};
pub use opkg::{
    MAX_OPKG_VERSION_BYTES, opkg_status_command, opkg_version_command, parse_opkg_status,
    validate_opkg_version,
};

pub const MAX_APK_VERSION_BYTES: usize = 256;

fn apk(args: &[&str]) -> CommandSpec {
    CommandSpec {
        program: "/usr/bin/env".into(),
        arguments: [
            "-i",
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "LANG=C",
            "APK_CONFIG=/dev/null",
            "/usr/bin/apk",
            "--no-network",
            "--no-cache",
            "--no-logfile",
        ]
        .into_iter()
        .chain(args.iter().copied())
        .map(str::to_owned)
        .collect(),
    }
}

pub fn apk_version_command() -> CommandSpec {
    apk(&["--version"])
}
pub fn apk_installed_command() -> CommandSpec {
    apk(&[
        "query",
        "--from",
        "installed",
        "--installed",
        "--all-matches",
        "--format",
        "json",
        "--fields",
        "name,version,arch,layer",
        "*",
    ])
}

pub fn validate_apk_version(bytes: &[u8]) -> Result<(), CodecError> {
    if bytes.len() > MAX_APK_VERSION_BYTES {
        return Err(CodecError::OutputLimit);
    }
    let banner = std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidObservation)?;
    let arch = banner
        .strip_prefix("apk-tools 3.0.5, compiled for ")
        .and_then(|s| s.strip_suffix(".\n"))
        .ok_or(CodecError::InvalidObservation)?;
    if arch.is_empty()
        || arch.len() > 64
        || !arch
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(CodecError::InvalidObservation);
    }
    Ok(())
}

pub fn parse_apk_installed(bytes: &[u8], maximum: usize) -> Result<PackageObservation, CodecError> {
    let value = parse_action_response(bytes, maximum.min(MAX_PACKAGE_SOURCE_BYTES))?;
    let rows = value.as_array().ok_or(CodecError::InvalidResponse)?;
    if rows.len() > MAX_PACKAGE_RECORDS {
        return Err(CodecError::OutputLimit);
    }
    let records = serde_json::from_value(value).map_err(|_| CodecError::InvalidResponse)?;
    PackageObservation::new(records).map_err(|error| match error {
        openwrt_mcp_core::CoreError::OutputLimit => CodecError::OutputLimit,
        _ => CodecError::InvalidResponse,
    })
}
