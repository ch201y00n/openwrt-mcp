//! ADR 0008 source, dependency, lint and native acceptance declarations.
use crate::{CheckResult, Contract};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WindowsReadContract {
    owner: String,
    native_file: String,
    policy_file: String,
    profile: String,
    paths: String,
    ancestors: String,
    leaf: String,
    descriptor: String,
    trusted: String,
    vault: String,
    unsafe_scope: String,
    sdk: String,
    capabilities: String,
    max_path_units: u64,
    max_components: u64,
    max_descriptor_bytes: u64,
    max_token_bytes: u64,
    required_test: String,
}

const OWNER: &str = "openwrt-mcp-host-platform";
const NATIVE: &str = "crates/host-platform/src/windows/native.rs";
const EXPECTED: &str = r#"owner = "openwrt-mcp-host-platform"
native_file = "crates/host-platform/src/windows/native.rs"
policy_file = "crates/host-platform/src/windows/policy.rs"
profile = "local_ntfs_handle_walk_v1"
paths = "absolute_dos_no_alias_stream_or_reparse"
ancestors = "held_no_delete_share_trusted_owner_dacl"
leaf = "single_link_read_only_share"
descriptor = "bounded_self_relative_conservative_allow_aces"
trusted = "process_user_system_administrators_root_trustedinstaller"
vault = "unsupported_no_fallback"
unsafe_scope = "expressions_in_native_file_only"
sdk = "windows-sys_0.61.2_windows_only"
capabilities = "separate_protected_reads_and_private_log"
max_path_units = 4096
max_components = 128
max_descriptor_bytes = 65536
max_token_bytes = 4096
required_test = "crates/host-platform/tests/windows.rs"
"#;

impl Contract {
    pub(crate) fn validate_windows_reads(&self, root: &Path) -> CheckResult {
        let actual = self
            .windows_read_contract
            .as_ref()
            .ok_or("v8 requires Windows read contract")?;
        let expected: WindowsReadContract =
            toml::from_str(EXPECTED).map_err(|_| "invalid native harness profile")?;
        if actual != &expected {
            return Err("Windows read contract must retain the reviewed closed profile".into());
        }
        for file in [
            &actual.native_file,
            &actual.policy_file,
            &actual.required_test,
        ] {
            if self.owner(file)?.0.name != OWNER || !root.join(file).is_file() {
                return Err("missing or unowned Windows read boundary/suite".into());
            }
        }
        if self.crates.iter().any(|rule| {
            rule.name != OWNER && rule.dependencies.iter().any(|dep| dep == "windows-sys")
        }) || !self.crates.iter().any(|rule| {
            rule.name == OWNER && rule.dependencies.iter().any(|dep| dep == "windows-sys")
        }) {
            return Err("Windows SDK belongs exclusively to host-platform".into());
        }
        for rule in self
            .crates
            .iter()
            .filter(|rule| rule.layer != "development")
        {
            let manifest: toml::Value = toml::from_str(
                &fs::read_to_string(root.join(&rule.path).join("Cargo.toml"))
                    .map_err(|_| "missing manifest")?,
            )
            .map_err(|_| "invalid manifest")?;
            check_windows_lints(&rule.name, &manifest)?;
        }
        let workspace: toml::Value = toml::from_str(
            &fs::read_to_string(root.join("Cargo.toml")).map_err(|_| "missing workspace")?,
        )
        .map_err(|_| "invalid workspace")?;
        if workspace["workspace"]["lints"]["rust"]["unsafe_code"].as_str() != Some("forbid") {
            return Err("workspace unsafe forbid must remain".into());
        }
        check_windows_suite(
            &fs::read_to_string(root.join(&actual.required_test))
                .map_err(|_| "missing Windows suite")?,
        )?;
        self.validate_portable_target(root, &actual.required_test)?;
        Ok(())
    }
}

pub fn check_windows_lints(owner: &str, manifest: &toml::Value) -> CheckResult {
    let expected: toml::Value = toml::from_str(if owner == OWNER {
        "[lints.rust]\nunsafe_code = 'deny'\n"
    } else {
        "[lints]\nworkspace = true\n"
    })
    .map_err(|_| "invalid lint profile")?;
    if manifest.get("lints") != expected.get("lints") {
        return Err("production lint boundary must remain exact".into());
    }
    Ok(())
}

pub fn check_windows_suite(source: &str) -> CheckResult {
    let mut ast = syn::parse_file(source).map_err(|_| "invalid native suite")?;
    let cfgs: Vec<_> = ast
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .collect();
    if cfgs.len() != 1
        || cfgs[0]
            .parse_args::<syn::Meta>()
            .map(|meta| quote::quote!(#meta).to_string())
            .ok()
            .as_deref()
            != Some("target_os = \"windows\"")
    {
        return Err("native suite must have exactly the Windows file guard".into());
    }
    ast.attrs.retain(|attr| !attr.path().is_ident("cfg"));
    crate::check_portable_suite(&quote::quote!(#ast).to_string())
}

pub fn check_windows_dependency(owner: &str, dependency: &serde_json::Value) -> CheckResult {
    let mut features: Vec<_> = dependency["features"]
        .as_array()
        .ok_or("missing SDK features")?
        .iter()
        .map(|v| v.as_str().unwrap_or(""))
        .collect();
    features.sort_unstable();
    let mut expected = vec![
        "Wdk_Foundation",
        "Wdk_Storage_FileSystem",
        "Win32_Foundation",
        "Win32_Security",
        "Win32_Storage_FileSystem",
        "Win32_System_IO",
        "Win32_System_Threading",
    ];
    expected.sort_unstable();
    if owner != OWNER
        || dependency["target"].as_str() != Some("cfg(target_os = \"windows\")")
        || dependency["req"].as_str() != Some("=0.61.2")
        || dependency["uses_default_features"].as_bool() != Some(false)
        || features != expected
        || !dependency["path"].is_null()
        || dependency["kind"].as_str() == Some("build")
    {
        return Err("Windows SDK must retain exact owner/target/version/features".into());
    }
    Ok(())
}

pub fn check_owned_source(
    contract: &Contract,
    file: &str,
    source: &str,
    aliases: &BTreeMap<String, String>,
) -> CheckResult {
    let (rule, development) = contract.owner(file)?;
    let native = contract.version >= 8 && rule.name == OWNER && file == NATIVE;
    let mut rule = rule.clone();
    if !native {
        rule.forbidden_paths.push("windows_sys".into());
    } else {
        let ast = syn::parse_file(source).map_err(|_| "invalid native source")?;
        for item in &ast.items {
            match item {
                syn::Item::Use(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native SDK re-exports forbidden".into());
                }
                syn::Item::Fn(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    let visibility = &item.vis;
                    let mut signature = item.sig.clone();
                    if signature.inputs.trailing_punct() {
                        signature.inputs.pop_punct();
                    }
                    let approved: syn::Signature = syn::parse_str("fn read(path: &Path, max_bytes: usize, secret: bool) -> Result<Zeroizing<Vec<u8>>, HostError>").map_err(|_| "invalid native interface profile")?;
                    if quote::quote!(#signature).to_string() != quote::quote!(#approved).to_string()
                        || quote::quote!(#visibility).to_string() != "pub (super)"
                    {
                        return Err(
                            "native boundary only exposes the parent protected-read function"
                                .into(),
                        );
                    }
                }
                syn::Item::Mod(_) => {
                    return Err(
                        "native boundary cannot extend unsafe scope into child modules".into(),
                    );
                }
                syn::Item::Struct(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native types cannot be exported".into());
                }
                syn::Item::Enum(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native types cannot be exported".into());
                }
                syn::Item::Type(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native types cannot be exported".into());
                }
                syn::Item::Const(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native values cannot be exported".into());
                }
                syn::Item::Static(item) if !matches!(item.vis, syn::Visibility::Inherited) => {
                    return Err("native values cannot be exported".into());
                }
                _ => {}
            }
        }
    }
    crate::source::check_source_profile(&rule, source, development, aliases, native)
}
