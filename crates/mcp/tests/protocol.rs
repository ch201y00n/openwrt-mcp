use async_trait::async_trait;
use openwrt_mcp_core::{
    CAPABILITY_TOOL_NAME, CapabilityObservation, MethodSignature, ObjectObservation, Policy,
    PreparedAction, ProbeRequest, ReviewedObject, UnknownReason,
};
use openwrt_mcp_runtime::{AuditEvent, AuditSink, Backend, Dispatcher, Limits, RuntimeError};
use openwrt_mcp_transport::McpServer;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct FixtureBackend {
    calls: AtomicUsize,
    probes: AtomicUsize,
    visible: AtomicBool,
}

impl FixtureBackend {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            probes: AtomicUsize::new(0),
            visible: AtomicBool::new(true),
        }
    }
}

#[async_trait]
impl Backend for FixtureBackend {
    fn capability_epoch(&self) -> Option<u64> {
        Some(1)
    }

    async fn probe(
        &self,
        request: ProbeRequest,
        _: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        self.probes.fetch_add(1, Ordering::SeqCst);
        if request != ProbeRequest::DescribeUbusObject(ReviewedObject::System)
            || !self.visible.load(Ordering::SeqCst)
        {
            return Ok(CapabilityObservation::Unknown(
                UnknownReason::NotObservedOrHidden,
            ));
        }
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object: ReviewedObject::System,
            methods: ["info", "board"]
                .into_iter()
                .map(|method| {
                    (
                        method.into(),
                        MethodSignature {
                            arguments: BTreeMap::new(),
                        },
                    )
                })
                .collect(),
        }))
    }

    async fn execute(
        &self,
        _invocation: &PreparedAction,
        _limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
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
    let policy: Policy = toml::from_str("[categories.system]\naccess = 'read'\n").unwrap();
    let backend = Arc::new(FixtureBackend::new());
    let audit = Arc::new(MemoryAudit::default());
    let dispatcher = Dispatcher::new(
        openwrt_mcp_features::catalog(vec![]).unwrap(),
        policy,
        backend.clone(),
        audit.clone(),
        Limits::default(),
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
    assert!(names.contains(&CAPABILITY_TOOL_NAME));
    assert_eq!(backend.probes.load(Ordering::SeqCst), 0);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);

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
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    assert_eq!(backend.probes.load(Ordering::SeqCst), 1);
    let audit_text = audit.0.lock().unwrap().join("\n");
    assert!(!audit_text.contains("synthetic"));
    assert!(audit_text.contains("system_info"));
    client.cancel().await.unwrap();
    server_task.await.unwrap();
}

fn request(name: &str, arguments: Value) -> CallToolRequestParams {
    serde_json::from_value(json!({"name": name, "arguments": arguments})).unwrap()
}

#[tokio::test]
async fn metadata_status_reuses_private_cache_and_failed_refresh_prevents_execution() {
    let policy: Policy = toml::from_str("[categories.system]\naccess='read'\n").unwrap();
    let backend = Arc::new(FixtureBackend::new());
    let audit = Arc::new(MemoryAudit::default());
    let dispatcher = Dispatcher::new(
        openwrt_mcp_features::catalog(vec![]).unwrap(),
        policy,
        backend.clone(),
        audit.clone(),
        Limits::default(),
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
    let metadata = serde_json::to_value(
        tools
            .iter()
            .find(|tool| tool.name == CAPABILITY_TOOL_NAME)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["inputSchema"]["additionalProperties"], false);
    assert_eq!(metadata["inputSchema"]["required"], json!(["operation"]));
    assert_eq!(metadata["annotations"]["readOnlyHint"], true);
    assert_eq!(backend.probes.load(Ordering::SeqCst), 0);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 0);

    for operation in ["system_info", "system_board"] {
        let result = serde_json::to_value(
            client
                .call_tool(request(
                    CAPABILITY_TOOL_NAME,
                    json!({"operation":operation}),
                ))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_ne!(result["isError"], true);
        let status = &result["structuredContent"];
        assert_eq!(status["compatibility"], "compatible");
        assert_eq!(status["reason"], "signature_matched");
        assert_eq!(status["scope"], "input_signature_only");
        assert_eq!(status["response_contract"], format!("{operation}.v1"));
        assert!(
            status["remaining_ttl_ms"]
                .as_u64()
                .is_some_and(|ttl| ttl <= 30_000)
        );
        assert_eq!(status.as_object().unwrap().len(), 5);
        assert!(!result.to_string().contains("synthetic"));
        assert_eq!(backend.probes.load(Ordering::SeqCst), 1);
        assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
    }
    let normal = serde_json::to_value(
        client
            .call_tool(request("system_info", json!({})))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(normal["structuredContent"]["/uptime"], 123);
    assert_eq!(backend.probes.load(Ordering::SeqCst), 1);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);

    backend.visible.store(false, Ordering::SeqCst);
    let refreshed = serde_json::to_value(
        client
            .call_tool(request(
                CAPABILITY_TOOL_NAME,
                json!({"operation":"system_info","refresh":true}),
            ))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_ne!(refreshed["isError"], true);
    assert_eq!(refreshed["structuredContent"]["compatibility"], "unknown");
    assert_eq!(
        refreshed["structuredContent"]["reason"],
        "not_observed_or_hidden"
    );
    let blocked = serde_json::to_value(
        client
            .call_tool(request("system_info", json!({})))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(blocked["isError"], true);
    let error: Value =
        serde_json::from_str(blocked["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(error, json!({"error":"capability_unknown"}));
    assert_eq!(backend.probes.load(Ordering::SeqCst), 2);
    assert_eq!(backend.calls.load(Ordering::SeqCst), 1);
    {
        let text = audit.0.lock().unwrap().join("\n");
        let events: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            events
                .iter()
                .filter(|event| event["kind"] == "capability")
                .count(),
            6
        );
        assert!(
            events
                .iter()
                .filter(|event| event["kind"] == "capability")
                .all(|event| event["operation"].as_str().is_some_and(|name| [
                    "system_info",
                    "system_board"
                ]
                .contains(&name)))
        );
        assert_eq!(events.last().unwrap()["outcome"], "failed");
        assert!(!text.contains("synthetic"));
        assert!(!text.contains("remaining_ttl_ms"));
        assert!(!text.contains("methods"));
    }
    client.cancel().await.unwrap();
    server_task.await.unwrap();
}

#[tokio::test]
async fn metadata_denied_unknown_and_malformed_requests_are_audited_without_device_io() {
    let policy: Policy = toml::from_str("[categories.system]\naccess='read'\n").unwrap();
    let backend = Arc::new(FixtureBackend::new());
    let audit = Arc::new(MemoryAudit::default());
    let dispatcher = Dispatcher::new(
        openwrt_mcp_features::catalog(vec![]).unwrap(),
        policy,
        backend.clone(),
        audit.clone(),
        Limits::default(),
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
    for (arguments, expected) in [
        (
            json!({"operation":"network_wan_status","refresh":true}),
            "permission_denied",
        ),
        (
            json!({"operation":"synthetic-private-unknown"}),
            "unknown_operation",
        ),
        (json!({}), "invalid_arguments"),
        (json!({"operation":true}), "invalid_arguments"),
        (
            json!({"operation":"system_info","refresh":"synthetic-private"}),
            "invalid_arguments",
        ),
        (
            json!({"operation":"system_info","refresh":null}),
            "invalid_arguments",
        ),
        (
            json!({"operation":"system_info","host":"synthetic-private"}),
            "invalid_arguments",
        ),
        (
            json!({"operation":"system_info","arguments":{"password":"synthetic-private"}}),
            "invalid_arguments",
        ),
    ] {
        let result = serde_json::to_value(
            client
                .call_tool(request(CAPABILITY_TOOL_NAME, arguments))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(result["isError"], true);
        let error: Value =
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(error, json!({"error":expected}));
        assert!(!result.to_string().contains("synthetic"));
        assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.probes.load(Ordering::SeqCst), 0);
    }
    {
        let text = audit.0.lock().unwrap().join("\n");
        let events: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 8);
        assert!(
            events
                .iter()
                .all(|event| event["kind"] == "capability" && event["phase"] == "rejection")
        );
        assert!(!text.contains("synthetic"));
    }
    client.cancel().await.unwrap();
    server_task.await.unwrap();
}
