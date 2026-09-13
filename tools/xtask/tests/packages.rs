//! Negative architecture checks; checkpoint declarations are not feature tests.
use std::{fs, path::PathBuf};
use xtask::{Contract, check_native_ci, check_portable_suite};

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
            .is_err()
    );
}

#[test]
fn package_contract_cannot_be_removed_weakened_or_extended_without_review() {
    let original = declaration();
    let mut missing = original.clone();
    missing.as_table_mut().unwrap().remove("package_contract");
    denied(&missing);
    for (field, value) in original["package_contract"].as_table().unwrap() {
        let mut removed = original.clone();
        removed["package_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&removed);
        let mut changed = original.clone();
        changed["package_contract"][field] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(number) => (number + 1).into(),
            toml::Value::Boolean(flag) => (!flag).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&changed);
    }
    let mut extra = original;
    extra["package_contract"]
        .as_table_mut()
        .unwrap()
        .insert("fallback".into(), true.into());
    denied(&extra);
}

#[test]
fn package_suites_cannot_be_missing_skipped_or_removed_from_native_ci() {
    let original = declaration();
    let contract = Contract::parse(&toml::to_string(&original).unwrap()).unwrap();
    contract.validate(&root()).unwrap();
    let workflow: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(&contract.native_ci)).unwrap())
            .unwrap();
    for suite in original["package_contract"]["required_tests"]
        .as_array()
        .unwrap()
    {
        let path = suite.as_str().unwrap();
        let mut missing = original.clone();
        missing["required_portable_tests"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s != suite);
        denied(&missing);
        let (owner, _) = contract.owner(path).unwrap();
        let command = format!("cargo test --locked -p {} --test packages", owner.name);
        let mut changed = workflow.clone();
        changed["jobs"]["rust"]["steps"]
            .as_array_mut()
            .unwrap()
            .retain(|step| step["run"].as_str() != Some(&command));
        assert!(check_native_ci(&contract, &changed.to_string()).is_err());
        check_portable_suite(&fs::read_to_string(root().join(path)).unwrap()).unwrap();
    }
    assert!(check_portable_suite("#[test] #[ignore] fn bypass() {}").is_err());
    assert!(check_portable_suite("#[cfg(unix)] #[test] fn bypass() {}").is_err());
}

#[test]
fn native_entropy_dependency_cannot_move_into_the_runtime_or_core() {
    for name in [
        "openwrt-mcp-runtime",
        "openwrt-mcp-core",
        "openwrt-mcp-transport",
    ] {
        let mut changed = declaration();
        changed["crates"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|rule| rule["name"].as_str() == Some(name))
            .unwrap()["dependencies"]
            .as_array_mut()
            .unwrap()
            .push("getrandom".into());
        denied(&changed);
    }
}

#[test]
fn opkg_status_requires_its_complete_exact_v14_contract() {
    let original = declaration();
    let validate = |value: &toml::Value| {
        Contract::parse(&toml::to_string(value).unwrap())
            .unwrap()
            .validate(&root())
    };
    validate(&original).unwrap();
    let mut old = original.clone();
    for rule in old["crates"].as_array_mut().unwrap() {
        if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
            rule["dependencies"]
                .as_array_mut()
                .unwrap()
                .retain(|d| !matches!(d.as_str(), Some("zeroize" | "flate2")));
        }
    }
    old["version"] = 13.into();
    assert!(
        validate(&old)
            .unwrap_err()
            .contains("opkg status expansion")
    );
    old.as_table_mut().unwrap().remove("opkg_status_contract");
    old.as_table_mut()
        .unwrap()
        .remove("management_effect_contract");
    old.as_table_mut()
        .unwrap()
        .remove("backup_archive_contract");
    old.as_table_mut().unwrap().remove("gzip_archive_contract");
    validate(&old).unwrap();
    old["version"] = 14.into();
    assert!(validate(&old).unwrap_err().contains("v14 requires"));
    for (field, value) in original["opkg_status_contract"].as_table().unwrap() {
        let mut removed = original.clone();
        removed["opkg_status_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        denied(&removed);
        let mut changed = original.clone();
        changed["opkg_status_contract"][field] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(number) => (number + 1).into(),
            toml::Value::Array(_) => toml::Value::Array(vec![]),
            _ => unreachable!(),
        };
        denied(&changed);
    }
    let mut extra = original;
    extra["opkg_status_contract"]
        .as_table_mut()
        .unwrap()
        .insert("fallback".into(), true.into());
    denied(&extra);
}

#[test]
fn opkg_status_keeps_all_native_suites_and_shared_package_bounds() {
    let original = declaration();
    for suite in original["opkg_status_contract"]["required_tests"]
        .as_array()
        .unwrap()
    {
        let mut changed = original.clone();
        changed["required_portable_tests"]
            .as_array_mut()
            .unwrap()
            .retain(|s| s != suite);
        denied(&changed);
    }
    for (field, replacement) in [
        ("commands", "opkg_list_installed"),
        ("admission", "version_only"),
        ("snapshot", "per_manager"),
        ("projection", "arbitrary_status_fields"),
        ("read_effects", "noaction_initialization"),
    ] {
        let mut changed = original.clone();
        changed["opkg_status_contract"][field] = replacement.into();
        denied(&changed);
    }
    for field in [
        "max_records",
        "max_source_bytes",
        "max_retained_bytes",
        "ttl_seconds",
    ] {
        let mut changed = original.clone();
        let value = changed["package_contract"][field].as_integer().unwrap();
        changed["package_contract"][field] = (value + 1).into();
        denied(&changed);
    }
}
