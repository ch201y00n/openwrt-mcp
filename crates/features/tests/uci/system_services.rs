//! Independent v20 expectations; never import production field declarations.
use super::{Category, Recipe};

pub(super) fn recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            name: "system_led_configuration",
            response: "system_led_configuration.v1",
            category: Category::System,
            config: "system",
            section_type: "led",
            options: &[
                ("name", 256),
                ("sysfs", 256),
                ("trigger", 64),
                ("dev", 256),
                ("default", 8),
                ("inverted", 8),
                ("brightness", 32),
                ("delayon", 32),
                ("delayoff", 32),
                ("interval", 32),
                ("port_state", 32),
                ("delay", 32),
                ("gpio", 32),
                ("port_mask", 32),
                ("speed_mask", 32),
            ],
        },
        Recipe {
            name: "system_dropbear_configuration",
            response: "system_dropbear_configuration.v1",
            category: Category::System,
            config: "dropbear",
            section_type: "dropbear",
            options: &[
                ("enable", 8),
                ("PasswordAuth", 8),
                ("RootPasswordAuth", 8),
                ("RootLogin", 8),
                ("GatewayPorts", 8),
                ("LocalPortForward", 8),
                ("RemotePortForward", 8),
                ("Port", 32),
                ("Interface", 256),
                ("DirectInterface", 256),
                ("SSHKeepAlive", 32),
                ("IdleTimeout", 32),
                ("MaxAuthTries", 32),
                ("RecvWindowSize", 32),
                ("mdns", 8),
            ],
        },
        Recipe {
            name: "system_uhttpd_configuration",
            response: "system_uhttpd_configuration.v1",
            category: Category::System,
            config: "uhttpd",
            section_type: "uhttpd",
            options: &[
                ("redirect_https", 8),
                ("rfc1918_filter", 8),
                ("max_requests", 32),
                ("max_connections", 32),
                ("script_timeout", 32),
                ("network_timeout", 32),
                ("http_keepalive", 32),
                ("tcp_keepalive", 32),
                ("no_symlinks", 8),
                ("no_dirlists", 8),
                ("no_ubusauth", 8),
            ],
        },
        Recipe {
            name: "dhcp_odhcpd_configuration",
            response: "dhcp_odhcpd_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "odhcpd",
            options: &[
                ("maindhcp", 8),
                ("loglevel", 32),
                ("leasefile", 1024),
                ("hostsdir", 1024),
                ("hostsfile", 1024),
                ("piodir", 1024),
                ("piofolder", 1024),
            ],
        },
    ]
}

pub(super) fn text_options(name: &str) -> &'static [(&'static str, usize)] {
    match name {
        "system_led_configuration" => &[("mode", 256), ("port", 256)],
        "system_uhttpd_configuration" => &[("listen_http", 256), ("listen_https", 256)],
        _ => &[],
    }
}

#[test]
fn system_service_documentation_matches_independent_field_contracts() {
    let definitions = recipes();
    let mut seen = std::collections::BTreeSet::new();
    for line in include_str!("../../../../docs/system-service-uci-observations.md").lines() {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        if columns.len() != 5 || !columns[1].contains("_configuration / ") {
            continue;
        }
        let identity: Vec<_> = columns[1].split(" / ").collect();
        assert_eq!(identity.len(), 3);
        let recipe = definitions.iter().find(|r| r.name == identity[0]).unwrap();
        assert!(seen.insert(recipe.name));
        assert_eq!(
            identity[1],
            serde_json::to_value(recipe.category)
                .unwrap()
                .as_str()
                .unwrap()
        );
        assert_eq!(
            identity[2],
            format!("{}:{}", recipe.config, recipe.section_type)
        );
        for (column, fields) in [
            (columns[2], recipe.options),
            (columns[3], text_options(recipe.name)),
        ] {
            let expected: Vec<_> = fields
                .iter()
                .map(|(name, max)| format!("{name} ({max})"))
                .collect();
            assert_eq!(
                column,
                if expected.is_empty() {
                    "None".into()
                } else {
                    expected.join(", ")
                }
            );
        }
    }
    assert_eq!(seen.len(), 4);
}

#[test]
fn system_service_reads_exclude_executables_secrets_and_unselected_variation() {
    for recipe in recipes() {
        let mut raw = super::row(&recipe);
        for field in [
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
            "private_key",
            "color_red",
            "future_option",
        ] {
            raw[field] = serde_json::json!({"secret":"synthetic-excluded-value"});
        }
        assert_eq!(
            super::observe(&recipe, serde_json::json!({"values":{"fixture":raw}})).unwrap(),
            serde_json::json!({"items":[{"section":"fixture","section_type":recipe.section_type,"anonymous":true,"index":42}]})
        );
    }
}
