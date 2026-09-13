use super::*;

async fn check(
    name: &'static str,
    config: &'static str,
    kind: &'static str,
    scalar: &'static str,
    option: Option<&'static str>,
    category: Category,
) {
    let mut raw = json!({".type":kind,".anonymous":false,".index":42});
    for excluded in [
        "message",
        "ForceCommand",
        "keyfile",
        "rsakeyfile",
        "BannerFile",
        "key",
        "cert",
        "config",
        "home",
        "realm",
        "httpauth",
        "cgi_prefix",
        "lua_prefix",
        "ucode_prefix",
        "interpreter",
        "ubus_prefix",
        "ubus_socket",
        "leasetrigger",
        "password",
        "private_key",
        "future_option",
    ] {
        raw[excluded] = json!(PRIVATE);
    }
    raw[scalar] = json!("fixture");
    let mut clean = json!({"section":"fixture","section_type":kind,"anonymous":false,"index":42});
    clean[scalar] = json!("fixture");
    let mut responses = vec![
        (json!({"values":{}}), json!({"items":[]})),
        (
            json!({"values":{"fixture":raw.clone()}}),
            json!({"items":[clean.clone()]}),
        ),
    ];
    let mut malformed = raw.clone();
    malformed[scalar] = json!(["wrong"]);
    let mut wrong_type = raw.clone();
    wrong_type[".type"] = json!("httpauth");
    let mut invalid_outputs = vec![
        (
            json!({"values":{"fixture":raw.clone()},"error":PRIVATE}),
            "invalid_output",
        ),
        (
            json!({"values":{"early":raw.clone(),"late":malformed}}),
            "invalid_output",
        ),
        (json!({"values":{"fixture":wrong_type}}), "invalid_output"),
    ];
    if let Some(field) = option {
        for (input, kind, values) in [
            (json!(""), "string", json!([""])),
            (json!("one two"), "string", json!(["one two"])),
            (json!([]), "list", json!([])),
            (
                json!(["two", "one", "one", ""]),
                "list",
                json!(["two", "one", "one", ""]),
            ),
        ] {
            let mut source = raw.clone();
            source[field] = input;
            let mut expected = clean.clone();
            expected[field] = json!({"kind":kind,"values":values});
            responses.push((
                json!({"values":{"fixture":source}}),
                json!({"items":[expected]}),
            ));
        }
        let mut invalid = raw.clone();
        invalid[field] = json!(["valid", null]);
        invalid_outputs.push((json!({"values":{"fixture":invalid}}), "invalid_output"));
        let mut huge = raw.clone();
        huge[field] = json!(vec![""; 128]);
        invalid_outputs.push((
            json!({"values":{"first":huge.clone(),"late":huge}}),
            "output_limit",
        ));
    }
    let rows: serde_json::Map<String, serde_json::Value> = (0..129)
        .map(|i| (format!("n{i:03}"), raw.clone()))
        .collect();
    invalid_outputs.push((json!({"values":rows}), "output_limit"));
    check_read_contract(ReadContract {
        name,
        category,
        visible: if category == Category::System {
            system_tools()
        } else {
            dhcp_tools()
        },
        arguments: json!({}),
        action: PreparedAction::Ubus {
            object: "uci".into(),
            method: "get".into(),
            arguments: json!({"config":config,"type":kind}),
        },
        invalid_arguments: vec![
            json!({"config":"wireless"}),
            json!({"type":"httpauth"}),
            json!({"option":"key"}),
            json!({"section":"lan3"}),
            json!({"method":"set"}),
            json!({"ubus_rpc_session":PRIVATE}),
            json!({"execute":true}),
        ],
        responses,
        invalid_outputs,
    })
    .await;
}

#[tokio::test]
async fn led_read_uses_authorized_audited_fixed_mcp_path() {
    check(
        "system_led_configuration",
        "system",
        "led",
        "trigger",
        Some("mode"),
        Category::System,
    )
    .await;
}
#[tokio::test]
async fn dropbear_read_is_system_not_generic_services_authority() {
    check(
        "system_dropbear_configuration",
        "dropbear",
        "dropbear",
        "Port",
        None,
        Category::System,
    )
    .await;
}
#[tokio::test]
async fn uhttpd_read_excludes_credentials_handlers_and_preserves_listeners() {
    check(
        "system_uhttpd_configuration",
        "uhttpd",
        "uhttpd",
        "redirect_https",
        Some("listen_https"),
        Category::System,
    )
    .await;
}
#[tokio::test]
async fn odhcpd_read_observes_paths_without_opening_or_executing_them() {
    check(
        "dhcp_odhcpd_configuration",
        "dhcp",
        "odhcpd",
        "hostsdir",
        None,
        Category::DhcpDns,
    )
    .await;
}
