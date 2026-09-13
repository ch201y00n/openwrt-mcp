//! Fixed category-owned UCI declarations; see docs/base-uci-observations.md.
use crate::definition::uci_read_with_options;
use openwrt_mcp_core::{Operation, uci::UciReadProfile};

pub(super) fn operations() -> Vec<Operation> {
    vec![uci_read_with_options(
        "system_timeserver_configuration",
        1,
        UciReadProfile::Timeservers,
        &[
            ("enabled", 8),
            ("enable_server", 8),
            ("use_dhcp", 8),
            ("interface", 256),
        ],
        &[("server", 1024), ("dhcp_interface", 256)],
    )]
}
