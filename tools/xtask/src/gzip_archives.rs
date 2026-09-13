//! ADR 0017: supplied gzip validation without I/O or backup authority.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::collections::BTreeMap;
use syn::visit::Visit;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GzipArchiveContract {
    pub owner: String,
    pub module: String,
    pub profile: String,
    pub input: String,
    pub header: String,
    pub integrity: String,
    pub completion: String,
    pub inner: String,
    pub failure: String,
    pub result: String,
    pub memory: String,
    pub effects: String,
    pub consumers: String,
    pub backend: String,
    pub max_chunk_bytes: u64,
    pub max_source_bytes: u64,
    pub max_header_bytes: u64,
    pub max_extra_bytes: u64,
    pub max_name_bytes: u64,
    pub max_comment_bytes: u64,
    pub output_buffer_bytes: u64,
    pub max_expansion_ratio: u64,
    pub expansion_slack_bytes: u64,
    pub expansion_check: String,
    pub required_tests: Vec<String>,
}
const EXPECTED: &str = r#"owner = "openwrt-mcp-device-codec"
module = "crates/device-codec/src/gzip"
profile = "single_gzip_regular_tar_v1"
input = "bounded_incremental_supplied_gzip_bytes"
header = "exact_deflate_reserved_zero_bounded_optional_metadata"
integrity = "backend_checked_header_crc_payload_crc32_and_isize"
completion = "one_member_exact_end_no_suffix_or_concatenation"
inner = "owned_regular_tar_manifest_validator_no_early_summary"
failure = "latched_first_error_no_reset_or_fallback"
result = "count_only_structure_and_corruption_check_not_authenticity"
memory = "zeroizing_owned_buffers_bounded_backend_window_not_scrub_guaranteed"
effects = "no_io_keys_crypto_extraction_or_authorization"
consumers = "none_until_separate_workflow_integration"
backend = "flate2_1.1.10_zlib_rs_only_low_level"
max_chunk_bytes = 65536
max_source_bytes = 75497472
max_header_bytes = 4096
max_extra_bytes = 4084
max_name_bytes = 1024
max_comment_bytes = 1024
output_buffer_bytes = 4096
max_expansion_ratio = 128
expansion_slack_bytes = 1048576
expansion_check = "final_complete_deflate_bytes_excluding_header_trailer"
required_tests = ["crates/device-codec/tests/action_response.rs"]
"#;
const NAMESPACE: &str = "openwrt_mcp_device_codec::gzip";

impl Contract {
    pub(crate) fn validate_gzip_archives(&self) -> CheckResult {
        if self.version < 17 {
            return if self.gzip_archive_contract.is_some() {
                Err("gzip archive validation requires architecture v17".into())
            } else {
                Ok(())
            };
        }
        let actual = self
            .gzip_archive_contract
            .as_ref()
            .ok_or("v17 requires bounded gzip contract")?;
        let expected: GzipArchiveContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid gzip harness contract")?;
        if actual != &expected {
            return Err("gzip requires its exact bounded single-member profile".into());
        }
        for suite in &actual.required_tests {
            if !self.required_portable_tests.contains(suite) {
                return Err("gzip requires the mandatory native response suite".into());
            }
        }
        for consumer in &self.crates {
            if consumer.layer != "development"
                && consumer
                    .dependencies
                    .iter()
                    .any(|d| d == "openwrt-mcp-device-codec")
                && !consumer.forbidden_paths.iter().any(|p| p == NAMESPACE)
            {
                return Err("gzip has no external production consumer before integration".into());
            }
        }
        Ok(())
    }
}

/// No actual dependency edge is required at the architecture-only checkpoint.
/// When introduced, every alias/kind/target must preserve the reviewed profile.
pub fn check_gzip_dependency(owner: &str, dependency: &serde_json::Value) -> CheckResult {
    if owner != "openwrt-mcp-device-codec"
        || dependency["req"] != "=1.1.10"
        || dependency["uses_default_features"] != false
        || dependency["features"] != serde_json::json!(["zlib-rs"])
        || dependency["optional"] != false
        || !dependency["kind"].is_null()
        || !dependency["target"].is_null()
        || !dependency["path"].is_null()
        || dependency["source"] != "registry+https://github.com/rust-lang/crates.io-index"
    {
        return Err(
            "gzip dependency must be pinned flate2 with only the reviewed Rust backend".into(),
        );
    }
    Ok(())
}

pub fn check_gzip_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    if file == "crates/device-codec/src/gzip.rs" {
        return Err("gzip uses its reviewed module directory".into());
    }
    if !file.starts_with("crates/device-codec/src/") {
        return Ok(());
    }
    let inside = file.starts_with("crates/device-codec/src/gzip/");
    if inside && contract.version < 17 {
        return Err("gzip source requires architecture v17".into());
    }
    let (rule, development) = contract.owner(file)?;
    let mut rule = rule.clone();
    if !inside {
        rule.forbidden_paths.push("flate2".into());
        // Reserved namespace references, including relative/raw aliases and
        // approved macro tokens, cannot hide a sibling consumer. Module
        // declarations themselves remain allowed at the producer root.
        struct NoSibling {
            archive: bool,
            rejected: bool,
        }
        impl<'ast> Visit<'ast> for NoSibling {
            fn visit_ident(&mut self, name: &'ast syn::Ident) {
                let name = name.to_string();
                let name = name.trim_start_matches("r#");
                self.rejected |= name == "gzip" || (self.archive && name == "archive");
            }
            fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
                if let Some((_, items)) = &item.content {
                    for item in items {
                        self.visit_item(item);
                    }
                }
            }
            fn visit_macro(&mut self, item: &'ast syn::Macro) {
                fn contains(stream: proc_macro2::TokenStream, archive: bool) -> bool {
                    stream.into_iter().any(|t| match t {
                        proc_macro2::TokenTree::Ident(n) => {
                            let n = n.to_string();
                            let n = n.trim_start_matches("r#");
                            n == "gzip" || (archive && n == "archive")
                        }
                        proc_macro2::TokenTree::Group(g) => contains(g.stream(), archive),
                        _ => false,
                    })
                }
                self.rejected |= contains(item.tokens.clone(), self.archive);
                self.visit_path(&item.path);
            }
        }
        let ast = syn::parse_file(source).map_err(|_| "invalid codec source")?;
        let mut check = NoSibling {
            archive: !file.starts_with("crates/device-codec/src/archive/"),
            rejected: false,
        };
        check.visit_file(&ast);
        if check.rejected {
            return Err("gzip admits no sibling archive consumer".into());
        }
        return crate::check_source_with_aliases(&rule, source, development, aliases);
    }
    rule.forbidden_paths.extend(
        [
            "serde",
            "serde_json",
            "openwrt_mcp_core",
            "std::io",
            "std::path",
            "flate2::read",
            "flate2::write",
            "flate2::bufread",
            "flate2::Compress",
            "flate2::Compression",
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
                | "Action"
                | "PreparedAction"
                | "Policy"
                | "Operation"
                | "reset"
                | "set_dictionary"
                | "decompress_uninit"
                | "decompress_vec"
        )
    }
    fn tokens(stream: proc_macro2::TokenStream) -> bool {
        stream.into_iter().any(|token| match token {
            proc_macro2::TokenTree::Ident(name) => {
                reserved(name.to_string().trim_start_matches("r#"))
            }
            proc_macro2::TokenTree::Group(group) => tokens(group.stream()),
            _ => false,
        })
    }
    #[derive(Default)]
    struct NoEscape {
        rejected: bool,
    }
    impl<'ast> Visit<'ast> for NoEscape {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            self.rejected |= reserved(name.to_string().trim_start_matches("r#"));
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.rejected |= tokens(item.tokens.clone());
        }
    }
    let ast = syn::parse_file(source).map_err(|_| "invalid gzip source")?;
    let mut check = NoEscape::default();
    check.visit_file(&ast);
    if check.rejected {
        Err("gzip cannot acquire action or decoder escape APIs".into())
    } else {
        Ok(())
    }
}
