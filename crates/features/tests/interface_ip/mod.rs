// Included by collection_contracts; not a separate Cargo integration target.
// Exact actions, capability IDs and the
// all-category access/execute matrix also exercise every fixture below.
use super::*;

pub(super) fn fixtures() -> Vec<Fixture> {
    [
        (
            "network_interface_addresses", "network_interface_addresses.v1", Category::Network,
            json!({"ipv4-address":[{"address":"192.0.2.1","mask":24,"ptpaddress":"192.0.2.2","preferred":0,"valid":4294967295u64,"class":"wan","secret":"synthetic-secret"}],"ipv6-address":[{"address":"2001:db8::1","mask":64}],"inactive":{"ipv4-address":[],"ipv6-address":[{"address":"2001:db8::2","mask":128}]}}),
            json!({"ipv4_addresses":[{"address":"192.0.2.1","mask":24,"ptpaddress":"192.0.2.2","preferred_seconds":0,"valid_seconds":4294967295u64,"class":"wan"}],"ipv6_addresses":[{"address":"2001:db8::1","mask":64}],"inactive_ipv4_addresses":[],"inactive_ipv6_addresses":[{"address":"2001:db8::2","mask":128}]}),
        ),
        (
            "network_interface_routes", "network_interface_routes.v1", Category::Network,
            json!({"route":[{"target":"0.0.0.0","mask":0,"nexthop":"192.0.2.1","source":"0.0.0.0/0","type":1,"proto":3,"mtu":1500,"metric":0,"table":4294967295u64,"valid":0,"secret":"synthetic-secret"}],"inactive":{"route":[{"target":"2001:db8::","mask":64,"nexthop":"::","source":"::/0"}]}}),
            json!({"routes":[{"target":"0.0.0.0","mask":0,"nexthop":"192.0.2.1","source":"0.0.0.0/0","type":1,"proto":3,"mtu":1500,"metric":0,"table":4294967295u64,"valid_seconds":0}],"inactive_routes":[{"target":"2001:db8::","mask":64,"nexthop":"::","source":"::/0"}]}),
        ),
        (
            "network_interface_neighbors", "network_interface_neighbors.v1", Category::Network,
            json!({"neighbors":[{"address":"192.0.2.2","mac":"02:00:00:00:00:01","proxy":1,"router":0,"secret":"synthetic-secret"},{"address":"2001:db8::2"}],"inactive":{"neighbors":[]}}),
            json!({"neighbors":[{"address":"192.0.2.2","mac":"02:00:00:00:00:01","proxy":1,"router":0},{"address":"2001:db8::2"}],"inactive_neighbors":[]}),
        ),
        (
            "dhcp_interface_dns", "dhcp_interface_dns.v1", Category::DhcpDns,
            json!({"dns-server":["192.0.2.53","2001:db8::53","192.0.2.53"],"dns-search":["example.invalid","example.invalid"],"inactive":{"dns-server":[],"dns-search":["inactive.invalid"]}}),
            json!({"servers":[{"address":"192.0.2.53"},{"address":"2001:db8::53"},{"address":"192.0.2.53"}],"search_domains":[{"domain":"example.invalid"},{"domain":"example.invalid"}],"inactive_servers":[],"inactive_search_domains":[{"domain":"inactive.invalid"}]}),
        ),
    ].into_iter().map(|(name,response,category,mut row,mut expected)| {
        row["interface"] = json!("guest"); row["up"] = json!(true);
        row["data"] = json!({"password":"synthetic-secret"});
        row["errors"] = json!([{"data":"synthetic-secret"}]);
        expected["interface"] = json!("guest"); expected["up"] = json!(true);
        Fixture { name, response, category, input:json!({"interface":"guest"}),object:"network.interface",method:"dump",arguments:json!({}),source:json!({"interface":[row,{"interface":"other","up":false}],"private":"synthetic-secret"}),expected }
    }).collect()
}

fn lists(name: &str) -> Vec<(&'static str, &'static str, Value)> {
    match name {
        "network_interface_addresses" => vec![
            (
                "/ipv4-address",
                "ipv4_addresses",
                json!({"address":"192.0.2.1","mask":24}),
            ),
            (
                "/ipv6-address",
                "ipv6_addresses",
                json!({"address":"2001:db8::1","mask":64}),
            ),
            (
                "/inactive/ipv4-address",
                "inactive_ipv4_addresses",
                json!({"address":"192.0.2.1","mask":24}),
            ),
            (
                "/inactive/ipv6-address",
                "inactive_ipv6_addresses",
                json!({"address":"2001:db8::1","mask":64}),
            ),
        ],
        "network_interface_routes" => vec![
            (
                "/route",
                "routes",
                json!({"target":"0.0.0.0","mask":0,"nexthop":"192.0.2.1","source":"0.0.0.0/0"}),
            ),
            (
                "/inactive/route",
                "inactive_routes",
                json!({"target":"::","mask":0,"nexthop":"::","source":"::/0"}),
            ),
        ],
        "network_interface_neighbors" => vec![
            ("/neighbors", "neighbors", json!({"address":"192.0.2.2"})),
            (
                "/inactive/neighbors",
                "inactive_neighbors",
                json!({"address":"2001:db8::2"}),
            ),
        ],
        "dhcp_interface_dns" => vec![
            ("/dns-server", "servers", json!("192.0.2.53")),
            ("/dns-search", "search_domains", json!("example.invalid")),
            (
                "/inactive/dns-server",
                "inactive_servers",
                json!("2001:db8::53"),
            ),
            (
                "/inactive/dns-search",
                "inactive_search_domains",
                json!("example.invalid"),
            ),
        ],
        _ => panic!("not an interface observation fixture"),
    }
}

fn set_list(row: &mut Value, path: &str, value: Value) {
    if let Some(key) = path.strip_prefix("/inactive/") {
        if row.get("inactive").is_none() {
            row["inactive"] = json!({});
        }
        row["inactive"][key] = value;
    } else {
        row[path.strip_prefix('/').unwrap()] = value;
    }
}

fn observe(name: &str, row: Value) -> Result<Value, CoreError> {
    project(
        name,
        &json!({"interface":"guest"}),
        &json!({"interface":[row]}),
    )
}

#[test]
fn interface_observation_presence_duplicates_and_all_unselected_rows_are_validated() {
    for fixture in fixtures() {
        let base = json!({"interface":"guest","up":false});
        assert_eq!(observe(fixture.name, base.clone()).unwrap(), base);
        for (path, output, item) in lists(fixture.name) {
            for entries in [vec![], vec![item.clone(), item.clone()]] {
                let mut row = base.clone();
                set_list(&mut row, path, json!(entries));
                let observed = observe(fixture.name, row).unwrap();
                assert_eq!(observed[output].as_array().unwrap().len(), entries.len());
                if !entries.is_empty() {
                    assert_eq!(observed[output][0], observed[output][1]);
                }
            }
            for bad in [
                json!(null),
                json!({}),
                json!(false),
                json!([null]),
                json!([item.clone(), null]),
            ] {
                let mut row = base.clone();
                set_list(&mut row, path, bad);
                assert_eq!(
                    observe(fixture.name, row.clone()),
                    Err(CoreError::InvalidOutput)
                );
                row["interface"] = json!("other");
                assert_eq!(
                    project(
                        fixture.name,
                        &fixture.input,
                        &json!({"interface":[base.clone(),row]})
                    ),
                    Err(CoreError::InvalidOutput)
                );
            }
            if let Some(fields) = item.as_object() {
                for field in fields.keys() {
                    let mut missing = item.clone();
                    missing.as_object_mut().unwrap().remove(field);
                    let mut row = base.clone();
                    set_list(&mut row, path, json!([missing]));
                    assert_eq!(observe(fixture.name, row), Err(CoreError::InvalidOutput));
                }
            }
        }
        for field in ["interface", "up"] {
            let mut row = base.clone();
            row.as_object_mut().unwrap().remove(field);
            assert_eq!(observe(fixture.name, row), Err(CoreError::InvalidOutput));
        }
        for bad in [
            json!(null),
            json!(0),
            json!(false),
            json!("synthetic-secret"),
        ] {
            assert_eq!(
                project(
                    fixture.name,
                    &fixture.input,
                    &json!({"interface":[base.clone()],"error":bad})
                ),
                Err(CoreError::InvalidOutput)
            );
        }
        assert_eq!(
            project(
                fixture.name,
                &fixture.input,
                &json!({"interface":[base.clone(),base.clone()]})
            ),
            Err(CoreError::InvalidOutput)
        );
        assert_eq!(
            project(fixture.name, &fixture.input, &json!({"interface":[]})),
            Err(CoreError::SelectionNotObserved)
        );
        let mut bad_parent = base.clone();
        bad_parent["inactive"] = json!(null);
        assert_eq!(
            observe(fixture.name, bad_parent),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn interface_observation_selectors_remain_exact_local_and_strictly_bounded() {
    for fixture in fixtures() {
        for name in ["guest; $() / ' 한글".into(), "x".repeat(256)] {
            let input = json!({"interface":name});
            let prepared = operation(fixture.name);
            let invocation = prepared.prepare_invocation(&input).unwrap();
            assert_eq!(
                invocation.action(),
                &PreparedAction::Ubus {
                    object: "network.interface".into(),
                    method: "dump".into(),
                    arguments: json!({})
                }
            );
            let row = json!({"interface":name,"up":false});
            assert_eq!(
                invocation
                    .project(&json!({"interface":[row.clone()]}))
                    .unwrap(),
                row
            );
        }
        for input in [
            json!({}),
            json!({"interface":null}),
            json!({"interface":0}),
            json!({"interface":""}),
            json!({"interface":"x".repeat(257)}),
            json!({"interface":"bad\u{0}"}),
            json!({"interface":"guest","path":"/data"}),
            json!({"interface":"guest","active":true}),
        ] {
            assert!(operation(fixture.name).prepare_invocation(&input).is_err());
        }
    }
}

#[test]
fn interface_observation_nested_limits_charge_all_lists_and_unselected_interfaces() {
    for fixture in fixtures() {
        let entries = lists(fixture.name);
        let base = json!({"interface":"guest","up":true});
        for (path, output, item) in &entries {
            let mut row = base.clone();
            set_list(&mut row, path, json!(vec![item.clone(); 128]));
            assert_eq!(
                observe(fixture.name, row.clone()).unwrap()[*output]
                    .as_array()
                    .unwrap()
                    .len(),
                128
            );
            set_list(&mut row, path, json!(vec![item.clone(); 129]));
            assert_eq!(
                observe(fixture.name, row.clone()),
                Err(CoreError::OutputLimit)
            );
            row["interface"] = json!("other");
            assert_eq!(
                project(
                    fixture.name,
                    &fixture.input,
                    &json!({"interface":[base.clone(),row]})
                ),
                Err(CoreError::OutputLimit)
            );
        }
        let mut row = base.clone();
        set_list(
            &mut row,
            entries[0].0,
            json!(vec![entries[0].2.clone(); 128]),
        );
        set_list(
            &mut row,
            entries[1].0,
            json!(vec![entries[1].2.clone(); 127]),
        );
        assert!(observe(fixture.name, row.clone()).is_ok()); // one interface + 255 nested rows
        set_list(
            &mut row,
            entries[1].0,
            json!(vec![entries[1].2.clone(); 128]),
        );
        assert_eq!(observe(fixture.name, row), Err(CoreError::OutputLimit));
        let mut rows: Vec<_> = (0..128)
            .map(|i| json!({"interface":format!("if{i}"),"up":false}))
            .collect();
        rows[0]["interface"] = json!("guest");
        assert!(
            project(
                fixture.name,
                &fixture.input,
                &json!({"interface":rows.clone()})
            )
            .is_ok()
        );
        rows.push(json!({"interface":"excess","up":false}));
        assert_eq!(
            project(fixture.name, &fixture.input, &json!({"interface":rows})),
            Err(CoreError::OutputLimit)
        );
    }
    let source = json!({"interface":"guest","up":true,"dns-search":vec!["\u{1}".repeat(256);128]});
    assert_eq!(
        observe("dhcp_interface_dns", source),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn interface_observation_numeric_types_and_text_bounds_are_not_coerced() {
    for (name, path, mut item, numeric, texts) in [
        (
            "network_interface_addresses",
            "/ipv4-address",
            json!({"address":"192.0.2.1","mask":24}),
            vec![
                ("mask", 32u64),
                ("preferred", u64::from(u32::MAX)),
                ("valid", u64::from(u32::MAX)),
            ],
            vec![("address", 15), ("ptpaddress", 15), ("class", 256)],
        ),
        (
            "network_interface_addresses",
            "/ipv6-address",
            json!({"address":"2001:db8::1","mask":64}),
            vec![("mask", 128u64)],
            vec![("address", 45), ("ptpaddress", 45)],
        ),
        (
            "network_interface_routes",
            "/route",
            json!({"target":"::","mask":0,"nexthop":"::","source":"::/0"}),
            vec![
                ("mask", 128u64),
                ("type", u64::from(u32::MAX)),
                ("proto", u64::from(u32::MAX)),
                ("mtu", u64::from(u32::MAX)),
                ("metric", u64::from(u32::MAX)),
                ("table", u64::from(u32::MAX)),
                ("valid", u64::from(u32::MAX)),
            ],
            vec![("target", 45), ("nexthop", 45), ("source", 49)],
        ),
        (
            "network_interface_neighbors",
            "/neighbors",
            json!({"address":"192.0.2.1"}),
            vec![
                ("proxy", u64::from(u32::MAX)),
                ("router", u64::from(u32::MAX)),
            ],
            vec![("address", 45), ("mac", 17)],
        ),
    ] {
        for (field, max) in numeric {
            for value in [
                json!(0),
                json!(max),
                json!(max + 1),
                json!(-1),
                json!(null),
                json!(true),
                json!(0.0),
                json!("0"),
                json!({}),
            ] {
                let valid = value == json!(0) || value == json!(max);
                let mut row = json!({"interface":"guest","up":true});
                let mut current = item.clone();
                current[field] = value;
                set_list(&mut row, path, json!([current]));
                assert_eq!(observe(name, row).is_ok(), valid, "{name}:{field}");
            }
        }
        for (field, max) in texts {
            for value in [
                json!("x".repeat(max)),
                json!("x".repeat(max + 1)),
                json!("\u{0}"),
                json!(null),
                json!(1),
                json!([]),
            ] {
                let valid = value == json!("x".repeat(max));
                let mut row = json!({"interface":"guest","up":true});
                let mut current = item.clone();
                current[field] = value;
                set_list(&mut row, path, json!([current]));
                assert_eq!(observe(name, row).is_ok(), valid, "{name}:{field}");
            }
        }
        item["private"] = json!("synthetic-secret");
        let mut row = json!({"interface":"guest","up":true});
        set_list(&mut row, path, json!([item]));
        assert!(
            !observe(name, row)
                .unwrap()
                .to_string()
                .contains("synthetic-secret")
        );
    }
    for (path, max) in [
        ("/dns-server", 45),
        ("/dns-search", 256),
        ("/inactive/dns-server", 45),
        ("/inactive/dns-search", 256),
    ] {
        for value in [
            json!("x".repeat(max)),
            json!("x".repeat(max + 1)),
            json!("\u{0}"),
            json!(null),
            json!(0),
            json!({}),
        ] {
            let valid = value == json!("x".repeat(max));
            let mut row = json!({"interface":"guest","up":true});
            set_list(&mut row, path, json!([value]));
            assert_eq!(observe("dhcp_interface_dns", row).is_ok(), valid);
        }
    }
}
