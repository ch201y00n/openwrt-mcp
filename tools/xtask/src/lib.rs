//! Development-only architecture checks; never linked into the shipped server.

mod backup_archives;
mod capability;
mod change;
mod gzip_archives;
mod management_effects;
mod metadata;
mod opkg_status;
mod packages;
mod portability;
mod projection;
mod sealing;
mod source;
mod spec;
mod uci_reads;
mod windows_logs;
mod windows_reads;

pub use backup_archives::check_archive_source;
pub use capability::{
    CapabilityContract, check_capability_registry, check_capability_registry_for_version,
    check_compatibility_evidence,
};
pub use change::validate_evolution;
pub use gzip_archives::{check_gzip_dependency, check_gzip_source};
pub use management_effects::check_management_source;
pub use metadata::check_metadata;
pub use portability::{check_native_ci, check_portable_source, check_portable_suite};
pub use projection::{ActionResponseContract, McpResultContract, ProjectionContract};
pub use sealing::check_sealing_source;
pub use source::{check_public_reexports, check_source, check_source_with_aliases};
pub use spec::{Contract, CrateRule};
pub use windows_logs::check_windows_log_source;
pub use windows_reads::{
    check_owned_source, check_windows_dependency, check_windows_lints, check_windows_suite,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

type CheckResult<T = ()> = Result<T, String>;

fn command(root: &Path, program: &str, args: &[&str]) -> CheckResult<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|_| format!("could not run {program}"))?;
    if !output.status.success() {
        return Err(format!("{program} failed while checking architecture"));
    }
    String::from_utf8(output.stdout).map_err(|_| "non-UTF-8 tool output".into())
}

fn relative(root: &Path, path: &Path) -> CheckResult<String> {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .map_err(|_| "source escaped workspace".into())
}

fn walk(root: &Path, directory: &Path, files: &mut Vec<String>) -> CheckResult {
    for entry in fs::read_dir(directory).map_err(|_| "cannot inventory workspace")? {
        let entry = entry.map_err(|_| "cannot inventory directory entry")?;
        if directory == root && matches!(entry.file_name().to_str(), Some(".git" | "target")) {
            continue;
        }
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|_| "cannot inspect source path")?;
        if kind.is_symlink() {
            return Err(format!(
                "symlink is not an owned source: {}",
                relative(root, &path)?
            ));
        }
        if kind.is_dir() {
            walk(root, &path, files)?;
        } else if kind.is_file() {
            files.push(relative(root, &path)?);
        }
    }
    Ok(())
}

/// Validate the current tree, then enforce documented evolution from a Git baseline.
pub fn architecture(root: &Path, baseline: Option<&str>) -> CheckResult {
    let root = root
        .canonicalize()
        .map_err(|_| "workspace does not exist")?;
    let text = fs::read_to_string(root.join("architecture/spec.toml"))
        .map_err(|_| "architecture/spec.toml is required")?;
    let contract = Contract::parse(&text)?;
    contract.validate(&root)?;
    let metadata: serde_json::Value = serde_json::from_str(&command(
        &root,
        "cargo",
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
    )?)
    .map_err(|_| "invalid Cargo metadata")?;
    check_metadata(&contract, &metadata)?;

    // Cargo crate roots do not use the non-root file-stem module directory
    // rule. This includes tests/examples named something other than main.rs.
    // Ownership/kinds/paths were checked above; never infer roots from a folder
    // name or exempt their descendants from the recursive source inventory.
    let crate_roots = target_roots(&metadata)?;

    let mut files = Vec::new();
    walk(&root, &root, &mut files)?;
    for file in &files {
        if file.ends_with("Cargo.toml")
            && file != "Cargo.toml"
            && !contract
                .crates
                .iter()
                .any(|rule| file == &format!("{}/Cargo.toml", rule.path))
        {
            return Err(format!("unowned Cargo manifest: {file}"));
        }
        if !file.ends_with(".rs") {
            continue;
        }
        let (rule, development) = contract.owner(file)?;
        let source = fs::read_to_string(root.join(file)).map_err(|_| "cannot read Rust source")?;
        if !development && contract.portable_crates.contains(&rule.name) {
            check_portable_source(&source).map_err(|error| format!("{file}: {error}"))?;
        }
        let aliases = dependency_aliases(&metadata, &rule.name)?;
        check_owned_source(&contract, file, &source, &aliases)
            .map_err(|error| format!("{file}: {error}"))?;
        check_windows_log_source(&contract, file, &source, &aliases)
            .map_err(|error| format!("{file}: {error}"))?;
        check_management_source(&contract, file, &source, &aliases)
            .map_err(|error| format!("{file}: {error}"))?;
        check_archive_source(&contract, file, &source, &aliases)
            .map_err(|error| format!("{file}: {error}"))?;
        check_sealing_source(&contract, file, &source, &aliases)
            .map_err(|error| format!("{file}: {error}"))?;
        if contract.version >= 17 || file.starts_with("crates/device-codec/src/gzip") {
            check_gzip_source(&contract, file, &source, &aliases)
                .map_err(|error| format!("{file}: {error}"))?;
        }
        if !development {
            // A consumer's private-namespace ban must survive producer-side
            // root aliases; otherwise `runtime::protection::T` becomes `runtime::T`.
            let namespace_prefix = format!("{}::", rule.name.replace('-', "_"));
            let protected: BTreeSet<_> = contract
                .crates
                .iter()
                .flat_map(|consumer| &consumer.forbidden_paths)
                .filter_map(|path| path.strip_prefix(&namespace_prefix))
                .map(str::to_owned)
                .collect();
            check_public_reexports(&source, &protected, &namespace_prefix)
                .map_err(|error| format!("{file}: {error}"))?;
        }
        validate_modules(&root, file, &source, crate_roots.contains(file))
            .map_err(|error| format!("{file}: {error}"))?;
    }

    let base = baseline.unwrap_or("HEAD");
    // Resolve first to prevent option/ref ambiguity and make the reviewed baseline exact.
    let revision = command(
        &root,
        "git",
        &["rev-parse", "--verify", &format!("{base}^{{commit}}")],
    )?;
    let revision = revision.trim();
    let prior = command(
        &root,
        "git",
        &["show", &format!("{revision}:architecture/spec.toml")],
    )
    .ok();
    let mut changed: BTreeSet<String> =
        command(&root, "git", &["diff", "--name-only", revision, "--"])?
            .lines()
            .map(str::to_owned)
            .collect();
    changed.extend(
        command(
            &root,
            "git",
            &["ls-files", "--others", "--exclude-standard"],
        )?
        .lines()
        .map(str::to_owned),
    );
    validate_evolution(prior.as_deref(), &text, &changed)?;
    println!(
        "architecture v{}: {} crate boundaries and recursive source checks passed",
        contract.version,
        contract.crates.len()
    );
    Ok(())
}

fn dependency_aliases(
    metadata: &serde_json::Value,
    name: &str,
) -> CheckResult<BTreeMap<String, String>> {
    let package = metadata["packages"]
        .as_array()
        .ok_or("missing packages")?
        .iter()
        .find(|package| package["name"] == name)
        .ok_or("missing source owner")?;
    let mut aliases = BTreeMap::new();
    for dependency in package["dependencies"]
        .as_array()
        .ok_or("missing dependencies")?
    {
        let original = dependency["name"]
            .as_str()
            .ok_or("missing dependency name")?
            .replace('-', "_");
        let renamed = dependency["rename"]
            .as_str()
            .unwrap_or(&original)
            .replace('-', "_");
        if let Some(existing) = aliases.insert(renamed, original.clone())
            && existing != original
        {
            return Err("target-dependent crate rename is ambiguous to the source checker".into());
        }
    }
    Ok(aliases)
}

fn target_roots(metadata: &serde_json::Value) -> CheckResult<BTreeSet<String>> {
    let workspace = metadata["workspace_root"]
        .as_str()
        .ok_or("missing workspace root")?
        .replace('\\', "/");
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing workspace members")?;
    let mut roots = BTreeSet::new();
    for package in metadata["packages"].as_array().ok_or("missing packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        for target in package["targets"].as_array().ok_or("missing targets")? {
            let source = target["src_path"]
                .as_str()
                .ok_or("missing target source")?
                .replace('\\', "/");
            let relative = source
                .strip_prefix(&format!("{workspace}/"))
                .ok_or("target source escaped workspace")?;
            roots.insert(relative.to_owned());
        }
    }
    Ok(roots)
}

fn validate_modules(root: &Path, file: &str, source: &str, crate_root: bool) -> CheckResult {
    let ast = syn::parse_file(source).map_err(|_| "invalid Rust source")?;
    let path = root.join(file);
    let parent = path.parent().ok_or("module has no directory")?;
    let module_directory =
        if crate_root || matches!(path.file_name().and_then(|v| v.to_str()), Some("mod.rs")) {
            parent.to_owned()
        } else {
            parent.join(path.file_stem().ok_or("module has no stem")?)
        };
    fn items(items: &[syn::Item], directory: &Path) -> CheckResult {
        for item in items {
            if let syn::Item::Mod(module) = item {
                let name = module.ident.to_string();
                if let Some((_, children)) = &module.content {
                    items_check(children, &directory.join(name))?;
                } else {
                    let flat = directory.join(format!("{name}.rs"));
                    let nested = directory.join(name).join("mod.rs");
                    if usize::from(flat.is_file()) + usize::from(nested.is_file()) != 1 {
                        return Err(
                            "external module must resolve to exactly one owned Rust file".into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
    fn items_check(children: &[syn::Item], directory: &Path) -> CheckResult {
        items(children, directory)
    }
    items(&ast.items, &module_directory)
}
