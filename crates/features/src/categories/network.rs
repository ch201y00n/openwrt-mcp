use crate::definition::{argument, read};
use openwrt_mcp_core::{Category, Operation, Parameter, ParameterKind};
use serde_json::json;

pub(crate) fn operations() -> Vec<Operation> {
    let mut device = read(
        "network_device_status",
        "Read selected link state and traffic counters for a named network device.",
        Category::Network,
        "network.device",
        "status",
        "network_device_status.v1",
        &[
            "/type",
            "/external",
            "/present",
            "/up",
            "/carrier",
            "/mtu",
            "/mtu6",
            "/txqueuelen",
            "/speed",
            "/duplex",
            "/statistics/rx_bytes",
            "/statistics/tx_bytes",
            "/statistics/rx_packets",
            "/statistics/tx_packets",
            "/statistics/rx_errors",
            "/statistics/tx_errors",
            "/statistics/rx_dropped",
            "/statistics/tx_dropped",
        ],
    );
    device.parameters.insert(
        "name".to_owned(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: Vec::new(),
        },
    );
    argument(&mut device, "name", ParameterKind::String, json!("{name}"));
    let fields = &[
        "/up",
        "/pending",
        "/available",
        "/autostart",
        "/dynamic",
        "/uptime",
        "/proto",
        "/device",
        "/l3_device",
        "/metric",
    ];
    vec![
        device,
        read(
            "network_lan_status",
            "Read selected state of the conventional lan interface; unavailable if absent or renamed.",
            Category::Network,
            "network.interface.lan",
            "status",
            "network_lan_status.v1",
            fields,
        ),
        read(
            "network_wan_status",
            "Read selected state of the conventional wan interface; unavailable if absent or renamed.",
            Category::Network,
            "network.interface.wan",
            "status",
            "network_wan_status.v1",
            fields,
        ),
        interface_status(),
    ]
}

fn interface_status() -> Operation {
    let mut operation = read(
        "network_interface_status",
        "Read selected state of a named logical interface; missing interfaces are unavailable.",
        Category::Network,
        "network.interface",
        "status",
        "network_interface_status.v1",
        &[
            "/up",
            "/pending",
            "/available",
            "/autostart",
            "/dynamic",
            "/uptime",
            "/proto",
            "/device",
            "/l3_device",
        ],
    );
    operation.parameters.insert(
        "interface".to_owned(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: Vec::new(),
        },
    );
    argument(
        &mut operation,
        "interface",
        ParameterKind::String,
        json!("{interface}"),
    );
    operation
}
