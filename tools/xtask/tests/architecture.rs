//! Adversarial fixtures for the architecture contract. These are deliberately
//! inline strings, never compiled or included into a production source tree.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use xtask::{
    Contract, CrateRule, check_metadata, check_source, check_source_with_aliases,
    validate_evolution,
};

fn rule() -> CrateRule {
    CrateRule {
        name: "pure".into(),
        path: "crates/pure".into(),
        layer: "domain".into(),
        dependencies: vec!["serde".into()],
        dev_dependencies: vec!["fixture".into()],
        build_dependencies: vec![],
        forbidden_paths: vec![
            "std::fs".into(),
            "std::process".into(),
            "tokio::process".into(),
        ],
        allowed_macros: vec!["json".into(), "format".into(), "vec".into()],
    }
}

fn contract() -> Contract {
    Contract {
        windows_read_contract: None,
        package_contract: None,
        version: 2,
        decision: "docs/adr/0002.md".into(),
        requirements: "docs/requirements.md".into(),
        architecture: "docs/architecture.md".into(),
        required_hosts: vec![],
        portable_crates: vec![],
        required_portable_tests: vec![],
        native_ci: String::new(),
        capability_contract: None,
        projection_contract: None,
        action_response_contract: None,
        mcp_result_contract: None,
        crates: vec![rule()],
    }
}

#[test]
fn direct_grouped_and_renamed_imports_cannot_hide_io() {
    for source in [
        "fn f() { std::fs::read(\"synthetic\"); }",
        "use std::{fs::{read as harmless}}; fn f() { harmless(\"synthetic\"); }",
        "use std as platform; fn f() { platform::fs::read(\"synthetic\"); }",
        "extern crate std as platform; fn f() { platform::fs::read(\"synthetic\"); }",
        "use std as a; use a as b; fn f() { b::fs::read(\"synthetic\"); }",
        "use tokio as async_runtime; fn f() { async_runtime::process::Command::new(\"synthetic\"); }",
        "use std::{self as platform}; fn f() { platform::fs::read(\"synthetic\"); }",
        "use std::*; fn f() {}",
        "use std::fs::*; fn f() {}",
        "fn f() { r#std::r#fs::read(\"synthetic\"); }",
        "use r#std::{r#fs as r#data}; fn f() { r#data::read(\"synthetic\"); }",
        "use r#std as r#platform; fn f() { r#platform::r#fs::read(\"synthetic\"); }",
        "mod platform { pub use std as os; } fn f() { platform::os::fs::read(\"synthetic\"); }",
    ] {
        assert!(
            check_source(&rule(), source, false).is_err(),
            "accepted forbidden source: {source}"
        );
    }
}

#[test]
fn nested_modules_inactive_cfg_and_macro_arguments_are_checked() {
    for source in [
        "mod deep { mod deeper { fn f() { std::fs::read(\"synthetic\"); } } }",
        "#[cfg(target_os = \"not_this_host\")] fn f() { std::fs::read(\"synthetic\"); }",
        "fn f() { json!({\"x\": std::fs::read(\"synthetic\")}); }",
        "use std as os; fn f() { format!(\"{:?}\", os::fs::read(\"synthetic\")); }",
        "fn f() { json!({\"x\": include_str!(\"outside\")}); }",
        "macro_rules! sneak { () => { std::fs::read(\"synthetic\") } }",
        "fn f() { include!(\"outside.rs\"); }",
        "fn f() { unreviewed!(); }",
        "fn f() { json!({ use std as hidden; hidden::fs::read(\"synthetic\") }); }",
        "fn f() { json!({\"x\": r#std::r#fs::read(\"synthetic\")}); }",
    ] {
        assert!(
            check_source(&rule(), source, false).is_err(),
            "accepted hidden source: {source}"
        );
    }
}

#[test]
fn cargo_renames_cannot_hide_forbidden_apis_of_allowed_dependencies() {
    let aliases = BTreeMap::from([("hidden".into(), "tokio".into())]);
    for source in [
        "fn f() { hidden::process::Command::new(\"synthetic\"); }",
        "use hidden as second; fn f() { second::process::Command::new(\"synthetic\"); }",
        "fn f() { json!({\"x\": hidden::process::Command::new(\"synthetic\")}); }",
    ] {
        assert!(check_source_with_aliases(&rule(), source, false, &aliases).is_err());
    }
}

#[test]
fn module_remapping_and_unreviewed_attributes_are_rejected() {
    for source in [
        "#[path = \"../../outside.rs\"] mod escaped;",
        "#[cfg_attr(unix, path = \"../../outside.rs\")] mod escaped;",
        "#[cfg_attr(unix, cfg_attr(windows, path = \"outside.rs\"))] mod escaped;",
        "#[unreviewed] fn f() {}",
        "#[derive(Unreviewed)] struct A;",
        "#[derive(Debug, thiserror::Error)] #[error(\"{}\", std::fs::read_to_string(\"synthetic\").unwrap())] struct Failure;",
        "#[derive(serde::Deserialize)] #[serde(default = \"std::process::abort\")] struct Failure;",
        "use std as hidden; #[derive(serde::Serialize)] struct A { #[serde(serialize_with = \"hidden::fs::read\")] value: String }",
        "#[derive(serde::Deserialize)] #[cfg_attr(unix, serde(default = \"std::process::abort\"))] struct Failure;",
        "unsafe extern \"C\" { fn foreign_device_call(); }",
        "fn f() { unsafe { foreign_device_call(); } }",
    ] {
        assert!(
            check_source(&rule(), source, false).is_err(),
            "accepted unreviewed item: {source}"
        );
    }
    assert!(check_source(&rule(), "#[path = \"outside.rs\"] mod escaped;", true).is_err());
}

#[test]
fn harmless_strings_and_explicit_fixture_io_do_not_trigger_false_positives() {
    for source in [
        "// std::fs::read is forbidden\n fn f() { let _ = \"std::process::Command\"; }",
        "use std::collections::{BTreeMap, BTreeSet}; fn f() { let _ = vec![1]; }",
        "#[cfg(test)] mod tests { use std::fs; #[test] fn fake() { fs::read(\"synthetic\"); } }",
        "mod backend { pub struct Type; } fn f() { let _ = backend::Type; } #[cfg(test)] mod tests { use std::fs as backend; }",
        "use serde_json::json as value; fn f() { value!({\"x\": 1}); }",
        "#[derive(Debug, thiserror::Error)] #[error(\"std::fs::read is forbidden\")] struct Failure;",
        "#[derive(serde::Serialize)] #[serde(rename = \"std::fs::read\")] struct Data;",
    ] {
        assert!(
            check_source(&rule(), source, false).is_ok(),
            "rejected harmless source: {source}"
        );
    }
    assert!(check_source(&rule(), "fn f() { std::fs::read(\"synthetic\"); }", true).is_ok());
}

fn metadata(dependencies: Value) -> Value {
    json!({"workspace_root":"/workspace", "workspace_members":["pure-id"], "packages":[{
        "id":"pure-id", "name":"pure", "manifest_path":"/workspace/crates/pure/Cargo.toml",
        "dependencies":dependencies, "targets":[{"kind":["lib"], "src_path":"/workspace/crates/pure/src/lib.rs"}]
    }]})
}

#[test]
fn original_package_names_and_every_dependency_kind_and_target_are_checked() {
    for kind in [Value::Null, json!("dev"), json!("build")] {
        let metadata = metadata(
            json!([{"name":"dangerous", "rename":"serde", "kind":kind, "target":"cfg(not_this_host)"}]),
        );
        assert!(check_metadata(&contract(), &metadata).is_err());
    }
    assert!(
        check_metadata(
            &contract(),
            &metadata(json!([{"name":"serde", "rename":"benign", "kind":null}]))
        )
        .is_ok()
    );
    assert!(
        check_metadata(
            &contract(),
            &metadata(json!([{"name":"fixture", "kind":"dev"}]))
        )
        .is_ok()
    );
    assert!(
        check_metadata(
            &contract(),
            &metadata(json!([{"name":"fixture", "kind":null}]))
        )
        .is_err()
    );
    assert!(
        check_metadata(
            &contract(),
            &metadata(json!([{"name":"serde", "kind":"build"}]))
        )
        .is_err()
    );
}

#[test]
fn unowned_packages_targets_path_dependencies_and_build_scripts_are_rejected() {
    let mut values = Vec::new();
    let mut unowned = metadata(json!([]));
    unowned["packages"][0]["name"] = json!("unowned");
    values.push(unowned);
    let mut escaped = metadata(json!([]));
    escaped["packages"][0]["targets"][0]["src_path"] = json!("/outside/source.rs");
    values.push(escaped);
    let mut build = metadata(json!([]));
    build["packages"][0]["targets"][0]["kind"] = json!(["custom-build"]);
    values.push(build);
    let mut wrong_directory = metadata(json!([]));
    wrong_directory["packages"][0]["manifest_path"] = json!("/workspace/other/Cargo.toml");
    values.push(wrong_directory);
    values.push(metadata(
        json!([{"name":"serde", "kind":null, "path":"/outside/serde"}]),
    ));
    for kind in ["lib", "bin"] {
        for directory in ["tests", "examples"] {
            let mut escaped = metadata(json!([]));
            escaped["packages"][0]["targets"][0]["kind"] = json!([kind]);
            escaped["packages"][0]["targets"][0]["src_path"] =
                json!(format!("/workspace/crates/pure/{directory}/escape.rs"));
            values.push(escaped);
        }
    }
    for value in values {
        assert!(check_metadata(&contract(), &value).is_err());
    }
}

#[test]
fn unowned_and_nonstandard_source_paths_are_rejected() {
    let contract = contract();
    for path in [
        "outside.rs",
        "crates/pure/build.rs",
        "crates/unknown/src/a.rs",
        "crates/pure/src/../../outside.rs",
    ] {
        assert!(contract.owner(path).is_err(), "accepted {path}");
    }
    assert!(!contract.owner("crates/pure/src/deep/module.rs").unwrap().1);
    assert!(contract.owner("crates/pure/tests/fixture.rs").unwrap().1);
}

fn spec(version: u64, decision: &str) -> String {
    format!(
        "version = {version}\ndecision = \"{decision}\"\nrequirements = \"docs/requirements.md\"\narchitecture = \"docs/architecture.md\"\ncrates = []\n"
    )
}

fn evidence() -> BTreeSet<String> {
    [
        "architecture/spec.toml",
        "docs/requirements.md",
        "docs/architecture.md",
        "docs/adr/0002.md",
        "tools/xtask/src/source.rs",
        "tools/xtask/tests/architecture.rs",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn contract_evolution_requires_version_new_adr_and_harness_regression() {
    let old = spec(1, "docs/adr/0001.md");
    let new = spec(2, "docs/adr/0002.md");
    let all = evidence();
    assert!(validate_evolution(Some(&old), &new, &all).is_ok());
    assert!(validate_evolution(None, &new, &all).is_ok());
    assert!(validate_evolution(Some(&old), &spec(1, "docs/adr/0002.md"), &all).is_err());
    assert!(validate_evolution(Some(&old), &spec(2, "docs/adr/0001.md"), &all).is_err());
    for required in &all {
        let mut missing = all.clone();
        missing.remove(required);
        assert!(
            validate_evolution(Some(&old), &new, &missing).is_err(),
            "accepted absent {required}"
        );
        assert!(
            validate_evolution(None, &new, &missing).is_err(),
            "accepted initial adoption without {required}"
        );
    }
    assert!(validate_evolution(Some(&new), &new, &BTreeSet::new()).is_ok());
}

#[test]
fn contract_unknown_fields_are_not_ignored() {
    let source = format!("{}\nskip_checks = true", spec(2, "docs/adr/0002.md"));
    assert!(Contract::parse(&source).is_err());
}
