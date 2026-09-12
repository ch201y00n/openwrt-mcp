use std::{collections::BTreeSet, fs, path::Path};

#[test]
fn dependency_graph_keeps_policy_pure_and_mcp_out_of_runtime() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("crates/core/Cargo.toml")).unwrap()).unwrap();
    let permitted: BTreeSet<_> = ["serde", "serde_json", "thiserror", "toml"]
        .into_iter()
        .collect();
    for name in core["dependencies"].as_table().unwrap().keys() {
        assert!(
            permitted.contains(name.as_str()),
            "core has forbidden dependency: {name}"
        );
    }
    for entry in fs::read_dir(root.join("crates/core/src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(path).unwrap();
            for forbidden in [
                "fs::",
                "process::",
                "net::",
                "io::",
                "thread::",
                "include_str!",
                "include_bytes!",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "core must not perform I/O: {forbidden}"
                );
            }
        }
    }
    let runtime: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("crates/runtime/Cargo.toml")).unwrap())
            .unwrap();
    for name in runtime["dependencies"].as_table().unwrap().keys() {
        assert!(
            !matches!(name.as_str(), "rmcp" | "openwrt-mcp"),
            "runtime depends on transport: {name}"
        );
    }
    let protocol = fs::read_to_string(root.join("crates/server/src/protocol.rs")).unwrap();
    for forbidden in [
        "std::process",
        "tokio::process",
        "std::fs",
        "tokio::fs",
        "LocalBackend",
        "Command::",
    ] {
        assert!(
            !protocol.contains(forbidden),
            "protocol bypasses runtime: {forbidden}"
        );
    }
}

#[test]
fn documented_example_policies_are_valid_and_non_mutating() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for example in ["read-only.toml", "deny-all.toml"] {
        // Parse fixtures directly; do not weaken live config permission checks for WSL mounts.
        let config: openwrt_mcp::config::Config =
            toml::from_str(&fs::read_to_string(root.join("config").join(example)).unwrap())
                .unwrap();
        let catalog = config.catalog().unwrap();
        for operation in catalog
            .operations()
            .iter()
            .filter(|op| config.policy.authorize(op).is_ok())
        {
            assert!(
                operation
                    .requirements
                    .iter()
                    .all(|r| r.permission == openwrt_mcp_core::Permission::Read)
            );
        }
    }
}
