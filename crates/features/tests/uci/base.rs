//! Independent expectations, never derived from production declarations.
use super::{Category, Recipe};

pub(super) fn recipes() -> Vec<Recipe> {
    vec![
        Recipe {
            name: "system_timeserver_configuration",
            response: "system_timeserver_configuration.v1",
            category: Category::System,
            config: "system",
            section_type: "timeserver",
            options: &[
                ("enabled", 8),
                ("enable_server", 8),
                ("use_dhcp", 8),
                ("interface", 256),
            ],
        },
        Recipe {
            name: "network_device_configuration",
            response: "network_device_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "device",
            options: &[
                ("name", 256),
                ("type", 64),
                ("ifname", 256),
                ("vid", 32),
                ("mtu", 32),
                ("mtu6", 32),
                ("macaddr", 17),
                ("enabled", 8),
                ("ipv6", 8),
                ("stp", 8),
                ("stp_kernel", 8),
                ("stp_proto", 64),
                ("forward_delay", 32),
                ("priority", 32),
                ("ageing_time", 32),
                ("igmp_snooping", 8),
                ("multicast_querier", 8),
                ("bridge_empty", 8),
                ("vlan_filtering", 8),
            ],
        },
        Recipe {
            name: "network_bridge_vlan_configuration",
            response: "network_bridge_vlan_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "bridge-vlan",
            options: &[("device", 256), ("vlan", 32), ("local", 8)],
        },
        Recipe {
            name: "network_route_v4_configuration",
            response: "network_route_v4_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "route",
            options: route_fields(),
        },
        Recipe {
            name: "network_route_v6_configuration",
            response: "network_route_v6_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "route6",
            options: route_fields(),
        },
        Recipe {
            name: "network_rule_v4_configuration",
            response: "network_rule_v4_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "rule",
            options: rule_fields(),
        },
        Recipe {
            name: "network_rule_v6_configuration",
            response: "network_rule_v6_configuration.v1",
            category: Category::Network,
            config: "network",
            section_type: "rule6",
            options: rule_fields(),
        },
        Recipe {
            name: "firewall_zone_configuration",
            response: "firewall_zone_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "zone",
            options: &[
                ("enabled", 8),
                ("name", 256),
                ("family", 32),
                ("input", 32),
                ("output", 32),
                ("forward", 32),
                ("masq", 8),
                ("masq6", 8),
                ("masq_allow_invalid", 8),
                ("mtu_fix", 8),
                ("log", 32),
                ("log_limit", 64),
                ("auto_helper", 8),
                ("counter", 8),
            ],
        },
        Recipe {
            name: "firewall_forwarding_configuration",
            response: "firewall_forwarding_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "forwarding",
            options: &[
                ("enabled", 8),
                ("name", 256),
                ("family", 32),
                ("src", 256),
                ("dest", 256),
            ],
        },
        Recipe {
            name: "firewall_rule_configuration",
            response: "firewall_rule_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "rule",
            options: &[
                ("enabled", 8),
                ("name", 256),
                ("family", 32),
                ("src", 256),
                ("dest", 256),
                ("device", 256),
                ("direction", 32),
                ("ipset", 256),
                ("helper", 256),
                ("set_helper", 256),
                ("limit", 64),
                ("limit_burst", 32),
                ("utc_time", 8),
                ("start_date", 32),
                ("stop_date", 32),
                ("start_time", 32),
                ("stop_time", 32),
                ("weekdays", 128),
                ("mark", 64),
                ("set_mark", 64),
                ("set_xmark", 64),
                ("dscp", 32),
                ("set_dscp", 32),
                ("counter", 8),
                ("log", 256),
                ("log_limit", 64),
                ("target", 32),
            ],
        },
        Recipe {
            name: "firewall_redirect_configuration",
            response: "firewall_redirect_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "redirect",
            options: &[
                ("enabled", 8),
                ("name", 256),
                ("family", 32),
                ("src", 256),
                ("dest", 256),
                ("ipset", 256),
                ("helper", 256),
                ("src_ip", 1024),
                ("src_port", 256),
                ("src_dip", 1024),
                ("src_dport", 256),
                ("dest_ip", 1024),
                ("dest_port", 256),
                ("limit", 64),
                ("limit_burst", 32),
                ("utc_time", 8),
                ("start_date", 32),
                ("stop_date", 32),
                ("start_time", 32),
                ("stop_time", 32),
                ("weekdays", 128),
                ("mark", 64),
                ("reflection", 8),
                ("reflection_src", 32),
                ("counter", 8),
                ("log", 256),
                ("log_limit", 64),
                ("target", 32),
            ],
        },
        Recipe {
            name: "firewall_nat_configuration",
            response: "firewall_nat_configuration.v1",
            category: Category::Firewall,
            config: "firewall",
            section_type: "nat",
            options: &[
                ("enabled", 8),
                ("name", 256),
                ("family", 32),
                ("src", 256),
                ("device", 256),
                ("src_ip", 1024),
                ("src_port", 256),
                ("snat_ip", 1024),
                ("snat_port", 256),
                ("dest_ip", 1024),
                ("dest_port", 256),
                ("limit", 64),
                ("limit_burst", 32),
                ("connlimit_ports", 8),
                ("utc_time", 8),
                ("start_date", 32),
                ("stop_date", 32),
                ("start_time", 32),
                ("stop_time", 32),
                ("weekdays", 128),
                ("mark", 64),
                ("counter", 8),
                ("log", 256),
                ("target", 32),
            ],
        },
        Recipe {
            name: "dhcp_pool_configuration",
            response: "dhcp_pool_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "dhcp",
            options: &[
                ("interface", 256),
                ("networkid", 256),
                ("ignore", 8),
                ("netmask", 64),
                ("force", 8),
                ("start", 32),
                ("limit", 32),
                ("leasetime", 64),
                ("dynamicdhcp", 8),
                ("dynamicdhcpv4", 8),
                ("dynamicdhcpv6", 8),
                ("dhcpv4", 32),
                ("dhcpv6", 32),
                ("ra", 32),
                ("ra_management", 32),
                ("ra_preference", 32),
            ],
        },
        Recipe {
            name: "dhcp_host_configuration",
            response: "dhcp_host_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "host",
            options: &[
                ("name", 256),
                ("ip", 256),
                ("hostid", 256),
                ("leasetime", 64),
                ("networkid", 256),
                ("enable", 8),
                ("dns", 8),
                ("force", 8),
                ("broadcast", 8),
            ],
        },
        Recipe {
            name: "dhcp_domain_configuration",
            response: "dhcp_domain_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "domain",
            options: &[("ip", 256)],
        },
        Recipe {
            name: "dhcp_cname_configuration",
            response: "dhcp_cname_configuration.v1",
            category: Category::DhcpDns,
            config: "dhcp",
            section_type: "cname",
            options: &[("target", 256)],
        },
        Recipe {
            name: "storage_global_configuration",
            response: "storage_global_configuration.v1",
            category: Category::Storage,
            config: "fstab",
            section_type: "global",
            options: &[
                ("anon_swap", 32),
                ("anon_mount", 32),
                ("auto_swap", 32),
                ("auto_mount", 32),
                ("delay_root", 32),
                ("check_fs", 32),
            ],
        },
        Recipe {
            name: "storage_swap_configuration",
            response: "storage_swap_configuration.v1",
            category: Category::Storage,
            config: "fstab",
            section_type: "swap",
            options: &[
                ("enabled", 8),
                ("uuid", 256),
                ("label", 256),
                ("device", 1024),
                ("priority", 32),
            ],
        },
    ]
}

fn route_fields() -> &'static [(&'static str, usize)] {
    &[
        ("interface", 256),
        ("target", 256),
        ("netmask", 64),
        ("gateway", 256),
        ("metric", 32),
        ("mtu", 32),
        ("table", 64),
        ("valid", 32),
        ("source", 256),
        ("onlink", 8),
        ("type", 64),
        ("proto", 64),
        ("disabled", 8),
    ]
}
fn rule_fields() -> &'static [(&'static str, usize)] {
    &[
        ("in", 256),
        ("out", 256),
        ("invert", 8),
        ("src", 256),
        ("dest", 256),
        ("priority", 32),
        ("tos", 32),
        ("mark", 64),
        ("lookup", 64),
        ("suppress_prefixlength", 32),
        ("uidrange", 64),
        ("action", 64),
        ("goto", 32),
        ("ipproto", 64),
        ("sport", 64),
        ("dport", 64),
        ("disabled", 8),
    ]
}
pub(super) fn text_options(name: &str) -> &'static [(&'static str, usize)] {
    match name {
        "system_timeserver_configuration" => &[("server", 1024), ("dhcp_interface", 256)],
        "network_device_configuration" => &[
            ("ports", 256),
            ("ingress_qos_mapping", 256),
            ("egress_qos_mapping", 256),
        ],
        "network_bridge_vlan_configuration" => &[("ports", 256), ("alias", 256)],
        "firewall_zone_configuration" => &[
            ("network", 256),
            ("device", 256),
            ("subnet", 1024),
            ("masq_src", 1024),
            ("masq_dest", 1024),
            ("helper", 256),
        ],
        "firewall_rule_configuration" => &[
            ("proto", 256),
            ("src_ip", 1024),
            ("src_mac", 1024),
            ("src_port", 1024),
            ("dest_ip", 1024),
            ("dest_port", 1024),
            ("icmp_type", 1024),
        ],
        "firewall_redirect_configuration" => {
            &[("proto", 256), ("src_mac", 1024), ("reflection_zone", 256)]
        }
        "firewall_nat_configuration" => &[("proto", 256)],
        "dhcp_pool_configuration" => &[
            ("dns", 1024),
            ("domain", 1024),
            ("tag", 256),
            ("interface_name", 1024),
        ],
        "dhcp_host_configuration" => &[
            ("mac", 1024),
            ("duid", 1024),
            ("tag", 256),
            ("match_tag", 256),
        ],
        "dhcp_domain_configuration" => &[("name", 1024)],
        "dhcp_cname_configuration" => &[("cname", 1024)],
        _ => &[],
    }
}

#[test]
fn documented_base_fields_and_independent_fixtures_cover_the_same_exact_contracts() {
    let definitions = recipes();
    let mut seen = std::collections::BTreeSet::new();
    for line in include_str!("../../../../docs/base-uci-observations.md").lines() {
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
            if column == "None" {
                assert!(expected.is_empty());
            } else if column == "Same fields/limits as route v4" {
                assert_eq!(fields, route_fields());
            } else if column == "Same fields/limits as rule v4" {
                assert_eq!(fields, rule_fields());
            } else {
                assert_eq!(column, expected.join(", "));
            }
        }
    }
    assert_eq!(seen.len(), 18);
}
