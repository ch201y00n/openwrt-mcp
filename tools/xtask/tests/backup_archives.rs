//! Declaration/source rejection, not archive behavior or backup acceptance.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{
    Contract, check_archive_source, check_native_ci, check_owned_source, check_public_reexports,
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
            .and_then(|c| c.validate(&root()))
            .is_err()
    );
}

#[test]
fn archive_profile_requires_every_exact_field_bound_and_version() {
    let original = declaration();
    parse(&original).validate(&root()).unwrap();
    let mut old = original.clone();
    old["version"] = 15.into();
    denied(&old);
    old.as_table_mut()
        .unwrap()
        .remove("backup_archive_contract");
    old.as_table_mut().unwrap().remove("gzip_archive_contract");
    for rule in old["crates"].as_array_mut().unwrap() {
        if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
            rule["dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| !matches!(d.as_str(), Some("zeroize" | "flate2")));
        }
    }
    parse(&old).validate(&root()).unwrap();
    old["version"] = 16.into();
    denied(&old);
    for (field, value) in original["backup_archive_contract"].as_table().unwrap() {
        let mut missing = original.clone();
        missing["backup_archive_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&missing);
        let mut changed = original.clone();
        changed["backup_archive_contract"][field] = match value {
            toml::Value::String(_) => "permissive".into(),
            toml::Value::Integer(n) => (n + 1).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&changed);
    }
    let mut extra = original;
    extra["backup_archive_contract"]
        .as_table_mut()
        .unwrap()
        .insert("allow_links".into(), true.into());
    denied(&extra);
}

#[test]
fn archive_cannot_gain_a_production_consumer_root_alias_or_unreviewed_dependency() {
    let original = declaration();
    let contract = parse(&original);
    for (index, rule) in original["crates"].as_array().unwrap().iter().enumerate() {
        if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
            for dependency in ["zeroize", "tokio"] {
                let mut changed = original.clone();
                let list = changed["crates"][index]["dependencies"]
                    .as_array_mut()
                    .unwrap();
                if dependency == "zeroize" {
                    list.retain(|d| d.as_str() != Some(dependency));
                } else {
                    list.push(dependency.into());
                }
                denied(&changed);
            }
        }
        if !rule["forbidden_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str() == Some("openwrt_mcp_device_codec::archive"))
        {
            continue;
        }
        let mut changed = original.clone();
        changed["crates"][index]["forbidden_paths"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some("openwrt_mcp_device_codec::archive"));
        denied(&changed);
        let file = format!("{}/src/lib.rs", rule["path"].as_str().unwrap());
        for code in [
            "use openwrt_mcp_device_codec::archive::Validator;",
            "use openwrt_mcp_device_codec as wire; use wire::archive as hidden;",
        ] {
            assert!(check_owned_source(&contract, &file, code, &BTreeMap::new()).is_err());
        }
    }
    for code in [
        "pub use archive::Validator;",
        "use crate::archive as hidden; pub use hidden::*;",
        "pub type Escaped = crate::archive::Validator;",
    ] {
        assert!(
            check_public_reexports(
                code,
                &["archive".into()].into(),
                "openwrt_mcp_device_codec::"
            )
            .is_err()
        );
    }
}

#[test]
fn archive_source_cannot_acquire_io_path_json_core_or_sibling_action_authority() {
    let contract = parse(&declaration());
    let file = "crates/device-codec/src/archive/mod.rs";
    let pure = "use zeroize::Zeroizing; fn f() { let _ = Zeroizing::new([0_u8;512]); }";
    check_archive_source(&contract, file, pure, &BTreeMap::new()).unwrap();
    for code in [
        "use std::io::Read;",
        "use std::path::Path;",
        "use std::fs::File;",
        "use serde::{Serialize as Wire}; #[derive(Wire)] struct Manifest;",
        "fn f() { let _ = json!({}); }",
        "use openwrt_mcp_core as domain;",
        "fn f() { crate::compile_action(); }",
        "use super::super::command as hidden;",
        "pub type Escape = crate::CommandSpec;",
        "fn f(value:usize) { let _ = matches!(value, crate::CommandSpec::Allowed); }",
    ] {
        assert!(
            check_archive_source(&contract, file, code, &BTreeMap::new()).is_err(),
            "accepted {code}"
        );
    }
    let aliases = [("hidden_wire".into(), "serde".into())].into();
    assert!(
        check_archive_source(&contract, file, "use hidden_wire::Serialize;", &aliases).is_err()
    );
    assert!(
        check_archive_source(
            &contract,
            "crates/device-codec/src/archive.rs",
            pure,
            &BTreeMap::new()
        )
        .is_err()
    );
    let mut old = contract.clone();
    old.version = 15;
    old.backup_archive_contract = None;
    assert!(check_archive_source(&old, file, pure, &BTreeMap::new()).is_err());
    check_archive_source(
        &contract,
        "crates/device-codec/tests/archive.rs",
        "use std::fs;",
        &BTreeMap::new(),
    )
    .unwrap();
}

#[test]
fn archive_keeps_required_native_suite_and_no_platform_skip() {
    let original = declaration();
    let mut missing = original.clone();
    missing["required_portable_tests"]
        .as_array_mut()
        .unwrap()
        .retain(|s| s.as_str() != Some("crates/device-codec/tests/action_response.rs"));
    denied(&missing);
    let contract = parse(&original);
    let mut ci: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap())
            .unwrap();
    check_native_ci(&contract, &ci.to_string()).unwrap();
    ci["jobs"]["rust"]["steps"]
        .as_array_mut()
        .unwrap()
        .retain(|step| {
            step["run"].as_str()
                != Some("cargo test --locked -p openwrt-mcp-device-codec --test action_response")
        });
    assert!(check_native_ci(&contract, &ci.to_string()).is_err());
}
