mod configuration;
mod observations;

use crate::definition::{argument, read, uci_read_with_options};
use openwrt_mcp_core::{
    Category, Collection, InnerRecord, Operation, OutputMode, Parameter, ParameterKind, Presence,
    SAFE_INTEGER_MAX, ScalarField, ScalarKind, Selection, TypedProjection,
};
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
    let mut operations = vec![
        uci_read_with_options(
            "network_interface_configuration",
            2,
            openwrt_mcp_core::uci::UciReadProfile::NetworkInterfaces,
            &[
                ("proto", 64),
                ("device", 256),
                ("mtu", 32),
                ("metric", 32),
                ("auto", 8),
                ("defaultroute", 8),
                ("peerdns", 8),
                ("delegate", 8),
                ("ip4table", 64),
                ("ip6table", 64),
            ],
            &[
                ("ipaddr", 1024),
                ("ip6addr", 1024),
                ("dns", 1024),
                ("ifname", 256),
            ],
        ),
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
        interfaces(),
        observations::addresses(),
        observations::routes(),
        observations::neighbors(),
    ];
    operations.extend(configuration::operations());
    operations
}

fn interface_status() -> Operation {
    let mut operation = read(
        "network_interface_status",
        "Read one exact logical interface from a bounded dump; v2 returns typed fields including interface identity.",
        Category::Network,
        "network.interface",
        "dump",
        "network_interface_status.v2",
        &[],
    );
    operation.parameters.insert(
        "interface".to_owned(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: Vec::new(),
        },
    );
    // Selection is local to the prepared projection, never an ubus argument.
    operation.output_mode =
        OutputMode::Typed(Box::new(interface_projection(Selection::ExactOne {
            parameter: "interface".into(),
        })));
    operation
}

fn interfaces() -> Operation {
    let mut operation = read(
        "network_interfaces",
        "List bounded logical-interface identity and selected state, without addresses, routes or configuration.",
        Category::Network,
        "network.interface",
        "dump",
        "network_interfaces.v1",
        &[],
    );
    operation.output_mode = OutputMode::Typed(Box::new(interface_projection(Selection::All {})));
    operation
}

fn interface_projection(selection: Selection) -> TypedProjection {
    let mut fields = vec![ScalarField {
        name: "interface".into(),
        source: "/interface".into(),
        presence: Presence::Required,
        value: ScalarKind::Text { max_bytes: 256 },
    }];
    fields.extend(
        ["up", "pending", "available", "autostart", "dynamic"]
            .into_iter()
            .map(|name| ScalarField {
                name: name.into(),
                source: format!("/{name}"),
                presence: Presence::Required,
                value: ScalarKind::Boolean {},
            }),
    );
    fields.push(ScalarField {
        name: "uptime".into(),
        source: "/uptime".into(),
        presence: Presence::Optional,
        value: ScalarKind::SafeInteger {
            min: 0,
            max: SAFE_INTEGER_MAX,
        },
    });
    fields.extend(
        ["proto", "device", "l3_device"]
            .into_iter()
            .map(|name| ScalarField {
                name: name.into(),
                source: format!("/{name}"),
                presence: Presence::Optional,
                value: ScalarKind::Text { max_bytes: 256 },
            }),
    );
    TypedProjection::Collection {
        reject_if_present: vec![],
        collection: Collection::ObjectArray {
            source: "/interface".into(),
            max_items: 128,
            identity: "interface".into(),
            record: InnerRecord {
                fields,
                collections: vec![],
            },
        },
        selection,
    }
}
