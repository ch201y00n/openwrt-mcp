//! Architecture rejection tests; no gzip implementation or backup authority.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{Contract, check_gzip_dependency, check_gzip_source, check_owned_source};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

mod profiles;
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
fn gzip_requires_exact_profile_bounds_and_v17() {
    let original = declaration();
    contract(&original).validate(&root()).unwrap();
    for (field, value) in original["gzip_archive_contract"].as_table().unwrap() {
        let mut v = original.clone();
        v["gzip_archive_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&v);
        let mut v = original.clone();
        v["gzip_archive_contract"][field] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(n) => (n + 1).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&v);
    }
    let mut v = original.clone();
    v["gzip_archive_contract"]
        .as_table_mut()
        .unwrap()
        .insert("fallback".into(), true.into());
    denied(&v);
    let mut v = original.clone();
    v.as_table_mut().unwrap().remove("gzip_archive_contract");
    v.as_table_mut().unwrap().remove("archive_sealing_contract");
    v.as_table_mut().unwrap().remove("windows_log_contract");
    for r in v["crates"].as_array_mut().unwrap() {
        if r["name"].as_str() == Some("openwrt-mcp") {
            r["dev_dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
        }
    }
    denied(&v);
    let mut v = original.clone();
    v["version"] = 16.into();
    profiles::before_v20(&mut v);
    denied(&v);
    v.as_table_mut().unwrap().remove("gzip_archive_contract");
    v.as_table_mut().unwrap().remove("archive_sealing_contract");
    v.as_table_mut().unwrap().remove("windows_log_contract");
    for r in v["crates"].as_array_mut().unwrap() {
        if r["name"].as_str() == Some("openwrt-mcp") {
            r["dev_dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
        }
    }
    v["backup_archive_contract"]["consumers"] = "none_until_separate_integration_checkpoint".into();
    for r in v["crates"].as_array_mut().unwrap() {
        if r["name"].as_str() == Some("openwrt-mcp-device-codec") {
            r["dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("flate2"));
        }
    }
    contract(&v).validate(&root()).unwrap();
    v.as_table_mut().unwrap().insert(
        "gzip_archive_contract".into(),
        original["gzip_archive_contract"].clone(),
    );
    denied(&v);
}

#[test]
fn gzip_cannot_gain_external_consumers_or_lose_native_suite() {
    let original = declaration();
    let c = contract(&original);
    for (index, r) in original["crates"].as_array().unwrap().iter().enumerate() {
        if !r["forbidden_paths"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str() == Some("openwrt_mcp_device_codec::gzip"))
        {
            continue;
        }
        let mut v = original.clone();
        v["crates"][index]["forbidden_paths"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some("openwrt_mcp_device_codec::gzip"));
        denied(&v);
        let file = format!("{}/src/lib.rs", r["path"].as_str().unwrap());
        for source in [
            "use openwrt_mcp_device_codec::gzip::Validator;",
            "use openwrt_mcp_device_codec as c; use c::gzip as hidden;",
        ] {
            assert!(check_owned_source(&c, &file, source, &BTreeMap::new()).is_err());
        }
    }
    let mut v = original;
    v["required_portable_tests"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p.as_str() != Some("crates/device-codec/tests/action_response.rs"));
    denied(&v);
}

#[test]
fn only_pinned_normal_registry_rust_backend_is_admitted() {
    use serde_json::json;
    let d = json!({"req":"=1.1.10","uses_default_features":false,"features":["zlib-rs"],"optional":false,"kind":null,"target":null,"source":"registry+https://github.com/rust-lang/crates.io-index"});
    let owner = "openwrt-mcp-device-codec";
    check_gzip_dependency(owner, &d).unwrap();
    assert!(check_gzip_dependency("openwrt-mcp-core", &d).is_err());
    for (field, value) in [
        ("req", json!("^1.1.10")),
        ("uses_default_features", json!(true)),
        ("optional", json!(true)),
        ("kind", json!("dev")),
        ("kind", json!("build")),
        ("target", json!("cfg(unix)")),
        ("path", json!("/unowned")),
        ("source", json!("git+unreviewed")),
        ("features", json!([])),
        ("features", json!(["zlib-rs", null])),
        ("features", json!(["zlib-rs", "rust_backend"])),
        ("features", json!(["zlib-rs", "zlib-rs"])),
    ] {
        let mut changed = d.clone();
        changed[field] = value;
        assert!(
            check_gzip_dependency(owner, &changed).is_err(),
            "accepted {field}"
        );
    }
}

#[test]
fn source_blocks_io_decoder_escape_and_relative_sibling_consumers() {
    let c = contract(&declaration());
    let file = "crates/device-codec/src/gzip/mod.rs";
    let pure =
        "use flate2::{Decompress, FlushDecompress, Status}; use crate::archive::ArchiveValidator;";
    check_gzip_source(&c, file, pure, &BTreeMap::new()).unwrap();
    for source in [
        "use std::io::Read;",
        "use std::path::Path;",
        "use serde::Serialize;",
        "use openwrt_mcp_core::Action;",
        "use flate2::read::GzDecoder;",
        "use flate2::Compress;",
        "fn f(d: D) { d.reset(false); }",
        "fn f(d: D) { d.r#reset(false); }",
        "fn f(d: D) { d.decompress_vec(); }",
        "fn f(d: D) { let _ = matches!(d, crate::CommandSpec::Allowed); }",
        "use super::super::command as hidden;",
        "fn f(d:D) { d.decompress_uninit(); }",
        "fn f(d:D) { d.set_dictionary(); }",
    ] {
        assert!(
            check_gzip_source(&c, file, source, &BTreeMap::new()).is_err(),
            "accepted {source}"
        );
    }
    let aliases = [("hidden".into(), "flate2".into())].into();
    assert!(check_gzip_source(&c, file, "use hidden::write::GzDecoder;", &aliases).is_err());
    for file in [
        "crates/device-codec/src/lib.rs",
        "crates/device-codec/src/response/mod.rs",
    ] {
        check_gzip_source(&c, file, "pub mod archive; pub mod gzip;", &BTreeMap::new()).unwrap();
        for source in [
            "use super::archive::ArchiveValidator;",
            "use r#archive as hidden;",
            "use crate::gzip as hidden;",
            "fn f(x: T) { let _ = matches!(x, super::gzip::S::Done); }",
            "use flate2::Decompress;",
        ] {
            assert!(
                check_gzip_source(&c, file, source, &BTreeMap::new()).is_err(),
                "accepted {source}"
            );
        }
    }
    assert!(
        check_gzip_source(
            &c,
            "crates/device-codec/src/archive/mod.rs",
            "use super::gzip::Validator;",
            &BTreeMap::new()
        )
        .is_err()
    );
    assert!(
        check_gzip_source(
            &c,
            "crates/device-codec/src/gzip.rs",
            pure,
            &BTreeMap::new()
        )
        .is_err()
    );
    let mut old = c;
    old.version = 16;
    assert!(check_gzip_source(&old, file, pure, &BTreeMap::new()).is_err());
}
