//! Synthetic metadata contracts, not proof of target availability or acceptance.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use openwrt_mcp_core::{
    Action, CapabilityObservation, CapabilityRequirement, Catalog, CoreError, MethodSignature,
    ObjectObservation, ParameterKind, Policy, ReviewedObject, UbusArgumentType, UnknownReason,
    Verdict,
};
use serde_json::json;

#[test]
fn every_builtin_declares_the_exact_reviewed_input_and_versioned_response_contract() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let mut expectations = vec![
        (
            "system_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "network_interface_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "wireless_radio_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "firewall_defaults_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "dhcp_dnsmasq_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "storage_mount_configuration",
            ReviewedObject::Uci,
            "get",
            vec![
                ("config", ParameterKind::String),
                ("type", ParameterKind::String),
            ],
        ),
        (
            "network_interface_addresses",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "network_interface_routes",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "network_interface_neighbors",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "dhcp_interface_dns",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "dhcp_v4_leases",
            ReviewedObject::LuciRpc,
            "getDHCPLeases",
            vec![("family", ParameterKind::Integer)],
        ),
        (
            "dhcp_v6_leases",
            ReviewedObject::LuciRpc,
            "getDHCPLeases",
            vec![("family", ParameterKind::Integer)],
        ),
        (
            "storage_mounts",
            ReviewedObject::Luci,
            "getMountPoints",
            vec![],
        ),
        (
            "storage_block_devices",
            ReviewedObject::Luci,
            "getBlockDevices",
            vec![],
        ),
        ("system_board", ReviewedObject::System, "board", vec![]),
        ("system_info", ReviewedObject::System, "info", vec![]),
        (
            "wireless_stations",
            ReviewedObject::Iwinfo,
            "assoclist",
            vec![("device", ParameterKind::String)],
        ),
        (
            "wireless_station_status",
            ReviewedObject::Iwinfo,
            "assoclist",
            vec![("device", ParameterKind::String)],
        ),
        (
            "wireless_countries",
            ReviewedObject::Iwinfo,
            "countrylist",
            vec![("device", ParameterKind::String)],
        ),
        (
            "network_device_status",
            ReviewedObject::NetworkDevice,
            "status",
            vec![("name", ParameterKind::String)],
        ),
        (
            "network_lan_status",
            ReviewedObject::NetworkInterfaceLan,
            "status",
            vec![],
        ),
        (
            "network_wan_status",
            ReviewedObject::NetworkInterfaceWan,
            "status",
            vec![],
        ),
        (
            "network_interface_status",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "network_interfaces",
            ReviewedObject::NetworkInterface,
            "dump",
            vec![],
        ),
        (
            "wireless_devices",
            ReviewedObject::Iwinfo,
            "devices",
            vec![],
        ),
        (
            "service_status",
            ReviewedObject::Service,
            "list",
            vec![
                ("name", ParameterKind::String),
                ("verbose", ParameterKind::Boolean),
            ],
        ),
        (
            "service_status_list",
            ReviewedObject::Service,
            "list",
            vec![("verbose", ParameterKind::Boolean)],
        ),
        (
            "wireless_radio_info",
            ReviewedObject::Iwinfo,
            "info",
            vec![("device", ParameterKind::String)],
        ),
        (
            "service_logd_status",
            ReviewedObject::Service,
            "list",
            vec![
                ("name", ParameterKind::String),
                ("verbose", ParameterKind::Boolean),
            ],
        ),
        (
            "service_sysntpd_status",
            ReviewedObject::Service,
            "list",
            vec![
                ("name", ParameterKind::String),
                ("verbose", ParameterKind::Boolean),
            ],
        ),
        (
            "diagnostics_watchdog_status",
            ReviewedObject::System,
            "watchdog",
            vec![],
        ),
    ];
    expectations.extend(
        [
            "system_timeserver_configuration",
            "network_device_configuration",
            "network_bridge_vlan_configuration",
            "network_route_v4_configuration",
            "network_route_v6_configuration",
            "network_rule_v4_configuration",
            "network_rule_v6_configuration",
            "firewall_zone_configuration",
            "firewall_forwarding_configuration",
            "firewall_rule_configuration",
            "firewall_redirect_configuration",
            "firewall_nat_configuration",
            "dhcp_pool_configuration",
            "dhcp_host_configuration",
            "dhcp_domain_configuration",
            "dhcp_cname_configuration",
            "storage_global_configuration",
            "storage_swap_configuration",
        ]
        .into_iter()
        .map(|name| {
            (
                name,
                ReviewedObject::Uci,
                "get",
                vec![
                    ("config", ParameterKind::String),
                    ("type", ParameterKind::String),
                ],
            )
        }),
    );
    assert_eq!(catalog.operations().len(), expectations.len() + 2);
    let mut response_ids = BTreeSet::new();
    for (name, action, capability, response) in [
        (
            "packages_apk_installed",
            Action::ApkInstalledPage {},
            CapabilityRequirement::ApkInstalledQuery {},
            "packages_apk_installed.v1",
        ),
        (
            "packages_opkg_status",
            Action::OpkgStatusPage {},
            CapabilityRequirement::OpkgRootStatusFile {},
            "packages_opkg_status.v1",
        ),
    ] {
        let operation = catalog.get(name).unwrap();
        assert_eq!(
            serde_json::to_value(&operation.action).unwrap(),
            serde_json::to_value(action).unwrap()
        );
        assert_eq!(operation.capability, capability);
        assert_eq!(operation.capability.probe_object(), None);
        assert_eq!(operation.capability.response_contract(), Some(response));
        assert!(response_ids.insert(response));
        assert_eq!(
            Policy::default().authorize(operation),
            Err(CoreError::PermissionDenied)
        );
    }
    for (name, object, method, arguments) in expectations {
        let operation = catalog.get(name).unwrap();
        assert_eq!(
            operation.capability,
            CapabilityRequirement::UbusMethod {
                object: object.as_str().into(),
                method: method.into(),
                arguments: arguments
                    .into_iter()
                    .map(|(key, kind)| (key.into(), kind))
                    .collect(),
                response_contract: format!(
                    "{name}.v{}",
                    if matches!(
                        name,
                        "network_interface_status"
                            | "storage_mounts"
                            | "storage_block_devices"
                            | "network_interface_configuration"
                            | "dhcp_dnsmasq_configuration"
                    ) {
                        2
                    } else {
                        1
                    }
                ),
            }
        );
        assert_eq!(operation.capability.probe_object(), Some(object));
        assert!(response_ids.insert(operation.capability.response_contract().unwrap()));
        assert_eq!(
            Policy::default().authorize(operation),
            Err(CoreError::PermissionDenied)
        );
        assert_eq!(
            operation.capability.evaluate(
                &CapabilityObservation::Unknown(UnknownReason::ProbeUnavailable),
                None
            ),
            Verdict::Unknown(UnknownReason::ProbeUnavailable)
        );
    }
}

#[test]
fn actual_builtin_objects_and_closed_probe_enum_match_the_architecture_registry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry: toml::Value = toml::from_str(
        &fs::read_to_string(root.join("architecture/capability-probes.toml")).unwrap(),
    )
    .unwrap();
    let operations = openwrt_mcp_features::builtins();
    assert_eq!(operations.len(), 51);
    let mut objects = BTreeSet::new();
    for operation in &operations {
        if matches!(operation.action, Action::OpkgStatusPage {}) {
            assert!(matches!(
                operation.capability,
                CapabilityRequirement::OpkgRootStatusFile {}
            ));
            continue;
        }
        if matches!(operation.action, Action::ApkInstalledPage {}) {
            assert!(matches!(
                operation.capability,
                CapabilityRequirement::ApkInstalledQuery {}
            ));
            continue;
        }
        let Action::Ubus { object, .. } = &operation.action else {
            panic!("the v5 builtins must use checked ubus reads");
        };
        objects.insert(object.as_str());
    }
    let recorded: BTreeSet<&str> = registry["probes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|probe| probe["object"].as_str().unwrap())
        .collect();
    let reviewed: BTreeSet<&str> = ReviewedObject::ALL
        .into_iter()
        .map(ReviewedObject::as_str)
        .collect();
    assert_eq!(recorded, reviewed);
    assert_eq!(objects, recorded);
    assert_eq!(
        registry["runtime"]["operation_metadata"].as_str(),
        Some("required_ubus_method_signature_response_contract")
    );
    assert_eq!(
        registry["runtime"]["process_capability"].as_str(),
        Some("unverified_blocked")
    );
}

#[test]
fn empty_watchdog_read_ignores_advertised_setters_and_never_adds_them() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let operation = catalog.get("diagnostics_watchdog_status").unwrap();
    let prepared = operation.prepare(&json!({})).unwrap();
    let observation = CapabilityObservation::Ubus(ObjectObservation {
        object: ReviewedObject::System,
        methods: BTreeMap::from([(
            "watchdog".into(),
            MethodSignature {
                arguments: BTreeMap::from([
                    ("stop".into(), UbusArgumentType::Boolean),
                    ("timeout".into(), UbusArgumentType::Integer),
                    ("futureSetter".into(), UbusArgumentType::Unknown),
                ]),
            },
        )]),
    });
    assert_eq!(
        operation.capability.evaluate(&observation, Some(&prepared)),
        Verdict::Compatible
    );
    assert_eq!(
        operation.capability.evaluate(&observation, None),
        Verdict::Compatible
    );
    assert!(operation.prepare(&json!({"stop":true})).is_err());
    let mut changed = operation.clone();
    let Action::Ubus { arguments, .. } = &mut changed.action else {
        unreachable!()
    };
    arguments.insert("stop".into(), json!(true));
    assert_eq!(
        Catalog::with_builtins(vec![changed], vec![]).unwrap_err(),
        CoreError::InvalidDefinition
    );
}

#[test]
fn interface_v2_requires_dump_and_does_not_invent_a_transmitted_selector() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let operation = catalog.get("network_interface_status").unwrap();
    let observation = CapabilityObservation::Ubus(ObjectObservation {
        object: ReviewedObject::NetworkInterface,
        methods: BTreeMap::from([(
            "status".into(),
            MethodSignature {
                arguments: BTreeMap::new(),
            },
        )]),
    });
    let action = operation.prepare(&json!({"interface":"loopback"})).unwrap();
    assert_eq!(
        operation.capability.evaluate(&observation, Some(&action)),
        Verdict::Unknown(UnknownReason::NotObservedOrHidden)
    );
    assert_eq!(
        operation.capability.evaluate(&observation, None),
        Verdict::Unknown(UnknownReason::NotObservedOrHidden)
    );
    let dump = CapabilityObservation::Ubus(ObjectObservation {
        object: ReviewedObject::NetworkInterface,
        methods: BTreeMap::from([(
            "dump".into(),
            MethodSignature {
                arguments: BTreeMap::new(),
            },
        )]),
    });
    assert_eq!(
        operation.capability.evaluate(&dump, Some(&action)),
        Verdict::Compatible
    );
    assert_eq!(
        operation.capability.evaluate(&dump, None),
        Verdict::Compatible
    );
}

#[test]
fn compatibility_evidence_only_names_actual_installed_builtin_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let evidence: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("compatibility/evidence.toml")).unwrap())
            .unwrap();
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    for record in evidence["records"].as_array().unwrap() {
        for operation in record["operations"].as_array().unwrap() {
            assert!(
                catalog.get(operation.as_str().unwrap()).is_some(),
                "evidence names an operation absent from the actual catalog"
            );
        }
    }
    // The empty initial manifest is not evidence of operation acceptance.
    assert!(catalog.get("unimplemented_fixture_operation").is_none());
}
