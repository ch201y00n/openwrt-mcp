use async_trait::async_trait;
use openwrt_mcp::{config::Config, protocol::McpServer};
use openwrt_mcp_core::Invocation;
use openwrt_mcp_runtime::{AuditEvent, AuditSink, Backend, Dispatcher, Limits, RuntimeError};
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct FixtureBackend(AtomicUsize);

#[async_trait]
impl Backend for FixtureBackend {
    async fn execute(
        &self,
        _invocation: &Invocation,
        _limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(
            json!({"uptime": 123, "memory": {"total": 1024, "free": 512}, "password": "synthetic-never-export", "private_key": "synthetic-key"}),
        )
    }
}

#[derive(Default)]
struct MemoryAudit(Mutex<Vec<String>>);

impl AuditSink for MemoryAudit {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        self.0
            .lock()
            .unwrap()
            .push(serde_json::to_string(event).unwrap());
        Ok(())
    }
}

#[tokio::test]
async fn real_mcp_handshake_discovery_calls_and_hidden_tool_denial() {
    let config: Config = toml::from_str("[policy.categories.system]\naccess = 'read'\n").unwrap();
    let backend = Arc::new(FixtureBackend(AtomicUsize::new(0)));
    let audit = Arc::new(MemoryAudit::default());
    let dispatcher = Dispatcher::new(
        config.catalog().unwrap(),
        config.policy,
        backend.clone(),
        audit.clone(),
        config.limits,
    )
    .unwrap();
    let (client_io, server_io) = tokio::io::duplex(65_536);
    let server_task = tokio::spawn(async move {
        McpServer::new(dispatcher)
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap()
    });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let names: Vec<_> = tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"system_info"));
    assert!(names.contains(&"system_board"));
    assert!(!names.contains(&"network_wan_status"));

    let call: CallToolRequestParams =
        serde_json::from_value(json!({"name": "system_info", "arguments": {}})).unwrap();
    let result = serde_json::to_value(client.call_tool(call).await.unwrap()).unwrap();
    assert_ne!(result["isError"], true);
    assert!(result.to_string().contains("123"));
    assert!(!result.to_string().contains("synthetic-never-export"));
    assert!(!result.to_string().contains("synthetic-key"));

    for name in ["network_wan_status", "unknown-synthetic-private-value"] {
        let call: CallToolRequestParams =
            serde_json::from_value(json!({"name": name, "arguments": {}})).unwrap();
        let result = serde_json::to_value(client.call_tool(call).await.unwrap()).unwrap();
        assert_eq!(result["isError"], true);
    }
    let malformed: CallToolRequestParams = serde_json::from_value(
        json!({"name": "system_info", "arguments": {"password": "synthetic-argument"}}),
    )
    .unwrap();
    let result = serde_json::to_value(client.call_tool(malformed).await.unwrap()).unwrap();
    assert_eq!(result["isError"], true);
    assert_eq!(backend.0.load(Ordering::SeqCst), 1);
    let audit_text = audit.0.lock().unwrap().join("\n");
    assert!(!audit_text.contains("synthetic"));
    assert!(audit_text.contains("system_info"));
    client.cancel().await.unwrap();
    server_task.await.unwrap();
}
