//! Adversarial ADR 0006 declarations; no response behavior or device acceptance.
use std::{fs, path::PathBuf};

use xtask::{Contract, check_native_ci, check_portable_source, check_portable_suite, check_source};

const TABLES: [&str; 3] = [
    "projection_contract",
    "action_response_contract",
    "mcp_result_contract",
];
const SUITES: [&str; 5] = [
    "crates/core/tests/collection_projection.rs",
    "crates/features/tests/collection_contracts.rs",
    "crates/runtime/tests/collection_projection.rs",
    "crates/device-codec/tests/action_response.rs",
    "crates/mcp/tests/bounded_results.rs",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn text() -> String {
    fs::read_to_string(root().join("architecture/spec.toml")).unwrap()
}

fn declaration() -> toml::Value {
    toml::from_str(&text()).unwrap()
}

fn denied(value: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(value).unwrap())
            .and_then(|contract| contract.validate(&root()))
            .is_err(),
        "accepted a malformed or weakened bounded response contract"
    );
}

#[test]
fn v6_requires_every_response_table_and_every_field_without_unknown_metadata() {
    Contract::parse(&text()).unwrap().validate(&root()).unwrap();
    for table in TABLES {
        let mut value = declaration();
        value.as_table_mut().unwrap().remove(table);
        denied(&value);
        for field in declaration()[table].as_table().unwrap().keys() {
            let mut value = declaration();
            value[table].as_table_mut().unwrap().remove(field);
            denied(&value);
        }
        let mut value = declaration();
        value[table]
            .as_table_mut()
            .unwrap()
            .insert("unreviewed_behavior".into(), true.into());
        denied(&value);
    }
}

#[test]
fn observation_rows_cannot_gain_selection_deduplication_raw_unions_or_unbounded_guards() {
    let source = declaration();
    for (field, values) in [
        (
            "observation_rows",
            vec!["deduplicate", "select_first", "caller_identity"],
        ),
        (
            "scalar_union",
            vec!["any_json", "coerce_false_to_zero", "boolean_or_integer"],
        ),
        (
            "root_guards",
            vec!["after_success_audit", "ignore_errors", "caller_paths"],
        ),
        ("profile", vec!["typed_collections_v1", "unbounded"]),
    ] {
        for value in values {
            let mut bad = source.clone();
            bad["projection_contract"][field] = value.into();
            denied(&bad);
        }
    }
    let mut before = source;
    before["version"] = 9.into();
    denied(&before);
}

#[test]
fn every_response_hard_ceiling_rejects_unbounded_zero_wrong_type_and_weaker_values() {
    for table in TABLES {
        for (field, original) in declaration()[table].as_table().unwrap() {
            if let Some(ceiling) = original.as_integer() {
                for invalid in [
                    toml::Value::Integer(0),
                    toml::Value::Integer(-1),
                    toml::Value::Integer(ceiling + 1),
                    toml::Value::Integer(ceiling - 1),
                    toml::Value::Float(ceiling as f64),
                    toml::Value::String("unbounded".into()),
                ] {
                    let mut value = declaration();
                    value[table][field] = invalid;
                    denied(&value);
                }
            }
        }
    }
}

#[test]
fn ownership_profiles_selection_failure_and_serialization_scope_cannot_be_replaced() {
    for table in TABLES {
        for (field, original) in declaration()[table].as_table().unwrap() {
            if original.is_str() {
                let mut value = declaration();
                value[table][field] = "unreviewed".into();
                denied(&value);
            }
        }
    }
    for forms in [
        vec![
            "Record",
            "ObjectArray",
            "ObjectEntries",
            "ScalarArray",
            "RecursiveNode",
        ],
        vec!["Record", "ObjectArray", "ObjectEntries", "Record"],
        vec!["Record", "ObjectArray", "ScalarArray"],
    ] {
        let mut value = declaration();
        value["projection_contract"]["node_forms"] =
            toml::Value::Array(forms.into_iter().map(Into::into).collect());
        denied(&value);
    }
    let mut value = declaration();
    value["action_response_contract"]["consumers"] =
        toml::Value::Array(vec!["openwrt-mcp-adapters".into()]);
    denied(&value);
}

#[test]
fn response_owners_and_required_suite_ownership_cannot_move_or_disappear() {
    let contract = Contract::parse(&text()).unwrap();
    for suite in SUITES {
        let (owner, _) = contract.owner(suite).unwrap();
        for field in ["name", "path", "layer"] {
            let mut value = declaration();
            let rule = value["crates"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|rule| rule["name"].as_str() == Some(&owner.name))
                .unwrap();
            rule[field] = if field == "layer" {
                "infrastructure".into()
            } else {
                "unreviewed".into()
            };
            // The codec is already infrastructure; use a different valid layer.
            if field == "layer" && owner.layer == "infrastructure" {
                rule[field] = "composition".into();
            }
            denied(&value);
        }
        let mut value = declaration();
        value["required_portable_tests"]
            .as_array_mut()
            .unwrap()
            .retain(|item| item.as_str() != Some(suite));
        denied(&value);
        for table in TABLES {
            if declaration()[table]["required_tests"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.as_str() == Some(suite))
            {
                let mut value = declaration();
                value[table]["required_tests"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|item| item.as_str() != Some(suite));
                denied(&value);
            }
        }
    }
}

#[test]
fn response_owners_keep_io_and_host_specific_behavior_out_of_portable_production() {
    let contract = Contract::parse(&text()).unwrap();
    for suite in SUITES {
        let (owner, _) = contract.owner(suite).unwrap();
        for source in [
            "fn leaked() { std::fs::read(\"synthetic\"); }",
            "use std::fs::read as hidden; fn leaked() { hidden(\"synthetic\"); }",
        ] {
            assert!(check_source(owner, source, false).is_err());
        }
    }
    assert!(check_portable_source("#[cfg(windows)] fn leaked() {}").is_err());
}

#[test]
fn all_five_response_suites_require_actual_native_ci_commands_and_non_skipped_tests() {
    let contract = Contract::parse(&text()).unwrap();
    let original: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(&contract.native_ci)).unwrap())
            .unwrap();
    for suite in SUITES {
        let (owner, _) = contract.owner(suite).unwrap();
        let target = PathBuf::from(suite)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let command = format!("cargo test --locked -p {} --test {target}", owner.name);
        let mut workflow = original.clone();
        workflow["jobs"]["rust"]["steps"]
            .as_array_mut()
            .unwrap()
            .retain(|step| step["run"].as_str() != Some(&command));
        assert!(check_native_ci(&contract, &workflow.to_string()).is_err());
        check_portable_suite(&fs::read_to_string(root().join(suite)).unwrap()).unwrap();
    }
    for source in [
        "#![cfg(unix)] #[test] fn hidden() {}",
        "#[test] #[ignore] fn hidden() {}",
        "#[cfg(any())] mod hidden { #[test] fn absent() {} }",
    ] {
        assert!(check_portable_suite(source).is_err());
    }
}
