//! Synthetic protocol fixtures, not device behavior or hardware acceptance.
use openwrt_mcp_core::{
    CapabilityObservation, PreparedAction, ProbeRequest, ReviewedObject, UbusArgumentType,
    UnknownReason,
};
use openwrt_mcp_device_codec::{
    CodecError, MAX_PROBE_BYTES, compile_action, compile_probe, encode_remote, parse_ubus_describe,
};
use serde_json::json;
use std::{fs, path::Path};

fn parse(bytes: &[u8]) -> Result<CapabilityObservation, CodecError> {
    parse_ubus_describe(ReviewedObject::System, bytes, MAX_PROBE_BYTES)
}

fn listing(fragment: &str) -> Vec<u8> {
    format!("'system' @0123abcd\n\t{fragment}\n").into_bytes()
}

#[test]
fn every_closed_probe_matches_the_reviewed_registry_and_remote_encoding() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = fs::read_to_string(root.join("architecture/capability-probes.toml")).unwrap();
    let objects = [
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
    assert_eq!(registry.matches("[[probes]]").count(), objects.len());
    assert_eq!(
        registry.matches("program = \"/bin/ubus\"").count(),
        objects.len()
    );
    for object in objects {
        assert!(registry.contains(&format!("arguments = [\"-v\", \"list\", \"{object}\"]")));
        let reviewed = ReviewedObject::from_name(object).unwrap();
        let command = compile_probe(ProbeRequest::DescribeUbusObject(reviewed));
        assert_eq!(command.program(), "/bin/ubus");
        assert_eq!(command.arguments(), ["-v", "list", object]);
        assert_eq!(
            encode_remote(&command).unwrap(),
            format!("exec '/bin/ubus' '-v' 'list' '{object}'").as_bytes()
        );
    }
    assert_eq!(ReviewedObject::ALL.len(), objects.len());
    for object in ["*", "system;reboot", "uci", "-S", "", "{object}"] {
        assert!(ReviewedObject::from_name(object).is_none());
    }
    assert!(!registry.contains("\"call\""));
    assert!(!registry.contains("\"-S\""));
}

#[test]
fn complete_signatures_preserve_each_type_and_unknown_future_labels() {
    let CapabilityObservation::Ubus(observed) = parse(&listing(
        "\"query\":{\"text\":\"String\",\"number\":\"Integer\",\"flag\":\"Boolean\",\"list\":\"Array\",\"map\":\"Table\",\"future\":\"FutureInteger\"}",
    )).unwrap() else { panic!("expected observed signature") };
    let fields = &observed.methods["query"].arguments;
    for (name, expected) in [
        ("text", UbusArgumentType::String),
        ("number", UbusArgumentType::Integer),
        ("flag", UbusArgumentType::Boolean),
        ("list", UbusArgumentType::Array),
        ("map", UbusArgumentType::Table),
        ("future", UbusArgumentType::Unknown),
    ] {
        assert_eq!(fields[name], expected);
    }
    assert_eq!(observed.object, ReviewedObject::System);
}

#[test]
fn luci_descriptions_are_exact_objects_and_do_not_turn_other_methods_into_actions() {
    for (object, lines, method) in [
        (
            ReviewedObject::Luci,
            "\t\"getMountPoints\":{}\n\t\"getBlockDevices\":{}\n\t\"setBlockDetect\":{}\n\t\"setPassword\":{\"username\":\"String\",\"password\":\"String\"}\n",
            "getMountPoints",
        ),
        (
            ReviewedObject::LuciRpc,
            "\t\"getDHCPLeases\":{\"family\":\"Integer\"}\n",
            "getDHCPLeases",
        ),
    ] {
        let source = format!("'{}' @00000001\n{lines}", object.as_str());
        let CapabilityObservation::Ubus(observation) =
            parse_ubus_describe(object, source.as_bytes(), MAX_PROBE_BYTES).unwrap()
        else {
            panic!("synthetic complete signature")
        };
        assert_eq!(observation.object, object);
        assert!(observation.methods.contains_key(method));
        let foreign = if object == ReviewedObject::Luci {
            ReviewedObject::LuciRpc
        } else {
            ReviewedObject::Luci
        };
        assert_eq!(
            parse_ubus_describe(foreign, source.as_bytes(), MAX_PROBE_BYTES),
            Err(CodecError::InvalidObservation)
        );
        assert_eq!(
            parse_ubus_describe(object, b"", MAX_PROBE_BYTES).unwrap(),
            CapabilityObservation::Unknown(UnknownReason::NotObservedOrHidden)
        );
        assert_eq!(
            compile_probe(ProbeRequest::DescribeUbusObject(object)).arguments(),
            ["-v", "list", object.as_str()]
        );
    }
}

#[test]
fn empty_or_partial_introspection_is_not_proven_absence() {
    assert_eq!(
        parse(b"").unwrap(),
        CapabilityObservation::Unknown(UnknownReason::NotObservedOrHidden)
    );
    let CapabilityObservation::Ubus(empty) = parse(b"'system' @00000001\n").unwrap() else {
        panic!("header is a partial observation")
    };
    assert!(empty.methods.is_empty());
    let CapabilityObservation::Ubus(observed) = parse_ubus_describe(
        ReviewedObject::NetworkInterface,
        b"'network.interface' @00000001\n\t\"status\":{}\n",
        MAX_PROBE_BYTES,
    )
    .unwrap() else {
        panic!("empty signature is valid metadata")
    };
    assert!(observed.methods["status"].arguments.is_empty());
}

#[test]
fn emulated_25_12_5_metadata_shapes_keep_unrelated_hyphenated_fields() {
    // Condensed from read-only SSH introspection of disposable ARM64 QEMU
    // OpenWrt 25.12.5 (image SHA256 f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be).
    // Object IDs are normalized, unrelated methods omitted. This fixture verifies
    // parser shapes only, not method execution or BPI-R4 hardware acceptance.
    for (object, lines, method) in [
        (
            ReviewedObject::System,
            "\t\"info\":{}\n\t\"watchdog\":{\"frequency\":\"Integer\",\"timeout\":\"Integer\",\"magicclose\":\"Boolean\",\"stop\":\"Boolean\"}\n",
            "info",
        ),
        (
            ReviewedObject::NetworkDevice,
            "\t\"status\":{\"name\":\"String\"}\n\t\"set_alias\":{\"alias\":\"Array\",\"device\":\"String\"}\n",
            "status",
        ),
        (
            ReviewedObject::NetworkInterface,
            "\t\"status\":{}\n\t\"add_device\":{\"name\":\"String\",\"link-ext\":\"Boolean\",\"vlan\":\"Array\"}\n",
            "status",
        ),
        (
            ReviewedObject::NetworkInterfaceLan,
            "\t\"status\":{}\n\t\"remove_device\":{\"name\":\"String\",\"link-ext\":\"Boolean\",\"vlan\":\"Array\"}\n",
            "status",
        ),
        (
            ReviewedObject::Iwinfo,
            "\t\"devices\":{}\n\t\"info\":{\"device\":\"String\"}\n",
            "info",
        ),
        (
            ReviewedObject::Service,
            "\t\"list\":{\"name\":\"String\",\"verbose\":\"Boolean\"}\n\t\"set_data\":{\"name\":\"String\",\"instance\":\"String\",\"data\":\"Table\"}\n",
            "list",
        ),
    ] {
        let bytes = format!("'{}' @00000001\n{lines}", object.as_str());
        let CapabilityObservation::Ubus(observed) =
            parse_ubus_describe(object, bytes.as_bytes(), MAX_PROBE_BYTES).unwrap()
        else {
            panic!("expected metadata")
        };
        assert!(observed.methods.contains_key(method));
        observed.validate().unwrap();
    }
}

#[test]
fn malformed_foreign_truncated_and_non_utf8_metadata_is_rejected() {
    for bytes in [
        b"'system' @00000001".as_slice(),
        b"'system' @00000001\n\t\"info\":{}",
        b"'system' @00000001\n\t\"info\":{\n",
        b"'service' @00000001\n\t\"info\":{}\n",
        b"'system' @0000000z\n",
        b"'system' @000000001\n",
        b"'system' @00000001 extra\n",
        b"'system' @00000001\n'system' @00000002\n",
        b"system\n",
        b" \n\t",
        b"'system' @00000001\n\n",
        b"'system' @00000001\n{\"info\":{}}\n",
        b"'system' @00000001\n\t\"info\":{}\xff\n",
    ] {
        assert_eq!(parse(bytes), Err(CodecError::InvalidObservation));
    }
}

#[test]
fn duplicate_methods_arguments_and_unexpected_nested_values_are_rejected() {
    for fragment in [
        "\"info\":{},\"info\":{}",
        "\"info\":{}\n\t\"info\":{}",
        "\"info\":{}\n\t\"\\u0069nfo\":{}",
        "\"info\":{\"name\":\"String\",\"name\":\"Integer\"}",
        "\"info\":{\"name\":\"String\",\"\\u006eame\":\"String\"}",
        "\"info\":[]",
        "\"info\":{\"name\":{\"secret\":\"String\"}}",
        "\"info\":{\"name\":null}",
        "\"info\":{\"name\":17}",
        "\"info\":{\"name\":true}",
        "\"info\":{\"name\":\"\"}",
        "\"info\":{\"name\":\"String\\n\"}",
        "\"bad method\":{}",
        "\"info\":{\"invalid field\":\"String\"}",
    ] {
        assert_eq!(
            parse(&listing(fragment)),
            Err(CodecError::InvalidObservation)
        );
    }
}

#[test]
fn method_argument_name_and_collection_bounds_are_checked_at_the_boundary() {
    for (count, accepted) in [(128, true), (129, false)] {
        let mut bytes = b"'system' @00000001\n".to_vec();
        for index in 0..count {
            bytes.extend_from_slice(format!("\t\"m{index}\":{{}}\n").as_bytes());
        }
        assert_eq!(parse(&bytes).is_ok(), accepted);
    }
    for (count, accepted) in [(64, true), (65, false)] {
        let arguments = (0..count)
            .map(|index| format!("\"a{index}\":\"String\""))
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse(&listing(&format!("\"info\":{{{arguments}}}"))).is_ok(),
            accepted
        );
    }
    for (method_length, argument_length, accepted) in
        [(128, 64, true), (129, 64, false), (128, 65, false)]
    {
        let fragment = format!(
            "\"{}\":{{\"{}\":\"String\"}}",
            "m".repeat(method_length),
            "a".repeat(argument_length)
        );
        assert_eq!(parse(&listing(&fragment)).is_ok(), accepted);
    }
    assert!(
        parse(&listing(&format!(
            "\"info\":{{\"a\":\"{}\"}}",
            "X".repeat(65)
        )))
        .is_err()
    );
}

#[test]
fn configured_and_hard_probe_byte_caps_apply_before_parsing() {
    let bytes = listing("\"info\":{}");
    assert!(parse_ubus_describe(ReviewedObject::System, &bytes, bytes.len()).is_ok());
    assert_eq!(
        parse_ubus_describe(ReviewedObject::System, &bytes, bytes.len() - 1),
        Err(CodecError::OutputLimit)
    );
    assert_eq!(
        parse_ubus_describe(ReviewedObject::System, b"", 0),
        Err(CodecError::OutputLimit)
    );
    assert_eq!(
        parse_ubus_describe(
            ReviewedObject::System,
            &vec![b'x'; MAX_PROBE_BYTES + 1],
            usize::MAX
        ),
        Err(CodecError::OutputLimit)
    );
}

#[test]
fn action_compilation_preserves_literal_arguments_and_same_local_remote_bounds() {
    let command = compile_action(&PreparedAction::Process {
        program: "/usr/bin/fixture".into(),
        args: vec!["a'b;$(literal)\n\"".into(), "".into()],
    })
    .unwrap();
    assert_eq!(command.arguments(), ["a'b;$(literal)\n\"", ""]);
    assert_eq!(
        encode_remote(&command).unwrap(),
        b"exec '/usr/bin/fixture' 'a'\\''b;$(literal)\n\"' ''"
    );
    for args in [
        vec!["'".repeat(1024); 64],
        vec!["x".repeat(1025)],
        vec!["nul\0".into()],
        vec![String::new(); 65],
    ] {
        assert!(
            compile_action(&PreparedAction::Process {
                program: "/usr/bin/fixture".into(),
                args
            })
            .is_err()
        );
    }
    for program in [
        "/",
        "/bin/../sh",
        "C:/Windows/host.exe",
        "/bin/sh;exec",
        "relative",
        "/bin/fixture/",
    ] {
        assert!(
            compile_action(&PreparedAction::Process {
                program: program.into(),
                args: vec![]
            })
            .is_err()
        );
    }
}

#[test]
fn ubus_action_compilation_rejects_unapproved_types_and_integer_widths() {
    for value in [
        json!(null),
        json!(1.5),
        json!([]),
        json!({}),
        json!(i64::MAX),
        json!(u64::MAX),
        json!(i64::from(i32::MIN) - 1),
        json!(i64::from(i32::MAX) + 1),
    ] {
        assert!(
            compile_action(&PreparedAction::Ubus {
                object: "system".into(),
                method: "info".into(),
                arguments: json!({"value":value})
            })
            .is_err()
        );
    }
    for value in [json!(i32::MIN), json!(i32::MAX), json!(true), json!("x'y")] {
        let command = compile_action(&PreparedAction::Ubus {
            object: "system".into(),
            method: "info".into(),
            arguments: json!({"value":value}),
        })
        .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&command.arguments()[4]).unwrap(),
            json!({"value":value})
        );
    }
}
