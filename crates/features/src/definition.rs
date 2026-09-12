use openwrt_mcp_core::{
    Action, CapabilityRequirement, Category, Operation, OutputMode, ParameterKind, Permission,
    Requirement,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn read(
    name: &str,
    description: &str,
    category: Category,
    object: &str,
    method: &str,
    response_contract: &str,
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
        capability: CapabilityRequirement::UbusMethod {
            object: object.to_owned(),
            method: method.to_owned(),
            arguments: BTreeMap::new(),
            response_contract: response_contract.to_owned(),
        },
        output_fields: fields.iter().map(|field| (*field).to_owned()).collect(),
        output_mode: OutputMode::Scalars,
    }
}

/// Keep the template and its explicitly declared prerequisite together. Catalog
/// validation independently checks the type against the value/parameter definition.
pub(crate) fn argument(operation: &mut Operation, name: &str, kind: ParameterKind, value: Value) {
    let (
        Action::Ubus { arguments, .. },
        CapabilityRequirement::UbusMethod {
            arguments: required,
            ..
        },
    ) = (&mut operation.action, &mut operation.capability)
    else {
        unreachable!("read definitions must use checked ubus metadata");
    };
    arguments.insert(name.to_owned(), value);
    required.insert(name.to_owned(), kind);
}
