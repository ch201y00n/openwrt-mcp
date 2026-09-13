//! Architecture-only finite option declarations; not behavior/device evidence.
use std::{fs, path::PathBuf};
use xtask::Contract;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn denied(value: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(value).unwrap())
            .and_then(|c| c.validate(&root()))
            .is_err()
    );
}

#[test]
fn text_options_require_every_closed_field_and_no_unknown_coercion_metadata() {
    let original = declaration();
    Contract::parse(&toml::to_string(&original).unwrap())
        .unwrap()
        .validate(&root())
        .unwrap();
    for field in [
        "text_options",
        "text_option_output",
        "text_option_budget",
        "max_text_option_items",
    ] {
        let mut bad = original.clone();
        bad["projection_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&bad);
    }
    let mut bad = original;
    bad["projection_contract"]
        .as_table_mut()
        .unwrap()
        .insert("split".into(), true.into());
    denied(&bad);
}

#[test]
fn nested_options_cannot_become_root_raw_coercing_selectable_or_unbounded_forms() {
    let original = declaration();
    for (field, values) in [
        (
            "text_options",
            vec![
                "any_json",
                "root_allowed",
                "scalar_or_any_list",
                "select_first",
                "recursive",
            ],
        ),
        (
            "text_option_output",
            vec![
                "raw",
                "joined_string",
                "split_string",
                "list_only",
                "caller_names",
            ],
        ),
        (
            "text_option_budget",
            vec!["per_field", "emitted_only", "bytes_after_clone"],
        ),
        (
            "profile",
            vec!["typed_collections_v3", "typed_collections_v2", "unbounded"],
        ),
    ] {
        for value in values {
            let mut bad = original.clone();
            bad["projection_contract"][field] = value.into();
            denied(&bad);
        }
    }
    for max in [0, 127, 129, 256, 1024] {
        let mut bad = original.clone();
        bad["projection_contract"]["max_text_option_items"] = max.into();
        denied(&bad);
    }
    for form in ["TextOrAny", "TextOption", "Recursive"] {
        let mut bad = original.clone();
        bad["projection_contract"]["node_forms"]
            .as_array_mut()
            .unwrap()
            .push(form.into());
        denied(&bad);
    }
    let mut bad = original;
    bad["projection_contract"]["node_forms"]
        .as_array_mut()
        .unwrap()
        .retain(|f| f.as_str() != Some("TextOption"));
    denied(&bad);
}

#[test]
fn uci_option_admission_keeps_current_closed_recipes_and_rejects_arbitrary_collections() {
    let original = declaration();
    for mode in [
        "any_collection",
        "scalar_array",
        "object_entries",
        "raw",
        "none",
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"]["option_collections"] = mode.into();
        denied(&bad);
    }
    let mut bad = original.clone();
    bad["uci_read_contract"]
        .as_table_mut()
        .unwrap()
        .remove("option_collections");
    denied(&bad);
    let mut bad = original;
    bad["uci_read_contract"]["profiles"]
        .as_array_mut()
        .unwrap()
        .push("network:*".into());
    denied(&bad);
}

#[test]
fn previous_version_rejects_new_options_even_with_current_valid_probe_registry() {
    let mut old = declaration();
    old["uci_read_contract"]["profiles"] = toml::Value::Array(
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
    );
    old["version"] = 11.into();
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
    let error = Contract::parse(&toml::to_string(&old).unwrap())
        .unwrap()
        .validate(&root())
        .unwrap_err();
    assert!(error.contains("text option expansion"));
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
        .retain(|f| f.as_str() != Some("TextOption"));
    let error = Contract::parse(&toml::to_string(&old).unwrap())
        .unwrap()
        .validate(&root())
        .unwrap_err();
    assert!(error.contains("nested UCI options require architecture v12"));
    old["uci_read_contract"]
        .as_table_mut()
        .unwrap()
        .remove("option_collections");
    Contract::parse(&toml::to_string(&old).unwrap())
        .unwrap()
        .validate(&root())
        .unwrap();
}
