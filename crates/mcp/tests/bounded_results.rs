//! Architecture-only ADR 0006 scaffold, not MCP serialization acceptance.

#[test]
fn architecture_checkpoint_caps_entire_call_tool_result() {
    let contract: toml::Value =
        toml::from_str(include_str!("../../../architecture/spec.toml")).unwrap();
    let result = &contract["mcp_result_contract"];
    assert_eq!(result["owner"].as_str(), Some("openwrt-mcp-transport"));
    assert_eq!(result["max_serialized_bytes"].as_integer(), Some(262_144));
    assert_eq!(
        result["serialized_scope"].as_str(),
        Some("entire_call_tool_result")
    );
}
