//! ADR 0019: a private audit writer is not generic host or artifact-store authority.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WindowsLogContract {
    owner: String,
    facade: String,
    native_file: String,
    profile: String,
    port: String,
    entry: String,
    creation: String,
    append: String,
    rotation: String,
    validation: String,
    failure: String,
    capabilities: String,
    durability: String,
    max_bytes: u64,
    max_retained: u64,
    max_chunk_bytes: u64,
    max_path_units: u64,
    max_component_units: u64,
    max_components: u64,
    max_descriptor_bytes: u64,
    required_test: String,
}

const EXPECTED: &str = r#"owner = "openwrt-mcp-host-platform"
facade = "crates/host-platform/src/windows/log.rs"
native_file = "crates/host-platform/src/windows/native.rs"
profile = "local_ntfs_private_audit_v1"
port = "opaque_parent_send_write_only_no_native_types"
entry = "open_log_exact_parent_box_log_writer"
creation = "current_user_protected_dacl_noinherit_create_nooverwrite"
append = "only_append_no_write_data_no_delete_share"
rotation = "preflight_held_generations_oldest_delete_descending_no_replace_create_new"
validation = "held_ancestors_owner_dacl_single_link_name_size_before_after"
failure = "terminal_latch_no_reopen_no_cleanup_unvalidated"
capabilities = "private_log_only_not_vault_syslog_or_ciphertext_store"
durability = "no_fsync_no_atomic_rotation_no_hard_cancellation"
max_bytes = 1073741824
max_retained = 100
max_chunk_bytes = 65536
max_path_units = 4096
max_component_units = 255
max_components = 128
max_descriptor_bytes = 65536
required_test = "crates/host-platform/tests/windows.rs"
"#;
const FACADE: &str = "crates/host-platform/src/windows/log.rs";

impl Contract {
    pub(crate) fn validate_windows_logs(&self, root: &Path) -> CheckResult {
        if self.version < 19 {
            return if self.windows_log_contract.is_some() {
                Err("Windows private logs require architecture v19".into())
            } else {
                Ok(())
            };
        }
        let actual = self
            .windows_log_contract
            .as_ref()
            .ok_or("v19 requires Windows log contract")?;
        let expected: WindowsLogContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid Windows log harness profile")?;
        if actual != &expected {
            return Err("Windows log contract must retain exact bounded NTFS profile".into());
        }
        for file in [&actual.facade, &actual.native_file, &actual.required_test] {
            let owner = self.owner(file)?.0;
            if owner.name != actual.owner || owner.layer != "infrastructure" {
                return Err("Windows log files remain owned by host-platform".into());
            }
        }
        // Existing suite is required before implementation; no new fake scaffold.
        let suite = fs::read_to_string(root.join(&actual.required_test))
            .map_err(|_| "missing Windows log suite")?;
        crate::check_windows_suite(&suite)?;
        self.validate_portable_target(root, &actual.required_test)?;
        Ok(())
    }
}

pub fn check_windows_log_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    if file != FACADE {
        return Ok(());
    }
    if contract.version < 19 {
        return Err("Windows log facade requires v19".into());
    }
    let (rule, development) = contract.owner(file)?;
    let mut rule = rule.clone();
    rule.forbidden_paths.extend(
        [
            "windows_sys",
            "std::fs",
            "std::io",
            "std::os",
            "std::env",
            "std::net",
            "std::process",
            "std::thread",
            "tokio",
        ]
        .map(str::to_owned),
    );
    crate::check_source_with_aliases(&rule, source, development, aliases)?;
    let ast = syn::parse_file(source).map_err(|_| "invalid log facade")?;
    let approved: syn::ItemTrait = syn::parse_str(
        "pub(super) trait LogWriter: Send { fn write(&mut self, bytes: &[u8]) -> Result<(), HostError>; }"
    ).map_err(|_| "invalid log port profile")?;
    let mut found = 0;
    for item in &ast.items {
        if let syn::Item::Trait(port) = item {
            let mut port = port.clone();
            if port.attrs.iter().any(|attr| !attr.path().is_ident("doc")) {
                return Err("log port cannot be conditional or attribute-transformed".into());
            }
            port.attrs.clear();
            for item in &mut port.items {
                if let syn::TraitItem::Fn(method) = item {
                    if method.attrs.iter().any(|attr| !attr.path().is_ident("doc")) {
                        return Err(
                            "log method cannot be conditional or attribute-transformed".into()
                        );
                    }
                    method.attrs.clear();
                }
            }
            if quote::quote!(#port).to_string() != quote::quote!(#approved).to_string() {
                return Err("log facade exposes only the exact opaque write port".into());
            }
            found += 1;
        }
    }
    if found != 1 {
        return Err("log facade requires one exact parent-only Send write port".into());
    }
    Ok(())
}
