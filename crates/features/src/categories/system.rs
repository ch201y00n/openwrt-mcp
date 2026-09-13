mod configuration;

use crate::definition::{read, uci_read};
use openwrt_mcp_core::{Category, Operation};

pub(crate) fn operations() -> Vec<Operation> {
    let mut operations = vec![
        uci_read(
            "system_configuration",
            openwrt_mcp_core::uci::UciReadProfile::System,
            &[("hostname", 256), ("timezone", 256), ("zonename", 256)],
        ),
        read(
            "system_board",
            "Read selected board and OpenWrt release identifiers.",
            Category::System,
            "system",
            "board",
            "system_board.v1",
            &[
                "/kernel",
                "/system",
                "/model",
                "/board_name",
                "/release/distribution",
                "/release/version",
                "/release/revision",
                "/release/target",
                "/release/description",
            ],
        ),
        read(
            "system_info",
            "Read uptime, memory and load counters; load uses OpenWrt's native units.",
            Category::System,
            "system",
            "info",
            "system_info.v1",
            &[
                "/localtime",
                "/uptime",
                "/load/0",
                "/load/1",
                "/load/2",
                "/memory/total",
                "/memory/free",
                "/memory/shared",
                "/memory/buffered",
                "/memory/available",
                "/swap/total",
                "/swap/free",
                "/root/total",
                "/root/free",
            ],
        ),
    ];
    operations.extend(configuration::operations());
    operations
}
