use crate::definition::{argument, read};
use openwrt_mcp_core::{
    Category, Collection, CollectionField, InnerRecord, LeafRecord, Operation, OutputMode,
    Parameter, ParameterKind, Presence, SAFE_INTEGER_MAX, ScalarField, ScalarKind, Selection,
    TextIdentity, TypedProjection,
};
use serde_json::json;

pub(crate) fn operations() -> Vec<Operation> {
    let mut logd = read(
        "service_logd_status",
        "Read running, pid and exit_code for the standard log/logd instance; a missing instance is unknown.",
        Category::Services,
        "service",
        "list",
        "service_logd_status.v1",
        &[
            "/log/instances/logd/running",
            "/log/instances/logd/pid",
            "/log/instances/logd/exit_code",
        ],
    );
    argument(&mut logd, "name", ParameterKind::String, json!("log"));
    argument(&mut logd, "verbose", ParameterKind::Boolean, json!(false));
    let mut sysntpd = read(
        "service_sysntpd_status",
        "Read running, pid and exit_code for the standard sysntpd/instance1 instance; a missing instance is unknown.",
        Category::Services,
        "service",
        "list",
        "service_sysntpd_status.v1",
        &[
            "/sysntpd/instances/instance1/running",
            "/sysntpd/instances/instance1/pid",
            "/sysntpd/instances/instance1/exit_code",
        ],
    );
    argument(
        &mut sysntpd,
        "name",
        ParameterKind::String,
        json!("sysntpd"),
    );
    argument(
        &mut sysntpd,
        "verbose",
        ParameterKind::Boolean,
        json!(false),
    );
    vec![logd, sysntpd, service_status(), service_status_list()]
}

fn service_status() -> Operation {
    let mut operation = read(
        "service_status",
        "Read one exact service's instance names and running/PID/exit metadata, not command lines, configuration or health.",
        Category::Services,
        "service",
        "list",
        "service_status.v1",
        &[],
    );
    operation.parameters.insert(
        "name".into(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: vec![],
        },
    );
    argument(
        &mut operation,
        "name",
        ParameterKind::String,
        json!("{name}"),
    );
    argument(
        &mut operation,
        "verbose",
        ParameterKind::Boolean,
        json!(false),
    );
    operation.output_mode = OutputMode::Typed(Box::new(service_projection(Selection::ExactOne {
        parameter: "name".into(),
    })));
    operation
}

fn service_status_list() -> Operation {
    let mut operation = read(
        "service_status_list",
        "List bounded service and instance names and running/PID/exit metadata across categories, without configuration or command lines.",
        Category::Services,
        "service",
        "list",
        "service_status_list.v1",
        &[],
    );
    argument(
        &mut operation,
        "verbose",
        ParameterKind::Boolean,
        json!(false),
    );
    operation.output_mode = OutputMode::Typed(Box::new(service_projection(Selection::All {})));
    operation
}

fn service_projection(selection: Selection) -> TypedProjection {
    TypedProjection::Collection {
        collection: Collection::ObjectEntries {
            source: String::new(),
            max_items: 128,
            key: TextIdentity {
                name: "name".into(),
                max_bytes: 256,
            },
            record: InnerRecord {
                fields: vec![],
                collections: vec![CollectionField {
                    name: "instances".into(),
                    presence: Presence::Optional,
                    collection: Collection::ObjectEntries {
                        source: "/instances".into(),
                        max_items: 128,
                        key: TextIdentity {
                            name: "name".into(),
                            max_bytes: 256,
                        },
                        record: LeafRecord {
                            fields: vec![
                                ScalarField {
                                    name: "running".into(),
                                    source: "/running".into(),
                                    presence: Presence::Required,
                                    value: ScalarKind::Boolean {},
                                },
                                ScalarField {
                                    name: "pid".into(),
                                    source: "/pid".into(),
                                    presence: Presence::Optional,
                                    value: ScalarKind::SafeInteger {
                                        min: 1,
                                        max: SAFE_INTEGER_MAX,
                                    },
                                },
                                ScalarField {
                                    name: "exit_code".into(),
                                    source: "/exit_code".into(),
                                    presence: Presence::Optional,
                                    value: ScalarKind::SafeInteger {
                                        min: 0,
                                        max: SAFE_INTEGER_MAX,
                                    },
                                },
                            ],
                        },
                    },
                }],
            },
        },
        selection,
    }
}
