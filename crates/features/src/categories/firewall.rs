mod configuration;

use crate::definition::uci_read;
use openwrt_mcp_core::{Operation, uci::UciReadProfile};

pub(crate) fn operations() -> Vec<Operation> {
    let mut operations = vec![uci_read(
        "firewall_defaults_configuration",
        UciReadProfile::FirewallDefaults,
        &[
            ("input", 32),
            ("output", 32),
            ("forward", 32),
            ("synflood_protect", 8),
            ("drop_invalid", 8),
            ("flow_offloading", 8),
            ("flow_offloading_hw", 8),
            ("disable_ipv6", 8),
        ],
    )];
    operations.extend(configuration::operations());
    operations
}
