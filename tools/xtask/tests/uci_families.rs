//! Versioned closed recipe declarations only, not device-operation evidence.
use std::{fs, path::PathBuf};
use xtask::Contract;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn validate(value: &toml::Value) -> Result<(), String> {
    Contract::parse(&toml::to_string(value).unwrap()).and_then(|c| c.validate(&root()))
}
fn initial() -> toml::Value {
    toml::Value::Array(
        [
            "system:system",
            "network:interface",
            "wireless:wifi-device",
            "firewall:defaults",
            "dhcp:dnsmasq",
            "fstab:mount",
        ]
        .into_iter()
        .map(Into::into)
        .collect(),
    )
}

#[test]
fn base_profiles_are_exact_complete_distinct_and_not_dynamic() {
    let original = declaration();
    validate(&original).unwrap();
    let expected = [
        "system:system",
        "network:interface",
        "wireless:wifi-device",
        "firewall:defaults",
        "dhcp:dnsmasq",
        "fstab:mount",
        "system:timeserver",
        "network:device",
        "network:bridge-vlan",
        "network:route",
        "network:route6",
        "network:rule",
        "network:rule6",
        "firewall:zone",
        "firewall:forwarding",
        "firewall:rule",
        "firewall:redirect",
        "firewall:nat",
        "dhcp:dhcp",
        "dhcp:host",
        "dhcp:domain",
        "dhcp:cname",
        "fstab:global",
        "fstab:swap",
    ];
    let profiles = original["uci_read_contract"]["profiles"]
        .as_array()
        .unwrap();
    assert_eq!(profiles.len(), expected.len());
    for name in expected {
        assert!(profiles.iter().any(|p| p.as_str() == Some(name)));
        for duplicate in [false, true] {
            let mut bad = original.clone();
            let list = bad["uci_read_contract"]["profiles"].as_array_mut().unwrap();
            if duplicate {
                list.push(name.into());
            } else {
                list.retain(|p| p.as_str() != Some(name));
            }
            assert!(validate(&bad).unwrap_err().contains("closed UCI profiles"));
        }
    }
    for name in [
        "network:*",
        "*:device",
        "network:{type}",
        "../network:route",
        "wireless:wifi-iface",
        "firewall:include",
        "dhcp:script",
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"]["profiles"].as_array_mut().unwrap()[6] = name.into();
        assert!(validate(&bad).unwrap_err().contains("closed UCI profiles"));
    }
}

#[test]
fn old_versions_keep_six_profiles_and_reject_base_expansion() {
    let original = declaration();
    let mut old = original.clone();
    old.as_table_mut().unwrap().remove("opkg_status_contract");
    old.as_table_mut()
        .unwrap()
        .remove("management_effect_contract");
    old.as_table_mut()
        .unwrap()
        .remove("backup_archive_contract");
    old.as_table_mut().unwrap().remove("gzip_archive_contract");
    old.as_table_mut()
        .unwrap()
        .remove("archive_sealing_contract");
    old.as_table_mut().unwrap().remove("windows_log_contract");
    for r in old["crates"].as_array_mut().unwrap() {
        if r["name"].as_str() == Some("openwrt-mcp") {
            r["dev_dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
        }
    }
    for rule in old["crates"].as_array_mut().unwrap() {
        if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
            rule["dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| !matches!(d.as_str(), Some("zeroize" | "flate2")));
        }
    }
    old["version"] = 12.into();
    assert!(
        validate(&old)
            .unwrap_err()
            .contains("requires architecture v13")
    );
    old["uci_read_contract"]["profiles"] = initial();
    validate(&old).unwrap();
    for name in original["uci_read_contract"]["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .skip(6)
    {
        let mut bad = old.clone();
        bad["uci_read_contract"]["profiles"]
            .as_array_mut()
            .unwrap()
            .push(name.clone());
        assert!(
            validate(&bad)
                .unwrap_err()
                .contains("requires architecture v13")
        );
    }
    old["version"] = 11.into();
    old["uci_read_contract"]
        .as_table_mut()
        .unwrap()
        .remove("option_collections");
    for field in [
        "text_options",
        "text_option_output",
        "text_option_budget",
        "max_text_option_items",
    ] {
        old["projection_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
    }
    old["projection_contract"]["profile"] = "typed_collections_v3".into();
    old["projection_contract"]["node_forms"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p.as_str() != Some("TextOption"));
    validate(&old).unwrap();
    old["uci_read_contract"]["profiles"] = original["uci_read_contract"]["profiles"].clone();
    assert!(
        validate(&old)
            .unwrap_err()
            .contains("requires architecture v13")
    );
}

#[test]
fn broader_profiles_do_not_relax_permissions_shapes_owners_or_shared_limits() {
    let original = declaration();
    for (field, value) in [
        ("requirement", "client_category"),
        ("actions", "get_any_config"),
        ("custom_uci", "read_allowed"),
        ("invocation", "feature_direct_io"),
        ("projection", "raw_values"),
        ("option_collections", "recursive"),
        ("view", "effective_state"),
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"][field] = value.into();
        assert!(validate(&bad).is_err());
    }
    for field in [
        "max_total_items",
        "max_emitted_items",
        "max_collection_nodes",
        "max_normalized_bytes",
    ] {
        let mut bad = original.clone();
        bad["projection_contract"][field] = 1000000.into();
        assert!(validate(&bad).is_err());
    }
    let mut bad = original;
    bad["uci_read_contract"]["required_tests"]
        .as_array_mut()
        .unwrap()
        .clear();
    assert!(validate(&bad).is_err());
}
