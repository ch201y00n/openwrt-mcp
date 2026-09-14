//! Closed P3 infrastructure admission, not behavior or native durability evidence.
use crate::{CheckResult, Contract, CrateRule};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};
use syn::visit::{self, Visit};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct CiphertextFoundationContract(BTreeMap<String, String>);

const RULES: &[(&str, &str)] = &[
    ("profile", "authenticated_preprovisioned_record_store_v1"),
    ("design", "docs/ciphertext-foundations.md"),
    (
        "scope",
        "provided_admitted_system_tar_or_single_gzip_no_hooks_no_mutation_authority",
    ),
    (
        "capture",
        "same_pinned_ssh_fixed_guardian_subsystem_binary_eof_exit_count_no_json",
    ),
    ("stage", "private_bounded_memory_only_no_plaintext_files"),
    (
        "store",
        "preexisting_authenticated_file_exclusive_lock_append_only_no_namespace_mutation",
    ),
    (
        "publication",
        "complete_authenticated_record_visibility_then_sync_all_or_unknown",
    ),
    (
        "provenance",
        "hmac_sha256_separate_32_byte_key_store_target_boot_job_scope_format_counts_ciphertext",
    ),
    (
        "restore",
        "verify_expected_binding_and_mac_then_full_age_auth_before_inspection",
    ),
    (
        "worker",
        "one_process_wide_joinable_thread_no_queue_cancel_on_drop_join_before_release",
    ),
    (
        "limitations",
        "operator_provisioned_durable_name_local_filesystem_no_acl_or_root_isolation_claim",
    ),
    ("max_plaintext_bytes", "131072"),
    ("max_ciphertext_bytes", "1048576"),
    ("max_store_records", "32"),
    ("max_store_bytes", "33562688"),
    ("max_transfer_bytes", "65536"),
    ("max_timeout_ms", "30000"),
    (
        "dependencies",
        "crypto-age:hmac=0.12.1,sha2=0.10.9;adapters:zeroize;backend-ssh:zeroize;key-sources:async-trait",
    ),
    (
        "tests",
        "tools/xtask/tests/ciphertext_foundations.rs;crates/server/tests/protection_composition.rs;crates/backend-ssh/tests/remote.rs",
    ),
];
pub(crate) const NAMESPACES: &[&str] = &[
    "openwrt_mcp_runtime::backups",
    "openwrt_mcp_host_platform::ciphertext",
];
const EDGES: &[(&str, &str)] = &[
    (
        "crates/adapters/src/backups/",
        "openwrt_mcp_runtime::backups",
    ),
    (
        "crates/backend-ssh/src/transactions/",
        "openwrt_mcp_runtime::backups",
    ),
    (
        "crates/crypto-age/src/provenance/",
        "openwrt_mcp_runtime::backups",
    ),
    (
        "crates/server/src/transactions/",
        "openwrt_mcp_runtime::backups",
    ),
    (
        "crates/adapters/src/backups/",
        "openwrt_mcp_host_platform::ciphertext",
    ),
];

impl Contract {
    pub(crate) fn validate_ciphertext_foundations(&self, root: &Path) -> CheckResult {
        if self.version < 22 {
            return if self.ciphertext_foundation_contract.is_some() {
                Err("ciphertext foundations require architecture v22".into())
            } else {
                Ok(())
            };
        }
        let actual = &self
            .ciphertext_foundation_contract
            .as_ref()
            .ok_or("v22 requires ciphertext foundations")?
            .0;
        let expected: BTreeMap<_, _> = RULES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        if actual != &expected {
            return Err("ciphertext foundations require exact reviewed declarations".into());
        }
        for path in [
            "docs/ciphertext-foundations.md",
            "tools/xtask/tests/ciphertext_foundations.rs",
        ] {
            let text = std::fs::read_to_string(root.join(path))
                .map_err(|_| "missing ciphertext contract evidence")?;
            if path.ends_with(".rs") {
                crate::check_portable_suite(&text)?;
            }
        }
        Ok(())
    }
}

pub(crate) fn scope(contract: &Contract, file: &str, rule: &mut CrateRule) {
    rule.forbidden_paths
        .extend(NAMESPACES.iter().map(|s| s.to_string()));
    if contract.version >= 22 && contract.ciphertext_foundation_contract.is_some() {
        for (directory, namespace) in EDGES {
            if file.starts_with(directory) {
                rule.forbidden_paths.retain(|s| s != namespace);
            }
        }
    }
    if !file.starts_with("crates/crypto-age/src/provenance/") || contract.version < 22 {
        rule.forbidden_paths.extend(["hmac".into(), "sha2".into()]);
    }
}

pub fn check_ciphertext_dependency(
    contract: &Contract,
    owner: &str,
    dependency: &serde_json::Value,
) -> CheckResult {
    let name = dependency["name"].as_str().unwrap_or("");
    let version = match name {
        "hmac" => "=0.12.1",
        "sha2" => "=0.10.9",
        _ => return Ok(()),
    };
    if contract.version < 22
        || owner != "openwrt-mcp-crypto-age"
        || dependency["req"].as_str() != Some(version)
        || !dependency["kind"].is_null()
        || !dependency["target"].is_null()
        || dependency["uses_default_features"].as_bool() != Some(false)
        || dependency["features"]
            .as_array()
            .is_none_or(|f| !f.is_empty())
    {
        return Err(
            "ciphertext authentication dependencies require exact reviewed owner/version/features"
                .into(),
        );
    }
    Ok(())
}

pub fn check_ciphertext_source(contract: &Contract, file: &str, source: &str) -> CheckResult {
    let (_, development) = contract.owner(file)?;
    if development {
        return Ok(());
    }
    let owned = [
        "crates/runtime/src/backups/",
        "crates/host-platform/src/ciphertext/",
        "crates/crypto-age/src/provenance/",
    ];
    if owned
        .iter()
        .any(|d| file == format!("{}.rs", d.trim_end_matches('/')))
    {
        return Err("ciphertext foundations require owned directories".into());
    }
    if owned.iter().any(|d| file.starts_with(d)) && contract.version < 22 {
        return Err("ciphertext sources require architecture v22".into());
    }
    struct Check {
        private: bool,
        storage: bool,
        rejected: bool,
    }
    impl Check {
        fn tokens(&mut self, tokens: proc_macro2::TokenStream) {
            for token in tokens {
                match token {
                    proc_macro2::TokenTree::Ident(n) => self.visit_ident(&n),
                    proc_macro2::TokenTree::Group(g) => self.tokens(g.stream()),
                    _ => {}
                }
            }
        }
    }
    impl<'ast> Visit<'ast> for Check {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            let name = name.to_string();
            let name = name.trim_start_matches("r#");
            self.rejected |= self.private
                && matches!(
                    name,
                    "Serialize"
                        | "Deserialize"
                        | "serde"
                        | "serde_json"
                        | "json"
                        | "PreparedAction"
                        | "Command"
                        | "Dispatcher"
                        | "Admission"
                        | "ValidatedPlan"
                        | "DurableBackup"
                );
            self.rejected |= self.storage
                && matches!(
                    name,
                    "create"
                        | "create_new"
                        | "truncate"
                        | "set_len"
                        | "rename"
                        | "hard_link"
                        | "remove_file"
                        | "remove_dir"
                        | "remove_dir_all"
                        | "set_permissions"
                        | "chmod"
                );
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.tokens(item.tokens.clone());
            self.visit_path(&item.path);
        }
        fn visit_attribute(&mut self, item: &'ast syn::Attribute) {
            if let syn::Meta::List(list) = &item.meta {
                self.tokens(list.tokens.clone());
            }
            visit::visit_attribute(self, item);
        }
    }
    let mut check = Check {
        private: file.starts_with("crates/runtime/src/backups/"),
        storage: file.starts_with("crates/host-platform/src/ciphertext/"),
        rejected: false,
    };
    check.visit_file(&syn::parse_file(source).map_err(|_| "invalid ciphertext source")?);
    if check.rejected {
        Err("ciphertext authority/serialization/namespace-write escape".into())
    } else {
        Ok(())
    }
}
