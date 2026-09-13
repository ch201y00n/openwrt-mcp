//! ADR 0011 declaration checks; no configuration I/O or runtime admission.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::collections::BTreeSet;

const PROFILES: [&str; 6] = [
    "system:system",
    "network:interface",
    "wireless:wifi-device",
    "firewall:defaults",
    "dhcp:dnsmasq",
    "fstab:mount",
];
const SUITES: [&str; 5] = [
    "crates/core/tests/capabilities.rs",
    "crates/core/tests/collection_projection.rs",
    "crates/features/tests/collection_contracts.rs",
    "crates/runtime/tests/collection_projection.rs",
    "crates/mcp/tests/read_contracts.rs",
];

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UciReadContract {
    pub domain_owner: String,
    pub definitions_owner: String,
    pub invocation: String,
    pub profiles: Vec<String>,
    pub actions: String,
    pub custom_uci: String,
    pub requirement: String,
    pub projection: String,
    pub view: String,
    pub required_tests: Vec<String>,
}

fn exact(values: &[String], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && values.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect()
}

impl Contract {
    pub(crate) fn validate_uci_reads(&self) -> CheckResult {
        if self.version < 11 {
            return if self.uci_read_contract.is_some() {
                Err("closed UCI reads require architecture v11".into())
            } else {
                Ok(())
            };
        }
        let uci = self
            .uci_read_contract
            .as_ref()
            .ok_or("v11 requires a closed UCI read contract")?;
        if uci.domain_owner != "openwrt-mcp-core"
            || uci.definitions_owner != "openwrt-mcp-features"
            || uci.invocation != "existing_authorized_audited_dispatcher"
            || !exact(&uci.profiles, &PROFILES)
            || uci.actions != "fixed_uci_get_config_type_no_parameters"
            || uci.custom_uci != "deny"
            || uci.requirement != "profile_category_read"
            || uci.projection != "typed_values_map_error_guard_exact_section_type"
            || uci.view != "rpcd_sessionless_shared_delta_non_atomic"
            || !exact(&uci.required_tests, &SUITES)
            || SUITES.iter().any(|suite| {
                !self
                    .required_portable_tests
                    .iter()
                    .any(|required| required == suite)
            })
        {
            return Err(
                "closed UCI reads require exact owners recipes denial projection and source scope"
                    .into(),
            );
        }
        for (path, name) in [
            ("crates/core", "openwrt-mcp-core"),
            ("crates/features", "openwrt-mcp-features"),
            ("crates/runtime", "openwrt-mcp-runtime"),
            ("crates/device-codec", "openwrt-mcp-device-codec"),
            ("crates/mcp", "openwrt-mcp-transport"),
        ] {
            if !self
                .crates
                .iter()
                .any(|rule| rule.path == path && rule.name == name)
                || !self.portable_crates.iter().any(|owner| owner == name)
            {
                return Err("UCI observations retain existing portable owners".into());
            }
        }
        Ok(())
    }
}
