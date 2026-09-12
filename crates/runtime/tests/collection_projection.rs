//! Architecture-only ADR 0006 scaffold, not dispatcher behavior acceptance.

#[test]
fn architecture_checkpoint_places_normalized_limit_before_finish_audit() {
    let specification = include_str!("../../../architecture/spec.toml").replace("\r\n", "\n");
    let result = specification
        .split("[mcp_result_contract]\n")
        .nth(1)
        .unwrap()
        .split("\n[")
        .next()
        .unwrap();
    for declaration in [
        "normalized_owner = \"openwrt-mcp-runtime\"",
        "normalized_phase = \"before_finish_audit\"",
    ] {
        assert!(result.lines().any(|line| line.trim() == declaration));
    }
    let projection = specification
        .split("[projection_contract]\n")
        .nth(1)
        .unwrap()
        .split("\n[")
        .next()
        .unwrap();
    assert!(
        projection
            .lines()
            .any(|line| line.trim() == "max_normalized_bytes = 65536")
    );
}
