//! ADR 0015 declaration and source boundaries, not device protection.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::collections::BTreeMap;
use syn::visit::Visit;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagementEffectContract {
    pub domain_owner: String,
    pub module: String,
    pub profile: String,
    pub purpose: String,
    pub identity: String,
    pub edges: String,
    pub history: String,
    pub completeness: String,
    pub protected: String,
    pub global: String,
    pub result: String,
    pub effects: String,
    pub serialization: String,
    pub consumers: String,
    pub max_nodes: u64,
    pub max_edges: u64,
    pub max_roots: u64,
    pub max_protected: u64,
    pub max_identity_bytes: u64,
    pub required_tests: Vec<String>,
}
const EXPECTED: &str = r#"domain_owner = "openwrt-mcp-core"
module = "crates/core/src/management"
profile = "bounded_influence_graph_v1"
purpose = "non_authorizing_supplied_graph_predicate"
identity = "typed_exact_ascii_no_alias_resolution"
edges = "directed_change_influences_dependent"
history = "conservative_before_after_union"
completeness = "reachable_unknown_denies"
protected = "nonempty_present_in_both_graphs"
global = "deny_with_protected_resources"
result = "count_only_no_known_protected_impact"
effects = "no_device_io_actions_or_policy"
serialization = "no_serde_no_json"
consumers = "none_until_separate_integration_checkpoint"
max_nodes = 4096
max_edges = 8192
max_roots = 256
max_protected = 128
max_identity_bytes = 128
required_tests = ["crates/core/tests/security.rs"]
"#;
const NAMESPACE: &str = "openwrt_mcp_core::management";

impl Contract {
    pub(crate) fn validate_management_effects(&self) -> CheckResult {
        if self.version < 15 {
            return if self.management_effect_contract.is_some() {
                Err("management effect model requires architecture v15".into())
            } else {
                Ok(())
            };
        }
        let actual = self
            .management_effect_contract
            .as_ref()
            .ok_or("v15 requires bounded management effect contract")?;
        let expected: ManagementEffectContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid harness management effect contract")?;
        if actual != &expected {
            return Err("management effects must retain the non-authorizing bounded model".into());
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("management effects require the native core security suite".into());
            }
        }
        let core = self
            .crates
            .iter()
            .find(|rule| rule.name == "openwrt-mcp-core")
            .ok_or("missing effect domain owner")?;
        if core.path != "crates/core" || core.layer != "domain" {
            return Err("management effects must stay in the pure core owner".into());
        }
        for consumer in &self.crates {
            if consumer.layer != "development"
                && consumer
                    .dependencies
                    .iter()
                    .any(|name| name == "openwrt-mcp-core")
                && !consumer
                    .forbidden_paths
                    .iter()
                    .any(|path| path == NAMESPACE)
            {
                return Err("management effects cannot enter production consumers before integration review".into());
            }
        }
        Ok(())
    }
}

/// Independent source overlay; does not add management rules to native OS code.
pub fn check_management_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    if file == "crates/core/src/management.rs" {
        return Err("management effects use the reviewed module directory".into());
    }
    if !file.starts_with("crates/core/src/management/") {
        return Ok(());
    }
    if contract.version < 15 {
        return Err("management effect source requires architecture v15".into());
    }
    let (rule, development) = contract.owner(file)?;
    let mut rule = rule.clone();
    rule.forbidden_paths
        .extend(["serde", "serde_json"].map(str::to_owned));
    rule.allowed_macros.retain(|name| name != "json");
    crate::check_source_with_aliases(&rule, source, development, aliases)?;
    let ast = syn::parse_file(source).map_err(|_| "invalid effect source")?;
    #[derive(Default)]
    struct NoAuthority {
        rejected: bool,
    }
    fn reserved(name: &str) -> bool {
        matches!(
            name,
            "Action"
                | "PreparedAction"
                | "PreparedInvocation"
                | "Operation"
                | "Catalog"
                | "Policy"
                | "Requirement"
                | "Grant"
                | "Permission"
                | "policy"
                | "operation"
                | "catalog"
        )
    }
    fn tokens(stream: proc_macro2::TokenStream) -> bool {
        stream.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Ident(name) => reserved(&name.to_string()),
            proc_macro2::TokenTree::Group(group) => tokens(group.stream()),
            _ => false,
        })
    }
    impl<'ast> Visit<'ast> for NoAuthority {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            self.rejected |= reserved(&name.to_string());
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.rejected |= tokens(item.tokens.clone());
        }
    }
    let mut check = NoAuthority::default();
    check.visit_file(&ast);
    if check.rejected {
        Err("management predicate cannot acquire action or policy authority".into())
    } else {
        Ok(())
    }
}
