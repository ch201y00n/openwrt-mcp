use crate::definition::{argument, read};
use openwrt_mcp_core::{
    Category, Collection, Operation, OutputMode, Parameter, ParameterKind, ScalarKind, Selection,
    TypedProjection,
};
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
    let mut devices = read(
        "wireless_devices",
        "List unique bounded iwinfo-supported device names; an empty result does not prove physical radios are absent.",
        Category::Wireless,
        "iwinfo",
        "devices",
        "wireless_devices.v1",
        &[],
    );
    devices.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        collection: Collection::ScalarArray {
            source: "/devices".into(),
            max_items: 128,
            name: "device".into(),
            value: ScalarKind::Text { max_bytes: 256 },
            unique: true,
        },
        selection: Selection::All {},
    }));
    vec![radio, devices]
}
