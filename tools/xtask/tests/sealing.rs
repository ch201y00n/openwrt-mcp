//! v18 declaration/source checks, not supplied-stream behavior or publication.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{Contract, check_native_ci, check_owned_source, check_sealing_source};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn contract(v: &toml::Value) -> Contract {
    Contract::parse(&toml::to_string(v).unwrap()).unwrap()
}
fn denied(v: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(v).unwrap())
            .and_then(|c| c.validate(&root()))
            .is_err()
    );
}

#[test]
fn sealing_requires_every_exact_field_bound_and_version() {
    let original = declaration();
    contract(&original).validate(&root()).unwrap();
    for (field, value) in original["archive_sealing_contract"].as_table().unwrap() {
        let mut v = original.clone();
        v["archive_sealing_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&v);
        let mut v = original.clone();
        v["archive_sealing_contract"][field] = match value {
            toml::Value::String(_) => "unsafe".into(),
            toml::Value::Integer(n) => (n + 1).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&v);
    }
    let mut v = original.clone();
    v["archive_sealing_contract"]
        .as_table_mut()
        .unwrap()
        .insert("retry_publish".into(), true.into());
    denied(&v);
    let mut v = original.clone();
    v.as_table_mut().unwrap().remove("archive_sealing_contract");
    denied(&v);
    v["version"] = 17.into();
    denied(&v); // New fixture edge is not retroactive.
    for r in v["crates"].as_array_mut().unwrap() {
        if r["name"].as_str() == Some("openwrt-mcp") {
            r["dev_dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
        }
    }
    contract(&v).validate(&root()).unwrap();
    v.as_table_mut().unwrap().insert(
        "archive_sealing_contract".into(),
        original["archive_sealing_contract"].clone(),
    );
    denied(&v);
}

#[test]
fn sealing_has_no_production_consumer_or_composition_dependency_escape() {
    let original = declaration();
    let c = contract(&original);
    for (index, r) in original["crates"].as_array().unwrap().iter().enumerate() {
        if r["name"].as_str() == Some("openwrt-mcp") {
            for kind in ["dependencies", "build_dependencies"] {
                let mut v = original.clone();
                v["crates"][index][kind]
                    .as_array_mut()
                    .unwrap()
                    .push("openwrt-mcp-device-codec".into());
                denied(&v);
            }
            let mut v = original.clone();
            v["crates"][index]["dev_dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
            denied(&v);
        }
        if !r["forbidden_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str() == Some("openwrt_mcp_runtime::sealing"))
        {
            continue;
        }
        let mut v = original.clone();
        v["crates"][index]["forbidden_paths"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some("openwrt_mcp_runtime::sealing"));
        denied(&v);
        let file = format!("{}/src/lib.rs", r["path"].as_str().unwrap());
        for code in [
            "use openwrt_mcp_runtime::sealing::Sealer;",
            "use openwrt_mcp_runtime as runtime; use runtime::sealing as hidden;",
        ] {
            assert!(check_owned_source(&c, &file, code, &BTreeMap::new()).is_err());
        }
    }
}

#[test]
fn sealing_source_cannot_open_handles_use_keys_or_acquire_sibling_authority() {
    let c = contract(&declaration());
    let file = "crates/runtime/src/sealing/mod.rs";
    check_sealing_source(
        &c,
        file,
        "use std::io::{Read,Write}; use crate::protection::EncryptionSession;",
        &BTreeMap::new(),
    )
    .unwrap();
    for code in [
        "use std::fs::File;",
        "use std::io::stdin;",
        "use std::io::copy;",
        "use serde::Serialize;",
        "use serde_json as json;",
        "use tokio::task;",
        "use std::thread;",
        "use std::path::Path;",
        "use crate::protection::IdentityMaterial;",
        "use crate::protection::KeyMaterial;",
        "use crate::protection::KeySource;",
        "use crate::protection::DecryptionSession;",
        "use super::super::dispatcher::Dispatcher;",
        "fn f(input:R) { input.read_to_end(); }",
        "fn f(input:R) { input.r#read_to_string(); }",
        "fn f(x:T) { let _=matches!(x,crate::Policy::Allowed); }",
        "fn f() { let _=Vec::<u8>::new(); }",
    ] {
        assert!(
            check_sealing_source(&c, file, code, &BTreeMap::new()).is_err(),
            "accepted {code}"
        );
    }
    let aliases = [("hidden".into(), "serde".into())].into();
    assert!(check_sealing_source(&c, file, "use hidden::Serialize;", &aliases).is_err());
    for file in [
        "crates/runtime/src/lib.rs",
        "crates/runtime/src/dispatcher.rs",
    ] {
        check_sealing_source(&c, file, "pub mod sealing;", &BTreeMap::new()).unwrap();
        for code in [
            "use super::sealing::Sealer;",
            "use r#sealing as hidden;",
            "fn f(x:T) { let _=matches!(x,crate::sealing::State::Done); }",
        ] {
            assert!(check_sealing_source(&c, file, code, &BTreeMap::new()).is_err());
        }
    }
    assert!(
        check_sealing_source(
            &c,
            "crates/runtime/src/sealing.rs",
            "pub struct S;",
            &BTreeMap::new()
        )
        .is_err()
    );
    let mut old = c;
    old.version = 17;
    assert!(check_sealing_source(&old, file, "pub struct S;", &BTreeMap::new()).is_err());
}

#[test]
fn both_sealing_suites_are_mandatory_on_every_native_host() {
    let original = declaration();
    let c = contract(&original);
    for suite in [
        "crates/runtime/tests/protection.rs",
        "crates/server/tests/protection_composition.rs",
    ] {
        let mut v = original.clone();
        v["required_portable_tests"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some(suite));
        denied(&v);
    }
    let original: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap())
            .unwrap();
    check_native_ci(&c, &original.to_string()).unwrap();
    for command in [
        "cargo test --locked -p openwrt-mcp-runtime --test protection",
        "cargo test --locked -p openwrt-mcp --test protection_composition",
    ] {
        let mut ci = original.clone();
        ci["jobs"]["rust"]["steps"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s["run"].as_str() != Some(command));
        assert!(check_native_ci(&c, &ci.to_string()).is_err());
    }
}
