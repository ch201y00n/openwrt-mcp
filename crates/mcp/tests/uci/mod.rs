use super::{
    Category, PRIVATE, PreparedAction, ReadContract, check_read_contract, dhcp_tools,
    network_tools, storage_tools, wireless_tools,
};
use serde_json::json;

#[tokio::test]
async fn closed_uci_mcp_reads_are_category_scoped_fixed_bounded_and_payload_free_in_audit() {
    for (name, category, config, section_type, visible) in [
        (
            "system_configuration",
            Category::System,
            "system",
            "system",
            vec!["system_board", "system_info", "system_configuration"],
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
            vec!["firewall_defaults_configuration"],
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
