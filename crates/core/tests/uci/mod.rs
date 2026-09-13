use openwrt_mcp_core::{
    Catalog, Category, CoreError, Operation, PreparedAction, uci::UciReadProfile,
};
use serde_json::{Value, json};

fn definition(profile: UciReadProfile) -> Value {
    json!({"name":"fixture_uci","description":"Synthetic closed configuration read.",
        "requirements":[{"category":profile.category(),"permission":"read"}],
        "parameters":{},"action":{"kind":"ubus","object":"uci","method":"get",
            "arguments":{"config":profile.config(),"type":profile.section_type()}},
        "capability":{"kind":"ubus_method","object":"uci","method":"get",
            "arguments":{"config":"string","type":"string"},"response_contract":"fixture_uci.v1"},
        "output_mode":{"typed":{"kind":"collection","reject_if_present":["/error"],
            "selection":{"kind":"all"},"collection":{"kind":"object_entries","source":"/values",
                "max_items":128,"key":{"name":"section","max_bytes":256},"record":{"fields":[
                    {"name":"section_type","source":"/.type","presence":"required",
                        "value":{"kind":"text_enum","max_bytes":16,"values":[profile.section_type()]}}
                ],"collections":[]}}}}})
}

fn reject(value: Value) {
    let operation: Operation = serde_json::from_value(value).unwrap();
    assert_eq!(
        operation.prepare_invocation(&json!({})).err(),
        Some(CoreError::InvalidDefinition)
    );
    assert_eq!(
        Catalog::with_builtins(vec![operation], vec![]).unwrap_err(),
        CoreError::InvalidDefinition
    );
}

#[test]
fn all_closed_uci_profiles_bind_exact_arguments_and_reject_even_privileged_custom_admission() {
    let expected = [
        ("system", "system"),
        ("network", "interface"),
        ("wireless", "wifi-device"),
        ("firewall", "defaults"),
        ("dhcp", "dnsmasq"),
        ("fstab", "mount"),
        ("system", "timeserver"),
        ("network", "device"),
        ("network", "bridge-vlan"),
        ("network", "route"),
        ("network", "route6"),
        ("network", "rule"),
        ("network", "rule6"),
        ("firewall", "zone"),
        ("firewall", "forwarding"),
        ("firewall", "rule"),
        ("firewall", "redirect"),
        ("firewall", "nat"),
        ("dhcp", "dhcp"),
        ("dhcp", "host"),
        ("dhcp", "domain"),
        ("dhcp", "cname"),
        ("fstab", "global"),
        ("fstab", "swap"),
    ];
    assert_eq!(
        UciReadProfile::ALL.map(|p| (p.config(), p.section_type())),
        expected
    );
    for profile in UciReadProfile::ALL {
        let category = match profile.config() {
            "system" => Category::System,
            "network" => Category::Network,
            "wireless" => Category::Wireless,
            "firewall" => Category::Firewall,
            "dhcp" => Category::DhcpDns,
            "fstab" => Category::Storage,
            _ => panic!("unreviewed config"),
        };
        assert_eq!(profile.category(), category);
        let mut value = definition(profile);
        let operation: Operation = serde_json::from_value(value.clone()).unwrap();
        Catalog::with_builtins(vec![operation.clone()], vec![]).unwrap();
        assert_eq!(
            operation.prepare(&json!({})).unwrap(),
            PreparedAction::Ubus {
                object: "uci".into(),
                method: "get".into(),
                arguments: json!({"config":profile.config(),"type":profile.section_type()})
            }
        );
        assert_eq!(
            Catalog::new(vec![operation]).unwrap_err(),
            CoreError::InvalidDefinition
        );
        value["requirements"].as_array_mut().unwrap().extend([
            json!({"category":"extensions","permission":"write"}),
            json!({"category":"extensions","permission":"execute"}),
        ]);
        assert_eq!(
            Catalog::new(vec![serde_json::from_value(value).unwrap()]).unwrap_err(),
            CoreError::InvalidDefinition
        );
        for method in [
            "state", "changes", "set", "add", "delete", "rename", "order", "commit", "revert",
            "apply", "confirm", "rollback", "configs", "get_all",
        ] {
            let mut bad = definition(profile);
            bad["action"]["method"] = json!(method);
            bad["capability"]["method"] = json!(method);
            reject(bad);
        }
        for config in ["../network", "/etc/config/network", "*", "other"] {
            let mut bad = definition(profile);
            bad["action"]["arguments"]["config"] = json!(config);
            reject(bad);
        }
        for field in ["config", "type"] {
            let mut bad = definition(profile);
            bad["action"]["arguments"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            bad["capability"]["arguments"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            reject(bad);
        }
        for argument in ["section", "option", "match", "ubus_rpc_session", "path"] {
            let mut bad = definition(profile);
            bad["action"]["arguments"][argument] = json!("fixture");
            bad["capability"]["arguments"][argument] = json!("string");
            reject(bad);
        }
        let mut bad = definition(profile);
        bad["parameters"] = json!({"config":{"kind":"string","required":true}});
        bad["action"]["arguments"]["config"] = json!("{config}");
        reject(bad);
        for requirements in [
            json!([{"category":profile.category(),"permission":"write"}]),
            json!([{"category":"extensions","permission":"read"},{"category":"extensions","permission":"write"},{"category":"extensions","permission":"execute"}]),
        ] {
            let mut bad = definition(profile);
            bad["requirements"] = requirements;
            reject(bad);
        }
    }
}

#[test]
fn closed_uci_recipe_cross_products_and_other_category_grants_never_expand_authority() {
    for profile in UciReadProfile::ALL {
        for other in UciReadProfile::ALL {
            if !UciReadProfile::ALL
                .iter()
                .any(|p| p.config() == profile.config() && p.section_type() == other.section_type())
            {
                let mut bad = definition(profile);
                bad["action"]["arguments"]["type"] = json!(other.section_type());
                bad["output_mode"]["typed"]["collection"]["record"]["fields"][0]["value"]["values"] =
                    json!([other.section_type()]);
                reject(bad);
            }
            if other.category() != profile.category() {
                let mut bad = definition(profile);
                bad["requirements"] = json!([
                    {"category":other.category(),"permission":"read"},
                    {"category":other.category(),"permission":"write"},
                    {"category":other.category(),"permission":"execute"}
                ]);
                reject(bad);
            }
        }
    }
}

#[test]
fn closed_uci_profiles_admit_only_bounded_terminal_text_option_collections() {
    for profile in UciReadProfile::ALL {
        let mut value = definition(profile);
        value["output_mode"]["typed"]["collection"]["record"]["collections"] = json!([
            {"name":"option","presence":"optional","collection":{"kind":"text_option","source":"/option","max_items":128,"max_bytes":1024}}
        ]);
        let op: Operation = serde_json::from_value(value.clone()).unwrap();
        Catalog::with_builtins(vec![op.clone()], vec![]).unwrap();
        assert_eq!(
            Catalog::new(vec![op]).unwrap_err(),
            CoreError::InvalidDefinition
        );
        for form in [
            json!({"kind":"row_array","source":"/option","max_items":1,"record":{"fields":[]}}),
            json!({"kind":"object_entries","source":"/option","max_items":1,"key":{"name":"key","max_bytes":8},"record":{"fields":[]}}),
            json!({"kind":"scalar_array","source":"/option","max_items":1,"name":"text","value":{"kind":"text","max_bytes":8},"unique":false}),
        ] {
            let mut bad = value.clone();
            bad["output_mode"]["typed"]["collection"]["record"]["collections"][0]["collection"] =
                form;
            reject(bad);
        }
    }
}

#[test]
fn uci_raw_untyped_unbounded_unguarded_or_wrong_section_contracts_are_not_admissible() {
    for profile in UciReadProfile::ALL {
        for mode in [json!("scalars"), json!("structured")] {
            let mut bad = definition(profile);
            bad["output_mode"] = mode;
            bad["output_fields"] = json!(["/values"]);
            reject(bad);
        }
        for guards in [json!([]), json!(["/other_error"])] {
            let mut bad = definition(profile);
            bad["output_mode"]["typed"]["reject_if_present"] = guards;
            reject(bad);
        }
        for source in ["", "/other", "/values/section"] {
            let mut bad = definition(profile);
            bad["output_mode"]["typed"]["collection"]["source"] = json!(source);
            reject(bad);
        }
        for count in [0, 129, 256] {
            let mut bad = definition(profile);
            bad["output_mode"]["typed"]["collection"]["max_items"] = json!(count);
            reject(bad);
        }
        for max in [0, 257, 1024] {
            let mut bad = definition(profile);
            bad["output_mode"]["typed"]["collection"]["key"]["max_bytes"] = json!(max);
            reject(bad);
        }
        for (field, value) in [
            ("presence", json!("optional")),
            ("source", json!("/type")),
            ("value", json!({"kind":"text","max_bytes":16})),
            (
                "value",
                json!({"kind":"text_enum","max_bytes":16,"values":["other"]}),
            ),
            (
                "value",
                json!({"kind":"text_enum","max_bytes":16,"values":[profile.section_type(),"other"]}),
            ),
        ] {
            let mut bad = definition(profile);
            bad["output_mode"]["typed"]["collection"]["record"]["fields"][0][field] = value;
            reject(bad);
        }
        let mut bad = definition(profile);
        bad["output_mode"]["typed"]["collection"]["record"]["fields"] = json!([]);
        reject(bad);
        let mut bad = definition(profile);
        bad["output_mode"]["typed"]["collection"]["record"]["collections"] = json!([
            {"name":"list","presence":"optional","collection":{"kind":"scalar_array","source":"/list","max_items":1,"name":"text","value":{"kind":"text","max_bytes":8},"unique":false}}]);
        reject(bad);
        let mut bad = definition(profile);
        bad["parameters"] = json!({"section":{"kind":"string","required":true}});
        bad["output_mode"]["typed"]["selection"] =
            json!({"kind":"exact_one","parameter":"section"});
        reject(bad);
    }
}
