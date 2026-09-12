//! Architecture-only ADR 0006 scaffold, not strict JSON decoder acceptance.

#[test]
fn architecture_checkpoint_declares_shared_strict_bounded_action_decoder() {
    let specification = include_str!("../../../architecture/spec.toml").replace("\r\n", "\n");
    let response = specification
        .split("[action_response_contract]\n")
        .nth(1)
        .unwrap()
        .split("\n[")
        .next()
        .unwrap();
    for declaration in [
        "owner = \"openwrt-mcp-device-codec\"",
        "profile = \"strict_json_v1\"",
        "consumers = [\"openwrt-mcp-adapters\", \"openwrt-mcp-backend-ssh\"]",
        "duplicate_keys = \"reject\"",
        "max_depth = 32",
        "max_nodes = 65536",
        "max_bytes = 16777216",
    ] {
        assert!(response.lines().any(|line| line.trim() == declaration));
    }
}
