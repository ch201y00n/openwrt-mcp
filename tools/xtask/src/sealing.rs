//! ADR 0018: provided streams do not grant storage, capture or mutation authority.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::collections::BTreeMap;
use syn::visit::{self, Visit};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SealingContract {
    owner: String,
    module: String,
    profile: String,
    inputs: String,
    completion: String,
    cipher: String,
    stage: String,
    publication: String,
    late_ack: String,
    cleanup: String,
    unwind: String,
    deadline: String,
    failure: String,
    memory: String,
    effects: String,
    consumers: String,
    fixture_owner: String,
    fixture_dependency: String,
    max_chunk_bytes: u64,
    default_source_bytes: u64,
    default_ciphertext_bytes: u64,
    max_source_bytes: u64,
    max_ciphertext_bytes: u64,
    default_timeout_ms: u64,
    max_timeout_ms: u64,
    max_files: u64,
    max_payload_bytes: u64,
    max_expanded_bytes: u64,
    required_tests: Vec<String>,
}

const EXPECTED: &str = r#"owner = "openwrt-mcp-runtime"
module = "crates/runtime/src/sealing"
profile = "provided_stream_sealing_v1"
inputs = "trusted_prebound_source_checker_cipher_private_stage"
completion = "true_eof_independent_counts_archive_and_producer_complete"
cipher = "public_recipient_encryption_session_bridge_no_identity"
stage = "required_private_atomic_durable_declared_by_trusted_port"
publication = "one_attempt_not_attempted_not_published_published_unknown"
late_ack = "known_published_error_no_abort_or_retry"
cleanup = "once_unfinished_source_and_unpublished_stage_preserve_uncertainty"
unwind = "best_effort_before_publish_unknown_before_callback"
deadline = "monotonic_before_after_callbacks_cooperative_not_cancellation"
failure = "shared_first_stream_error_latch_no_raw_errors"
memory = "counters_and_one_byte_zeroizing_probe_no_whole_stream"
effects = "no_handles_target_policy_decryption_or_mcp"
consumers = "none_until_authorized_workflow_checkpoint"
fixture_owner = "openwrt-mcp"
fixture_dependency = "device_codec_development_only"
max_chunk_bytes = 65536
default_source_bytes = 67108864
default_ciphertext_bytes = 68157440
max_source_bytes = 75497472
max_ciphertext_bytes = 76546048
default_timeout_ms = 30000
max_timeout_ms = 300000
max_files = 4096
max_payload_bytes = 67108864
max_expanded_bytes = 75497472
required_tests = ["crates/runtime/tests/protection.rs", "crates/server/tests/protection_composition.rs"]
"#;

impl Contract {
    pub(crate) fn validate_sealing(&self) -> CheckResult {
        if self.version < 18 {
            if self.archive_sealing_contract.is_some() {
                return Err("archive sealing requires architecture v18".into());
            }
            if self.crates.iter().any(|r| {
                r.name == "openwrt-mcp"
                    && r.dev_dependencies
                        .iter()
                        .any(|d| d == "openwrt-mcp-device-codec")
            }) {
                return Err("sealing fixture codec edge requires architecture v18".into());
            }
            return Ok(());
        }
        let actual = self
            .archive_sealing_contract
            .as_ref()
            .ok_or("v18 requires archive sealing contract")?;
        let expected: SealingContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid sealing harness profile")?;
        if actual != &expected {
            return Err("sealing requires exact bounded supplied-stream profile".into());
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("sealing requires both mandatory native suites".into());
            }
        }
        let owner = self
            .crates
            .iter()
            .find(|r| r.name == actual.owner)
            .ok_or("missing sealing owner")?;
        let mut dependencies = owner.dependencies.clone();
        dependencies.sort();
        if owner.path != "crates/runtime"
            || owner.layer != "application"
            || !self.portable_crates.contains(&owner.name)
            || dependencies
                != [
                    "async-trait",
                    "openwrt-mcp-core",
                    "serde",
                    "serde_json",
                    "thiserror",
                    "tokio",
                    "zeroize",
                ]
            || !owner.dev_dependencies.is_empty()
            || !owner.build_dependencies.is_empty()
        {
            return Err(
                "sealing must retain the portable application owner and existing dependencies"
                    .into(),
            );
        }
        for r in &self.crates {
            if r.layer != "development"
                && r.dependencies.iter().any(|d| d == &owner.name)
                && !r
                    .forbidden_paths
                    .iter()
                    .any(|p| p == "openwrt_mcp_runtime::sealing")
            {
                return Err("sealing has no external production consumer".into());
            }
        }
        let fixture = self
            .crates
            .iter()
            .find(|r| r.name == actual.fixture_owner)
            .ok_or("missing sealing composition fixture")?;
        let mut dependencies = fixture.dev_dependencies.clone();
        dependencies.sort();
        if dependencies
            != [
                "age",
                "async-trait",
                "openwrt-mcp-device-codec",
                "rmcp",
                "russh",
                "zip",
            ]
            || fixture
                .dependencies
                .iter()
                .chain(&fixture.build_dependencies)
                .any(|d| d == "openwrt-mcp-device-codec")
        {
            return Err("sealing codec composition edge must be development-only".into());
        }
        Ok(())
    }
}

pub fn check_sealing_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    if file == "crates/runtime/src/sealing.rs" {
        return Err("sealing uses its reviewed module directory".into());
    }
    if !file.starts_with("crates/runtime/src/") {
        return Ok(());
    }
    let inside = file.starts_with("crates/runtime/src/sealing/");
    if inside && contract.version < 18 {
        return Err("sealing source requires architecture v18".into());
    }
    if !inside && contract.version < 18 {
        return Ok(());
    }
    let (rule, development) = contract.owner(file)?;
    let mut rule = crate::guarded_mutations::scoped_rule(contract, file, rule);
    if inside {
        rule.forbidden_paths.extend(
            [
                "serde",
                "serde_json",
                "openwrt_mcp_core",
                "tokio",
                "std::thread",
                "std::path",
                "std::io::copy",
                "std::panic",
            ]
            .map(str::to_owned),
        );
        rule.allowed_macros.retain(|m| m != "json" && m != "vec");
    }
    crate::check_source_with_aliases(&rule, source, development, aliases)?;
    fn forbidden(name: &str, inside: bool) -> bool {
        let name = name.trim_start_matches("r#");
        if !inside {
            return name == "sealing";
        }
        matches!(
            name,
            "Backend"
                | "Dispatcher"
                | "PreparedAction"
                | "Operation"
                | "Policy"
                | "Action"
                | "backend"
                | "dispatcher"
                | "capability"
                | "packages"
                | "audit"
                | "KeySource"
                | "KeyMaterial"
                | "RecipientMaterial"
                | "IdentityMaterial"
                | "DecryptionSession"
                | "DecryptionProvider"
                | "read_to_end"
                | "read_to_string"
                | "to_vec"
                | "Vec"
        )
    }
    fn tokens(stream: proc_macro2::TokenStream, inside: bool) -> bool {
        stream.into_iter().any(|t| match t {
            proc_macro2::TokenTree::Ident(n) => forbidden(&n.to_string(), inside),
            proc_macro2::TokenTree::Group(g) => tokens(g.stream(), inside),
            _ => false,
        })
    }
    struct Scoped {
        inside: bool,
        rejected: bool,
    }
    impl<'ast> Visit<'ast> for Scoped {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            self.rejected |= forbidden(&name.to_string(), self.inside);
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.rejected |= tokens(item.tokens.clone(), self.inside);
            self.visit_path(&item.path);
        }
        fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
            if self.inside {
                visit::visit_item_mod(self, item);
            } else if let Some((_, items)) = &item.content {
                for item in items {
                    self.visit_item(item);
                }
            }
        }
    }
    let ast = syn::parse_file(source).map_err(|_| "invalid sealing source")?;
    // v21 admits only this internal authorized workflow consumer. The producer's
    // stronger no-action/no-key rules above remain unchanged.
    if !inside
        && contract.version >= 21
        && contract.guarded_mutation_contract.is_some()
        && file.starts_with("crates/runtime/src/transactions/")
    {
        return Ok(());
    }
    let mut visitor = Scoped {
        inside,
        rejected: false,
    };
    visitor.visit_file(&ast);
    if visitor.rejected {
        Err("sealing cannot acquire key/action/whole-stream or sibling authority".into())
    } else {
        Ok(())
    }
}
