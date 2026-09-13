use openwrt_mcp_core::{
    Action, CapabilityRequirement, Category, Collection, CollectionField, InnerRecord, LeafRecord,
    Operation, OutputMode, Parameter, ParameterKind, Permission, Presence, Requirement,
    ScalarField, ScalarKind, Selection, TypedProjection,
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

/// Fixed netifd read with an invocation-bound local interface selector. Category
/// owners supply only their reviewed finite fields, not a device action template.
pub(crate) fn interface_observation(
    name: &str,
    description: &str,
    category: Category,
    collections: Vec<CollectionField<Collection<LeafRecord>>>,
) -> Operation {
    let mut operation = read(
        name,
        description,
        category,
        "network.interface",
        "dump",
        &format!("{name}.v1"),
        &[],
    );
    operation.parameters.insert(
        "interface".into(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: Vec::new(),
        },
    );
    operation.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        reject_if_present: vec!["/error".into()],
        collection: Collection::ObjectArray {
            source: "/interface".into(),
            max_items: 128,
            identity: "interface".into(),
            record: InnerRecord {
                fields: vec![
                    ScalarField {
                        name: "interface".into(),
                        source: "/interface".into(),
                        presence: Presence::Required,
                        value: ScalarKind::Text { max_bytes: 256 },
                    },
                    ScalarField {
                        name: "up".into(),
                        source: "/up".into(),
                        presence: Presence::Required,
                        value: ScalarKind::Boolean {},
                    },
                ],
                collections,
            },
        },
        selection: Selection::ExactOne {
            parameter: "interface".into(),
        },
    }));
    operation
}
