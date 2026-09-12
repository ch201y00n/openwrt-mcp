use crate::definition::{argument, read};
use openwrt_mcp_core::{Category, Operation, ParameterKind};
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
    vec![logd, sysntpd]
}
