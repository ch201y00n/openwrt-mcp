use std::{fs, path::Path};

#[test]
fn documented_example_policies_are_valid_and_non_mutating() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for example in [
        "read-only.toml",
        "deny-all.toml",
        "observability.toml",
        "protection-environment.toml",
        "ssh-environment.toml",
    ] {
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

#[test]
fn observability_example_exposes_builtins_without_privileged_extensions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config: openwrt_mcp::config::Config =
        toml::from_str(&fs::read_to_string(root.join("config/observability.toml")).unwrap())
            .unwrap();
    let catalog = config.catalog().unwrap();
    assert_eq!(catalog.operations().len(), 14);
    assert!(
        catalog
            .operations()
            .iter()
            .all(|operation| config.policy.authorize(operation).is_ok())
    );
    assert!(
        config
            .policy
            .categories
            .values()
            .all(|grant| !grant.execute)
    );
    assert!(
        !config
            .policy
            .categories
            .contains_key(&openwrt_mcp_core::Category::Extensions)
    );
}
