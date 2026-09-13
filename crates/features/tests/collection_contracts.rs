//! Synthetic v6 response contracts, not device acceptance or complete visibility.
mod interface_ip;
mod uci;

use std::collections::BTreeSet;

use openwrt_mcp_core::{
    Access, CapabilityRequirement, Category, CoreError, Grant, Operation, OutputMode, Permission,
    Policy, PreparedAction, Requirement, SAFE_INTEGER_MAX,
};
use serde_json::{Value, json};

struct Fixture {
    name: &'static str,
    response: &'static str,
    category: Category,
    input: Value,
    object: &'static str,
    method: &'static str,
    arguments: Value,
    source: Value,
    expected: Value,
}

fn operation(name: &str) -> Operation {
    openwrt_mcp_features::catalog(vec![])
        .unwrap()
        .get(name)
        .unwrap()
        .clone()
}

fn project(name: &str, input: &Value, source: &Value) -> Result<Value, CoreError> {
    operation(name).prepare_invocation(input)?.project(source)
}

fn network_record(name: &str) -> Value {
    json!({"interface":name,"up":true,"pending":false,"available":true,"autostart":true,"dynamic":false})
}

fn fixtures() -> Vec<Fixture> {
    let mut network = network_record("guest");
    network["uptime"] = json!(123);
    network["proto"] = json!("static");
    network["device"] = json!("br-fixture");
    network["l3_device"] = json!("br-fixture");
    let approved = network.clone();
    network["ipv4-address"] = json!([{"address":"synthetic-private-address"}]);
    network["route"] = json!([{"target":"synthetic-private-route"}]);
    network["dns-server"] = json!(["synthetic-private-dns"]);
    network["data"] = json!({"password":"synthetic-secret"});
    network["config"] = json!({"credential":"synthetic-secret"});
    network["metric"] = json!(100);
    let networks =
        json!({"interface":[network,network_record("other")],"secret":"synthetic-secret"});
    let services = json!({
        "fixture":{
            "instances":{
                "one":{"running":true,"pid":42,"exit_code":0,
                    "command":["synthetic-private-command"],"env":{"TOKEN":"synthetic-secret"},
                    "data":{"password":"synthetic-secret"},"errors":["synthetic-secret"]},
                "two":{"running":false,"respawn":{"credential":"synthetic-secret"}}
            },
            "data":{"password":"synthetic-secret"},"bundle":"synthetic-private-path"
        },
        "vpn-fixture":{"data":{"password":"synthetic-secret"}}
    });
    let service = json!({"name":"fixture","instances":[{"name":"one","running":true,"pid":42,"exit_code":0},{"name":"two","running":false}]});
    let mut fixtures = vec![
        Fixture {
            name: "dhcp_v4_leases",
            response: "dhcp_v4_leases.v1",
            category: Category::DhcpDns,
            input: json!({}),
            object: "luci-rpc",
            method: "getDHCPLeases",
            arguments: json!({"family":4}),
            source: json!({"dhcp_leases":[{"ipaddr":"192.0.2.2","expires":false,"hostname":"fixture","interface":"guest","macaddr":"02:00:00:00:00:01","duid":"00040000","iaid":"00000001","secret":"synthetic-secret"},{"ipaddr":"192.0.2.2","expires":0}]}),
            expected: json!({"items":[{"ipaddr":"192.0.2.2","expires_seconds":false,"hostname":"fixture","interface":"guest","macaddr":"02:00:00:00:00:01","duid":"00040000","iaid":"00000001"},{"ipaddr":"192.0.2.2","expires_seconds":0}]}),
        },
        Fixture {
            name: "dhcp_v6_leases",
            response: "dhcp_v6_leases.v1",
            category: Category::DhcpDns,
            input: json!({}),
            object: "luci-rpc",
            method: "getDHCPLeases",
            arguments: json!({"family":6}),
            source: json!({"dhcp6_leases":[{"ip6addr":"2001:db8::2","expires":123,"duid":"00040000","ip6addrs":["2001:db8::2/128","2001:db8::2/128"],"password":"synthetic-secret"}]}),
            expected: json!({"items":[{"ip6addr":"2001:db8::2","expires_seconds":123,"duid":"00040000","ip6addrs":[{"address_prefix":"2001:db8::2/128"},{"address_prefix":"2001:db8::2/128"}]}]}),
        },
        Fixture {
            name: "storage_mounts",
            response: "storage_mounts.v2",
            category: Category::Storage,
            input: json!({}),
            object: "luci",
            method: "getMountPoints",
            arguments: json!({}),
            source: json!({"result":[{"device":"/dev/loop0","mount":"/mnt/test","size":9007199254740993_u64,"avail":64,"free":128,"options":{"password":"synthetic-secret"}}],"error_detail":"synthetic-secret"}),
            expected: json!({"items":[{"device":"/dev/loop0","mount":"/mnt/test","size_bytes":"9007199254740993","available_bytes":"64","free_bytes":"128"}]}),
        },
        Fixture {
            name: "storage_block_devices",
            response: "storage_block_devices.v2",
            category: Category::Storage,
            input: json!({}),
            object: "luci",
            method: "getBlockDevices",
            arguments: json!({}),
            source: json!({"loop0":{"dev":"/dev/loop0","size":u64::MAX,"type":"ext4","uuid":"fixture-uuid","label":"Fixture","version":"1.0","mount":"/mnt/test","password":"synthetic-secret"},"swap:/swapfile":{"dev":"/swapfile","size":1024,"type":"swap"}}),
            expected: json!({"items":[{"source_key":"loop0","device":"/dev/loop0","size_bytes":"18446744073709551615","filesystem":"ext4","uuid":"fixture-uuid","label":"Fixture","version":"1.0","mount":"/mnt/test"},{"source_key":"swap:/swapfile","device":"/swapfile","size_bytes":"1024","filesystem":"swap"}]}),
        },
        Fixture {
            name: "wireless_stations",
            response: "wireless_stations.v1",
            category: Category::Wireless,
            input: json!({"device":"phy0-ap0"}),
            object: "iwinfo",
            method: "assoclist",
            arguments: json!({"device":"phy0-ap0"}),
            source: json!({"results":[{"mac":"02:00:00:00:00:01","signal":-48,"noise":-95,"ssid":"synthetic-secret","rx":{"bytes":9007199254740993_u64,"key":"synthetic-secret"}}]}),
            expected: json!({"items":[{"mac":"02:00:00:00:00:01","signal_dbm":-48,"noise_dbm":-95,"rx_bytes":"9007199254740993"}]}),
        },
        Fixture {
            name: "wireless_station_status",
            response: "wireless_station_status.v1",
            category: Category::Wireless,
            input: json!({"device":"phy0-ap0","mac":"02:00:00:00:00:01"}),
            object: "iwinfo",
            method: "assoclist",
            arguments: json!({"device":"phy0-ap0"}),
            source: json!({"results":[{"mac":"02:00:00:00:00:02","signal":-55,"noise":-95},{"mac":"02:00:00:00:00:01","signal":-48,"noise":-95,"key":"synthetic-secret"}]}),
            expected: json!({"mac":"02:00:00:00:00:01","signal_dbm":-48,"noise_dbm":-95}),
        },
        Fixture {
            name: "wireless_countries",
            response: "wireless_countries.v1",
            category: Category::Wireless,
            input: json!({"device":"phy0"}),
            object: "iwinfo",
            method: "countrylist",
            arguments: json!({"device":"phy0"}),
            source: json!({"results":[{"iso3166":"KR","code":"KR","country":"South Korea","active":true,"config":{"key":"synthetic-secret"}}]}),
            expected: json!({"items":[{"iso3166":"KR","code":"KR","country":"South Korea","active":true}]}),
        },
        Fixture {
            name: "network_interfaces",
            response: "network_interfaces.v1",
            category: Category::Network,
            input: json!({}),
            object: "network.interface",
            method: "dump",
            arguments: json!({}),
            source: networks.clone(),
            expected: json!({"items":[approved,network_record("other")]}),
        },
        Fixture {
            name: "network_interface_status",
            response: "network_interface_status.v2",
            category: Category::Network,
            input: json!({"interface":"guest"}),
            object: "network.interface",
            method: "dump",
            arguments: json!({}),
            source: networks,
            expected: approved,
        },
        Fixture {
            name: "wireless_devices",
            response: "wireless_devices.v1",
            category: Category::Wireless,
            input: json!({}),
            object: "iwinfo",
            method: "devices",
            arguments: json!({}),
            source: json!({"devices":["phy0-ap0","phy1-ap0"],"config":{"key":"synthetic-secret"}}),
            expected: json!({"items":[{"device":"phy0-ap0"},{"device":"phy1-ap0"}]}),
        },
        Fixture {
            name: "service_status",
            response: "service_status.v1",
            category: Category::Services,
            input: json!({"name":"fixture"}),
            object: "service",
            method: "list",
            arguments: json!({"name":"fixture","verbose":false}),
            source: services.clone(),
            expected: service.clone(),
        },
        Fixture {
            name: "service_status_list",
            response: "service_status_list.v1",
            category: Category::Services,
            input: json!({}),
            object: "service",
            method: "list",
            arguments: json!({"verbose":false}),
            source: services,
            expected: json!({"items":[service,{"name":"vpn-fixture"}]}),
        },
    ];
    fixtures.extend(interface_ip::fixtures());
    fixtures.extend(uci::fixtures());
    fixtures
}

#[test]
fn actual_typed_catalog_and_response_ids_have_exactly_matching_exercised_fixtures() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let actual: BTreeSet<_> = catalog
        .operations()
        .iter()
        .filter(|operation| matches!(&operation.output_mode, OutputMode::Typed(_)))
        .map(|operation| {
            (
                operation.name.as_str(),
                operation.capability.response_contract().unwrap(),
            )
        })
        .collect();
    let cases = fixtures();
    let covered: BTreeSet<_> = cases
        .iter()
        .map(|fixture| (fixture.name, fixture.response))
        .collect();
    assert_eq!(actual, covered);
    assert_eq!(covered.len(), cases.len(), "duplicate fixture contract");
    for fixture in cases {
        let projected = project(fixture.name, &fixture.input, &fixture.source).unwrap();
        assert_eq!(projected, fixture.expected, "{}", fixture.name);
        assert!(
            !projected.to_string().contains("synthetic-"),
            "{}",
            fixture.name
        );
        assert!(projected.is_object());
    }
}

#[test]
fn dhcp_expiry_rows_and_optional_metadata_are_preserved_not_coerced_or_deduplicated() {
    for (name, key, row) in [
        (
            "dhcp_v4_leases",
            "dhcp_leases",
            json!({"ipaddr":"192.0.2.1","expires":false}),
        ),
        (
            "dhcp_v6_leases",
            "dhcp6_leases",
            json!({"ip6addr":"2001:db8::1","expires":false,"ip6addrs":[]}),
        ),
    ] {
        assert_eq!(
            project(name, &json!({}), &json!({key:[]})).unwrap(),
            json!({"items":[]})
        );
        let result = project(name, &json!({}), &json!({key:[row.clone(),row.clone()]})).unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), 2);
        assert_eq!(result["items"][0], result["items"][1]);
        assert!(result["items"][0].get("hostname").is_none());
        for expiry in [json!(false), json!(0), json!(u32::MAX)] {
            let mut source = row.clone();
            source["expires"] = expiry.clone();
            assert_eq!(
                project(name, &json!({}), &json!({key:[source]})).unwrap()["items"][0]["expires_seconds"],
                expiry
            );
        }
        for expiry in [
            json!(true),
            json!(null),
            json!(-1),
            json!(0.0),
            json!("0"),
            json!(u64::from(u32::MAX) + 1),
            json!({}),
        ] {
            let mut source = row.clone();
            source["expires"] = expiry;
            assert_eq!(
                project(name, &json!({}), &json!({key:[source]})),
                Err(CoreError::InvalidOutput)
            );
        }
        for field in row.as_object().unwrap().keys() {
            let mut source = row.clone();
            source.as_object_mut().unwrap().remove(field);
            assert_eq!(
                project(name, &json!({}), &json!({key:[source]})),
                Err(CoreError::InvalidOutput)
            );
        }
        for (field, max) in [
            ("hostname", 512),
            ("interface", 256),
            ("macaddr", 17),
            ("duid", 512),
            ("iaid", 64),
        ] {
            for size in [max, max + 1] {
                let mut source = row.clone();
                source[field] = json!("x".repeat(size));
                assert_eq!(
                    project(name, &json!({}), &json!({key:[source]})).is_ok(),
                    size == max
                );
            }
            for invalid in [
                json!(null),
                json!(42),
                json!({}),
                json!([]),
                json!("bad\u{0}"),
            ] {
                let mut source = row.clone();
                source[field] = invalid;
                assert_eq!(
                    project(name, &json!({}), &json!({key:[source]})),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
        for input in [
            json!({"family":0}),
            json!({"path":"synthetic-secret"}),
            json!({"ipaddr":"192.0.2.1"}),
        ] {
            assert!(operation(name).prepare_invocation(&input).is_err());
        }
    }
}

#[test]
fn dhcp_outer_nested_and_byte_limits_are_separate_and_never_silently_truncate() {
    for count in [128, 129] {
        let result = project(
            "dhcp_v4_leases",
            &json!({}),
            &json!({"dhcp_leases":vec![json!({"ipaddr":"192.0.2.1","expires":false});count]}),
        );
        if count == 128 {
            assert_eq!(result.unwrap()["items"].as_array().unwrap().len(), count);
        } else {
            assert_eq!(result, Err(CoreError::OutputLimit));
        }
    }
    for count in [10, 11] {
        let result = project(
            "dhcp_v6_leases",
            &json!({}),
            &json!({"dhcp6_leases":[{"ip6addr":"2001:db8::1","expires":false,"ip6addrs":vec!["2001:db8::1/128";count]}]}),
        );
        assert_eq!(result.is_ok(), count == 10);
    }
    for count in [128, 129] {
        // 128 lease rows + 128 address rows exactly exhaust the shared budget.
        let result = project(
            "dhcp_v6_leases",
            &json!({}),
            &json!({"dhcp6_leases":vec![json!({"ip6addr":"2001:db8::1","expires":false,"ip6addrs":["2001:db8::1/128"]});count]}),
        );
        assert_eq!(result.is_ok(), count == 128);
    }
    let mut rows =
        vec![json!({"ip6addr":"2001:db8::1","expires":false,"ip6addrs":["2001:db8::1/128"]}); 128];
    rows[127]["ip6addrs"] = json!(["2001:db8::1/128", "2001:db8::2/128"]);
    assert_eq!(
        project("dhcp_v6_leases", &json!({}), &json!({"dhcp6_leases":rows})),
        Err(CoreError::OutputLimit)
    );
    let rows = vec![
        json!({"ipaddr":"192.0.2.1","expires":0,"hostname":"x".repeat(512),"duid":"y".repeat(512)});
        128
    ];
    assert_eq!(
        project("dhcp_v4_leases", &json!({}), &json!({"dhcp_leases":rows})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn reviewed_luci_consumers_reject_mixed_error_success_values_without_error_disclosure() {
    for (name, source) in [
        ("dhcp_v4_leases", json!({"dhcp_leases":[]})),
        ("dhcp_v6_leases", json!({"dhcp6_leases":[]})),
        ("storage_mounts", json!({"result":[]})),
        ("storage_block_devices", json!({})),
    ] {
        for value in [
            json!(null),
            json!(false),
            json!("synthetic-secret"),
            json!({"secret":"synthetic-secret"}),
        ] {
            let mut mixed = source.clone();
            mixed["error"] = value;
            assert_eq!(
                project(name, &json!({}), &mixed),
                Err(CoreError::InvalidOutput)
            );
        }
    }
}

#[test]
fn storage_observations_reject_errors_ambiguous_identity_and_lossy_or_incomplete_fields() {
    let mount = json!({"device":"/dev/loop0","mount":"/mnt/한글","size":1024,"avail":0,"free":0});
    let block = json!({"dev":"/dev/loop0","size":1024,"type":"ext4"});
    assert_eq!(
        project("storage_mounts", &json!({}), &json!({"result":[]})).unwrap(),
        json!({"items":[]})
    );
    assert_eq!(
        project("storage_block_devices", &json!({}), &json!({})).unwrap(),
        json!({"items":[]})
    );
    assert_eq!(
        project(
            "storage_mounts",
            &json!({}),
            &json!({"result":[mount.clone(),mount.clone()]})
        ),
        Err(CoreError::InvalidOutput)
    );
    for (name, record, fields) in [
        (
            "storage_mounts",
            mount,
            vec!["device", "mount", "size", "avail", "free"],
        ),
        ("storage_block_devices", block, vec!["dev", "size", "type"]),
    ] {
        let wrap = |row: Value| {
            if name == "storage_mounts" {
                json!({"result":[row]})
            } else {
                json!({"loop0":row})
            }
        };
        for source in [json!(null), json!([]), json!({"error":"synthetic-secret"})] {
            assert_eq!(
                project(name, &json!({}), &source),
                Err(CoreError::InvalidOutput)
            );
        }
        for field in fields {
            let mut missing = record.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert_eq!(
                project(name, &json!({}), &wrap(missing)),
                Err(CoreError::InvalidOutput)
            );
            for invalid in [
                json!(null),
                json!(true),
                json!({"password":"synthetic-secret"}),
                json!([]),
            ] {
                let mut bad = record.clone();
                bad[field] = invalid;
                assert_eq!(
                    project(name, &json!({}), &wrap(bad)),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
        for size in [json!(-1), json!(1.0), json!("1024"), json!(1e30)] {
            let mut bad = record.clone();
            bad["size"] = size;
            assert_eq!(
                project(name, &json!({}), &wrap(bad)),
                Err(CoreError::InvalidOutput)
            );
        }
        let mut zero = record.clone();
        zero["size"] = json!(0);
        assert!(project(name, &json!({}), &wrap(zero)).is_ok());
    }
    for key in ["", "bad\u{0}key", &"x".repeat(1025)] {
        assert_eq!(
            project(
                "storage_block_devices",
                &json!({}),
                &json!({key:{"dev":"/dev/loop0","size":1,"type":"ext4"}})
            ),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn storage_rows_text_and_serialized_size_have_independent_hard_bounds() {
    for count in [128, 129] {
        let mounts: Vec<_> = (0..count).map(|i| json!({"mount":format!("/mnt/{i}"),"device":"/dev/loop0","size":1024,"avail":0,"free":1})).collect();
        let blocks: serde_json::Map<String, Value> = (0..count)
            .map(|i| {
                (
                    format!("loop{i}"),
                    json!({"dev":"/dev/loop0","size":1024,"type":"ext4"}),
                )
            })
            .collect();
        for (name, source) in [
            ("storage_mounts", json!({"result":mounts})),
            ("storage_block_devices", json!(blocks)),
        ] {
            let result = project(name, &json!({}), &source);
            if count == 128 {
                assert_eq!(result.unwrap()["items"].as_array().unwrap().len(), 128);
            } else {
                assert_eq!(result, Err(CoreError::OutputLimit));
            }
        }
    }
    for (field, max) in [
        ("dev", 1024),
        ("type", 64),
        ("uuid", 256),
        ("label", 256),
        ("version", 64),
        ("mount", 1024),
    ] {
        for size in [max, max + 1] {
            let mut row = json!({"dev":"/dev/loop0","size":1,"type":"ext4"});
            row[field] = json!("x".repeat(size));
            assert_eq!(
                project("storage_block_devices", &json!({}), &json!({"loop0":row})).is_ok(),
                size == max
            );
        }
    }
    let rows: Vec<_> = (0..80).map(|i| json!({"mount":format!("/mnt/{i}"),"device":"x".repeat(1024),"size":1,"avail":0,"free":0})).collect();
    assert_eq!(
        project("storage_mounts", &json!({}), &json!({"result":rows})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn reviewed_actions_and_capabilities_never_transmit_projection_only_arguments_or_setters() {
    for fixture in fixtures() {
        let operation = operation(fixture.name);
        let invocation = operation.prepare_invocation(&fixture.input).unwrap();
        assert_eq!(
            invocation.action(),
            &PreparedAction::Ubus {
                object: fixture.object.into(),
                method: fixture.method.into(),
                arguments: fixture.arguments.clone(),
            }
        );
        let CapabilityRequirement::UbusMethod {
            object,
            method,
            arguments,
            response_contract,
        } = &operation.capability
        else {
            panic!("typed read requires reviewed ubus capability")
        };
        assert_eq!(object, fixture.object);
        assert_eq!(method, fixture.method);
        assert_eq!(response_contract, fixture.response);
        assert_eq!(
            arguments.keys().collect::<BTreeSet<_>>(),
            fixture.arguments.as_object().unwrap().keys().collect()
        );
        assert!(operation.output_fields.is_empty());
        for (key, value) in [
            ("verbose", json!(true)),
            ("method", json!("set")),
            ("object", json!("uci")),
            ("execute", json!(true)),
            ("force", json!(true)),
        ] {
            let mut input = fixture.input.clone();
            input[key] = value;
            assert_eq!(
                operation.prepare_invocation(&input).err(),
                Some(CoreError::UnknownArgument),
                "{}",
                fixture.name
            );
        }
    }
}

#[test]
fn every_typed_read_is_default_denied_and_only_its_exact_read_category_authorizes_it() {
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
    for fixture in fixtures() {
        let operation = operation(fixture.name);
        assert_eq!(
            operation.requirements,
            vec![Requirement {
                category: fixture.category,
                permission: Permission::Read
            }]
        );
        assert_eq!(
            Policy::default().authorize(&operation),
            Err(CoreError::PermissionDenied)
        );
        for category in categories {
            for access in [Access::Deny, Access::Read, Access::ReadWrite] {
                for execute in [false, true] {
                    let policy = Policy {
                        categories: [(category, Grant { access, execute })].into(),
                        ..Policy::default()
                    };
                    assert_eq!(
                        policy.authorize(&operation).is_ok(),
                        category == fixture.category && access != Access::Deny
                    );
                }
            }
        }
        let mut requires_execution = operation.clone();
        requires_execution.requirements.push(Requirement {
            category: fixture.category,
            permission: Permission::Execute,
        });
        let policy = Policy {
            categories: [(
                fixture.category,
                Grant {
                    access: Access::ReadWrite,
                    execute: false,
                },
            )]
            .into(),
            ..Policy::default()
        };
        assert!(policy.authorize(&requires_execution).is_err());
    }
}

#[test]
fn generic_service_metadata_can_be_denied_without_removing_fixed_service_reads() {
    let policy = Policy {
        categories: [(
            Category::Services,
            Grant {
                access: Access::Read,
                execute: false,
            },
        )]
        .into(),
        deny_operations: ["service_status".into(), "service_status_list".into()].into(),
        ..Policy::default()
    };
    for name in ["service_status", "service_status_list"] {
        assert!(policy.authorize(&operation(name)).is_err());
    }
    for name in ["service_logd_status", "service_sysntpd_status"] {
        assert!(policy.authorize(&operation(name)).is_ok());
    }
}

#[test]
fn selectors_are_required_nonempty_bounded_utf8_strings_with_literal_exact_semantics() {
    for (name, parameter) in [
        ("network_interface_status", "interface"),
        ("service_status", "name"),
    ] {
        let operation = operation(name);
        let schema = operation.input_schema();
        assert_eq!(schema["properties"][parameter]["type"], json!("string"));
        assert_eq!(schema["properties"][parameter]["minLength"], json!(1));
        assert_eq!(schema["properties"][parameter]["maxLength"], json!(256));
        assert_eq!(schema["required"], json!([parameter]));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            operation.prepare_invocation(&json!({})).err(),
            Some(CoreError::MissingArgument)
        );
        for invalid in [
            json!(null),
            json!(true),
            json!(3),
            json!(3.5),
            json!([]),
            json!({}),
            json!(""),
            json!("bad\u{0}name"),
            json!("x".repeat(257)),
            json!("é".repeat(129)),
        ] {
            assert_eq!(
                operation
                    .prepare_invocation(&json!({(parameter):invalid}))
                    .err(),
                Some(CoreError::InvalidArguments)
            );
        }
        for literal in [
            "x".repeat(256),
            "é".repeat(128),
            "fixture'; $(synthetic-command) | * / ~ `synthetic`\n".into(),
        ] {
            let input = json!({(parameter):literal});
            let source = if parameter == "interface" {
                json!({"interface":[network_record(&literal)]})
            } else {
                json!({(literal.clone()):{}})
            };
            let projected = operation
                .prepare_invocation(&input)
                .unwrap()
                .project(&source)
                .unwrap();
            assert_eq!(projected[parameter], json!(literal));
        }
    }
}

#[test]
fn lists_have_no_selector_and_empty_valid_lists_are_not_synthesized_health_states() {
    for (name, source) in [
        ("network_interfaces", json!({"interface":[]})),
        ("wireless_devices", json!({"devices":[]})),
        ("service_status_list", json!({})),
    ] {
        assert_eq!(
            project(name, &json!({}), &source).unwrap(),
            json!({"items":[]})
        );
        assert_eq!(
            operation(name)
                .prepare_invocation(&json!({"name":"fixture"}))
                .err(),
            Some(CoreError::UnknownArgument)
        );
    }
    assert_eq!(
        project(
            "network_interface_status",
            &json!({"interface":"guest"}),
            &json!({"interface":[]})
        ),
        Err(CoreError::SelectionNotObserved)
    );
    assert_eq!(
        project("service_status", &json!({"name":"fixture"}), &json!({})),
        Err(CoreError::SelectionNotObserved)
    );
    assert_eq!(
        project(
            "service_status",
            &json!({"name":"fixture"}),
            &json!({"fixture":{}})
        )
        .unwrap(),
        json!({"name":"fixture"})
    );
    assert_eq!(
        project(
            "service_status",
            &json!({"name":"fixture"}),
            &json!({"fixture":{"instances":{}}})
        )
        .unwrap(),
        json!({"name":"fixture","instances":[]})
    );
}

#[test]
fn exact_selection_never_returns_another_resource_or_accepts_malformed_unselected_rows() {
    for wrong in ["guest-extra", "Guest", "other"] {
        assert_eq!(
            project(
                "network_interface_status",
                &json!({"interface":"guest"}),
                &json!({"interface":[network_record(wrong)]})
            ),
            Err(CoreError::SelectionNotObserved)
        );
        assert_eq!(
            project(
                "service_status",
                &json!({"name":"guest"}),
                &json!({(wrong):{}})
            ),
            Err(CoreError::SelectionNotObserved)
        );
    }
    for rows in [
        json!([network_record("guest"), network_record("guest")]),
        json!([network_record("guest"),{"interface":"other"}]),
        json!([
            network_record("guest"),
            network_record("other"),
            network_record("other")
        ]),
    ] {
        assert_eq!(
            project(
                "network_interface_status",
                &json!({"interface":"guest"}),
                &json!({"interface":rows})
            ),
            Err(CoreError::InvalidOutput)
        );
    }
    assert_eq!(
        project(
            "service_status",
            &json!({"name":"guest"}),
            &json!({"guest":{},"other":{"instances":{"one":{"running":"false"}}}})
        ),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn every_network_field_has_strict_type_presence_and_no_parent_subtree_escape() {
    for name in ["network_interfaces", "network_interface_status"] {
        let input = if name == "network_interface_status" {
            json!({"interface":"guest"})
        } else {
            json!({})
        };
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
            for invalid in [
                json!(null),
                json!({"password":"synthetic-secret"}),
                json!(["synthetic-secret"]),
            ] {
                let mut row = network_record("guest");
                row[field] = invalid;
                assert_eq!(
                    project(name, &input, &json!({"interface":[row]})),
                    Err(CoreError::InvalidOutput),
                    "{name}.{field}"
                );
            }
        }
        for required in [
            "interface",
            "up",
            "pending",
            "available",
            "autostart",
            "dynamic",
        ] {
            let mut row = network_record("guest");
            row.as_object_mut().unwrap().remove(required);
            assert_eq!(
                project(name, &input, &json!({"interface":[row]})),
                Err(CoreError::InvalidOutput)
            );
        }
        for source in [
            json!({}),
            json!(null),
            json!([]),
            json!({"interface":null}),
            json!({"interface":{}}),
            json!({"interface":[[]]}),
            json!({"interface":[null]}),
        ] {
            assert_eq!(
                project(name, &input, &source),
                Err(CoreError::InvalidOutput)
            );
        }
    }
}

#[test]
fn network_cardinality_and_text_limits_apply_even_when_the_first_row_matches() {
    for count in [128, 129] {
        let rows: Vec<_> = (0..count)
            .map(|index| network_record(&format!("interface{index}")))
            .collect();
        let source = json!({"interface":rows});
        for (name, input) in [
            ("network_interfaces", json!({})),
            (
                "network_interface_status",
                json!({"interface":"interface0"}),
            ),
        ] {
            assert_eq!(project(name, &input, &source).is_ok(), count == 128);
        }
    }
    for field in ["interface", "proto", "device", "l3_device"] {
        for (text, valid) in [
            ("é".repeat(128), true),
            ("é".repeat(128) + "x", false),
            ("x\0".into(), false),
        ] {
            let mut row = network_record("guest");
            row[field] = json!(text);
            assert_eq!(
                project(
                    "network_interfaces",
                    &json!({}),
                    &json!({"interface":[row]})
                )
                .is_ok(),
                valid
            );
        }
    }
    let mut empty = network_record("");
    empty["proto"] = json!("");
    assert_eq!(
        project(
            "network_interfaces",
            &json!({}),
            &json!({"interface":[empty]})
        ),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn scalar_device_rows_require_unique_nonempty_bounded_text_not_arbitrary_subtrees() {
    for source in [
        json!({}),
        json!(null),
        json!([]),
        json!({"devices":null}),
        json!({"devices":{}}),
    ] {
        assert_eq!(
            project("wireless_devices", &json!({}), &source),
            Err(CoreError::InvalidOutput)
        );
    }
    for values in [
        json!([""]),
        json!(["x\u{0}"]),
        json!(["dup", "dup"]),
        json!([1]),
        json!([true]),
        json!([null]),
        json!([{"password":"synthetic-secret"}]),
        json!([["synthetic-secret"]]),
        json!(["é".repeat(128) + "x"]),
    ] {
        assert_eq!(
            project("wireless_devices", &json!({}), &json!({"devices":values})),
            Err(CoreError::InvalidOutput)
        );
    }
    for count in [128, 129] {
        let devices: Vec<_> = (0..count).map(|index| format!("device{index}")).collect();
        assert_eq!(
            project("wireless_devices", &json!({}), &json!({"devices":devices})).is_ok(),
            count == 128
        );
    }
    assert!(
        project(
            "wireless_devices",
            &json!({}),
            &json!({"devices":["é".repeat(128)]})
        )
        .is_ok()
    );
}

#[test]
fn service_and_instance_identity_types_and_optional_containers_are_strict() {
    for name in ["service_status", "service_status_list"] {
        let input = if name == "service_status" {
            json!({"name":"fixture"})
        } else {
            json!({})
        };
        for source in [
            json!(null),
            json!([]),
            json!({"fixture":null}),
            json!({"fixture":[]}),
            json!({"fixture":{"instances":null}}),
            json!({"fixture":{"instances":[]}}),
            json!({"fixture":{"instances":{"one":null}}}),
            json!({"fixture":{"instances":{"one":{}}}}),
        ] {
            assert_eq!(
                project(name, &input, &source),
                Err(CoreError::InvalidOutput)
            );
        }
        for field in ["running", "pid", "exit_code"] {
            for invalid in [
                json!(null),
                json!({"password":"synthetic-secret"}),
                json!(["synthetic-secret"]),
                json!("1"),
            ] {
                let mut instance = json!({"running":true});
                instance[field] = invalid;
                assert_eq!(
                    project(
                        name,
                        &input,
                        &json!({"fixture":{"instances":{"one":instance}}})
                    ),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
    }
    for identity in [
        "".to_owned(),
        "bad\0name".into(),
        "x".repeat(257),
        "é".repeat(129),
    ] {
        assert_eq!(
            project(
                "service_status_list",
                &json!({}),
                &json!({(identity.clone()):{}})
            ),
            Err(CoreError::InvalidOutput)
        );
        assert_eq!(
            project(
                "service_status_list",
                &json!({}),
                &json!({"fixture":{"instances":{(identity):{"running":true}}}})
            ),
            Err(CoreError::InvalidOutput)
        );
    }
    let identity = "é".repeat(128);
    assert!(
        project(
            "service_status_list",
            &json!({}),
            &json!({(identity.clone()):{"instances":{(identity):{"running":true}}}})
        )
        .is_ok()
    );
}

fn instances(count: usize) -> Value {
    Value::Object(
        (0..count)
            .map(|index| (format!("instance{index}"), json!({"running":true})))
            .collect(),
    )
}

#[test]
fn service_collection_limits_and_aggregate_budget_are_shared_across_nested_records() {
    for count in [128, 129] {
        let services = Value::Object(
            (0..count)
                .map(|index| (format!("service{index}"), json!({})))
                .collect(),
        );
        assert_eq!(
            project("service_status_list", &json!({}), &services).is_ok(),
            count == 128
        );
        assert_eq!(
            project(
                "service_status_list",
                &json!({}),
                &json!({"fixture":{"instances":instances(count)}})
            )
            .is_ok(),
            count == 128
        );
    }
    for count in [127, 128] {
        let source = json!({"a":{"instances":instances(127)},"b":{"instances":instances(count)}});
        for (name, input) in [
            ("service_status_list", json!({})),
            ("service_status", json!({"name":"a"})),
        ] {
            assert_eq!(
                project(name, &input, &source).is_ok(),
                count == 127,
                "root rows + all nested rows must share 256"
            );
        }
    }
}

#[test]
fn reviewed_integer_ranges_reject_floats_coercion_and_precision_loss() {
    for (field, min) in [("uptime", 0), ("pid", 1), ("exit_code", 0)] {
        for (value, valid) in [
            (json!(min), true),
            (json!(SAFE_INTEGER_MAX), true),
            (json!(min - 1), false),
            (json!(SAFE_INTEGER_MAX + 1), false),
            (json!(1.0), false),
            (json!("1"), false),
            (json!(true), false),
        ] {
            let result = if field == "uptime" {
                let mut row = network_record("guest");
                row[field] = value;
                project(
                    "network_interfaces",
                    &json!({}),
                    &json!({"interface":[row]}),
                )
            } else {
                let mut instance = json!({"running":true});
                instance[field] = value;
                project(
                    "service_status_list",
                    &json!({}),
                    &json!({"fixture":{"instances":{"one":instance}}}),
                )
            };
            assert_eq!(result.is_ok(), valid, "{field}");
        }
    }
}

#[test]
fn individually_valid_fields_cannot_amplify_a_result_beyond_normalized_bytes() {
    let rows: Vec<_> = (0..128)
        .map(|index| {
            let mut row = network_record(&format!("interface{index}"));
            for field in ["proto", "device", "l3_device"] {
                row[field] = json!("é".repeat(128));
            }
            row
        })
        .collect();
    assert_eq!(
        project("network_interfaces", &json!({}), &json!({"interface":rows})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn the_actual_network_contract_accepts_exact_normalized_bytes_and_rejects_escaped_overflow() {
    let mut rows: Vec<_> = (0..128)
        .map(|index| {
            let mut row = network_record(&format!("interface{index}"));
            for field in ["proto", "device", "l3_device"] {
                row[field] = json!("");
            }
            row
        })
        .collect();
    let base = serde_json::to_vec(&json!({"items":rows})).unwrap().len();
    let mut remaining = 65_536usize.checked_sub(base).unwrap();
    for row in &mut rows {
        for field in ["proto", "device", "l3_device"] {
            let bytes = remaining.min(256);
            row[field] = json!("x".repeat(bytes));
            remaining -= bytes;
        }
    }
    assert_eq!(remaining, 0);
    let projected = project("network_interfaces", &json!({}), &json!({"interface":rows})).unwrap();
    assert_eq!(serde_json::to_vec(&projected).unwrap().len(), 65_536);
    assert_eq!(rows.last().unwrap()["l3_device"], json!(""));
    // One source byte needs two JSON bytes; neither per-field nor row count is
    // excessive, but the actual normalized encoding must reject the overflow.
    rows.last_mut().unwrap()["l3_device"] = json!("\"");
    assert_eq!(
        project("network_interfaces", &json!({}), &json!({"interface":rows})),
        Err(CoreError::OutputLimit)
    );
}

fn station(mac: &str) -> Value {
    json!({"mac":mac,"signal":-48,"noise":-95})
}

#[test]
fn station_identity_disclosure_can_be_denied_without_hiding_basic_wireless_reads() {
    let policy = Policy {
        categories: [(
            Category::Wireless,
            Grant {
                access: Access::Read,
                execute: false,
            },
        )]
        .into(),
        deny_operations: ["wireless_stations".into(), "wireless_station_status".into()].into(),
        ..Policy::default()
    };
    for name in ["wireless_stations", "wireless_station_status"] {
        assert!(policy.authorize(&operation(name)).is_err());
    }
    for name in [
        "wireless_devices",
        "wireless_radio_info",
        "wireless_countries",
    ] {
        assert!(policy.authorize(&operation(name)).is_ok());
    }
}

#[test]
fn passive_wireless_inputs_never_accept_actions_or_send_the_local_mac_selector() {
    for name in [
        "wireless_stations",
        "wireless_station_status",
        "wireless_countries",
    ] {
        let operation = operation(name);
        let mut input = json!({"device":"phy'; $(not-a-command)\n--scan"});
        if name == "wireless_station_status" {
            input["mac"] = json!("02:00:00:00:00:01");
            for invalid in [
                json!(""),
                json!("x".repeat(18)),
                json!("\0"),
                json!(1),
                json!(null),
            ] {
                let mut bad = input.clone();
                bad["mac"] = invalid;
                assert!(operation.prepare_invocation(&bad).is_err());
            }
            let mut missing = input.clone();
            missing.as_object_mut().unwrap().remove("mac");
            assert!(operation.prepare_invocation(&missing).is_err());
        }
        let invocation = operation.prepare_invocation(&input).unwrap();
        let PreparedAction::Ubus {
            object,
            method,
            arguments,
        } = invocation.action()
        else {
            panic!("not an ubus read")
        };
        assert_eq!(object, "iwinfo");
        assert_eq!(
            method,
            if name == "wireless_countries" {
                "countrylist"
            } else {
                "assoclist"
            }
        );
        assert_eq!(arguments, &json!({"device":input["device"]}));
        for forbidden in [
            "scan",
            "disconnect",
            "country",
            "method",
            "execute",
            "profile",
        ] {
            let mut invalid = input.clone();
            invalid[forbidden] = json!(true);
            assert!(operation.prepare_invocation(&invalid).is_err());
        }
        for invalid in [
            json!({}),
            json!({"device":true}),
            json!({"device":"x".repeat(1025)}),
        ] {
            assert!(operation.prepare_invocation(&invalid).is_err());
        }
    }
}

#[test]
fn station_exact_selection_checks_every_identity_and_unselected_required_field() {
    let mac = "02:00:00:00:00:AB";
    let input = json!({"device":"phy0-ap0","mac":mac});
    assert_eq!(
        project(
            "wireless_station_status",
            &input,
            &json!({"results":[station("02:00:00:00:00:ab")]})
        ),
        Err(CoreError::SelectionNotObserved)
    );
    for bad in [
        json!({}),
        json!(null),
        json!([]),
        station(""),
        station(&"x".repeat(18)),
        json!({"mac":"other","signal":-48,"noise":null}),
    ] {
        assert_eq!(
            project(
                "wireless_station_status",
                &input,
                &json!({"results":[station(mac),bad]})
            ),
            Err(CoreError::InvalidOutput)
        );
    }
    for rows in [
        json!([station(mac), station(mac)]),
        json!([station(mac), station("other"), station("other")]),
    ] {
        assert_eq!(
            project("wireless_station_status", &input, &json!({"results":rows})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn station_types_ranges_and_large_counters_are_not_coerced_or_rounded() {
    let input = json!({"device":"phy0-ap0"});
    for (source, min, max) in [
        ("signal", -128, 127),
        ("noise", -128, 127),
        ("signal_avg", -128, 127),
        ("inactive", 0, i32::MAX as i64),
        ("connected_time", 0, i32::MAX as i64),
        ("thr", 0, i32::MAX as i64),
    ] {
        for (value, valid) in [
            (json!(min), true),
            (json!(max), true),
            (json!(min - 1), false),
            (json!(max + 1), false),
            (json!(1.0), false),
            (json!("1"), false),
            (json!(null), false),
            (json!({}), false),
        ] {
            let mut row = station("02:00:00:00:00:01");
            row[source] = value;
            assert_eq!(
                project("wireless_stations", &input, &json!({"results":[row]})).is_ok(),
                valid,
                "{source}"
            );
        }
    }
    for source in ["authorized", "authenticated", "wme", "mfp"] {
        for (value, valid) in [
            (json!(false), true),
            (json!(true), true),
            (json!(0), false),
            (json!("false"), false),
            (json!(null), false),
        ] {
            let mut row = station("02:00:00:00:00:01");
            row[source] = value;
            assert_eq!(
                project("wireless_stations", &input, &json!({"results":[row]})).is_ok(),
                valid
            );
        }
    }
    for (direction, source, output) in [
        ("rx", "bytes", "rx_bytes"),
        ("tx", "bytes", "tx_bytes"),
        ("rx", "drop_misc", "rx_dropped"),
    ] {
        for (value, valid) in [
            (json!(0), true),
            (json!(9_007_199_254_740_993_u64), true),
            (json!(i64::MAX), true),
            (json!(-1), false),
            (json!(1.5), false),
            (json!("100"), false),
            (json!(null), false),
        ] {
            let mut row = station("02:00:00:00:00:01");
            row[direction] = json!({(source):value});
            let result = project("wireless_stations", &input, &json!({"results":[row]}));
            assert_eq!(result.is_ok(), valid);
            if valid {
                assert_eq!(
                    result.unwrap()["items"][0][output],
                    json!(value.to_string())
                );
            }
        }
    }
    for direction in ["rx", "tx"] {
        for source in ["rate", "mhz"] {
            for value in [
                json!(-1),
                json!(i64::from(i32::MAX) + 1),
                json!(1.0),
                json!(null),
            ] {
                let mut row = station("02:00:00:00:00:01");
                row[direction] = json!({(source):value});
                assert_eq!(
                    project("wireless_stations", &input, &json!({"results":[row]})),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
    }
}

#[test]
fn country_observations_require_typed_metadata_and_unique_bounded_identity() {
    let input = json!({"device":"phy0"});
    let clean = json!({"iso3166":"KR","code":"KR","country":"South Korea"});
    for field in ["iso3166", "code", "country"] {
        let mut row = clean.clone();
        row.as_object_mut().unwrap().remove(field);
        assert_eq!(
            project("wireless_countries", &input, &json!({"results":[row]})),
            Err(CoreError::InvalidOutput)
        );
        for invalid in [json!(null), json!(true), json!([]), json!({}), json!("x\0")] {
            let mut row = clean.clone();
            row[field] = invalid;
            assert_eq!(
                project("wireless_countries", &input, &json!({"results":[row]})),
                Err(CoreError::InvalidOutput)
            );
        }
    }
    for (field, limit) in [("iso3166", 2), ("code", 4), ("country", 256)] {
        for (size, valid) in [(limit, true), (limit + 1, false)] {
            let mut row = clean.clone();
            row[field] = json!("x".repeat(size));
            assert_eq!(
                project("wireless_countries", &input, &json!({"results":[row]})).is_ok(),
                valid
            );
        }
    }
    let mut empty = clean.clone();
    empty["iso3166"] = json!("");
    let mut wrong = clean.clone();
    wrong["active"] = json!(1);
    for rows in [
        json!([clean.clone(), clean]),
        json!([empty]),
        json!([wrong]),
    ] {
        assert_eq!(
            project("wireless_countries", &input, &json!({"results":rows})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn passive_wireless_empty_observations_are_not_synthesized_from_missing_results() {
    for name in [
        "wireless_stations",
        "wireless_station_status",
        "wireless_countries",
    ] {
        let input = if name == "wireless_station_status" {
            json!({"device":"phy0","mac":"02:00:00:00:00:01"})
        } else {
            json!({"device":"phy0"})
        };
        for malformed in [
            json!({}),
            json!(null),
            json!([]),
            json!({"results":null}),
            json!({"results":{}}),
        ] {
            assert_eq!(
                project(name, &input, &malformed),
                Err(CoreError::InvalidOutput)
            );
        }
        let empty = project(name, &input, &json!({"results":[]}));
        if name == "wireless_station_status" {
            assert_eq!(empty, Err(CoreError::SelectionNotObserved));
        } else {
            assert_eq!(empty.unwrap(), json!({"items":[]}));
        }
    }
}

#[test]
fn passive_wireless_cardinality_limits_apply_before_selecting_or_truncating() {
    for size in [128, 129] {
        let rows: Vec<_> = (0..size)
            .map(|index| station(&format!("02:00:00:00:00:{index:02X}")))
            .collect();
        for name in ["wireless_stations", "wireless_station_status"] {
            let input = if name == "wireless_station_status" {
                json!({"device":"phy0","mac":"02:00:00:00:00:00"})
            } else {
                json!({"device":"phy0"})
            };
            let result = project(name, &input, &json!({"results":rows}));
            assert_eq!(result.is_ok(), size == 128);
            if size == 129 {
                assert_eq!(result, Err(CoreError::OutputLimit));
            }
        }
    }
    for size in [256, 257] {
        let rows: Vec<_> = (0..size)
            .map(|index| {
                let code = format!(
                    "{}{}",
                    char::from(b'A' + (index / 26) as u8),
                    char::from(b'A' + (index % 26) as u8)
                );
                json!({"iso3166":code,"code":code,"country":"Fixture"})
            })
            .collect();
        let result = project(
            "wireless_countries",
            &json!({"device":"phy0"}),
            &json!({"results":rows}),
        );
        assert_eq!(result.is_ok(), size == 256);
        if size == 257 {
            assert_eq!(result, Err(CoreError::OutputLimit));
        }
    }
}
