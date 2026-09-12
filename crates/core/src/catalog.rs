use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use crate::{
    Action, Category, CoreError, Operation, Parameter, ParameterKind, Permission, Requirement,
};

const MAX_CUSTOM_OPERATIONS: usize = 1024;

pub(crate) fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "system_board"
            | "system_info"
            | "network_device_status"
            | "network_lan_status"
            | "network_wan_status"
    )
}

/// Immutable, validated operation metadata owned by the server operator.
#[derive(Debug, Clone)]
pub struct Catalog {
    operations: Vec<Operation>,
    index: BTreeMap<String, usize>,
}

impl Catalog {
    pub fn new(custom: Vec<Operation>) -> Result<Self, CoreError> {
        if custom.len() > MAX_CUSTOM_OPERATIONS {
            return Err(CoreError::CatalogLimitExceeded);
        }
        let mut operations = builtins();
        let mut names: BTreeSet<String> = operations
            .iter()
            .map(|operation| operation.name.clone())
            .collect();
        for mut operation in custom {
            if !names.insert(operation.name.clone()) {
                return Err(CoreError::DuplicateOperation);
            }
            // Validate before injection so extensions must declare their own
            // affected categories and cannot omit all permissions.
            operation.validate()?;
            for permission in [Permission::Write, Permission::Execute] {
                let required = Requirement {
                    category: Category::Extensions,
                    permission,
                };
                if !operation.requirements.contains(&required) {
                    operation.requirements.push(required);
                }
            }
            operation.validate()?;
            operations.push(operation);
        }
        for operation in &operations {
            operation.validate()?;
        }
        let index = operations
            .iter()
            .enumerate()
            .map(|(index, operation)| (operation.name.clone(), index))
            .collect();
        Ok(Self { operations, index })
    }

    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }

    pub fn get(&self, name: &str) -> Option<&Operation> {
        self.index.get(name).map(|index| &self.operations[*index])
    }
}

fn read_operation(
    name: &str,
    description: &str,
    category: Category,
    object: &str,
    method: &str,
    fields: &[&str],
) -> Operation {
    Operation {
        name: name.to_owned(),
        description: description.to_owned(),
        requirements: vec![Requirement {
            category,
            permission: Permission::Read,
        }],
        parameters: BTreeMap::new(),
        action: Action::Ubus {
            object: object.to_owned(),
            method: method.to_owned(),
            arguments: BTreeMap::new(),
        },
        output_fields: fields.iter().map(|field| (*field).to_owned()).collect(),
    }
}

fn builtins() -> Vec<Operation> {
    let board = read_operation(
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
    );
    let info = read_operation(
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
    );
    let mut device = read_operation(
        "network_device_status",
        "Read selected link state and traffic counters for a named network device.",
        Category::Network,
        "network.device",
        "status",
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
    if let Action::Ubus { arguments, .. } = &mut device.action {
        arguments.insert("name".to_owned(), json!("{name}"));
    }
    let interface_fields = &[
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
    let lan = read_operation(
        "network_lan_status",
        "Read selected state of the conventional lan interface; unavailable if absent or renamed.",
        Category::Network,
        "network.interface.lan",
        "status",
        interface_fields,
    );
    let wan = read_operation(
        "network_wan_status",
        "Read selected state of the conventional wan interface; unavailable if absent or renamed.",
        Category::Network,
        "network.interface.wan",
        "status",
        interface_fields,
    );
    vec![board, info, device, lan, wan]
}
