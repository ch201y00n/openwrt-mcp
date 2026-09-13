//! ADR 0007 declaration checks, not device acceptance.
use crate::{CheckResult, Contract};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PackageContract {
    pub domain_owner: String,
    pub workflow_owner: String,
    pub codec_owner: String,
    pub entropy_owner: String,
    pub profile: String,
    pub admission: String,
    pub permission: String,
    pub snapshot: String,
    pub continuation: String,
    pub refresh: String,
    pub capability_status: String,
    pub whole_device_complete: bool,
    pub max_source_bytes: u64,
    pub max_records: u64,
    pub max_retained_bytes: u64,
    pub page_records: u64,
    pub ttl_seconds: u64,
    pub nonce_bytes: u64,
    pub max_name_bytes: u64,
    pub max_version_bytes: u64,
    pub max_arch_bytes: u64,
    pub required_tests: Vec<String>,
}

const EXPECTED: &str = r#"domain_owner = "openwrt-mcp-core"
workflow_owner = "openwrt-mcp-runtime"
codec_owner = "openwrt-mcp-device-codec"
entropy_owner = "openwrt-mcp-adapters"
profile = "apk_3_0_5_visible_v1"
admission = "same_backend_version_and_complete_query"
permission = "packages_read"
snapshot = "single_private_immutable"
continuation = "nonce_operation_epoch_absolute_ttl"
refresh = "invalidate_before_attempt"
capability_status = "unknown_capture_required"
whole_device_complete = false
max_source_bytes = 4194304
max_records = 4096
max_retained_bytes = 2097152
page_records = 16
ttl_seconds = 120
nonce_bytes = 16
max_name_bytes = 256
max_version_bytes = 256
max_arch_bytes = 64
required_tests = ["crates/core/tests/packages.rs", "crates/features/tests/packages.rs", "crates/runtime/tests/packages.rs", "crates/device-codec/tests/packages.rs", "crates/mcp/tests/packages.rs"]
"#;

impl Contract {
    pub(crate) fn validate_packages(&self) -> CheckResult {
        let actual = self
            .package_contract
            .as_ref()
            .ok_or("v7 requires package contract")?;
        let expected: PackageContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid harness profile")?;
        if actual != &expected {
            return Err("package contract must retain the reviewed bounded closed profile".into());
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("package suites must run on all required native hosts".into());
            }
        }
        let entropy = self
            .crates
            .iter()
            .find(|rule| rule.name == actual.entropy_owner)
            .ok_or("missing entropy owner")?;
        if entropy.path != "crates/adapters"
            || entropy.layer != "infrastructure"
            || !entropy.dependencies.iter().any(|name| name == "getrandom")
            || self.crates.iter().any(|rule| {
                rule.name != actual.entropy_owner
                    && rule.dependencies.iter().any(|name| name == "getrandom")
            })
        {
            return Err("OS entropy belongs exclusively to adapters".into());
        }
        Ok(())
    }
}
