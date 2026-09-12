use openwrt_mcp_core::{
    Access, Category, CoreError, Grant, Operation, OutputMode, Permission, Policy, PreparedAction,
    Requirement,
};
use serde_json::json;

const NEW_READS: [(&str, Category); 5] = [
    ("network_interface_status", Category::Network),
    ("wireless_radio_info", Category::Wireless),
    ("service_logd_status", Category::Services),
    ("service_sysntpd_status", Category::Services),
    ("diagnostics_watchdog_status", Category::Diagnostics),
];

fn operation(name: &str) -> Operation {
    openwrt_mcp_features::catalog(vec![])
        .unwrap()
        .get(name)
        .unwrap()
        .clone()
}

fn grant(category: Category, access: Access, execute: bool) -> Policy {
    Policy {
        categories: [(category, Grant { access, execute })].into(),
        ..Policy::default()
    }
}

#[test]
fn new_reads_prepare_exact_fixed_objects_methods_and_arguments() {
    for (name, input, object, method, arguments) in [
        (
            "network_interface_status",
            json!({"interface":"guest"}),
            "network.interface",
            "dump",
            json!({}),
        ),
        (
            "wireless_radio_info",
            json!({"device":"phy0-ap0"}),
            "iwinfo",
            "info",
            json!({"device":"phy0-ap0"}),
        ),
        (
            "service_logd_status",
            json!({}),
            "service",
            "list",
            json!({"name":"log","verbose":false}),
        ),
        (
            "service_sysntpd_status",
            json!({}),
            "service",
            "list",
            json!({"name":"sysntpd","verbose":false}),
        ),
        (
            "diagnostics_watchdog_status",
            json!({}),
            "system",
            "watchdog",
            json!({}),
        ),
    ] {
        assert_eq!(
            operation(name).prepare(&input).unwrap(),
            PreparedAction::Ubus {
                object: object.into(),
                method: method.into(),
                arguments,
            },
            "{name}"
        );
    }
}

#[test]
fn selectors_reject_missing_unknown_and_invalid_arguments() {
    for (name, parameter) in [
        ("network_interface_status", "interface"),
        ("wireless_radio_info", "device"),
    ] {
        let operation = operation(name);
        assert_eq!(operation.parameters.len(), 1);
        assert_eq!(
            operation.prepare(&json!({})),
            Err(CoreError::MissingArgument)
        );
        assert_eq!(
            operation.prepare(&json!({(parameter):"fixture","method":"set"})),
            Err(CoreError::UnknownArgument)
        );
        for invalid in [
            json!(null),
            json!(true),
            json!(12),
            json!(["fixture"]),
            json!({"name":"fixture"}),
            json!("bad\u{0}selector"),
            json!("x".repeat(1025)),
        ] {
            assert_eq!(
                operation.prepare(&json!({(parameter):invalid})),
                Err(CoreError::InvalidArguments),
                "{name}"
            );
        }
        for input in [json!(null), json!([]), json!("fixture")] {
            assert_eq!(operation.prepare(&input), Err(CoreError::InvalidArguments));
        }
    }
}

#[test]
fn selector_injection_text_remains_json_data() {
    let literal = "fixture\"; $(synthetic-command) | x\n`synthetic`";
    for (name, parameter, object, method) in [
        (
            "network_interface_status",
            "interface",
            "network.interface",
            "dump",
        ),
        ("wireless_radio_info", "device", "iwinfo", "info"),
    ] {
        assert_eq!(
            operation(name)
                .prepare(&json!({(parameter):literal}))
                .unwrap(),
            PreparedAction::Ubus {
                object: object.into(),
                method: method.into(),
                arguments: if parameter == "interface" {
                    json!({})
                } else {
                    json!({(parameter):literal})
                },
            }
        );
    }
}

#[test]
fn fixed_read_actions_reject_selectors_and_watchdog_setters() {
    for name in [
        "service_logd_status",
        "service_sysntpd_status",
        "diagnostics_watchdog_status",
    ] {
        let operation = operation(name);
        assert!(operation.parameters.is_empty());
        for arguments in [
            json!({"name":"other-service"}),
            json!({"verbose":true}),
            json!({"timeout":30}),
            json!({"frequency":5}),
            json!({"magicclose":true}),
            json!({"stop":true}),
            json!({"method":"set"}),
        ] {
            assert_eq!(
                operation.prepare(&arguments),
                Err(CoreError::UnknownArgument),
                "{name}"
            );
        }
        for arguments in [json!(null), json!([]), json!(true)] {
            assert_eq!(
                operation.prepare(&arguments),
                Err(CoreError::InvalidArguments)
            );
        }
    }
}

#[test]
fn reads_use_only_their_exact_category_and_unrelated_grants_cannot_authorize_them() {
    let categories = [
        Category::System,
        Category::Network,
        Category::Wireless,
        Category::Firewall,
        Category::DhcpDns,
        Category::Services,
        Category::Packages,
        Category::Storage,
        Category::Vpn,
        Category::Firmware,
        Category::Diagnostics,
        Category::Extensions,
    ];
    for (name, category) in NEW_READS {
        let operation = operation(name);
        if name == "network_interface_status" {
            assert!(matches!(operation.output_mode, OutputMode::Typed(_)));
        } else {
            assert_eq!(operation.output_mode, OutputMode::Scalars);
        }
        assert_eq!(
            operation.requirements,
            vec![Requirement {
                category,
                permission: Permission::Read
            }]
        );
        assert!(Policy::default().authorize(&operation).is_err());
        for candidate in categories {
            assert_eq!(
                grant(candidate, Access::ReadWrite, true)
                    .authorize(&operation)
                    .is_ok(),
                candidate == category,
                "{name} with {candidate:?}"
            );
        }
        for access in [Access::Deny, Access::Read, Access::ReadWrite] {
            for execute in [false, true] {
                assert_eq!(
                    grant(category, access, execute)
                        .authorize(&operation)
                        .is_ok(),
                    access != Access::Deny,
                    "{name} with {access:?} execute={execute}"
                );
            }
        }
    }
}

#[test]
fn read_and_read_write_grants_do_not_imply_execution_permission() {
    for (name, category) in NEW_READS {
        let mut operation = operation(name);
        operation.requirements.push(Requirement {
            category,
            permission: Permission::Execute,
        });
        for access in [Access::Read, Access::ReadWrite] {
            assert!(
                grant(category, access, false)
                    .authorize(&operation)
                    .is_err()
            );
            assert!(grant(category, access, true).authorize(&operation).is_ok());
        }
    }
}

#[test]
fn interface_projection_excludes_address_route_dns_and_data_subtrees() {
    let operation = operation("network_interface_status");
    let projected = operation.prepare_invocation(&json!({"interface":"guest"})).unwrap().project(&json!({"interface":[{
        "interface":"guest",
        "up":true, "pending":false, "available":true, "autostart":true,
        "dynamic":false, "uptime":123, "proto":"static", "device":"br-fixture", "l3_device":"br-fixture",
        "ipv4-address":[{"address":"synthetic-private-address"}],
        "ipv6-address":[{"address":"synthetic-private-address"}],
        "route":[{"target":"synthetic-private-route"}],
        "dns-server":["synthetic-private-dns"],
        "data":{"credential":"synthetic-secret"},
        "config":{"password":"synthetic-secret"},
        "metric":100
    }]})).unwrap();
    assert_eq!(
        projected,
        json!({
            "interface":"guest", "up":true, "pending":false, "available":true, "autostart":true,
            "dynamic":false, "uptime":123, "proto":"static", "device":"br-fixture", "l3_device":"br-fixture"
        })
    );
    assert!(!projected.to_string().contains("synthetic-"));
}

#[test]
fn radio_projection_excludes_ssid_bssid_and_encryption_credentials() {
    let projected = operation("wireless_radio_info").project(&json!({
        "phy":"phy0", "mode":"Master", "country":"XX", "channel":36,
        "frequency":5180, "txpower":18, "quality":55, "quality_max":70,
        "signal":-40, "noise":-90, "bitrate":1200000,
        "encryption":{"enabled":true,"key":"synthetic-secret","password":"synthetic-secret"},
        "ssid":"synthetic-private-ssid", "bssid":"synthetic-private-bssid",
        "config":{"credential":"synthetic-secret"}
    }));
    assert_eq!(
        projected,
        json!({
            "/phy":"phy0", "/mode":"Master", "/country":"XX", "/channel":36,
            "/frequency":5180, "/txpower":18, "/quality":55, "/quality_max":70,
            "/signal":-40, "/noise":-90, "/bitrate":1200000, "/encryption/enabled":true
        })
    );
    assert!(!projected.to_string().contains("synthetic-"));
}

#[test]
fn service_projection_excludes_commands_environment_and_other_instances() {
    for (name, service, instance) in [
        ("service_logd_status", "log", "logd"),
        ("service_sysntpd_status", "sysntpd", "instance1"),
    ] {
        let prefix = format!("/{service}/instances/{instance}");
        let projected = operation(name).project(&json!({
            (service):{
                "instances":{
                    (instance):{
                        "running":true,"pid":42,"exit_code":0,
                        "command":["synthetic-private-command"],
                        "env":{"APP_TOKEN":"synthetic-secret"},
                        "data":{"credential":"synthetic-secret"}
                    },
                    "other":{"running":false,"command":["synthetic-private-command"]}
                },
                "data":{"password":"synthetic-secret"}
            },
            "other-service":{"instances":{"other":{"running":true,"pid":43}}}
        }));
        assert_eq!(
            projected,
            json!({
                (format!("{prefix}/running")):true,
                (format!("{prefix}/pid")):42,
                (format!("{prefix}/exit_code")):0
            })
        );
        assert!(!projected.to_string().contains("synthetic-"));
    }
}

#[test]
fn watchdog_projection_is_limited_to_documented_scalar_status_fields() {
    let projected = operation("diagnostics_watchdog_status").project(&json!({
        "status":"running", "timeout":30, "frequency":5, "magicclose":false,
        "device":"synthetic-private-device", "config":{"password":"synthetic-secret"}
    }));
    assert_eq!(
        projected,
        json!({"/status":"running","/timeout":30,"/frequency":5,"/magicclose":false})
    );
    assert!(!projected.to_string().contains("synthetic-"));
}

#[test]
fn every_new_read_field_rejects_object_and_array_substitutions() {
    for (name, _) in NEW_READS {
        let operation = operation(name);
        if name == "network_interface_status" {
            let invocation = operation
                .prepare_invocation(&json!({"interface":"guest"}))
                .unwrap();
            for field in [
                "interface",
                "up",
                "pending",
                "available",
                "autostart",
                "dynamic",
                "uptime",
                "proto",
                "device",
                "l3_device",
            ] {
                for malformed in [
                    json!({"unexpected":"synthetic-secret"}),
                    json!(["synthetic-secret"]),
                ] {
                    let mut row = json!({"interface":"guest","up":true,"pending":false,"available":true,"autostart":true,"dynamic":false});
                    row[field] = malformed;
                    assert_eq!(
                        invocation.project(&json!({"interface":[row]})),
                        Err(CoreError::InvalidOutput)
                    );
                }
            }
            continue;
        }
        for pointer in &operation.output_fields {
            for malformed in [
                json!({"unexpected":"synthetic-secret"}),
                json!(["synthetic-secret"]),
            ] {
                let source = pointer
                    .strip_prefix('/')
                    .unwrap()
                    .rsplit('/')
                    .fold(malformed, |value, key| json!({(key):value}));
                assert_eq!(
                    operation.project(&source),
                    json!({}),
                    "{name} pointer {pointer}"
                );
            }
        }
    }
}

#[test]
fn missing_results_are_not_synthesized_as_healthy_or_stopped_states() {
    for (name, _) in NEW_READS {
        let operation = operation(name);
        for source in [json!({}), json!(null), json!([])] {
            if name == "network_interface_status" {
                assert_eq!(
                    operation
                        .prepare_invocation(&json!({"interface":"guest"}))
                        .unwrap()
                        .project(&source),
                    Err(CoreError::InvalidOutput)
                );
            } else {
                assert_eq!(operation.project(&source), json!({}), "{name}");
            }
        }
    }
    for (name, service, instance) in [
        ("service_logd_status", "log", "logd"),
        ("service_sysntpd_status", "sysntpd", "instance1"),
    ] {
        let operation = operation(name);
        assert_eq!(
            operation.project(&json!({(service):{"instances":{}}})),
            json!({})
        );
        let projected =
            operation.project(&json!({(service):{"instances":{(instance):{"running":false}}}}));
        assert_eq!(
            projected,
            json!({(format!("/{service}/instances/{instance}/running")):false})
        );
        assert_eq!(projected.as_object().unwrap().len(), 1);
    }
}
