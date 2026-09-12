use crate::definition::read;
use openwrt_mcp_core::{Category, Operation};

pub(crate) fn operations() -> Vec<Operation> {
    vec![
        read(
            "system_board",
            "Read selected board and OpenWrt release identifiers.",
            Category::System,
            "system",
            "board",
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
    ]
}
