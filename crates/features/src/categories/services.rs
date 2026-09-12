use crate::definition::read;
use openwrt_mcp_core::{Action, Category, Operation};
use serde_json::json;

pub(crate) fn operations() -> Vec<Operation> {
    let mut logd = read(
        "service_logd_status",
        "Read running, pid and exit_code for the standard log/logd instance; a missing instance is unknown.",
        Category::Services,
        "service",
        "list",
        &[
            "/log/instances/logd/running",
            "/log/instances/logd/pid",
            "/log/instances/logd/exit_code",
        ],
    );
    if let Action::Ubus { arguments, .. } = &mut logd.action {
        arguments.insert("name".to_owned(), json!("log"));
        arguments.insert("verbose".to_owned(), json!(false));
    }
    let mut sysntpd = read(
        "service_sysntpd_status",
        "Read running, pid and exit_code for the standard sysntpd/instance1 instance; a missing instance is unknown.",
        Category::Services,
        "service",
        "list",
        &[
            "/sysntpd/instances/instance1/running",
            "/sysntpd/instances/instance1/pid",
            "/sysntpd/instances/instance1/exit_code",
        ],
    );
    if let Action::Ubus { arguments, .. } = &mut sysntpd.action {
        arguments.insert("name".to_owned(), json!("sysntpd"));
        arguments.insert("verbose".to_owned(), json!(false));
    }
    vec![logd, sysntpd]
}
