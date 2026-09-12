use crate::definition::{argument, read};
use openwrt_mcp_core::{Category, Operation, Parameter, ParameterKind};
use serde_json::json;

pub(crate) fn operations() -> Vec<Operation> {
    let mut radio = read(
        "wireless_radio_info",
        "Read selected radio statistics without SSID or BSSID; requires rpcd-mod-iwinfo and a supported driver.",
        Category::Wireless,
        "iwinfo",
        "info",
        "wireless_radio_info.v1",
        &[
            "/phy",
            "/mode",
            "/country",
            "/channel",
            "/frequency",
            "/txpower",
            "/quality",
            "/quality_max",
            "/signal",
            "/noise",
            "/bitrate",
            "/encryption/enabled",
        ],
    );
    radio.parameters.insert(
        "device".to_owned(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: Vec::new(),
        },
    );
    argument(
        &mut radio,
        "device",
        ParameterKind::String,
        json!("{device}"),
    );
    vec![radio]
}
