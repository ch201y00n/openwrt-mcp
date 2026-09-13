use std::collections::BTreeMap;

use openwrt_mcp_core::{
    Action, CAPABILITY_TOOL_NAME, CapabilityObservation, CapabilityRequirement, Catalog, Category,
    CoreError, IncompatibilityReason, MAX_OBSERVED_ARGUMENTS, MAX_OBSERVED_METHODS,
    MethodSignature, ObjectObservation, Operation, OutputMode, Parameter, ParameterKind,
    Permission, PreparedAction, ProbeRequest, Requirement, ReviewedObject, UBUS_INTEGER_MAX,
    UBUS_INTEGER_MIN, UbusArgumentType, UnknownReason, Verdict,
};
use serde_json::{Value, json};

fn operation(kind: ParameterKind, required: bool) -> Operation {
    Operation {
        name: "fixture_query".into(),
        description: "Synthetic signature matching fixture.".into(),
        requirements: vec![Requirement {
            category: Category::Network,
            permission: Permission::Read,
        }],
        parameters: BTreeMap::from([(
            "selector".into(),
            Parameter {
                kind,
                required,
                allowed_values: vec![],
            },
        )]),
        action: Action::Ubus {
            object: "network.interface".into(),
            method: "status".into(),
            arguments: BTreeMap::from([("interface".into(), json!("{selector}"))]),
        },
        capability: CapabilityRequirement::UbusMethod {
            object: "network.interface".into(),
            method: "status".into(),
            arguments: BTreeMap::from([("interface".into(), kind)]),
            response_contract: "fixture_interface.v1".into(),
        },
        output_fields: vec!["/up".into()],
        output_mode: OutputMode::Scalars,
    }
}

fn observation(arguments: &[(&str, UbusArgumentType)]) -> CapabilityObservation {
    CapabilityObservation::Ubus(ObjectObservation {
        object: ReviewedObject::NetworkInterface,
        methods: BTreeMap::from([(
            "status".into(),
            MethodSignature {
                arguments: arguments
                    .iter()
                    .map(|(name, kind)| ((*name).into(), *kind))
                    .collect(),
            },
        )]),
    })
}

fn is_invalid(operation: Operation) {
    assert_eq!(
        Catalog::new(vec![operation]).unwrap_err(),
        CoreError::InvalidDefinition
    );
}

#[test]
fn mandatory_metadata_and_strict_struct_variants_reject_silent_downgrades() {
    let encoded = serde_json::to_value(operation(ParameterKind::String, true)).unwrap();
    let mut missing = encoded.clone();
    missing.as_object_mut().unwrap().remove("capability");
    assert!(serde_json::from_value::<Operation>(missing).is_err());
    for capability in [
        json!({"kind":"unverified","force":true}),
        json!({"kind":"unverified","object":"system"}),
        json!({"kind":"ubus_method","object":"system","method":"info","response_contract":"system.v1"}),
        json!({"kind":"ubus_method","object":"system","method":"info","arguments":{},"response_contract":"system.v1","force":true}),
        json!({"kind":"assume_available"}),
    ] {
        let mut definition = encoded.clone();
        definition["capability"] = capability;
        assert!(serde_json::from_value::<Operation>(definition).is_err());
    }
    assert!(serde_json::from_value::<Operation>(encoded).is_ok());
}

#[test]
fn catalog_binds_object_method_argument_keys_and_types_to_the_action() {
    let original = operation(ParameterKind::String, true);
    let mut cases = vec![];
    for change in 0..5 {
        let mut value = original.clone();
        let CapabilityRequirement::UbusMethod {
            object,
            method,
            arguments,
            ..
        } = &mut value.capability
        else {
            unreachable!()
        };
        match change {
            0 => *object = "system".into(),
            1 => *method = "info".into(),
            2 => {
                arguments.clear();
            }
            3 => {
                arguments.insert("extra".into(), ParameterKind::String);
            }
            _ => {
                arguments.insert("interface".into(), ParameterKind::Integer);
            }
        }
        cases.push(value);
    }
    for value in cases {
        is_invalid(value);
    }
    assert!(Catalog::new(vec![original]).is_ok());
}

#[test]
fn unverified_only_describes_a_process_and_never_establishes_compatibility() {
    let mut value = operation(ParameterKind::Integer, true);
    value.capability = CapabilityRequirement::Unverified {};
    is_invalid(value.clone());
    value.action = Action::Process {
        program: "/usr/bin/fixture".into(),
        args: vec!["{selector}".into()],
    };
    let action = value.prepare(&json!({"selector":u64::MAX})).unwrap();
    assert!(matches!(action, PreparedAction::Process { .. }));
    assert_eq!(value.capability.probe_object(), None);
    assert_eq!(value.capability.response_contract(), None);
    assert_eq!(
        value.capability.evaluate(&observation(&[]), Some(&action)),
        Verdict::Unknown(UnknownReason::UnreviewedProbe)
    );
    value.capability = operation(ParameterKind::Integer, true).capability;
    is_invalid(value);
}

#[test]
fn checked_ubus_rejects_null_and_non_scalar_templates() {
    for literal in [Value::Null, json!([]), json!({}), json!(1.5)] {
        let mut value = operation(ParameterKind::Integer, true);
        value.parameters.clear();
        let Action::Ubus { arguments, .. } = &mut value.action else {
            unreachable!()
        };
        arguments.insert("interface".into(), literal);
        is_invalid(value);
    }
}

#[test]
fn response_contract_requires_a_bounded_versioned_identifier() {
    for invalid in [
        "",
        "fixture",
        "fixture.v0",
        "fixture.v01",
        "fixture.v",
        "fixture.v-1",
        "fixture.v4294967296",
        "fixture-secret\n.v1",
        "fixture space.v1",
        &format!("{}.v1", "a".repeat(126)),
    ] {
        let mut value = operation(ParameterKind::String, true);
        let CapabilityRequirement::UbusMethod {
            response_contract, ..
        } = &mut value.capability
        else {
            unreachable!()
        };
        *response_contract = invalid.into();
        is_invalid(value);
    }
    let value = operation(ParameterKind::String, true);
    assert_eq!(
        value.capability.response_contract(),
        Some("fixture_interface.v1")
    );
}

#[test]
fn metadata_tool_name_is_reserved_for_both_builtin_and_custom_catalogs() {
    let mut value = operation(ParameterKind::String, true);
    value.name = CAPABILITY_TOOL_NAME.into();
    is_invalid(value.clone());
    assert_eq!(
        Catalog::with_builtins(vec![value], vec![]).unwrap_err(),
        CoreError::InvalidDefinition
    );
}

#[test]
fn probe_registry_is_closed_and_exact_not_a_wildcard_or_parameter() {
    let expected = [
        "system",
        "network.device",
        "network.interface",
        "network.interface.lan",
        "network.interface.wan",
        "iwinfo",
        "service",
        "luci",
        "luci-rpc",
    ];
    assert_eq!(ReviewedObject::ALL.map(ReviewedObject::as_str), expected);
    for object in ReviewedObject::ALL {
        assert_eq!(ReviewedObject::from_name(object.as_str()), Some(object));
        assert_eq!(
            ProbeRequest::DescribeUbusObject(object),
            ProbeRequest::DescribeUbusObject(object)
        );
    }
    for name in [
        "System",
        "network.*",
        "network.interface.other",
        "uci",
        "file",
        "{object}",
        "system\n",
    ] {
        assert!(ReviewedObject::from_name(name).is_none());
    }
}

#[test]
fn custom_unreviewed_object_is_valid_metadata_but_cannot_be_probed_or_run() {
    let mut value = operation(ParameterKind::String, true);
    let Action::Ubus { object, .. } = &mut value.action else {
        unreachable!()
    };
    *object = "fixture.custom".into();
    let CapabilityRequirement::UbusMethod { object, .. } = &mut value.capability else {
        unreachable!()
    };
    *object = "fixture.custom".into();
    assert!(Catalog::new(vec![value.clone()]).is_ok());
    assert_eq!(value.capability.probe_object(), None);
    assert_eq!(
        value.capability.evaluate(&observation(&[]), None),
        Verdict::Unknown(UnknownReason::UnreviewedProbe)
    );
}

#[test]
fn only_transmitted_optional_fields_are_checked_but_status_is_conservative() {
    let value = operation(ParameterKind::String, false);
    let omitted = value.prepare(&json!({})).unwrap();
    let sent = value.prepare(&json!({"selector":"fixture"})).unwrap();
    let observed = observation(&[]);
    assert_eq!(
        value.capability.evaluate(&observed, Some(&omitted)),
        Verdict::Compatible
    );
    assert_eq!(
        value.capability.evaluate(&observed, Some(&sent)),
        Verdict::Unknown(UnknownReason::IncompleteSignature)
    );
    assert_eq!(
        value.capability.evaluate(&observed, None),
        Verdict::Unknown(UnknownReason::IncompleteSignature)
    );
}

#[test]
fn advertised_empty_global_interface_signature_is_unknown_not_proven_incompatible() {
    let value = operation(ParameterKind::String, true);
    let sent = value.prepare(&json!({"selector":"loopback"})).unwrap();
    // Models an incomplete advertisement; no direct-call/source/fixture success
    // can substitute for the missing runtime signature evidence.
    assert_eq!(
        value.capability.evaluate(&observation(&[]), Some(&sent)),
        Verdict::Unknown(UnknownReason::IncompleteSignature)
    );
}

#[test]
fn extra_target_setters_or_unknown_types_never_become_transmitted_arguments() {
    let value = operation(ParameterKind::String, true);
    let sent = value.prepare(&json!({"selector":"fixture"})).unwrap();
    let observed = observation(&[
        ("interface", UbusArgumentType::String),
        ("setter", UbusArgumentType::Unknown),
        ("reset", UbusArgumentType::Boolean),
    ]);
    assert_eq!(
        value.capability.evaluate(&observed, Some(&sent)),
        Verdict::Compatible
    );
    assert_eq!(
        value.capability.evaluate(&observed, None),
        Verdict::Compatible
    );
    assert_eq!(
        sent,
        PreparedAction::Ubus {
            object: "network.interface".into(),
            method: "status".into(),
            arguments: json!({"interface":"fixture"})
        }
    );
}

#[test]
fn hidden_method_unknown_type_and_supported_type_conflict_stay_distinct() {
    let value = operation(ParameterKind::String, true);
    let hidden = CapabilityObservation::Ubus(ObjectObservation {
        object: ReviewedObject::NetworkInterface,
        methods: BTreeMap::new(),
    });
    assert_eq!(
        value.capability.evaluate(&hidden, None),
        Verdict::Unknown(UnknownReason::NotObservedOrHidden)
    );
    assert_eq!(
        value.capability.evaluate(
            &observation(&[("interface", UbusArgumentType::Unknown)]),
            None
        ),
        Verdict::Unknown(UnknownReason::UnrecognizedType)
    );
    for kind in [
        UbusArgumentType::Integer,
        UbusArgumentType::Boolean,
        UbusArgumentType::Array,
        UbusArgumentType::Table,
    ] {
        assert_eq!(
            value
                .capability
                .evaluate(&observation(&[("interface", kind)]), None),
            Verdict::Incompatible(IncompatibilityReason::ArgumentTypeMismatch)
        );
    }
    for reason in [
        UnknownReason::ProbeUnavailable,
        UnknownReason::InvalidObservation,
        UnknownReason::StaleObservation,
    ] {
        assert_eq!(
            value
                .capability
                .evaluate(&CapabilityObservation::Unknown(reason), None),
            Verdict::Unknown(reason)
        );
    }
}

#[test]
fn foreign_or_unbounded_observations_cannot_establish_compatibility() {
    let value = operation(ParameterKind::String, true);
    let CapabilityObservation::Ubus(valid) =
        observation(&[("interface", UbusArgumentType::String)])
    else {
        unreachable!()
    };
    let mut foreign = valid.clone();
    foreign.object = ReviewedObject::System;
    assert_eq!(
        value
            .capability
            .evaluate(&CapabilityObservation::Ubus(foreign), None),
        Verdict::Unknown(UnknownReason::InvalidObservation)
    );
    let mut invalid = vec![];
    let mut many_methods = valid.clone();
    many_methods.methods = (0..=MAX_OBSERVED_METHODS)
        .map(|index| {
            (
                format!("method{index}"),
                MethodSignature {
                    arguments: BTreeMap::new(),
                },
            )
        })
        .collect();
    invalid.push(many_methods);
    let mut many_arguments = valid.clone();
    many_arguments.methods.get_mut("status").unwrap().arguments = (0..=MAX_OBSERVED_ARGUMENTS)
        .map(|index| (format!("argument{index}"), UbusArgumentType::String))
        .collect();
    invalid.push(many_arguments);
    for name in ["a".repeat(129), "bad\nmethod".into(), "*.status".into()] {
        let mut bad = valid.clone();
        bad.methods.insert(
            name,
            MethodSignature {
                arguments: BTreeMap::new(),
            },
        );
        invalid.push(bad);
    }
    for name in [
        "a".repeat(65),
        "bad argument".into(),
        "bad*argument".into(),
        "bad\nargument".into(),
    ] {
        let mut bad = valid.clone();
        bad.methods
            .get_mut("status")
            .unwrap()
            .arguments
            .insert(name, UbusArgumentType::String);
        invalid.push(bad);
    }
    for bad in invalid {
        assert!(bad.validate().is_err());
        assert_eq!(
            value
                .capability
                .evaluate(&CapabilityObservation::Ubus(bad), None),
            Verdict::Unknown(UnknownReason::InvalidObservation)
        );
    }
}

#[test]
fn unrelated_observed_hyphenated_fields_do_not_expand_action_argument_names() {
    let value = operation(ParameterKind::String, true);
    let CapabilityObservation::Ubus(mut observed) =
        observation(&[("interface", UbusArgumentType::String)])
    else {
        unreachable!()
    };
    observed.methods.insert(
        "add_device".into(),
        MethodSignature {
            arguments: BTreeMap::from([("link-ext".into(), UbusArgumentType::Boolean)]),
        },
    );
    assert!(observed.validate().is_ok());
    assert_eq!(
        value
            .capability
            .evaluate(&CapabilityObservation::Ubus(observed), None),
        Verdict::Compatible
    );
    let mut changed = value;
    let CapabilityRequirement::UbusMethod { arguments, .. } = &mut changed.capability else {
        unreachable!()
    };
    arguments.insert("link-ext".into(), ParameterKind::Boolean);
    let Action::Ubus { arguments, .. } = &mut changed.action else {
        unreachable!()
    };
    arguments.insert("link-ext".into(), json!(true));
    is_invalid(changed);
}

#[test]
fn defensive_prepared_action_checks_reject_method_object_shape_and_type_changes() {
    let value = operation(ParameterKind::String, true);
    let observed = observation(&[("interface", UbusArgumentType::String)]);
    let mut actions = vec![PreparedAction::Process {
        program: "/bin/true".into(),
        args: vec![],
    }];
    for (object, method, arguments) in [
        ("system", "status", json!({"interface":"fixture"})),
        ("network.interface", "down", json!({"interface":"fixture"})),
        ("network.interface", "status", json!([])),
        ("network.interface", "status", json!({"extra":true})),
        ("network.interface", "status", json!({"interface":42})),
        ("network.interface", "status", json!({"interface":null})),
        (
            "network.interface",
            "status",
            json!({"interface":"a".repeat(1025)}),
        ),
        (
            "network.interface",
            "status",
            json!({"interface":"bad\u{0}value"}),
        ),
    ] {
        actions.push(PreparedAction::Ubus {
            object: object.into(),
            method: method.into(),
            arguments,
        });
    }
    for action in actions {
        assert_eq!(
            value.capability.evaluate(&observed, Some(&action)),
            Verdict::Incompatible(IncompatibilityReason::InvalidPreparedAction)
        );
    }
}

#[test]
fn checked_integer_inputs_schema_and_defensive_matching_use_exact_i32_range() {
    let value = operation(ParameterKind::Integer, true);
    let observed = observation(&[("interface", UbusArgumentType::Integer)]);
    assert_eq!(
        value.input_schema()["properties"]["selector"]["minimum"],
        json!(UBUS_INTEGER_MIN)
    );
    assert_eq!(
        value.input_schema()["properties"]["selector"]["maximum"],
        json!(UBUS_INTEGER_MAX)
    );
    for number in [json!(UBUS_INTEGER_MIN), json!(0), json!(UBUS_INTEGER_MAX)] {
        let action = value.prepare(&json!({"selector":number})).unwrap();
        assert_eq!(
            value.capability.evaluate(&observed, Some(&action)),
            Verdict::Compatible
        );
    }
    for number in [
        json!(i64::from(i32::MIN) - 1),
        json!(i64::from(i32::MAX) + 1),
        json!(u64::MAX),
        json!(1.0),
        json!("1"),
    ] {
        assert_eq!(
            value.prepare(&json!({"selector":number})),
            Err(CoreError::InvalidArguments)
        );
        let action = PreparedAction::Ubus {
            object: "network.interface".into(),
            method: "status".into(),
            arguments: json!({"interface":number}),
        };
        assert_eq!(
            value.capability.evaluate(&observed, Some(&action)),
            Verdict::Incompatible(IncompatibilityReason::InvalidPreparedAction)
        );
    }
}

#[test]
fn checked_integer_literals_and_allowed_values_cannot_advertise_wider_wire_types() {
    for number in [i64::from(i32::MIN) - 1, i64::from(i32::MAX) + 1] {
        let mut literal = operation(ParameterKind::Integer, true);
        literal.parameters.clear();
        let Action::Ubus { arguments, .. } = &mut literal.action else {
            unreachable!()
        };
        arguments.insert("interface".into(), json!(number));
        is_invalid(literal);
        let mut allowed = operation(ParameterKind::Integer, true);
        allowed
            .parameters
            .get_mut("selector")
            .unwrap()
            .allowed_values = vec![number.to_string()];
        is_invalid(allowed);
    }
    let mut value = operation(ParameterKind::Integer, true);
    value.parameters.get_mut("selector").unwrap().allowed_values =
        vec![i32::MIN.to_string(), i32::MAX.to_string()];
    assert!(Catalog::new(vec![value]).is_ok());
}

#[test]
fn safe_reason_codes_have_no_raw_target_metadata() {
    for reason in [
        UnknownReason::NotObservedOrHidden,
        UnknownReason::IncompleteSignature,
        UnknownReason::UnrecognizedType,
        UnknownReason::UnreviewedProbe,
        UnknownReason::ProbeUnavailable,
        UnknownReason::InvalidObservation,
        UnknownReason::StaleObservation,
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(reason.code()));
        assert!(!reason.code().contains("fixture"));
    }
    for reason in [
        IncompatibilityReason::ArgumentTypeMismatch,
        IncompatibilityReason::InvalidPreparedAction,
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(reason.code()));
    }
}
