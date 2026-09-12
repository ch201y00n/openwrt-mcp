use crate::{CheckResult, Contract};
use serde_json::Value;
use std::collections::BTreeSet;

/// Cargo resolves dependency renames, workspace inheritance and target tables for us.
pub fn check_metadata(contract: &Contract, metadata: &Value) -> CheckResult {
    let packages = metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing workspace members")?;
    let root = metadata["workspace_root"]
        .as_str()
        .ok_or("missing workspace root")?
        .replace('\\', "/");
    let mut seen = BTreeSet::new();
    for id in members {
        let package = packages
            .iter()
            .find(|package| &package["id"] == id)
            .ok_or("missing workspace package")?;
        let name = package["name"].as_str().ok_or("missing package name")?;
        let rule = contract
            .crates
            .iter()
            .find(|rule| rule.name == name)
            .ok_or_else(|| format!("unowned workspace package: {name}"))?;
        if !seen.insert(name) {
            return Err("duplicate workspace package".into());
        }
        let manifest = package["manifest_path"]
            .as_str()
            .ok_or("missing manifest path")?
            .replace('\\', "/");
        if manifest != format!("{root}/{}/Cargo.toml", rule.path) {
            return Err(format!("wrong owner path for {name}"));
        }
        for dependency in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            let original = dependency["name"]
                .as_str()
                .ok_or("missing dependency package name")?;
            let kind = dependency["kind"].as_str().unwrap_or("normal");
            let allowed = match kind {
                "normal" => rule.dependencies.iter().any(|value| value == original),
                "dev" => rule
                    .dependencies
                    .iter()
                    .chain(&rule.dev_dependencies)
                    .any(|value| value == original),
                "build" => rule
                    .build_dependencies
                    .iter()
                    .any(|value| value == original),
                _ => false,
            };
            if !allowed {
                return Err(format!(
                    "{name}: forbidden {kind} dependency {original} (all aliases/targets checked)"
                ));
            }
            if contract.version >= 4 && matches!(original, "rustix" | "libc") {
                let target = dependency["target"]
                    .as_str()
                    .unwrap_or_default()
                    .split_whitespace()
                    .collect::<String>();
                if !matches!(target.as_str(), "cfg(unix)" | "cfg(target_os=\"linux\")") {
                    return Err(format!(
                        "{name}: platform dependency {original} must retain its reviewed Unix/Linux target condition"
                    ));
                }
            }
            if let Some(owner) = contract.crates.iter().find(|owner| owner.name == original) {
                let actual = dependency["path"].as_str().unwrap_or("").replace('\\', "/");
                if actual != format!("{root}/{}", owner.path) {
                    return Err(format!(
                        "{name}: dependency does not use owned workspace package {original}"
                    ));
                }
            } else if !dependency["path"].is_null() {
                return Err(format!("{name}: unowned path dependency {original}"));
            }
        }
        for target in package["targets"].as_array().ok_or("missing targets")? {
            let kinds = target["kind"].as_array().ok_or("missing target kind")?;
            if kinds
                .iter()
                .any(|kind| kind == "custom-build" || kind == "proc-macro")
            {
                return Err(format!(
                    "{name}: build scripts and workspace procedural macros require a new architecture"
                ));
            }
            let source = target["src_path"]
                .as_str()
                .ok_or("missing target source")?
                .replace('\\', "/");
            let relative = source
                .strip_prefix(&format!("{root}/"))
                .ok_or("target source escaped workspace")?;
            let (owner, _) = contract.owner(relative)?;
            if owner.name != name {
                return Err("target source belongs to another package".into());
            }
            if kinds.is_empty() {
                return Err("target has no reviewed kind".into());
            }
            for kind in kinds {
                let directory = match kind.as_str() {
                    Some("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "bin") => "src",
                    Some("test") => "tests",
                    Some("example") => "examples",
                    _ => return Err("target kind is not covered by the architecture".into()),
                };
                if !relative.starts_with(&format!("{}/{directory}/", rule.path)) {
                    return Err(format!(
                        "{name}: target kind must use its standard {directory}/ directory"
                    ));
                }
            }
        }
    }
    if seen.len() != contract.crates.len() {
        return Err("contract contains missing workspace package".into());
    }
    Ok(())
}
