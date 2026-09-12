//! Architecture checkpoint only: real metadata gating tests follow ADR 0005's gate.
use std::{collections::BTreeSet, fs, path::Path};

use openwrt_mcp_core::Action;

#[test]
fn checkpoint_covers_the_existing_builtin_object_surface_without_device_claims() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = fs::read_to_string(root.join("architecture/capability-probes.toml")).unwrap();
    let operations = openwrt_mcp_features::builtins();
    assert_eq!(operations.len(), 10);
    let mut objects = BTreeSet::new();
    for operation in &operations {
        let Action::Ubus { object, .. } = &operation.action else {
            panic!("the v5 checkpoint is limited to the ten existing ubus reads");
        };
        assert!(registry.contains(&format!("object = \"{object}\"")));
        objects.insert(object.as_str());
    }
    assert_eq!(objects.len(), 7);
    assert!(
        registry
            .contains("operation_metadata = \"required_ubus_method_signature_response_contract\"")
    );
    assert!(registry.contains("process_capability = \"unverified_blocked\""));
}

#[test]
fn compatibility_evidence_only_names_actual_installed_builtin_operations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let evidence: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("compatibility/evidence.toml")).unwrap())
            .unwrap();
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    for record in evidence["records"].as_array().unwrap() {
        for operation in record["operations"].as_array().unwrap() {
            assert!(
                catalog.get(operation.as_str().unwrap()).is_some(),
                "evidence names an operation absent from the actual catalog"
            );
        }
    }
    // The empty initial manifest is not evidence of operation acceptance.
    assert!(catalog.get("unimplemented_fixture_operation").is_none());
}
