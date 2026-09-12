//! Architecture-only ADR 0006 scaffold, not implemented projection acceptance.
//! Replace/extend after the validated architecture checkpoint with behavior tests.

#[test]
fn architecture_checkpoint_declares_finite_bounded_collection_forms() {
    let contract: toml::Value =
        toml::from_str(include_str!("../../../architecture/spec.toml")).unwrap();
    let projection = &contract["projection_contract"];
    assert_eq!(projection["owner"].as_str(), Some("openwrt-mcp-core"));
    assert_eq!(
        projection["prepared_invocation"].as_str(),
        Some("private_core")
    );
    assert_eq!(
        projection["finite_schema"].as_str(),
        Some("two_collection_levels")
    );
    assert_eq!(
        projection["node_forms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Record", "ObjectArray", "ObjectEntries", "ScalarArray"]
    );
    for (field, ceiling) in [
        ("max_collection_depth", 2),
        ("max_collection_nodes", 8),
        ("max_total_items", 256),
        ("max_items_per_collection", 256),
        ("max_emitted_items", 256),
        ("max_fields_per_record", 64),
        ("max_schema_depth", 8),
        ("max_pointer_depth", 8),
        ("max_string_bytes", 1024),
        ("max_pointer_bytes", 512),
        ("max_output_name_bytes", 64),
        ("max_normalized_bytes", 65_536),
    ] {
        assert_eq!(projection[field].as_integer(), Some(ceiling));
    }
}
