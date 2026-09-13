//! Architecture-only negative declarations; not implemented topology protection.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{
    Contract, check_management_source, check_native_ci, check_owned_source, check_public_reexports,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn parse(value: &toml::Value) -> Contract {
    Contract::parse(&toml::to_string(value).unwrap()).unwrap()
}
fn denied(value: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(value).unwrap())
            .and_then(|contract| contract.validate(&root()))
            .is_err()
    );
}

#[test]
fn every_effect_field_and_budget_requires_exact_versioned_non_authorizing_contract() {
    let original = declaration();
    parse(&original).validate(&root()).unwrap();
    let mut old = original.clone();
    old["version"] = 14.into();
    old.as_table_mut()
        .unwrap()
        .remove("backup_archive_contract");
    for rule in old["crates"].as_array_mut().unwrap() {
        if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
            rule["dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("zeroize"));
        }
    }
    denied(&old);
    old.as_table_mut()
        .unwrap()
        .remove("management_effect_contract");
    parse(&old).validate(&root()).unwrap();
    old["version"] = 15.into();
    denied(&old);
    for (field, value) in original["management_effect_contract"].as_table().unwrap() {
        let mut missing = original.clone();
        missing["management_effect_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&missing);
        let mut changed = original.clone();
        changed["management_effect_contract"][field] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(number) => (number + 1).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&changed);
    }
    let mut extra = original;
    extra["management_effect_contract"]
        .as_table_mut()
        .unwrap()
        .insert("allow_unknown".into(), true.into());
    denied(&extra);
}

#[test]
fn effect_model_cannot_acquire_a_production_consumer_or_flatten_its_namespace() {
    let original = declaration();
    for (index, rule) in original["crates"].as_array().unwrap().iter().enumerate() {
        if !rule["forbidden_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str() == Some("openwrt_mcp_core::management"))
        {
            continue;
        }
        let mut changed = original.clone();
        changed["crates"][index]["forbidden_paths"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some("openwrt_mcp_core::management"));
        denied(&changed);
        let contract = parse(&original);
        let file = format!("{}/src/lib.rs", rule["path"].as_str().unwrap());
        for code in [
            "use openwrt_mcp_core::management::EffectGraph;",
            "use openwrt_mcp_core as domain; use domain::management as graph;",
        ] {
            assert!(check_owned_source(&contract, &file, code, &BTreeMap::new()).is_err());
        }
    }
    for code in [
        "pub use management::EffectGraph;",
        "use crate::management as hidden; pub use hidden::*;",
        "pub type Escaped = crate::management::EffectGraph;",
    ] {
        assert!(
            check_public_reexports(code, &["management".into()].into(), "openwrt_mcp_core::")
                .is_err()
        );
    }
}

#[test]
fn management_sources_are_pure_non_serializable_and_have_no_action_policy_api() {
    let contract = parse(&declaration());
    let file = "crates/core/src/management/mod.rs";
    let pure = "use std::collections::BTreeSet; pub fn bounded() { let _: BTreeSet<usize> = BTreeSet::new(); }";
    check_management_source(&contract, file, pure, &BTreeMap::new()).unwrap();
    for code in [
        "use serde::{Serialize as Wire}; #[derive(Wire)] struct Graph;",
        "fn f() { serde_json::from_str::<usize>(\"0\"); }",
        "fn f() { let _ = json!({}); }",
        "fn f() { std::fs::read(\"synthetic\"); }",
        "fn f() -> crate::Policy { unreachable!() }",
        "fn f() -> super::super::PreparedAction { unreachable!() }",
        "fn f() { crate::policy::helper(); }",
        "use crate::Policy as Rules; fn f() -> Rules { unreachable!() }",
        "fn f(value:usize) { let _ = matches!(value, crate::Policy::Allowed); }",
    ] {
        assert!(
            check_management_source(&contract, file, code, &BTreeMap::new()).is_err(),
            "accepted {code}"
        );
    }
    assert!(
        check_management_source(
            &contract,
            "crates/core/src/management.rs",
            pure,
            &BTreeMap::new()
        )
        .is_err()
    );
    let mut old = contract.clone();
    old.version = 14;
    old.management_effect_contract = None;
    assert!(check_management_source(&old, file, pure, &BTreeMap::new()).is_err());
    check_management_source(
        &contract,
        "crates/core/src/packages.rs",
        "use serde::Serialize;",
        &BTreeMap::new(),
    )
    .unwrap();
    check_management_source(
        &contract,
        "crates/core/tests/management.rs",
        "use std::fs;",
        &BTreeMap::new(),
    )
    .unwrap();
}

#[test]
fn existing_core_security_suite_must_run_explicitly_on_every_native_host() {
    let original = declaration();
    let mut missing = original.clone();
    missing["required_portable_tests"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s.as_str() != Some("crates/core/tests/security.rs"));
    denied(&missing);
    let contract = parse(&original);
    let mut workflow: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap())
            .unwrap();
    check_native_ci(&contract, &workflow.to_string()).unwrap();
    workflow["jobs"]["rust"]["steps"]
        .as_array_mut()
        .unwrap()
        .retain(|step| {
            step["run"].as_str() != Some("cargo test --locked -p openwrt-mcp-core --test security")
        });
    assert!(check_native_ci(&contract, &workflow.to_string()).is_err());
}
