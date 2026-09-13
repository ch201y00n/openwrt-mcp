//! ADR 0014 declarations; no production commands or device acceptance here.
use crate::{CheckResult, Contract};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpkgStatusContract {
    pub profile: String,
    pub admission: String,
    pub commands: String,
    pub parser: String,
    pub projection: String,
    pub read_effects: String,
    pub snapshot: String,
    pub max_line_bytes: u64,
    pub max_fields_per_stanza: u64,
    pub max_field_name_bytes: u64,
    pub max_status_bytes: u64,
    pub required_tests: Vec<String>,
}

const EXPECTED: &str = r#"profile = "opkg_38eccbb1_root_status_v1"
admission = "same_backend_exact_version_then_fixed_root_status_read"
commands = "env_i_fixed_opkg_version_and_cat_root_status"
parser = "bounded_utf8_lf_stanzas_unique_case_insensitive_fields"
projection = "name_version_arch_status_no_layer_no_filter"
read_effects = "no_opkg_init_no_config_no_scripts"
snapshot = "shared_with_apk_profile_operation_epoch_nonce_absolute_ttl"
max_line_bytes = 8192
max_fields_per_stanza = 64
max_field_name_bytes = 64
max_status_bytes = 128
required_tests = ["crates/core/tests/packages.rs", "crates/features/tests/packages.rs", "crates/runtime/tests/packages.rs", "crates/device-codec/tests/packages.rs", "crates/mcp/tests/packages.rs", "crates/backend-ssh/tests/remote.rs"]
"#;

impl Contract {
    pub(crate) fn validate_opkg_status(&self) -> CheckResult {
        if self.version < 14 {
            return if self.opkg_status_contract.is_some() {
                Err("opkg status expansion requires architecture v14".into())
            } else {
                Ok(())
            };
        }
        let actual = self
            .opkg_status_contract
            .as_ref()
            .ok_or("v14 requires closed opkg status contract")?;
        let expected: OpkgStatusContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid harness opkg contract")?;
        if actual != &expected {
            return Err("opkg status must retain the reviewed closed bounded contract".into());
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("opkg status suites must run on every required native host".into());
            }
        }
        // Common ownership, permissions, memory/lifetime and entropy rules are
        // checked by the unchanged package contract, not duplicated here.
        self.validate_packages()
    }
}
