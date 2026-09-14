use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{Contract, check_ciphertext_dependency, check_ciphertext_source, check_owned_source};
mod profiles;
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn parse(v: &toml::Value) -> Contract {
    Contract::parse(&toml::to_string(v).unwrap()).unwrap()
}
#[test]
fn exact_foundation_contract_and_historical_boundary() {
    let original = declaration();
    parse(&original).validate(&root()).unwrap();
    for (key, _) in original["ciphertext_foundation_contract"]
        .as_table()
        .unwrap()
    {
        let mut bad = original.clone();
        bad["ciphertext_foundation_contract"]
            .as_table_mut()
            .unwrap()
            .remove(key);
        assert!(parse(&bad).validate(&root()).is_err());
        let mut bad = original.clone();
        bad["ciphertext_foundation_contract"][key] = "unreviewed".into();
        assert!(parse(&bad).validate(&root()).is_err());
    }
    let mut old = original.clone();
    old["version"] = 21.into();
    assert!(parse(&old).validate(&root()).is_err());
    profiles::before_v22(&mut old);
    parse(&old).validate(&root()).unwrap();
    old["version"] = 20.into();
    profiles::before_v21(&mut old);
    parse(&old).validate(&root()).unwrap();
    old["version"] = 19.into();
    profiles::before_v20(&mut old);
    parse(&old).validate(&root()).unwrap();
    let mut bad = original;
    bad["ciphertext_foundation_contract"]
        .as_table_mut()
        .unwrap()
        .insert("unsafe_fallback".into(), "yes".into());
    assert!(parse(&bad).validate(&root()).is_err());
}
#[test]
fn scoped_namespaces_cannot_escape_via_siblings_aliases_or_macros() {
    let c = parse(&declaration());
    for (file, code) in [
        (
            "crates/mcp/src/handler.rs",
            "use openwrt_mcp_runtime::backups::CapturedArchive;",
        ),
        (
            "crates/adapters/src/sibling.rs",
            "use openwrt_mcp_host_platform::ciphertext::LockedFile;",
        ),
        ("crates/crypto-age/src/sibling.rs", "use hmac::Hmac;"),
        (
            "crates/key-sources/src/secrets/mod.rs",
            "use openwrt_mcp_runtime::backups::RecordAuthenticator;",
        ),
    ] {
        assert!(check_owned_source(&c, file, code, &BTreeMap::new()).is_err());
    }
    check_owned_source(
        &c,
        "crates/adapters/src/backups/mod.rs",
        "use openwrt_mcp_runtime::backups::CapturedArchive;",
        &BTreeMap::new(),
    )
    .unwrap();
    let aliases = BTreeMap::from([("renamed".into(), "openwrt_mcp_runtime".into())]);
    assert!(
        check_owned_source(
            &c,
            "crates/mcp/src/handler.rs",
            "format!(\"{}\", renamed::backups::Secret);",
            &aliases
        )
        .is_err()
    );
}
#[test]
fn private_types_and_native_store_cannot_gain_authority_or_namespace_writes() {
    let c = parse(&declaration());
    for code in [
        "#[derive(serde::Serialize)] struct Private;",
        "use crate::mutation_ports::Admission;",
        "fn x() { format!(\"{}\", serde_json::Value::Null); }",
    ] {
        assert!(check_ciphertext_source(&c, "crates/runtime/src/backups/mod.rs", code).is_err());
    }
    for call in [
        "create_new",
        "truncate",
        "rename",
        "set_len",
        "remove_file",
        "set_permissions",
    ] {
        assert!(
            check_ciphertext_source(
                &c,
                "crates/host-platform/src/ciphertext/mod.rs",
                &format!("fn x() {{ std::fs::{call}(\"inert\"); }}")
            )
            .is_err()
        );
    }
}
#[test]
fn authentication_dependencies_remain_exact() {
    let c = parse(&declaration());
    let dependency = serde_json::json!({"name":"hmac", "req":"=0.12.1", "kind":null, "target":null,"features":[],"uses_default_features":false});
    check_ciphertext_dependency(&c, "openwrt-mcp-crypto-age", &dependency).unwrap();
    for owner in [
        "openwrt-mcp-runtime",
        "openwrt-mcp-adapters",
        "openwrt-mcp-host-platform",
    ] {
        assert!(check_ciphertext_dependency(&c, owner, &dependency).is_err());
    }
    for (key, value) in [
        ("req", serde_json::json!("*")),
        ("features", serde_json::json!(["std"])),
        ("kind", serde_json::json!("build")),
        ("uses_default_features", serde_json::json!(true)),
    ] {
        let mut bad = dependency.clone();
        bad[key] = value;
        assert!(check_ciphertext_dependency(&c, "openwrt-mcp-crypto-age", &bad).is_err());
    }
}
