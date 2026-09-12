//! ADR 0006 checks declared ownership and bounded response profiles.
//! These checks are not a substitute for behavioral tests or device acceptance.
use std::collections::BTreeSet;

use serde::Deserialize;

use crate::{CheckResult, Contract};

const PROJECTION_SUITES: [&str; 3] = [
    "crates/core/tests/collection_projection.rs",
    "crates/features/tests/collection_contracts.rs",
    "crates/runtime/tests/collection_projection.rs",
];
const CODEC_SUITE: &str = "crates/device-codec/tests/action_response.rs";
const MCP_SUITE: &str = "crates/mcp/tests/bounded_results.rs";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProjectionContract {
    pub owner: String,
    pub profile: String,
    pub prepared_invocation: String,
    pub node_forms: Vec<String>,
    pub finite_schema: String,
    pub root_selection: String,
    pub map_keys: String,
    pub builtin_raw_subtrees: String,
    pub invalid_shape: String,
    pub overflow: String,
    pub max_collection_depth: u64,
    pub max_collection_nodes: u64,
    pub max_total_items: u64,
    pub max_emitted_items: u64,
    pub max_items_per_collection: u64,
    pub max_fields_per_record: u64,
    pub max_schema_depth: u64,
    pub max_pointer_depth: u64,
    pub max_pointer_bytes: u64,
    pub max_output_name_bytes: u64,
    pub max_string_bytes: u64,
    pub max_normalized_bytes: u64,
    pub required_tests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ActionResponseContract {
    pub owner: String,
    pub profile: String,
    pub consumers: Vec<String>,
    pub duplicate_keys: String,
    pub max_depth: u64,
    pub max_nodes: u64,
    pub max_bytes: u64,
    pub required_tests: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct McpResultContract {
    pub owner: String,
    pub normalized_owner: String,
    pub normalized_phase: String,
    pub max_serialized_bytes: u64,
    pub serialized_scope: String,
    pub required_tests: Vec<String>,
}

impl Contract {
    pub(crate) fn validate_projection(&self) -> CheckResult {
        let projection = self
            .projection_contract
            .as_ref()
            .ok_or("v6 requires a projection contract")?;
        let response = self
            .action_response_contract
            .as_ref()
            .ok_or("v6 requires a strict action response contract")?;
        let mcp = self
            .mcp_result_contract
            .as_ref()
            .ok_or("v6 requires a bounded MCP result contract")?;

        if projection.owner != "openwrt-mcp-core"
            || projection.profile != "typed_collections_v1"
            || projection.prepared_invocation != "private_core"
            || projection.finite_schema != "two_collection_levels"
            || !exact(
                &projection.node_forms,
                &["Record", "ObjectArray", "ObjectEntries", "ScalarArray"],
            )
            || projection.root_selection != "exact_parameter_equality"
            || projection.map_keys != "explicit_bounded_field"
            || projection.builtin_raw_subtrees != "forbidden"
            || projection.invalid_shape != "reject"
            || projection.overflow != "reject"
        {
            return Err("projection requires the reviewed finite pure-core profile".into());
        }
        // These are architecture hard ceilings; individual operations may narrow
        // their limits, not raise or silently truncate them.
        if projection.max_collection_depth != 2
            || projection.max_collection_nodes != 8
            || projection.max_total_items != 256
            || projection.max_emitted_items != 256
            || projection.max_items_per_collection != 256
            || projection.max_fields_per_record != 64
            || projection.max_schema_depth != 8
            || projection.max_pointer_depth != 8
            || projection.max_pointer_bytes != 512
            || projection.max_output_name_bytes != 64
            || projection.max_string_bytes != 1024
            || projection.max_normalized_bytes != 65_536
        {
            return Err("projection hard ceilings must match the reviewed bounded profile".into());
        }
        if response.owner != "openwrt-mcp-device-codec"
            || response.profile != "strict_json_v1"
            || !exact(
                &response.consumers,
                &["openwrt-mcp-adapters", "openwrt-mcp-backend-ssh"],
            )
            || response.duplicate_keys != "reject"
            || response.max_depth != 32
            || response.max_nodes != 65_536
            || response.max_bytes != 16_777_216
        {
            return Err("action responses require one bounded strict shared codec".into());
        }
        if mcp.owner != "openwrt-mcp-transport"
            || mcp.normalized_owner != "openwrt-mcp-runtime"
            || mcp.normalized_phase != "before_finish_audit"
            || mcp.serialized_scope != "entire_call_tool_result"
            || mcp.max_serialized_bytes != 262_144
        {
            return Err(
                "MCP results require complete serialization and pre-audit normalization bounds"
                    .into(),
            );
        }
        for (name, path, layer) in [
            ("openwrt-mcp-core", "crates/core", "domain"),
            ("openwrt-mcp-features", "crates/features", "features"),
            ("openwrt-mcp-runtime", "crates/runtime", "application"),
            (
                "openwrt-mcp-device-codec",
                "crates/device-codec",
                "infrastructure",
            ),
            ("openwrt-mcp-transport", "crates/mcp", "transport"),
        ] {
            if !self
                .crates
                .iter()
                .any(|rule| rule.name == name && rule.path == path && rule.layer == layer)
                || !self.portable_crates.iter().any(|owner| owner == name)
            {
                return Err(
                    "v6 response owners must keep their portable architecture boundaries".into(),
                );
            }
        }
        for (actual, expected) in [
            (&projection.required_tests, PROJECTION_SUITES.as_slice()),
            (&response.required_tests, &[CODEC_SUITE][..]),
            (&mcp.required_tests, &[MCP_SUITE][..]),
        ] {
            if !exact(actual, expected)
                || actual
                    .iter()
                    .any(|suite| !self.required_portable_tests.contains(suite))
            {
                return Err("v6 requires all five owned portable response suites".into());
            }
        }
        Ok(())
    }
}

fn exact(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect::<BTreeSet<_>>()
}
