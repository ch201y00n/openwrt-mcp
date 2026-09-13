//! Independent exact UCI field contracts; synthetic inputs only.
use super::{Fixture, operation, project};
use openwrt_mcp_core::{
    Category, Collection, CoreError, OutputMode, Presence, ScalarKind, TypedProjection,
};
use serde_json::{Value, json};

struct Recipe {
    name: &'static str,
    response: &'static str,
    category: Category,
    config: &'static str,
    section_type: &'static str,
    options: &'static [(&'static str, usize)],
}

fn recipes() -> [Recipe; 6] {
    [
        Recipe {
            name: "system_configuration",
            response: "system_configuration.v1",
            category: Category::System,
            config: "system",
            section_type: "system",
            options: &[("hostname", 256), ("timezone", 256), ("zonename", 256)],
        },
        Recipe {
            name: "network_interface_configuration",
            response: "network_interface_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "interface",
            options: &[
                ("proto", 64),
                ("device", 256),
                ("mtu", 32),
                ("metric", 32),
                ("auto", 8),
                ("defaultroute", 8),
                ("peerdns", 8),
                ("delegate", 8),
                ("ip4table", 64),
                ("ip6table", 64),
            ],
        },
        Recipe {
            name: "wireless_radio_configuration",
            response: "wireless_radio_configuration.v1",
            category: Category::Wireless,
            config: "wireless",
            section_type: "wifi-device",
            options: &[
                ("type", 64),
                ("path", 256),
                ("macaddr", 17),
                ("disabled", 8),
                ("country", 8),
                ("channel", 32),
                ("htmode", 32),
                ("band", 16),
                ("txpower", 32),
            ],
        },
        Recipe {
            name: "firewall_defaults_configuration",
            response: "firewall_defaults_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "defaults",
            options: &[
                ("input", 32),
                ("output", 32),
                ("forward", 32),
                ("synflood_protect", 8),
                ("drop_invalid", 8),
                ("flow_offloading", 8),
                ("flow_offloading_hw", 8),
                ("disable_ipv6", 8),
            ],
        },
        Recipe {
            name: "dhcp_dnsmasq_configuration",
            response: "dhcp_dnsmasq_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "dnsmasq",
            options: &[
                ("domain", 256),
                ("local", 256),
                ("port", 32),
                ("cachesize", 32),
                ("expandhosts", 8),
                ("domainneeded", 8),
                ("boguspriv", 8),
                ("rebind_protection", 8),
                ("noresolv", 8),
                ("localservice", 8),
                ("authoritative", 8),
                ("strictorder", 8),
                ("logqueries", 8),
            ],
        },
        Recipe {
            name: "storage_mount_configuration",
            response: "storage_mount_configuration.v1",
            category: Category::Storage,
            config: "fstab",
            section_type: "mount",
            options: &[
                ("device", 1024),
                ("uuid", 256),
                ("label", 256),
                ("target", 1024),
                ("fstype", 64),
                ("enabled", 8),
                ("enabled_fsck", 8),
            ],
        },
    ]
}

fn row(recipe: &Recipe) -> Value {
    json!({".type":recipe.section_type,".anonymous":true,".index":42})
}

pub(super) fn fixtures() -> Vec<Fixture> {
    recipes().into_iter().map(|recipe| {
        let mut raw = row(&recipe);
        let mut clean = json!({"section":"cfg_fixture","section_type":recipe.section_type,"anonymous":true,"index":42});
        for (name, _) in recipe.options { raw[name] = json!("1"); clean[name] = json!("1"); }
        for name in [".name", "password", "key", "options", "ssid", "command", "data", "server", "ipaddr", "ifname", "dns"] {
            raw[name] = json!({"value":"synthetic-secret"});
        }
        Fixture { name: recipe.name, response: recipe.response, category: recipe.category,
            input: json!({}), object: "uci", method: "get", arguments: json!({"config":recipe.config,"type":recipe.section_type}),
            source: json!({"values":{"cfg_fixture":raw},"other":"synthetic-secret"}), expected: json!({"items":[clean]}) }
    }).collect()
}

fn observe(recipe: &Recipe, value: Value) -> Result<Value, CoreError> {
    project(recipe.name, &json!({}), &value)
}

#[test]
fn every_uci_option_is_exact_optional_bounded_text_and_no_extra_field_is_declared() {
    for recipe in recipes() {
        let operation = operation(recipe.name);
        assert!(operation.parameters.is_empty());
        let OutputMode::Typed(projection) = operation.output_mode else {
            panic!()
        };
        let TypedProjection::Collection {
            collection: Collection::ObjectEntries { record, .. },
            ..
        } = *projection
        else {
            panic!()
        };
        assert_eq!(record.fields.len(), recipe.options.len() + 3);
        assert!(record.collections.is_empty());
        assert_eq!(
            observe(&recipe, json!({"values":{}})).unwrap(),
            json!({"items":[]})
        );
        assert_eq!(
            observe(&recipe, json!({"values":{"n":row(&recipe)}})).unwrap(),
            json!({"items":[{"section":"n","section_type":recipe.section_type,"anonymous":true,"index":42}]})
        );
        for (name, max) in recipe.options {
            let field = record.fields.iter().find(|f| f.name == *name).unwrap();
            assert_eq!(field.source, format!("/{name}"));
            assert_eq!(field.presence, Presence::Optional);
            assert_eq!(field.value, ScalarKind::Text { max_bytes: *max });
            for text in [
                String::new(),
                "0".into(),
                "1".into(),
                "x".repeat(*max),
                "é".repeat(max / 2),
            ] {
                let mut raw = row(&recipe);
                raw[name] = json!(text);
                assert_eq!(
                    observe(&recipe, json!({"values":{"n":raw}})).unwrap()["items"][0][name],
                    json!(text)
                );
            }
            for invalid in [
                json!("x".repeat(max + 1)),
                json!("é".repeat(max / 2 + 1)),
                json!("bad\u{0}text"),
                json!([]),
                json!(["0"]),
                json!({}),
                json!(null),
                json!(true),
                json!(0),
            ] {
                let mut raw = row(&recipe);
                raw[name] = invalid;
                assert_eq!(
                    observe(&recipe, json!({"values":{"n":raw}})),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
    }
}

#[test]
fn uci_section_metadata_wrong_types_root_errors_and_client_arguments_fail_closed() {
    for recipe in recipes() {
        for invalid in [
            json!(null),
            json!([]),
            json!({}),
            json!({"values":null}),
            json!({"values":[]}),
            json!({"values":{"n":null}}),
        ] {
            assert_eq!(observe(&recipe, invalid), Err(CoreError::InvalidOutput));
        }
        for value in [
            json!(null),
            json!(false),
            json!(0),
            json!("synthetic-secret"),
            json!({}),
        ] {
            assert_eq!(
                observe(&recipe, json!({"values":{},"error":value})),
                Err(CoreError::InvalidOutput)
            );
        }
        for name in [".type", ".anonymous", ".index"] {
            let mut raw = row(&recipe);
            raw.as_object_mut().unwrap().remove(name);
            assert_eq!(
                observe(&recipe, json!({"values":{"n":raw}})),
                Err(CoreError::InvalidOutput)
            );
        }
        for (name, values) in [
            (
                ".type",
                vec![
                    json!("other"),
                    json!(recipe.section_type.to_uppercase()),
                    json!(format!(" {}", recipe.section_type)),
                    json!(null),
                    json!(1),
                    json!([recipe.section_type]),
                ],
            ),
            (".anonymous", vec![json!("1"), json!(1), json!(null)]),
            (
                ".index",
                vec![
                    json!(-1),
                    json!(4294967296_u64),
                    json!("0"),
                    json!(1.5),
                    json!(null),
                ],
            ),
        ] {
            for value in values {
                let mut raw = row(&recipe);
                raw[name] = value;
                assert_eq!(
                    observe(&recipe, json!({"values":{"n":raw}})),
                    Err(CoreError::InvalidOutput)
                );
            }
        }
        let mut raw = row(&recipe);
        raw[".anonymous"] = json!(false);
        raw[".index"] = json!(u32::MAX);
        assert!(observe(&recipe, json!({"values":{"named":raw}})).is_ok());
        for args in [
            json!({"config":"wireless"}),
            json!({"type":"wifi-iface"}),
            json!({"section":"@interface[0]"}),
            json!({"option":"key"}),
            json!({"ubus_rpc_session":"fake"}),
            json!({"execute":true}),
            json!({"method":"set"}),
        ] {
            assert!(operation(recipe.name).prepare_invocation(&args).is_err());
        }
    }
}

#[test]
fn uci_map_identity_row_and_serialized_byte_limits_do_not_silently_truncate() {
    for recipe in recipes() {
        for key in ["".into(), "x".repeat(257), "bad\0key".into()] {
            let source = json!({"values":{key:row(&recipe)}});
            assert_eq!(observe(&recipe, source), Err(CoreError::InvalidOutput));
        }
        for key in ["x".repeat(256), "한글 ' / $()".into()] {
            assert_eq!(
                observe(&recipe, json!({"values":{key.clone():row(&recipe)}})).unwrap()["items"][0]
                    ["section"],
                key
            );
        }
        for count in [128, 129] {
            let values: serde_json::Map<String, Value> = (0..count)
                .map(|i| (format!("n{i:03}"), row(&recipe)))
                .collect();
            let result = observe(&recipe, json!({"values":values}));
            if count == 128 {
                assert_eq!(result.unwrap()["items"].as_array().unwrap().len(), 128);
            } else {
                assert_eq!(result, Err(CoreError::OutputLimit));
            }
        }
    }
    let recipe = recipes()
        .into_iter()
        .find(|r| r.name == "storage_mount_configuration")
        .unwrap();
    let mut raw = row(&recipe);
    raw["target"] = json!("\u{1}".repeat(1024));
    let values: serde_json::Map<String, Value> =
        (0..128).map(|i| (format!("n{i}"), raw.clone())).collect();
    assert_eq!(
        observe(&recipe, json!({"values":values})),
        Err(CoreError::OutputLimit)
    );
}
