//! Architecture acceptance only: inert source/contract fixtures, no device jobs.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{Contract, check_guarded_mutation_source, check_owned_source, check_sealing_source};

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
fn validate(v: &toml::Value) -> Result<(), String> {
    Contract::parse(&toml::to_string(v).unwrap()).and_then(|c| c.validate(&root()))
}

#[test]
fn exact_p2_contract_and_previous_checkpoints_remain_distinct() {
    let current = declaration();
    validate(&current).unwrap();
    let mut old = current.clone();
    old["version"] = 20.into();
    assert!(
        validate(&old)
            .unwrap_err()
            .contains("require architecture v21")
    );
    profiles::before_v21(&mut old);
    validate(&old).unwrap();
    old["version"] = 19.into();
    profiles::before_v20(&mut old);
    validate(&old).unwrap();
    let mut absent = current;
    absent
        .as_table_mut()
        .unwrap()
        .remove("guarded_mutation_contract");
    assert!(validate(&absent).unwrap_err().contains("v21 requires"));
}

#[test]
fn every_rule_limit_state_port_permission_and_edge_is_closed() {
    let current = declaration();
    let contract = current["guarded_mutation_contract"].as_table().unwrap();
    for (field, value) in contract {
        let mut bad = current.clone();
        bad["guarded_mutation_contract"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        assert!(validate(&bad).is_err(), "missing {field}");
        let mut bad = current.clone();
        bad["guarded_mutation_contract"][field] = match value {
            toml::Value::Integer(n) => (n + 1).into(),
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Array(a) => {
                let mut changed = a.clone();
                changed.push("unreviewed".into());
                changed.into()
            }
            _ => panic!("unreviewed declaration type"),
        };
        assert!(validate(&bad).is_err(), "widened {field}");
        if let toml::Value::Array(items) = value {
            for index in 0..items.len() {
                let mut bad = current.clone();
                bad["guarded_mutation_contract"][field]
                    .as_array_mut()
                    .unwrap()
                    .remove(index);
                assert!(validate(&bad).is_err(), "missing {field}[{index}]");
                let mut bad = current.clone();
                bad["guarded_mutation_contract"][field]
                    .as_array_mut()
                    .unwrap()[index] = "unreviewed".into();
                assert!(validate(&bad).is_err(), "altered {field}[{index}]");
            }
        }
    }
    let mut bad = current;
    bad["guarded_mutation_contract"]
        .as_table_mut()
        .unwrap()
        .insert("allow_raw_apply".into(), true.into());
    assert!(validate(&bad).is_err());
}

#[test]
fn unsafe_state_shortcuts_and_security_downgrades_are_rejected() {
    let current = declaration();
    for shortcut in [
        "planned|apply|applying|permission_only",
        "backup_complete|apply|applying|no_recovery",
        "applying|confirm|confirmed|ssh_success",
        "outcome_unknown|retry|applying|lost_reply",
        "applying|cancel|cancelled|audit_failed",
        "recovering|verify_recovery|recovered|command_exit_zero",
        "awaiting_confirmation|confirm|confirmed|client_timestamp",
        "recovery_failed|admit|admitted|delete_journal",
    ] {
        let mut bad = current.clone();
        bad["guarded_mutation_contract"]["transitions"]
            .as_array_mut()
            .unwrap()
            .push(shortcut.into());
        assert!(validate(&bad).is_err(), "{shortcut}");
    }
    for (field, value) in [
        ("recovery", "volatile_memory_only"),
        ("admission", "caller_claimed_owner"),
        ("concurrency", "lock_is_root_isolation"),
        ("effects", "system_reload"),
        ("restore", "stream_unverified_plaintext_to_target"),
        ("worker", "detached_spawn_blocking"),
        ("storage", "rename_is_durable"),
    ] {
        let mut bad = current.clone();
        bad["guarded_mutation_contract"][field] = value.into();
        assert!(validate(&bad).is_err());
    }
}

#[test]
fn exact_module_consumers_are_admitted_without_opening_siblings_or_old_versions() {
    let current = declaration();
    let c = parse(&current);
    let mut old = current.clone();
    old["version"] = 20.into();
    profiles::before_v21(&mut old);
    let old = parse(&old);
    for edge in current["guarded_mutation_contract"]["edges"]
        .as_array()
        .unwrap()
    {
        let (directory, namespace) = edge.as_str().unwrap().split_once(" -> ").unwrap();
        let source = format!("use {namespace}::FixtureType;\n");
        let file = format!("{directory}mod.rs");
        check_owned_source(&c, &file, &source, &BTreeMap::new()).unwrap();
        assert!(
            check_owned_source(&old, &file, &source, &BTreeMap::new()).is_err(),
            "old {edge}"
        );
        let sibling = format!(
            "{}sibling.rs",
            directory.split_once("src/").unwrap().0.to_owned() + "src/"
        );
        assert!(
            check_owned_source(&c, &sibling, &source, &BTreeMap::new()).is_err(),
            "sibling {edge}"
        );
        let lookalike = file.replace("/mod.rs", "_escape/mod.rs");
        assert!(
            check_owned_source(&c, &lookalike, &source, &BTreeMap::new()).is_err(),
            "lookalike {edge}"
        );
    }
}

#[test]
fn mcp_alias_macro_and_raw_identifier_routes_do_not_gain_private_ports() {
    let c = parse(&declaration());
    let file = "crates/mcp/src/handler.rs";
    let aliases = BTreeMap::from([("hidden".into(), "openwrt_mcp_runtime".into())]);
    for code in [
        "use hidden::mutation_ports::DeviceJobControl;",
        "use openwrt_mcp_runtime::r#mutation_ports as hidden;",
        "use openwrt_mcp_runtime as app; use app::transactions::*;",
        "fn f() { format!(\"{}\", hidden::mutation_ports::SecretValue); }",
        "#[cfg(any())] use hidden::mutation_ports::DeviceAdmission;",
    ] {
        assert!(
            check_owned_source(&c, file, code, &aliases).is_err(),
            "{code}"
        );
    }
}

#[test]
fn workflow_has_no_host_io_or_legacy_action_and_sealing_is_not_open_to_siblings() {
    let c = parse(&declaration());
    let file = "crates/runtime/src/transactions/mod.rs";
    for source in ["use std::fs::File;", "use tokio::process::Command;"] {
        assert!(check_owned_source(&c, file, source, &BTreeMap::new()).is_err());
    }
    for source in [
        "use crate::PreparedAction;",
        "fn f() { spawn_blocking(work); }",
    ] {
        assert!(check_guarded_mutation_source(&c, file, source).is_err());
    }
    let source = "use crate::sealing::SealError;";
    check_sealing_source(&c, file, source, &BTreeMap::new()).unwrap();
    assert!(
        check_sealing_source(&c, "crates/runtime/src/read.rs", source, &BTreeMap::new()).is_err()
    );
    assert!(
        check_sealing_source(
            &c,
            "crates/runtime/src/sealing/mod.rs",
            "use crate::Dispatcher;",
            &BTreeMap::new()
        )
        .is_err()
    );
}

#[test]
fn private_models_and_secrets_cannot_serialize_or_acquire_printing_traits() {
    let c = parse(&declaration());
    for file in [
        "crates/core/src/transactions/mod.rs",
        "crates/runtime/src/mutation_ports/mod.rs",
    ] {
        for source in [
            "#[derive(serde::Serialize)] struct PrivateBaseline;",
            "use serde_json::Value;",
            "fn leak() { json!({\"data\": value}); }",
            "use crate::Backend;",
        ] {
            assert!(check_guarded_mutation_source(&c, file, source).is_err());
        }
    }
    let file = "crates/runtime/src/mutation_ports/secrets/mod.rs";
    for source in [
        "#[derive(Debug)] struct SecretValue;",
        "impl Display for SecretValue {}",
        "#[derive(Clone)] struct SecretValue;",
        "#[cfg_attr(any(), derive(Debug))] struct SecretValue;",
    ] {
        assert!(check_guarded_mutation_source(&c, file, source).is_err());
    }
    check_guarded_mutation_source(&c, file, "struct SecretValue { bytes: Zeroizing<Vec<u8>> }")
        .unwrap();
}

#[test]
fn ports_cannot_move_or_gain_a_generic_execute_method() {
    let current = declaration();
    let c = parse(&current);
    let file = "crates/runtime/src/mutation_ports/mod.rs";
    for port in current["guarded_mutation_contract"]["ports"]
        .as_array()
        .unwrap()
    {
        let port = port.as_str().unwrap();
        check_guarded_mutation_source(&c, file, port).unwrap();
        check_guarded_mutation_source(&c, file, &port.replace("WorkBudget)", "WorkBudget,)"))
            .unwrap();
        assert!(check_guarded_mutation_source(&c, "crates/mcp/src/handler.rs", port).is_err());
        let changed = port.replacen("-> Result<", "-> Other<", 1);
        assert!(check_guarded_mutation_source(&c, file, &changed).is_err());
    }
    assert!(
        check_guarded_mutation_source(
            &c,
            file,
            "trait RawExecute { fn execute(&self, args: Value); }"
        )
        .is_err()
    );
}
