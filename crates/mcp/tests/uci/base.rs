use super::super::{
    Category, PRIVATE, PreparedAction, ReadContract, check_read_contract, dhcp_tools,
    firewall_tools, network_tools, storage_tools, system_tools,
};
use serde_json::{Value, json};

#[tokio::test]
async fn every_base_uci_family_has_fixed_category_scoped_mcp_behavior_and_safe_audit() {
    for (name, category, config, kind, scalar, option) in [
        (
            "system_timeserver_configuration",
            Category::System,
            "system",
            "timeserver",
            "interface",
            Some("server"),
        ),
        (
            "network_device_configuration",
            Category::Network,
            "network",
            "device",
            "name",
            Some("ports"),
        ),
        (
            "network_bridge_vlan_configuration",
            Category::Network,
            "network",
            "bridge-vlan",
            "device",
            Some("ports"),
        ),
        (
            "network_route_v4_configuration",
            Category::Network,
            "network",
            "route",
            "target",
            None,
        ),
        (
            "network_route_v6_configuration",
            Category::Network,
            "network",
            "route6",
            "target",
            None,
        ),
        (
            "network_rule_v4_configuration",
            Category::Network,
            "network",
            "rule",
            "src",
            None,
        ),
        (
            "network_rule_v6_configuration",
            Category::Network,
            "network",
            "rule6",
            "src",
            None,
        ),
        (
            "firewall_zone_configuration",
            Category::Firewall,
            "firewall",
            "zone",
            "name",
            Some("network"),
        ),
        (
            "firewall_forwarding_configuration",
            Category::Firewall,
            "firewall",
            "forwarding",
            "src",
            None,
        ),
        (
            "firewall_rule_configuration",
            Category::Firewall,
            "firewall",
            "rule",
            "target",
            Some("proto"),
        ),
        (
            "firewall_redirect_configuration",
            Category::Firewall,
            "firewall",
            "redirect",
            "target",
            Some("proto"),
        ),
        (
            "firewall_nat_configuration",
            Category::Firewall,
            "firewall",
            "nat",
            "target",
            Some("proto"),
        ),
        (
            "dhcp_pool_configuration",
            Category::DhcpDns,
            "dhcp",
            "dhcp",
            "interface",
            Some("dns"),
        ),
        (
            "dhcp_host_configuration",
            Category::DhcpDns,
            "dhcp",
            "host",
            "name",
            Some("mac"),
        ),
        (
            "dhcp_domain_configuration",
            Category::DhcpDns,
            "dhcp",
            "domain",
            "ip",
            Some("name"),
        ),
        (
            "dhcp_cname_configuration",
            Category::DhcpDns,
            "dhcp",
            "cname",
            "target",
            Some("cname"),
        ),
        (
            "storage_global_configuration",
            Category::Storage,
            "fstab",
            "global",
            "delay_root",
            None,
        ),
        (
            "storage_swap_configuration",
            Category::Storage,
            "fstab",
            "swap",
            "device",
            None,
        ),
    ] {
        let visible = match category {
            Category::System => system_tools(),
            Category::Network => network_tools(),
            Category::Firewall => firewall_tools(),
            Category::DhcpDns => dhcp_tools(),
            Category::Storage => storage_tools(),
            _ => panic!(),
        };
        let mut raw = json!({".type":kind,".anonymous":false,".index":42});
        for excluded in [
            ".name",
            "password",
            "key",
            "options",
            "extra",
            "extra_src",
            "extra_dest",
            "command",
            "script",
            "dhcp_option",
            "extraconftext",
            "private_key",
            "ssid",
        ] {
            raw[excluded] = json!(PRIVATE);
        }
        raw[scalar] = json!("fixture");
        let mut clean =
            json!({"section":"synthetic","section_type":kind,"anonymous":false,"index":42});
        clean[scalar] = json!("fixture");
        let mut responses = vec![
            (json!({"values":{}}), json!({"items":[]})),
            (
                json!({"values":{"synthetic":raw.clone()}}),
                json!({"items":[clean.clone()]}),
            ),
        ];
        if let Some(field) = option {
            for (input, representation, values) in [
                (json!("one two"), "string", json!(["one two"])),
                (
                    json!(["two", "one", "one", ""]),
                    "list",
                    json!(["two", "one", "one", ""]),
                ),
                (json!([]), "list", json!([])),
            ] {
                let mut source = raw.clone();
                source[field] = input;
                let mut expected = clean.clone();
                expected[field] = json!({"kind":representation,"values":values});
                responses.push((
                    json!({"values":{"synthetic":source}}),
                    json!({"items":[expected]}),
                ));
            }
        }
        let mut malformed = raw.clone();
        malformed[scalar] = json!(["not scalar"]);
        let mut wrong = raw.clone();
        wrong[".type"] = json!("wrong");
        let oversized: serde_json::Map<String, Value> = (0..129)
            .map(|i| (format!("n{i:03}"), raw.clone()))
            .collect();
        let mut invalid_outputs = vec![
            (json!({"values":{},"error":false}), "invalid_output"),
            (
                json!({"values":{"first":raw.clone(),"late":malformed}}),
                "invalid_output",
            ),
            (json!({"values":{"synthetic":wrong}}), "invalid_output"),
            (json!({"values":oversized}), "output_limit"),
        ];
        if let Some(field) = option {
            let mut malformed = raw.clone();
            malformed[field] = json!(["fine", null]);
            invalid_outputs.push((
                json!({"values":{"first":raw.clone(),"late":malformed}}),
                "invalid_output",
            ));
            let mut huge = raw.clone();
            huge[field] = json!(vec![""; 128]);
            invalid_outputs.push((
                json!({"values":{"first":huge.clone(),"late":huge}}),
                "output_limit",
            ));
        }
        check_read_contract(ReadContract {
            name,
            category,
            visible,
            arguments: json!({}),
            action: PreparedAction::Ubus {
                object: "uci".into(),
                method: "get".into(),
                arguments: json!({"config":config,"type":kind}),
            },
            invalid_arguments: vec![
                json!({"config":"wireless"}),
                json!({"type":"wifi-iface"}),
                json!({"ubus_rpc_session":PRIVATE}),
                json!({"method":"set"}),
                json!({"execute":true}),
                json!({"section":"lan3"}),
            ],
            responses,
            invalid_outputs,
        })
        .await;
    }
}
