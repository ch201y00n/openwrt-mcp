use openwrt_mcp_core::{Permission, Policy};
use serde_json::json;

#[test]
fn builtins_are_unique_read_only_and_deny_by_default() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    assert_eq!(catalog.operations().len(), 22);
    for operation in catalog.operations() {
        assert!(
            operation
                .requirements
                .iter()
                .all(|required| required.permission == Permission::Read)
        );
        assert!(Policy::default().authorize(operation).is_err());
    }
}

#[test]
fn malformed_board_fields_cannot_expand_into_secret_subtrees() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let operation = catalog.get("system_board").unwrap();
    let projected =
        operation.project(&json!({"kernel":{"password":"synthetic-secret"}, "model":"fixture"}));
    assert_eq!(projected, json!({"/model":"fixture"}));
}

#[test]
fn device_status_projects_counters_without_identifiers_or_raw_configuration() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let operation = catalog.get("network_device_status").unwrap();
    let projected = operation.project(&json!({
        "up":true,
        "macaddr":"synthetic-private-identifier",
        "statistics":{"rx_bytes":100,"tx_bytes":200},
        "config":{"password":"synthetic-secret"}
    }));
    assert_eq!(
        projected,
        json!({"/up":true,"/statistics/rx_bytes":100,"/statistics/tx_bytes":200})
    );
}
