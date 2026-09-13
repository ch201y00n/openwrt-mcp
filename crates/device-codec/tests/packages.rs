//! Architecture checkpoint declaration only; no behavior or device acceptance.
#[test]
fn package_architecture_checkpoint_is_declared() {
    assert!(include_str!("../../../architecture/spec.toml").contains("apk_3_0_5_visible_v1"));
}
