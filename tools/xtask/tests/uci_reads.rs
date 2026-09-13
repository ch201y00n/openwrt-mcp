//! Architecture-only UCI recipe declarations, not device or behavior evidence.
use std::{fs, path::PathBuf};
use xtask::{Contract, check_capability_registry_for_version, check_native_ci};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn denied(value: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(value).unwrap())
            .and_then(|contract| contract.validate(&root()))
            .is_err(),
        "accepted unsafe UCI declaration"
    );
}

#[test]
fn uci_contract_requires_every_closed_field_and_rejects_unknown_or_earlier_version() {
    let original = declaration();
    Contract::parse(&toml::to_string(&original).unwrap())
        .unwrap()
        .validate(&root())
        .unwrap();
    for field in original["uci_read_contract"].as_table().unwrap().keys() {
        let mut bad = original.clone();
        bad["uci_read_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&bad);
    }
    let mut bad = original.clone();
    bad.as_table_mut().unwrap().remove("uci_read_contract");
    denied(&bad);
    let mut bad = original.clone();
    bad["uci_read_contract"]
        .as_table_mut()
        .unwrap()
        .insert("escape".into(), true.into());
    denied(&bad);
    let mut bad = original;
    bad["version"] = 10.into();
    denied(&bad);
}

#[test]
fn uci_profiles_cannot_gain_custom_calls_setters_paths_or_false_state_guarantees() {
    let original = declaration();
    for (field, value) in [
        ("domain_owner", "openwrt-mcp-transport"),
        ("definitions_owner", "openwrt-mcp-runtime"),
        ("invocation", "direct_backend"),
        ("actions", "any_uci_method"),
        ("actions", "client_config_type"),
        ("custom_uci", "privileged_allowed"),
        ("requirement", "extensions_only"),
        ("projection", "structured_raw"),
        ("projection", "typed_without_guard"),
        ("view", "committed_only"),
        ("view", "effective_state"),
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"][field] = value.into();
        denied(&bad);
    }
    for profile in [
        "network:*",
        "*:interface",
        "dhcp:host",
        "system:system",
        "system:set",
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"]["profiles"]
            .as_array_mut()
            .unwrap()
            .push(profile.into());
        denied(&bad);
    }
    for remove in [true, false] {
        let mut bad = original.clone();
        let profiles = bad["uci_read_contract"]["profiles"].as_array_mut().unwrap();
        if remove {
            profiles.pop();
        } else {
            profiles[0] = "system:other".into();
        }
        denied(&bad);
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let contract = Contract::parse(&toml::to_string(&original).unwrap()).unwrap();
    for (suite, command) in [
        (
            "crates/core/tests/capabilities.rs",
            "cargo test --locked -p openwrt-mcp-core --test capabilities",
        ),
        (
            "crates/mcp/tests/read_contracts.rs",
            "cargo test --locked -p openwrt-mcp-transport --test read_contracts",
        ),
    ] {
        let mut bad = original.clone();
        bad["required_portable_tests"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p.as_str() != Some(suite));
        denied(&bad);
        assert!(
            check_native_ci(
                &contract,
                &workflow.replace(command, "echo missing_required_suite")
            )
            .is_err()
        );
    }
}

#[test]
fn text_enums_require_exact_finite_values_without_projection_downgrades() {
    let original = declaration();
    for value in ["coerce", "case_insensitive", "regex", "any_string"] {
        let mut bad = original.clone();
        bad["projection_contract"]["text_enums"] = value.into();
        denied(&bad);
    }
    for max in [0, 15, 17, 1024] {
        let mut bad = original.clone();
        bad["projection_contract"]["max_text_enum_values"] = max.into();
        denied(&bad);
    }
    for profile in [
        "typed_collections_v1",
        "typed_collections_v2",
        "generic_union",
    ] {
        let mut bad = original.clone();
        bad["projection_contract"]["profile"] = profile.into();
        denied(&bad);
    }
    // Downgrade rejection is exercised by the assembled v10 fixture with its
    // own old registry, independent of this checkout's production migration.
}

#[test]
fn uci_probe_requires_v11_and_only_one_additional_exact_description() {
    let mut registry: toml::Value = toml::from_str(
        &fs::read_to_string(root().join("architecture/capability-probes.toml")).unwrap(),
    )
    .unwrap();
    registry["schema_version"] = 3.into();
    registry["probes"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["object"].as_str() != Some("uci"));
    let probe:toml::Value=toml::from_str("id='ubus.uci'\nobject='uci'\nprogram='/bin/ubus'\narguments=['-v','list','uci']\neffect='read'\n").unwrap();
    registry["probes"].as_array_mut().unwrap().push(probe);
    let source = toml::to_string(&registry).unwrap();
    check_capability_registry_for_version(&source, 11).unwrap();
    assert!(check_capability_registry_for_version(&source, 10).is_err());
    for object in ["uci.*", "file", "session", "{object}"] {
        let mut bad = registry.clone();
        bad["probes"].as_array_mut().unwrap().last_mut().unwrap()["object"] = object.into();
        assert!(
            check_capability_registry_for_version(&toml::to_string(&bad).unwrap(), 11).is_err()
        );
    }
    for arguments in [
        vec!["call", "uci", "get", "{}"],
        vec!["call", "uci", "set", "{}"],
        vec!["-v", "list", "*"],
    ] {
        let mut bad = registry.clone();
        bad["probes"].as_array_mut().unwrap().last_mut().unwrap()["arguments"] =
            toml::Value::Array(arguments.into_iter().map(Into::into).collect());
        assert!(
            check_capability_registry_for_version(&toml::to_string(&bad).unwrap(), 11).is_err()
        );
    }
    registry["probes"].as_array_mut().unwrap().pop();
    assert!(
        check_capability_registry_for_version(&toml::to_string(&registry).unwrap(), 11).is_err()
    );
}
