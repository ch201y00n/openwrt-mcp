use openwrt_mcp_core::{Action, Category, Operation, OutputMode, Permission, Requirement};
use std::collections::BTreeMap;

pub(crate) fn read(
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
        output_mode: OutputMode::Scalars,
    }
}
