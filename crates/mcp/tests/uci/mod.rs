mod base;
mod system_services;

use super::{
    Category, PRIVATE, PreparedAction, ReadContract, check_read_contract, dhcp_tools,
    firewall_tools, network_tools, storage_tools, system_tools, wireless_tools,
};
use serde_json::json;

#[tokio::test]
async fn uci_option_mcp_preserves_text_list_forms_and_reports_late_or_shared_limit_failures() {
    for (name, category, config, section_type, field, other, visible) in [
        (
            "network_interface_configuration",
            Category::Network,
            "network",
            "interface",
            "ipaddr",
            "dns",
            network_tools(),
        ),
        (
            "dhcp_dnsmasq_configuration",
            Category::DhcpDns,
            "dhcp",
            "dnsmasq",
            "server",
            "address",
            dhcp_tools(),
        ),
    ] {
        let base = json!({".type":section_type,".anonymous":true,".index":0,"key":PRIVATE});
        let mut responses = Vec::new();
        for (value, normalized) in [
            (json!(""), json!({"kind":"string","values":[""]})),
            (json!([]), json!({"kind":"list","values":[]})),
            (
                json!("one two"),
                json!({"kind":"string","values":["one two"]}),
            ),
            (
                json!(["two", "one", "one"]),
                json!({"kind":"list","values":["two","one","one"]}),
            ),
        ] {
            let mut raw = base.clone();
            raw[field] = value;
            let mut clean =
                json!({"section":"fixture","section_type":section_type,"anonymous":true,"index":0});
            clean[field] = normalized;
            responses.push((json!({"values":{"fixture":raw}}), json!({"items":[clean]})));
        }
        let mut invalid = base.clone();
        invalid[field] = json!(["fine", null]);
        let mut huge = base.clone();
        huge[field] = json!(vec![""; 128]);
        huge[other] = json!(vec![""; 128]);
        let mut escaped = base;
        escaped[field] = json!(vec!["\u{1}".repeat(1024); 128]);
        check_read_contract(ReadContract {
            name,
            category,
            visible,
            arguments: json!({}),
            action: PreparedAction::Ubus {
                object: "uci".into(),
                method: "get".into(),
                arguments: json!({"config":config,"type":section_type}),
            },
            invalid_arguments: vec![
                json!({"option":field}),
                json!({"values":["injected"]}),
                json!({"execute":true}),
            ],
            responses,
            invalid_outputs: vec![
                (json!({"values":{"fixture":invalid}}), "invalid_output"),
                (json!({"values":{"fixture":huge}}), "output_limit"),
                (json!({"values":{"fixture":escaped}}), "output_limit"),
            ],
        })
        .await;
    }
}

#[tokio::test]
async fn closed_uci_mcp_reads_are_category_scoped_fixed_bounded_and_payload_free_in_audit() {
    for (name, category, config, section_type, visible) in [
        (
            "system_configuration",
            Category::System,
            "system",
            "system",
            system_tools(),
        ),
        (
            "network_interface_configuration",
            Category::Network,
            "network",
            "interface",
            network_tools(),
        ),
        (
            "wireless_radio_configuration",
            Category::Wireless,
            "wireless",
            "wifi-device",
            wireless_tools(),
        ),
        (
            "firewall_defaults_configuration",
            Category::Firewall,
            "firewall",
            "defaults",
            firewall_tools(),
        ),
        (
            "dhcp_dnsmasq_configuration",
            Category::DhcpDns,
            "dhcp",
            "dnsmasq",
            dhcp_tools(),
        ),
        (
            "storage_mount_configuration",
            Category::Storage,
            "fstab",
            "mount",
            storage_tools(),
        ),
    ] {
        let raw = json!({".type":section_type,".anonymous":false,".index":0,
            ".name":PRIVATE,"key":PRIVATE,"options":PRIVATE,"command":PRIVATE});
        let clean =
            json!({"section":"fixture","section_type":section_type,"anonymous":false,"index":0});
        let many: serde_json::Map<String, serde_json::Value> =
            (0..129).map(|i| (format!("n{i}"), raw.clone())).collect();
        let mut wrong = raw.clone();
        wrong[".type"] = json!("other");
        check_read_contract(ReadContract {
            name,
            category,
            visible,
            arguments: json!({}),
            action: PreparedAction::Ubus {
                object: "uci".into(),
                method: "get".into(),
                arguments: json!({"config":config,"type":section_type}),
            },
            invalid_arguments: vec![
                json!({"config":"wireless"}),
                json!({"type":"wifi-iface"}),
                json!({"method":"set"}),
                json!({"option":"key"}),
                json!({"ubus_rpc_session":PRIVATE}),
                json!({"execute":true}),
            ],
            responses: vec![
                (json!({"values":{"fixture":raw}}), json!({"items":[clean]})),
                (json!({"values":{}}), json!({"items":[]})),
            ],
            invalid_outputs: vec![
                (json!({}), "invalid_output"),
                (json!({"values":{},"error":PRIVATE}), "invalid_output"),
                (json!({"values":{"fixture":wrong}}), "invalid_output"),
                (json!({"values":many}), "output_limit"),
            ],
        })
        .await;
    }
}
