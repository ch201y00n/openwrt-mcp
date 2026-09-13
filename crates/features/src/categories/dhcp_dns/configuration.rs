//! Fixed UCI definitions; see base-uci- and system-service-uci-observations.md.
use crate::definition::uci_read_with_options;
use openwrt_mcp_core::{Operation, uci::UciReadProfile};

pub(super) fn operations() -> Vec<Operation> {
    vec![
        uci_read_with_options(
            "dhcp_odhcpd_configuration",
            1,
            UciReadProfile::Odhcpd,
            &[
                ("maindhcp", 8),
                ("loglevel", 32),
                ("leasefile", 1024),
                ("hostsdir", 1024),
                ("hostsfile", 1024),
                ("piodir", 1024),
                ("piofolder", 1024),
            ],
            &[],
        ),
        uci_read_with_options(
            "dhcp_pool_configuration",
            1,
            UciReadProfile::DhcpPools,
            &[
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
            &[
                ("dns", 1024),
                ("domain", 1024),
                ("tag", 256),
                ("interface_name", 1024),
            ],
        ),
        uci_read_with_options(
            "dhcp_host_configuration",
            1,
            UciReadProfile::DhcpHosts,
            &[
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
            &[
                ("mac", 1024),
                ("duid", 1024),
                ("tag", 256),
                ("match_tag", 256),
            ],
        ),
        uci_read_with_options(
            "dhcp_domain_configuration",
            1,
            UciReadProfile::DnsDomains,
            &[("ip", 256)],
            &[("name", 1024)],
        ),
        uci_read_with_options(
            "dhcp_cname_configuration",
            1,
            UciReadProfile::DnsCnames,
            &[("target", 256)],
            &[("cname", 1024)],
        ),
    ]
}
