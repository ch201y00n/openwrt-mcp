//! Synthetic end-to-end MCP package paging; no live-device acceptance.
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, Category, Grant, Policy, PreparedAction,
    packages::{OpkgStatusRecord, PackageObservation, PackageRecord},
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
    async fn capture_opkg_status(&self, _: &Limits) -> Result<PackageObservation, RuntimeError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(PackageObservation::new_opkg_status(
            (0..281)
                .map(|i| OpkgStatusRecord {
                    name: format!("fixture{i:04}"),
                    version: "1~test-r1".into(),
                    arch: "aarch64".into(),
                    status: "install hold,user unpacked".into(),
                })
                .collect(),
        )
        .unwrap())
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
struct Log(std::sync::Mutex<Vec<AuditEvent>>);
impl AuditSink for Log {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        self.0.lock().unwrap().push(event.clone());
        Ok(())
    }
}
async fn scenario(allowed: bool, name: &str) {
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
    let log = Arc::new(Log(Default::default()));
    let dispatcher = Dispatcher::new(
        openwrt_mcp_features::catalog(vec![]).unwrap(),
        policy,
        target.clone(),
        log.clone(),
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
    assert_eq!(tools.len(), if allowed { 3 } else { 0 });
    assert_eq!(target.0.load(Ordering::SeqCst), 0);
    if allowed {
        let tool = tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(
            serde_json::to_value(&tool.annotations).unwrap()["readOnlyHint"],
            true
        );
        assert_eq!(
            serde_json::to_value(&tool.annotations).unwrap()["idempotentHint"],
            false
        );
        let request: CallToolRequestParams = serde_json::from_value(
            json!({"name":"operation_capability","arguments":{"operation":name}}),
        )
        .unwrap();
        let status = serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
        assert_eq!(status["structuredContent"]["reason"], "capture_required");
        assert_eq!(
            status["structuredContent"]["response_contract"],
            format!("{name}.v1")
        );
        assert_eq!(
            status["structuredContent"]["scope"],
            if name == "packages_opkg_status" {
                "closed_file_response"
            } else {
                "closed_query_response"
            }
        );
        for arguments in [
            json!({"path":"/etc/shadow"}),
            json!({"manager":"auto"}),
            json!({"cursor":null}),
            json!({"cursor":""}),
        ] {
            let request: CallToolRequestParams =
                serde_json::from_value(json!({"name":name,"arguments":arguments})).unwrap();
            let response = serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
            assert_eq!(response["isError"], true);
            assert!(response.get("structuredContent").is_none());
            let error: Value =
                serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(error.as_object().unwrap().len(), 1);
            assert!(error["error"].as_str().is_some());
        }
        assert_eq!(target.0.load(Ordering::SeqCst), 0);
    }
    let mut args = json!({});
    let mut count = 0;
    let mut first_cursor = None;
    loop {
        let request: CallToolRequestParams =
            serde_json::from_value(json!({"name":name,"arguments":args})).unwrap();
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
        assert_eq!(
            page["scope"],
            if name == "packages_opkg_status" {
                "opkg_root_status_file"
            } else {
                "apk_query_installed_visible"
            }
        );
        for row in page["items"].as_array().unwrap() {
            assert_eq!(row.as_object().unwrap().len(), 4);
            if name == "packages_opkg_status" {
                assert_eq!(row["status"], "install hold,user unpacked");
                assert!(row.get("layer").is_none());
            } else {
                assert_eq!(row["layer"], 0);
                assert!(row.get("status").is_none());
            }
        }
        if first_cursor.is_none() {
            first_cursor = page.get("next_cursor").cloned();
        }
        assert!(page["items"].as_array().unwrap().len() <= 16);
        count += page["items"].as_array().unwrap().len();
        let Some(cursor) = page.get("next_cursor") else {
            break;
        };
        args = json!({"cursor":cursor});
    }
    assert_eq!(count, if allowed { 281 } else { 0 });
    assert_eq!(target.0.load(Ordering::SeqCst), usize::from(allowed));
    if allowed {
        let other = if name == "packages_opkg_status" {
            "packages_apk_installed"
        } else {
            "packages_opkg_status"
        };
        let request: CallToolRequestParams = serde_json::from_value(
            json!({"name":other,"arguments":{"cursor":first_cursor.unwrap()}}),
        )
        .unwrap();
        let response = serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
        assert_eq!(response["isError"], true);
        assert_eq!(
            serde_json::from_str::<Value>(response["content"][0]["text"].as_str().unwrap())
                .unwrap(),
            json!({"error":"invalid_cursor"})
        );
        assert!(response.get("structuredContent").is_none());
        assert_eq!(target.0.load(Ordering::SeqCst), 1);
    }
    client.cancel().await.unwrap();
    server.await.unwrap();
    let events = log.0.lock().unwrap();
    assert_eq!(events.len(), if allowed { 44 } else { 1 });
    let encoded = serde_json::to_string(&*events).unwrap();
    for event in events.iter() {
        let value = serde_json::to_value(event).unwrap();
        let keys: std::collections::BTreeSet<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "kind",
                "timestamp_ms",
                "request_sequence",
                "phase",
                "operation",
                "outcome",
                "duration_ms"
            ]
            .into()
        );
    }
    for excluded in [
        "fixture0000",
        "aarch64",
        "hold,user",
        "next_cursor",
        "/etc/shadow",
        "\"arguments\":",
    ] {
        assert!(!encoded.contains(excluded));
    }
}
#[tokio::test]
async fn protocol_enumerates_all_pages_and_preserves_both_bounded_result_copies() {
    for name in ["packages_apk_installed", "packages_opkg_status"] {
        tokio::time::timeout(std::time::Duration::from_secs(10), scenario(true, name))
            .await
            .unwrap();
    }
}
#[tokio::test]
async fn denied_package_tool_is_hidden_and_cannot_capture() {
    for name in ["packages_apk_installed", "packages_opkg_status"] {
        tokio::time::timeout(std::time::Duration::from_secs(10), scenario(false, name))
            .await
            .unwrap();
    }
}
