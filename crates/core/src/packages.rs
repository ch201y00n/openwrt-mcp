//! Bounded source-specific observations, not whole-device inventories or baselines.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{CoreError, check_normalized_result};

pub const MAX_PACKAGE_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_PACKAGE_RECORDS: usize = 4096;
pub const MAX_PACKAGE_RETAINED_BYTES: usize = 2 * 1024 * 1024;
pub const PACKAGE_PAGE_RECORDS: usize = 16;
pub const PACKAGE_RESPONSE_CONTRACT: &str = "packages_apk_installed.v1";
pub const OPKG_RESPONSE_CONTRACT: &str = "packages_opkg_status.v1";
pub const MAX_OPKG_STATUS_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackageProfile {
    Apk3_0_5,
    Opkg38eccbb1RootStatus,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageRecord {
    pub name: String,
    pub version: String,
    pub arch: String,
    #[serde(default)]
    pub layer: u8,
}

#[derive(Clone, Serialize)]
pub struct OpkgStatusRecord {
    pub name: String,
    pub version: String,
    pub arch: String,
    pub status: String,
}

enum Records {
    Apk(Vec<PackageRecord>),
    Opkg(Vec<OpkgStatusRecord>),
}

/// Private fields prevent bypassing complete validation and deterministic ordering.
/// No raw Debug or serialization of the entire retained observation.
pub struct PackageObservation {
    records: Records,
}

impl PackageObservation {
    pub fn new(mut records: Vec<PackageRecord>) -> Result<Self, CoreError> {
        if records.len() > MAX_PACKAGE_RECORDS {
            return Err(CoreError::OutputLimit);
        }
        let mut retained = 0_usize;
        for row in &records {
            for (text, maximum) in [(&row.name, 256), (&row.version, 256), (&row.arch, 64)] {
                if text.is_empty() || text.len() > maximum || text.chars().any(char::is_control) {
                    return Err(CoreError::InvalidOutput);
                }
                retained = retained
                    .checked_add(text.len())
                    .ok_or(CoreError::OutputLimit)?;
                if retained > MAX_PACKAGE_RETAINED_BYTES {
                    return Err(CoreError::OutputLimit);
                }
            }
            if row.layer > 1 {
                return Err(CoreError::InvalidOutput);
            }
        }
        records.sort_by(|left, right| (left.layer, &left.name).cmp(&(right.layer, &right.name)));
        if records
            .windows(2)
            .any(|pair| pair[0].layer == pair[1].layer && pair[0].name == pair[1].name)
        {
            return Err(CoreError::InvalidOutput);
        }
        Ok(Self {
            records: Records::Apk(records),
        })
    }

    pub fn new_opkg_status(mut records: Vec<OpkgStatusRecord>) -> Result<Self, CoreError> {
        if records.len() > MAX_PACKAGE_RECORDS {
            return Err(CoreError::OutputLimit);
        }
        let mut retained = 0_usize;
        for row in &records {
            for (text, maximum) in [
                (&row.name, 256),
                (&row.version, 256),
                (&row.arch, 64),
                (&row.status, MAX_OPKG_STATUS_BYTES),
            ] {
                if text.is_empty() || text.len() > maximum || text.chars().any(char::is_control) {
                    return Err(CoreError::InvalidOutput);
                }
                retained = retained
                    .checked_add(text.len())
                    .ok_or(CoreError::OutputLimit)?;
                if retained > MAX_PACKAGE_RETAINED_BYTES {
                    return Err(CoreError::OutputLimit);
                }
            }
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        if records.windows(2).any(|pair| pair[0].name == pair[1].name) {
            return Err(CoreError::InvalidOutput);
        }
        Ok(Self {
            records: Records::Opkg(records),
        })
    }

    pub fn profile(&self) -> PackageProfile {
        match &self.records {
            Records::Apk(_) => PackageProfile::Apk3_0_5,
            Records::Opkg(_) => PackageProfile::Opkg38eccbb1RootStatus,
        }
    }

    pub fn page(&self, nonce: &[u8; 16], offset: usize) -> Result<Value, CoreError> {
        let count = match &self.records {
            Records::Apk(rows) => rows.len(),
            Records::Opkg(rows) => rows.len(),
        };
        if *nonce == [0; 16]
            || !offset.is_multiple_of(PACKAGE_PAGE_RECORDS)
            || (offset != 0 && offset >= count)
        {
            return Err(CoreError::InvalidArguments);
        }
        let end = offset.saturating_add(PACKAGE_PAGE_RECORDS).min(count);
        // At most 16 validated records with <=704 bytes each. Even worst-case
        // escaping fits the normalized ceiling before these bounded copies.
        let (source, scope, items) = match &self.records {
            Records::Apk(rows) => (
                "apk_3_0_5",
                "apk_query_installed_visible",
                json!(&rows[offset..end]),
            ),
            Records::Opkg(rows) => (
                "opkg_38eccbb1",
                "opkg_root_status_file",
                json!(&rows[offset..end]),
            ),
        };
        let mut result = json!({
            "source":source, "scope":scope,
            "consistency":"non_atomic_observation", "whole_device_complete":false,
            "captured_count":count, "offset":offset
        });
        result["items"] = items;
        if end < count {
            result["next_cursor"] = json!(PackageCursor::encode(nonce, end));
        }
        check_normalized_result(&result)?;
        Ok(result)
    }
}

/// Syntax validation only. Runtime must bind nonce, operation, epoch and TTL.
pub struct PackageCursor {
    nonce: [u8; 16],
    offset: usize,
}

impl PackageCursor {
    pub fn parse(value: &str) -> Result<Self, CoreError> {
        let invalid = CoreError::InvalidArguments;
        if value.len() > 37 {
            return Err(invalid);
        }
        let (hex, offset) = value.split_once('.').ok_or(invalid)?;
        if hex.len() != 32
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid);
        }
        let mut nonce = [0; 16];
        for (index, byte) in nonce.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).map_err(|_| invalid)?;
        }
        let number: usize = offset.parse().map_err(|_| invalid)?;
        if nonce == [0; 16]
            || number == 0
            || number >= MAX_PACKAGE_RECORDS
            || !number.is_multiple_of(PACKAGE_PAGE_RECORDS)
            || number.to_string() != offset
        {
            return Err(invalid);
        }
        Ok(Self {
            nonce,
            offset: number,
        })
    }

    pub fn nonce(&self) -> &[u8; 16] {
        &self.nonce
    }
    pub fn offset(&self) -> usize {
        self.offset
    }

    fn encode(nonce: &[u8; 16], offset: usize) -> String {
        let hex: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("{hex}.{offset}")
    }
}
