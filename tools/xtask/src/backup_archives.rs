//! ADR 0016: pure archive codec, not backup or restore authority.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::collections::BTreeMap;
use syn::visit::Visit;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BackupArchiveContract {
    pub owner: String,
    pub module: String,
    pub profile: String,
    pub formats: String,
    pub input: String,
    pub manifest: String,
    pub paths: String,
    pub entries: String,
    pub numbers: String,
    pub completion: String,
    pub failure: String,
    pub memory: String,
    pub result: String,
    pub effects: String,
    pub consumers: String,
    pub max_chunk_bytes: u64,
    pub max_archive_bytes: u64,
    pub max_payload_bytes: u64,
    pub max_file_bytes: u64,
    pub max_files: u64,
    pub max_path_bytes: u64,
    pub max_path_components: u64,
    pub max_component_bytes: u64,
    pub max_tail_bytes: u64,
    pub required_tests: Vec<String>,
}
const EXPECTED: &str = r#"owner = "openwrt-mcp-device-codec"
module = "crates/device-codec/src/archive"
profile = "regular_tar_manifest_v1"
formats = "exact_ustar_or_gnu_without_extensions"
input = "incremental_supplied_uncompressed_bytes"
manifest = "nonempty_exact_paths_and_sizes_all_once_no_file_ancestors"
paths = "bounded_relative_ascii_no_normalization"
entries = "regular_only_no_links_directories_extensions_or_special"
numbers = "unsigned_octal_no_base256_or_embedded_terminators"
completion = "two_zero_blocks_bounded_aligned_zero_tail_and_full_manifest"
failure = "latched_first_error_no_reset"
memory = "zeroizing_headers_paths_no_payload_retention"
result = "count_only_structure_not_provenance_or_producer_success"
effects = "no_io_crypto_actions_extraction_or_authorization"
consumers = "none_until_separate_integration_checkpoint"
max_chunk_bytes = 65536
max_archive_bytes = 75497472
max_payload_bytes = 67108864
max_file_bytes = 8388608
max_files = 4096
max_path_bytes = 256
max_path_components = 32
max_component_bytes = 100
max_tail_bytes = 32768
required_tests = ["crates/device-codec/tests/action_response.rs"]
"#;
const NAMESPACE: &str = "openwrt_mcp_device_codec::archive";

impl Contract {
    pub(crate) fn validate_backup_archives(&self) -> CheckResult {
        if self.version < 16 {
            return if self.backup_archive_contract.is_some() {
                Err("backup archive validation requires architecture v16".into())
            } else {
                Ok(())
            };
        }
        let actual = self
            .backup_archive_contract
            .as_ref()
            .ok_or("v16 requires bounded backup archive contract")?;
        let mut expected: BackupArchiveContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid harness archive contract")?;
        if self.version >= 17 {
            expected.consumers = "gzip_wrapper_only_no_external_consumers".into();
        }
        if actual != &expected {
            return Err(
                "archive codec must retain its exact bounded non-authorizing profile".into(),
            );
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("archive codec requires its native response suite".into());
            }
        }
        let codec = self
            .crates
            .iter()
            .find(|r| r.name == "openwrt-mcp-device-codec")
            .ok_or("missing archive codec owner")?;
        let mut dependencies = codec.dependencies.clone();
        dependencies.sort();
        let expected_dependencies: &[&str] = if self.version >= 17 {
            &[
                "flate2",
                "openwrt-mcp-core",
                "serde",
                "serde_json",
                "zeroize",
            ]
        } else {
            &["openwrt-mcp-core", "serde", "serde_json", "zeroize"]
        };
        if codec.path != "crates/device-codec"
            || codec.layer != "infrastructure"
            || !self.portable_crates.contains(&codec.name)
            || dependencies != expected_dependencies
            || !codec.dev_dependencies.is_empty()
            || !codec.build_dependencies.is_empty()
        {
            return Err(
                "archive codec requires its pure portable owner and reviewed dependencies".into(),
            );
        }
        for consumer in &self.crates {
            if consumer.layer != "development"
                && consumer.dependencies.iter().any(|d| d == &codec.name)
                && !consumer.forbidden_paths.iter().any(|p| p == NAMESPACE)
            {
                return Err(
                    "archive codec has no production consumers before integration review".into(),
                );
            }
        }
        Ok(())
    }
}

pub fn check_archive_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    if file == "crates/device-codec/src/archive.rs" {
        return Err("archive codec uses the reviewed module directory".into());
    }
    if !file.starts_with("crates/device-codec/src/archive/") {
        return Ok(());
    }
    if contract.version < 16 {
        return Err("archive codec source requires architecture v16".into());
    }
    let (rule, development) = contract.owner(file)?;
    let mut rule = rule.clone();
    rule.forbidden_paths.extend(
        [
            "serde",
            "serde_json",
            "openwrt_mcp_core",
            "std::io",
            "std::path",
        ]
        .map(str::to_owned),
    );
    rule.allowed_macros.retain(|m| m != "json");
    crate::check_source_with_aliases(&rule, source, development, aliases)?;
    fn reserved(name: &str) -> bool {
        matches!(
            name,
            "command"
                | "describe"
                | "packages"
                | "response"
                | "CommandSpec"
                | "compile_action"
                | "compile_probe"
                | "encode_remote"
                | "parse_action_response"
                | "parse_ubus_describe"
                | "Action"
                | "PreparedAction"
                | "Policy"
                | "Operation"
        )
    }
    fn tokens(stream: proc_macro2::TokenStream) -> bool {
        stream.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Ident(name) => reserved(&name.to_string()),
            proc_macro2::TokenTree::Group(group) => tokens(group.stream()),
            _ => false,
        })
    }
    #[derive(Default)]
    struct NoAuthority {
        rejected: bool,
    }
    impl<'ast> Visit<'ast> for NoAuthority {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            self.rejected |= reserved(&name.to_string());
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.rejected |= tokens(item.tokens.clone());
        }
    }
    let ast = syn::parse_file(source).map_err(|_| "invalid archive source")?;
    let mut check = NoAuthority::default();
    check.visit_file(&ast);
    if check.rejected {
        Err("archive predicate cannot acquire command or action authority".into())
    } else {
        Ok(())
    }
}
