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

/// Assemble one closed recipe. Category modules own their reviewed scalar fields.
pub(crate) fn uci_read(
    name: &str,
    profile: openwrt_mcp_core::uci::UciReadProfile,
    options: &[(&str, usize)],
) -> Operation {
    let mut operation = read(
        name,
        "Read selected non-secret scalar UCI options and section metadata. Sessionless shared-delta, non-atomic view; not committed-only or effective service state. Option values remain text; lists are not coerced. Section names/indices are not durable mutation handles. No configuration changes.",
        profile.category(),
        "uci",
        "get",
        &format!("{name}.v1"),
        &[],
    );
    argument(
        &mut operation,
        "config",
        ParameterKind::String,
        Value::from(profile.config()),
    );
    argument(
        &mut operation,
        "type",
        ParameterKind::String,
        Value::from(profile.section_type()),
    );
    let mut fields = vec![
        ScalarField {
            name: "section_type".into(),
            source: "/.type".into(),
            presence: Presence::Required,
            value: ScalarKind::TextEnum {
                max_bytes: 16,
                values: vec![profile.section_type().into()],
            },
        },
        ScalarField {
            name: "anonymous".into(),
            source: "/.anonymous".into(),
            presence: Presence::Required,
            value: ScalarKind::Boolean {},
        },
        ScalarField {
            name: "index".into(),
            source: "/.index".into(),
            presence: Presence::Required,
            value: ScalarKind::SafeInteger {
                min: 0,
                max: u32::MAX.into(),
            },
        },
    ];
    fields.extend(options.iter().map(|(name, max_bytes)| ScalarField {
        name: (*name).into(),
        source: format!("/{name}"),
        presence: Presence::Optional,
        value: ScalarKind::Text {
            max_bytes: *max_bytes,
        },
    }));
    operation.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        reject_if_present: vec!["/error".into()],
        collection: Collection::ObjectEntries {
            source: "/values".into(),
            max_items: 128,
            key: openwrt_mcp_core::TextIdentity {
                name: "section".into(),
                max_bytes: 256,
            },
            record: InnerRecord {
                fields,
                collections: vec![],
            },
        },
        selection: Selection::All {},
    }));
    operation
}

/// Extend a closed UCI projection, without changing its action or permission.
pub(crate) fn uci_read_with_options(
    name: &str,
    profile: openwrt_mcp_core::uci::UciReadProfile,
    scalars: &[(&str, usize)],
    options: &[(&str, usize)],
) -> Operation {
    let mut operation = uci_read(name, profile, scalars);
    operation.description = "Read selected non-secret UCI scalar and text/list options with section metadata. Sessionless shared-delta, non-atomic view; not committed-only or effective state. Text/list fields preserve representation as kind/values, with shared bounded items and no splitting/coercion. Discloses selected addresses/domains/paths; no file contents or configuration changes.".into();
    let CapabilityRequirement::UbusMethod {
        response_contract, ..
    } = &mut operation.capability
    else {
        unreachable!("closed UCI definition has Ubus metadata");
    };
    *response_contract = format!("{name}.v2");
    let OutputMode::Typed(projection) = &mut operation.output_mode else {
        unreachable!("closed UCI definition has typed output");
    };
    let TypedProjection::Collection {
        collection: Collection::ObjectEntries { record, .. },
        ..
    } = projection.as_mut()
    else {
        unreachable!("closed UCI definition has a section map");
    };
    record
        .collections
        .extend(options.iter().map(|(name, max_bytes)| CollectionField {
            name: (*name).into(),
            presence: Presence::Optional,
            collection: Collection::TextOption {
                source: format!("/{name}"),
                max_items: 128,
                max_bytes: *max_bytes,
            },
        }));
    operation
}
