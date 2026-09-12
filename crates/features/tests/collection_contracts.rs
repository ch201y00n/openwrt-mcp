//! Architecture-only ADR 0006 scaffold; no collection operation is claimed yet.

#[test]
fn architecture_checkpoint_forbids_builtin_raw_subtrees_and_implicit_map_keys() {
    let contract: toml::Value =
        toml::from_str(include_str!("../../../architecture/spec.toml")).unwrap();
    let projection = &contract["projection_contract"];
    for (field, expected) in [
        ("profile", "typed_collections_v1"),
        ("root_selection", "exact_parameter_equality"),
        ("map_keys", "explicit_bounded_field"),
        ("builtin_raw_subtrees", "forbidden"),
        ("invalid_shape", "reject"),
        ("overflow", "reject"),
    ] {
        assert_eq!(projection[field].as_str(), Some(expected));
    }
}
