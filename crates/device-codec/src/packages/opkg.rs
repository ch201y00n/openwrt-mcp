//! Fixed root status-file observation; never initialize opkg configuration.
use crate::{CodecError, CommandSpec};
use openwrt_mcp_core::packages::{
    MAX_PACKAGE_RECORDS, MAX_PACKAGE_SOURCE_BYTES, OpkgStatusRecord, PackageObservation,
};
use std::collections::BTreeSet;

pub const MAX_OPKG_VERSION_BYTES: usize = 256;
const MAX_LINE_BYTES: usize = 8192;
const MAX_FIELDS: usize = 64;
const MAX_FIELD_NAME_BYTES: usize = 64;

fn command(program: &str, argument: &str) -> CommandSpec {
    CommandSpec {
        program: "/usr/bin/env".into(),
        arguments: [
            "-i",
            "PATH=/usr/sbin:/usr/bin:/sbin:/bin",
            "LANG=C",
            program,
            argument,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    }
}

pub fn opkg_version_command() -> CommandSpec {
    command("/bin/opkg", "--version")
}
pub fn opkg_status_command() -> CommandSpec {
    command("/bin/cat", "/usr/lib/opkg/status")
}

pub fn validate_opkg_version(bytes: &[u8]) -> Result<(), CodecError> {
    if bytes.len() > MAX_OPKG_VERSION_BYTES {
        return Err(CodecError::OutputLimit);
    }
    if bytes != b"opkg version 38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)\n" {
        return Err(CodecError::InvalidObservation);
    }
    Ok(())
}

pub fn parse_opkg_status(bytes: &[u8], maximum: usize) -> Result<PackageObservation, CodecError> {
    if bytes.len() > maximum.min(MAX_PACKAGE_SOURCE_BYTES) {
        return Err(CodecError::OutputLimit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidResponse)?;
    if (!text.is_empty() && !text.ends_with("\n\n"))
        || text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(CodecError::InvalidResponse);
    }
    let mut rows = Vec::new();
    let mut fields = BTreeSet::new();
    let mut selected: [Option<&str>; 4] = [None; 4];
    let mut previous_selected = false;
    for line in text.split_terminator('\n') {
        if line.len() > MAX_LINE_BYTES {
            return Err(CodecError::OutputLimit);
        }
        if line.is_empty() {
            if fields.is_empty() {
                continue;
            }
            if rows.len() >= MAX_PACKAGE_RECORDS {
                return Err(CodecError::OutputLimit);
            }
            let [Some(name), Some(version), Some(arch), Some(status)] = selected else {
                return Err(CodecError::InvalidResponse);
            };
            rows.push(OpkgStatusRecord {
                name: name.into(),
                version: version.into(),
                arch: arch.into(),
                status: status.into(),
            });
            fields.clear();
            selected = [None; 4];
            previous_selected = false;
            continue;
        }
        if line.starts_with([' ', '\t']) {
            if fields.is_empty() || previous_selected {
                return Err(CodecError::InvalidResponse);
            }
            continue;
        }
        let (name, raw) = line.split_once(':').ok_or(CodecError::InvalidResponse)?;
        if name.is_empty()
            || name.len() > MAX_FIELD_NAME_BYTES
            || !name.as_bytes()[0].is_ascii_alphabetic()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(CodecError::InvalidResponse);
        }
        if fields.len() >= MAX_FIELDS {
            return Err(CodecError::OutputLimit);
        }
        let name = name.to_ascii_lowercase();
        let value = if raw.is_empty() {
            raw
        } else {
            raw.strip_prefix(' ').ok_or(CodecError::InvalidResponse)?
        };
        previous_selected = if let Some(index) = ["package", "version", "architecture", "status"]
            .iter()
            .position(|field| *field == name)
        {
            let bound = [
                256,
                256,
                64,
                openwrt_mcp_core::packages::MAX_OPKG_STATUS_BYTES,
            ][index];
            if value.is_empty() || value.len() > bound || value.chars().any(char::is_control) {
                return Err(CodecError::InvalidResponse);
            }
            selected[index] = Some(value);
            true
        } else {
            false
        };
        if !fields.insert(name) {
            return Err(CodecError::InvalidResponse);
        }
    }
    // The terminal blank-line requirement prevents silently accepting a last
    // incomplete stanza. Constructors independently enforce retained limits.
    PackageObservation::new_opkg_status(rows).map_err(|error| match error {
        openwrt_mcp_core::CoreError::OutputLimit => CodecError::OutputLimit,
        _ => CodecError::InvalidResponse,
    })
}
