//! Versioned recipe declarations only; no service or router acceptance evidence.
use std::{fs, path::PathBuf};
use xtask::Contract;

mod profiles;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn validate(v: &toml::Value) -> Result<(), String> {
    Contract::parse(&toml::to_string(v).unwrap()).and_then(|c| c.validate(&root()))
}

#[test]
fn system_service_recipes_require_v20_without_loosening_the_v19_set() {
    let current = declaration();
    validate(&current).unwrap();
    let mut older = current.clone();
    profiles::before_v21(&mut older);
    older["version"] = 19.into();
    assert!(
        validate(&older)
            .unwrap_err()
            .contains("requires architecture v20")
    );
    profiles::before_v20(&mut older);
    assert_eq!(
        older["uci_read_contract"]["profiles"]
            .as_array()
            .unwrap()
            .len(),
        24
    );
    validate(&older).unwrap();
    for recipe in [
        "system:led",
        "dropbear:dropbear",
        "uhttpd:uhttpd",
        "dhcp:odhcpd",
    ] {
        let mut bad = older.clone();
        bad["uci_read_contract"]["profiles"]
            .as_array_mut()
            .unwrap()
            .push(recipe.into());
        assert!(
            validate(&bad)
                .unwrap_err()
                .contains("requires architecture v20")
        );
        let mut bad = older.clone();
        bad["uci_read_contract"]["profiles"].as_array_mut().unwrap()[0] = recipe.into();
        assert!(validate(&bad).unwrap_err().contains("closed UCI profiles"));
    }
    let mut incomplete = older;
    incomplete["version"] = 20.into();
    assert!(
        validate(&incomplete)
            .unwrap_err()
            .contains("closed UCI profiles")
    );
    let mut reordered = current;
    reordered["uci_read_contract"]["profiles"]
        .as_array_mut()
        .unwrap()
        .reverse();
    validate(&reordered).unwrap();
}

#[test]
fn new_recipes_retain_every_owner_permission_source_budget_and_required_suite() {
    let original = declaration();
    for field in [
        "domain_owner",
        "definitions_owner",
        "invocation",
        "actions",
        "custom_uci",
        "requirement",
        "projection",
        "view",
        "option_collections",
    ] {
        let mut bad = original.clone();
        bad["uci_read_contract"][field] = "unreviewed".into();
        assert!(validate(&bad).is_err(), "{field}");
    }
    for field in [
        "max_total_items",
        "max_emitted_items",
        "max_collection_nodes",
        "max_normalized_bytes",
        "max_text_option_items",
    ] {
        let mut bad = original.clone();
        bad["projection_contract"][field] =
            (bad["projection_contract"][field].as_integer().unwrap() + 1).into();
        assert!(validate(&bad).is_err(), "{field}");
    }
    for suite in original["uci_read_contract"]["required_tests"]
        .as_array()
        .unwrap()
    {
        for location in ["uci", "portable"] {
            let mut bad = original.clone();
            let list = if location == "uci" {
                &mut bad["uci_read_contract"]["required_tests"]
            } else {
                &mut bad["required_portable_tests"]
            };
            list.as_array_mut().unwrap().retain(|p| p != suite);
            assert!(validate(&bad).is_err());
        }
    }
}
