use crate::definition::read;
use openwrt_mcp_core::{Category, Operation};

pub(crate) fn operations() -> Vec<Operation> {
    // The ubus method also accepts setters. This read contract never supplies them.
    vec![read(
        "diagnostics_watchdog_status",
        "Read watchdog status using an empty-argument query; hardware support is device-dependent.",
        Category::Diagnostics,
        "system",
        "watchdog",
        "diagnostics_watchdog_status.v1",
        &["/status", "/timeout", "/frequency", "/magicclose"],
    )]
}
