use std::collections::{BTreeMap, BTreeSet};

use openwrt_mcp_core::{
    Access, Action, CapabilityRequirement, Catalog, Category, CoreError, Grant, Operation,
    OutputMode, Parameter, ParameterKind, Permission, Policy, PreparedAction, Requirement,
};
use serde_json::json;

fn custom() -> Operation {
    Operation {
        name: "diagnostic_example".into(),
        description: "Synthetic fixture action; not a router command.".into(),
        requirements: vec![Requirement {
            category: Category::Diagnostics,
            permission: Permission::Read,
        }],
        parameters: BTreeMap::from([(
            "device".into(),
            Parameter {
                kind: ParameterKind::String,
                required: true,
                allowed_values: Vec::new(),
            },
        )]),
        action: Action::Process {
            program: "/usr/bin/fixture".into(),
            args: vec!["--".into(), "{device}".into()],
        },
        capability: CapabilityRequirement::Unverified {},
        output_fields: Vec::new(),
        output_mode: OutputMode::Scalars,
    }
}

fn fixture_read() -> Operation {
    Operation {
        name: "fixture_read".into(),
        description: "Synthetic, source-independent read definition.".into(),
        requirements: vec![Requirement {
            category: Category::System,
            permission: Permission::Read,
        }],
        parameters: BTreeMap::new(),
        action: Action::Ubus {
            object: "fixture".into(),
            method: "read".into(),
            arguments: BTreeMap::new(),
        },
        capability: CapabilityRequirement::UbusMethod {
            object: "fixture".into(),
            method: "read".into(),
            arguments: BTreeMap::new(),
            response_contract: "fixture_read.v1".into(),
        },
        output_fields: vec!["/status".into(), "/model".into(), "/version/number".into()],
        output_mode: OutputMode::Scalars,
    }
}

fn fixture_catalog(custom: Vec<Operation>) -> Result<Catalog, CoreError> {
    Catalog::with_builtins(vec![fixture_read()], custom)
}

fn policy(category: Category, access: Access, execute: bool) -> Policy {
    Policy {
        categories: BTreeMap::from([(category, Grant { access, execute })]),
        ..Policy::default()
    }
}

#[test]
fn complete_access_execution_matrix() {
    let mut operation = custom();
    for access in [Access::Deny, Access::Read, Access::ReadWrite] {
        for execute in [false, true] {
            for permission in [Permission::Read, Permission::Write, Permission::Execute] {
                operation.requirements = vec![Requirement {
                    category: Category::Diagnostics,
                    permission,
                }];
                let expected = match permission {
                    Permission::Read => access != Access::Deny,
                    Permission::Write => access == Access::ReadWrite,
                    Permission::Execute => access != Access::Deny && execute,
                };
                assert_eq!(
                    policy(Category::Diagnostics, access, execute)
                        .authorize(&operation)
                        .is_ok(),
                    expected,
                    "{access:?} {execute} {permission:?}"
                );
            }
        }
    }
    assert_eq!(
        Policy::default().authorize(&operation),
        Err(CoreError::PermissionDenied)
    );
}

#[test]
fn allowlist_is_exact_denials_win_and_cross_category_needs_every_grant() {
    let mut operation = custom();
    operation.requirements.push(Requirement {
        category: Category::Network,
        permission: Permission::Write,
    });
    let mut configured = policy(Category::Diagnostics, Access::Read, false);
    assert!(configured.authorize(&operation).is_err());
    configured.categories.insert(
        Category::Network,
        Grant {
            access: Access::ReadWrite,
            execute: false,
        },
    );
    assert!(configured.authorize(&operation).is_ok());
    configured.allow_operations = Some(BTreeSet::new());
    assert!(configured.authorize(&operation).is_err());
    configured.allow_operations = Some(BTreeSet::from(["diagnostic_*".into()]));
    assert!(configured.authorize(&operation).is_err());
    configured.allow_operations = Some(BTreeSet::from([operation.name.clone()]));
    assert!(configured.authorize(&operation).is_ok());
    configured.deny_operations.insert(operation.name.clone());
    assert!(configured.authorize(&operation).is_err());
}

#[test]
fn extensions_always_require_write_and_execute() {
    let catalog = fixture_catalog(vec![custom()]).unwrap();
    let operation = catalog.get("diagnostic_example").unwrap();
    for access in [Access::Deny, Access::Read, Access::ReadWrite] {
        for execute in [false, true] {
            let mut configured = policy(Category::Diagnostics, Access::Read, false);
            configured
                .categories
                .insert(Category::Extensions, Grant { access, execute });
            assert_eq!(
                configured.authorize(operation).is_ok(),
                access == Access::ReadWrite && execute
            );
        }
    }
    assert_eq!(operation.requirements.len(), 3);
}

#[test]
fn builtins_cannot_be_replaced_and_unknown_names_stay_unknown() {
    let mut operation = custom();
    operation.name = "fixture_read".into();
    assert_eq!(
        fixture_catalog(vec![operation]).unwrap_err(),
        CoreError::DuplicateOperation
    );
    assert_eq!(
        fixture_catalog(vec![custom(), custom()]).unwrap_err(),
        CoreError::DuplicateOperation
    );
    let catalog = fixture_catalog(Vec::new()).unwrap();
    assert!(catalog.get("not_installed").is_none());
    let configured = Policy {
        deny_operations: BTreeSet::from(["not_installed".into()]),
        ..Policy::default()
    };
    assert_eq!(
        configured.validate(&catalog),
        Err(CoreError::UnknownOperationReference)
    );
}

#[test]
fn catalog_metadata_is_not_mutated_through_a_clone() {
    let catalog = fixture_catalog(Vec::new()).unwrap();
    let mut cloned = catalog.get("fixture_read").unwrap().clone();
    cloned.requirements.clear();
    assert_eq!(catalog.get("fixture_read").unwrap().requirements.len(), 1);
    assert!(
        policy(Category::System, Access::Read, false)
            .authorize(&cloned)
            .is_err()
    );
}

#[test]
fn scalar_inputs_reject_missing_unknown_nested_wrong_types_and_overlong_values() {
    let catalog = fixture_catalog(vec![custom()]).unwrap();
    let operation = catalog.get("diagnostic_example").unwrap();
    assert_eq!(
        operation.prepare(&json!({})),
        Err(CoreError::MissingArgument)
    );
    assert_eq!(
        operation.prepare(&json!({"device":"eth0", "extra":"fake-secret"})),
        Err(CoreError::UnknownArgument)
    );
    for input in [
        json!([]),
        json!(null),
        json!({"device":true}),
        json!({"device":{"name":"eth0"}}),
        json!({"device":"bad\u{0}name"}),
        json!({"device":"x".repeat(1025)}),
        json!({"device":"가".repeat(342)}),
    ] {
        assert_eq!(operation.prepare(&input), Err(CoreError::InvalidArguments));
    }
    assert!(
        operation
            .prepare(&json!({"device":"가".repeat(341)}))
            .is_ok()
    );
}

#[test]
fn shell_text_is_one_literal_argument_and_program_is_never_client_selected() {
    let catalog = fixture_catalog(vec![custom()]).unwrap();
    let operation = catalog.get("diagnostic_example").unwrap();
    let payload = "eth0; $(echo fake-secret) | sh\n`id`";
    let invocation = operation.prepare(&json!({"device":payload})).unwrap();
    assert_eq!(
        invocation,
        PreparedAction::Process {
            program: "/usr/bin/fixture".into(),
            args: vec!["--".into(), payload.into()],
        }
    );
    assert!(
        operation
            .prepare(&json!({"device":"eth0","program":"/bin/sh"}))
            .is_err()
    );
}

#[test]
fn ubus_substitution_preserves_json_types_and_escapes_strings() {
    let mut operation = custom();
    operation.parameters.insert(
        "count".into(),
        Parameter {
            kind: ParameterKind::Integer,
            required: true,
            allowed_values: vec!["4".into()],
        },
    );
    operation.parameters.insert(
        "enabled".into(),
        Parameter {
            kind: ParameterKind::Boolean,
            required: false,
            allowed_values: vec!["true".into()],
        },
    );
    operation.action = Action::Ubus {
        object: "fixture.object".into(),
        method: "query".into(),
        arguments: BTreeMap::from([
            ("name".into(), json!("{device}")),
            ("count".into(), json!("{count}")),
            ("enabled".into(), json!("{enabled}")),
            ("literal".into(), json!("prefix-{device}")),
        ]),
    };
    operation.capability = CapabilityRequirement::UbusMethod {
        object: "fixture.object".into(),
        method: "query".into(),
        arguments: BTreeMap::from([
            ("name".into(), ParameterKind::String),
            ("count".into(), ParameterKind::Integer),
            ("enabled".into(), ParameterKind::Boolean),
            ("literal".into(), ParameterKind::String),
        ]),
        response_contract: "fixture_query.v1".into(),
    };
    let catalog = fixture_catalog(vec![operation]).unwrap();
    let operation = catalog.get("diagnostic_example").unwrap();
    let invocation = operation
        .prepare(&json!({"device":"a\"; fake-secret", "count":4, "enabled":true}))
        .unwrap();
    assert_eq!(
        invocation,
        PreparedAction::Ubus {
            object: "fixture.object".into(),
            method: "query".into(),
            arguments: json!({"name":"a\"; fake-secret", "count":4, "enabled":true, "literal":"prefix-{device}"}),
        }
    );
    let optional = operation
        .prepare(&json!({"device":"eth0", "count":4}))
        .unwrap();
    assert!(
        matches!(optional, PreparedAction::Ubus { arguments, .. } if arguments.get("enabled").is_none())
    );
    for input in [
        json!({"device":"eth0","count":"4"}),
        json!({"device":"eth0","count":4.0}),
        json!({"device":"eth0","count":5}),
        json!({"device":"eth0","count":4,"enabled":false}),
    ] {
        assert_eq!(operation.prepare(&input), Err(CoreError::InvalidArguments));
    }
    assert_eq!(
        operation.input_schema()["properties"]["count"]["enum"],
        json!([4])
    );
    assert_eq!(
        operation.input_schema()["properties"]["enabled"]["enum"],
        json!([true])
    );
}

#[test]
fn invalid_definitions_are_rejected_without_echoing_them() {
    let mut cases = Vec::new();
    let mut action = custom();
    action.name = "bad;fake-secret".into();
    cases.push(action);
    let mut action = custom();
    action.requirements.clear();
    cases.push(action);
    let mut action = custom();
    action.output_fields = vec!["".into()];
    cases.push(action);
    let mut action = custom();
    action.output_fields = vec!["/bad~2escape".into()];
    cases.push(action);
    let mut action = custom();
    action.output_fields = vec!["/a".into(), "/a".into()];
    cases.push(action);
    let mut action = custom();
    action.action = Action::Process {
        program: "relative/fixture".into(),
        args: vec!["{device}".into()],
    };
    cases.push(action);
    let mut action = custom();
    action.action = Action::Process {
        program: "/usr/../bin/fixture".into(),
        args: vec!["{device}".into()],
    };
    cases.push(action);
    let mut action = custom();
    action.action = Action::Process {
        program: "{device}".into(),
        args: vec![],
    };
    cases.push(action);
    let mut action = custom();
    action.action = Action::Process {
        program: "/bin/fixture".into(),
        args: vec!["{unknown}".into()],
    };
    cases.push(action);
    let mut action = custom();
    action.action = Action::Ubus {
        object: "system;fake-secret".into(),
        method: "info".into(),
        arguments: BTreeMap::new(),
    };
    cases.push(action);
    let mut action = custom();
    action.parameters.get_mut("device").unwrap().allowed_values = vec!["x".repeat(1025)];
    cases.push(action);
    let mut action = custom();
    action.action = Action::Process {
        program: "/bin/fixture".into(),
        args: vec!["literal".into()],
    };
    cases.push(action);
    for action in cases {
        let error = fixture_catalog(vec![action]).unwrap_err();
        assert_eq!(error, CoreError::InvalidDefinition);
        assert!(!error.to_string().contains("fake-secret"));
    }
    assert_eq!(
        fixture_catalog(vec![custom(); 1025]).unwrap_err(),
        CoreError::CatalogLimitExceeded
    );
}

#[test]
fn projection_defaults_to_nothing_and_redacts_sensitive_keys_recursively() {
    let mut action = custom();
    let source = json!({"result": {"ok":true, "password":"fake-password", "WiFi_PSK":"fake-wifi", "private_key":"fake-key", "nested":[{"value":7,"API_TOKEN":"fake-token"}]}, "credential":{"visible":"fake-visible"}, "a/b":{"~value":3}, "other":"fake-other"});
    assert_eq!(action.project(&source), json!({}));
    action.output_mode = OutputMode::Structured;
    action.output_fields = vec![
        "/result".into(),
        "/credential/visible".into(),
        "/a~1b/~0value".into(),
        "/missing".into(),
    ];
    assert_eq!(
        action.project(&source),
        json!({"/result":{"ok":true,"nested":[{"value":7}]},"/a~1b/~0value":3})
    );
    assert!(!action.project(&source).to_string().contains("fake-"));
}

#[test]
fn scalar_projection_omits_raw_configuration_and_malformed_subtrees() {
    let operation = fixture_read();
    let source = json!({
        "status":"available",
        "model":{"unrecognized":"fake-secret"},
        "version":{"number":"fixture","password":"fake-password"},
        "configuration":{"key":"fake-secret"},
    });
    assert_eq!(
        operation.project(&source),
        json!({"/status":"available","/version/number":"fixture"})
    );
}

#[test]
fn output_mode_defaults_to_scalars_and_never_depends_on_a_reserved_name() {
    let mut operation = fixture_read();
    operation.name = "extension_fixture".into();
    let mut serialized = serde_json::to_value(operation).unwrap();
    serialized.as_object_mut().unwrap().remove("output_mode");
    let defaulted: Operation = serde_json::from_value(serialized).unwrap();
    assert_eq!(defaulted.output_mode, OutputMode::Scalars);
    assert_eq!(
        defaulted.project(&json!({"model":{"unexpected":"fake-secret"}})),
        json!({})
    );
}

#[test]
fn catalog_rejects_duplicates_among_reviewed_definitions() {
    assert_eq!(
        Catalog::with_builtins(vec![fixture_read(), fixture_read()], vec![]).unwrap_err(),
        CoreError::DuplicateOperation
    );
    assert!(Catalog::new(vec![]).unwrap().operations().is_empty());
}

#[test]
fn config_rejects_unknown_fields_and_categories() {
    assert!(toml::from_str::<Policy>("[categories.typo]\naccess='read'").is_err());
    assert!(toml::from_str::<Policy>("[categories.network]\naccess='read'\nexecut=true").is_err());
    assert!(serde_json::from_value::<Operation>(json!({"name":"a","description":"b","requirements":[],"action":{"kind":"process","program":"/bin/true","shell":true}})).is_err());
    let configured: Policy = toml::from_str("[categories.network]\naccess='read_write'").unwrap();
    assert!(!configured.categories[&Category::Network].execute);
}

#[test]
fn optional_process_parameters_are_rejected_to_preserve_argument_positions() {
    let mut action = custom();
    action.parameters.get_mut("device").unwrap().required = false;
    action.action = Action::Process {
        program: "/usr/bin/fixture".into(),
        args: vec![
            "--device".into(),
            "{device}".into(),
            "--mode".into(),
            "status".into(),
        ],
    };
    assert_eq!(
        fixture_catalog(vec![action.clone()]).unwrap_err(),
        CoreError::InvalidDefinition
    );
    assert_eq!(
        action.prepare(&json!({})),
        Err(CoreError::InvalidDefinition)
    );

    action.parameters.get_mut("device").unwrap().required = true;
    let catalog = fixture_catalog(vec![action]).unwrap();
    let action = catalog.get("diagnostic_example").unwrap();
    assert_eq!(action.prepare(&json!({})), Err(CoreError::MissingArgument));
    assert_eq!(
        action.prepare(&json!({"device":"eth0"})).unwrap(),
        PreparedAction::Process {
            program: "/usr/bin/fixture".into(),
            args: vec![
                "--device".into(),
                "eth0".into(),
                "--mode".into(),
                "status".into()
            ],
        }
    );
}

#[test]
fn overlapping_projection_paths_are_rejected_before_cloning_output() {
    for fields in [
        vec!["/a", "/a/b"],
        vec!["/a/b", "/a"],
        vec!["/a~1b", "/a~1b/child"],
        vec!["/~01", "/~01/child"],
        vec!["/", "//child"],
    ] {
        let mut action = custom();
        action.output_fields = fields.iter().map(|field| (*field).to_owned()).collect();
        assert_eq!(
            fixture_catalog(vec![action.clone()]).unwrap_err(),
            CoreError::InvalidDefinition
        );
        assert_eq!(action.project(&json!({"a":{"b":"fixture"}})), json!({}));
    }
    let mut nested = custom();
    nested.output_fields = (1..=64).map(|depth| "/a".repeat(depth)).collect();
    assert_eq!(
        fixture_catalog(vec![nested]).unwrap_err(),
        CoreError::InvalidDefinition
    );
}

#[test]
fn projection_overlap_check_compares_decoded_segments_not_string_prefixes() {
    let mut action = custom();
    action.output_fields = ["/a", "/ab", "/a~1b", "/~01", "/~1"]
        .map(str::to_owned)
        .to_vec();
    let catalog = fixture_catalog(vec![action]).unwrap();
    let action = catalog.get("diagnostic_example").unwrap();
    assert_eq!(
        action.project(&json!({"a":1,"ab":2,"a/b":3,"~1":4,"/":5})),
        json!({"/a":1,"/ab":2,"/a~1b":3,"/~01":4,"/~1":5})
    );

    let mut slash = custom();
    slash.output_fields = vec!["/a~1b".into(), "/a/b".into()];
    assert!(fixture_catalog(vec![slash]).is_ok());
}
