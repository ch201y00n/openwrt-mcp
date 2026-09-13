//! Synthetic end-to-end MCP package paging; no live-device acceptance.
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, Category, Grant, Policy, PreparedAction,
    packages::{PackageObservation, PackageRecord},
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditSink, Backend, Dispatcher, Limits, RuntimeError, packages::SnapshotTokens,
};
use openwrt_mcp_transport::{MAX_CALL_RESULT_BYTES, McpServer};
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
struct Target(AtomicUsize);
#[async_trait]
impl Backend for Target {
    fn capability_epoch(&self) -> Option<u64> {
        Some(1)
    }
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        panic!("unexpected generic execute")
    }
    async fn capture_apk_installed(&self, _: &Limits) -> Result<PackageObservation, RuntimeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(PackageObservation::new(
            (0..281)
                .map(|i| PackageRecord {
                    name: format!("fixture{i:04}"),
                    version: "1~test-r1".into(),
                    arch: "aarch64".into(),
                    layer: 0,
                })
                .collect(),
        )
        .unwrap())
    }
}
struct Tokens;
#[async_trait]
impl SnapshotTokens for Tokens {
    async fn nonce(&self) -> Result<[u8; 16], RuntimeError> {
        Ok([1; 16])
    }
}
struct Log;
impl AuditSink for Log {
    fn record(&self, _: &AuditEvent) -> Result<(), RuntimeError> {
        Ok(())
    }
}
async fn scenario(allowed: bool) {
    let policy = if allowed {
        Policy {
            categories: [(
                Category::Packages,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            )]
            .into(),
            ..Default::default()
        }
    } else {
        Policy::default()
    };
    let target = Arc::new(Target(AtomicUsize::new(0)));
    let dispatcher = Dispatcher::new(
        openwrt_mcp_features::catalog(vec![]).unwrap(),
        policy,
        target.clone(),
        Arc::new(Log),
        Limits::default(),
    )
    .unwrap()
    .with_snapshot_tokens(Arc::new(Tokens));
    let (client_io, server_io) = tokio::io::duplex(65536);
    let server = tokio::spawn(async move {
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
    assert_eq!(tools.len(), if allowed { 2 } else { 0 });
    assert_eq!(target.0.load(Ordering::SeqCst), 0);
    if allowed {
        let tool = tools
            .iter()
            .find(|t| t.name == "packages_apk_installed")
            .unwrap();
        assert_eq!(
            serde_json::to_value(&tool.annotations).unwrap()["readOnlyHint"],
            true
        );
        assert_eq!(
            serde_json::to_value(&tool.annotations).unwrap()["idempotentHint"],
            false
        );
        let request:CallToolRequestParams=serde_json::from_value(json!({"name":"operation_capability","arguments":{"operation":"packages_apk_installed"}})).unwrap();
        let status = serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
        assert_eq!(status["structuredContent"]["reason"], "capture_required");
        assert_eq!(target.0.load(Ordering::SeqCst), 0);
    }
    let mut args = json!({});
    let mut count = 0;
    loop {
        let request: CallToolRequestParams =
            serde_json::from_value(json!({"name":"packages_apk_installed","arguments":args}))
                .unwrap();
        let response = serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
        assert!(serde_json::to_vec(&response).unwrap().len() <= MAX_CALL_RESULT_BYTES);
        if !allowed {
            assert_eq!(response["isError"], true);
            break;
        }
        assert_ne!(response["isError"], true);
        let page = &response["structuredContent"];
        assert_eq!(
            serde_json::from_str::<Value>(response["content"][0]["text"].as_str().unwrap())
                .unwrap(),
            *page
        );
        assert_eq!(page["whole_device_complete"], false);
        assert!(page["items"].as_array().unwrap().len() <= 16);
        count += page["items"].as_array().unwrap().len();
        let Some(cursor) = page.get("next_cursor") else {
            break;
        };
        args = json!({"cursor":cursor});
    }
    assert_eq!(count, if allowed { 281 } else { 0 });
    assert_eq!(target.0.load(Ordering::SeqCst), usize::from(allowed));
    client.cancel().await.unwrap();
    server.await.unwrap();
}
#[tokio::test]
async fn protocol_enumerates_all_pages_and_preserves_both_bounded_result_copies() {
    tokio::time::timeout(std::time::Duration::from_secs(10), scenario(true))
        .await
        .unwrap();
}
#[tokio::test]
async fn denied_package_tool_is_hidden_and_cannot_capture() {
    tokio::time::timeout(std::time::Duration::from_secs(10), scenario(false))
        .await
        .unwrap();
}
