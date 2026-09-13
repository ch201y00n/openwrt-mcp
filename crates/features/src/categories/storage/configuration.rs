//! Fixed category-owned UCI declarations; see docs/base-uci-observations.md.
use crate::definition::uci_read;
use openwrt_mcp_core::{Operation, uci::UciReadProfile};

pub(super) fn operations() -> Vec<Operation> {
    vec![
        uci_read(
            "storage_global_configuration",
            UciReadProfile::StorageGlobals,
            &[
                ("anon_swap", 32),
                ("anon_mount", 32),
                ("auto_swap", 32),
                ("auto_mount", 32),
                ("delay_root", 32),
                ("check_fs", 32),
            ],
        ),
        uci_read(
            "storage_swap_configuration",
            UciReadProfile::Swaps,
            &[
                ("enabled", 8),
                ("uuid", 256),
                ("label", 256),
                ("device", 1024),
                ("priority", 32),
            ],
        ),
    ]
}
