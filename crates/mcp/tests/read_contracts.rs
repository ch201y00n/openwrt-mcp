use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, CAPABILITY_TOOL_NAME, CapabilityObservation, Category, Grant, MethodSignature,
    ObjectObservation, Policy, PreparedAction, ProbeRequest, ReviewedObject, UbusArgumentType,
    UnknownReason,
};
use openwrt_mcp_runtime::{AuditEvent, AuditSink, Backend, Dispatcher, Limits, RuntimeError};
use openwrt_mcp_transport::McpServer;
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};

const PRIVATE: &str = "synthetic-private-never-export";

struct RecordingBackend {
    calls: Mutex<Vec<PreparedAction>>,
    probes: Mutex<Vec<ProbeRequest>>,
    responses: Mutex<VecDeque<Value>>,
}

#[async_trait]
impl Backend for RecordingBackend {
    fn capability_epoch(&self) -> Option<u64> {
        Some(1)
    }

    async fn probe(
        &self,
        request: ProbeRequest,
        _: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        self.probes.lock().unwrap().push(request);
        let ProbeRequest::DescribeUbusObject(object) = request;
        let (method, arguments) = match object {
            ReviewedObject::Iwinfo => ("info", vec![("device", UbusArgumentType::String)]),
            ReviewedObject::System => (
                "watchdog",
                vec![
                    ("frequency", UbusArgumentType::Integer),
                    ("timeout", UbusArgumentType::Integer),
                    ("magicclose", UbusArgumentType::Boolean),
                    ("stop", UbusArgumentType::Boolean),
                ],
            ),
            ReviewedObject::Service => (
                "list",
                vec![
                    ("name", UbusArgumentType::String),
                    ("verbose", UbusArgumentType::Boolean),
                ],
            ),
            _ => {
                return Ok(CapabilityObservation::Unknown(
                    UnknownReason::NotObservedOrHidden,
                ));
            }
        };
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object,
            methods: BTreeMap::from([(
                method.into(),
                MethodSignature {
                    arguments: arguments
                        .into_iter()
                        .map(|(name, kind)| (name.into(), kind))
                        .collect(),
                },
            )]),
        }))
    }

    async fn execute(
        &self,
        action: &PreparedAction,
        _limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        self.calls.lock().unwrap().push(action.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(RuntimeError::BackendFailed)
    }
}

#[derive(Default)]
struct MemoryAudit(Mutex<Vec<Value>>);

impl AuditSink for MemoryAudit {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        self.0
            .lock()
            .unwrap()
            .push(serde_json::to_value(event).unwrap());
        Ok(())
    }
}

struct ReadContract {
    name: &'static str,
    category: Category,
    visible: Vec<&'static str>,
    arguments: Value,
    action: PreparedAction,
    invalid_arguments: Vec<Value>,
    responses: Vec<(Value, Value)>,
}

fn request(name: &str, arguments: Value) -> CallToolRequestParams {
    serde_json::from_value(json!({"name": name, "arguments": arguments})).unwrap()
}

fn assert_last_audit(audit: &MemoryAudit, name: &str, phase: &str, outcome: &str) {
    let events = audit.0.lock().unwrap();
    let event = events.last().unwrap();
    assert_eq!(event["operation"], name);
    assert_eq!(event["phase"], phase);
    assert_eq!(event["outcome"], outcome);
}

async fn check_read_contract(contract: ReadContract) {
    // No system grant or execution flag is needed or implicitly added.
    let policy = Policy {
        categories: BTreeMap::from([(
            contract.category,
            Grant {
                access: Access::Read,
                execute: false,
            },
        )]),
        ..Policy::default()
    };
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let hidden: Vec<String> = catalog
        .operations()
        .iter()
        .filter(|operation| !contract.visible.contains(&operation.name.as_str()))
        .map(|operation| operation.name.clone())
        .collect();
    let backend = Arc::new(RecordingBackend {
        calls: Mutex::new(Vec::new()),
        probes: Mutex::new(Vec::new()),
        responses: Mutex::new(
            contract
                .responses
                .iter()
                .map(|(response, _)| response.clone())
                .collect(),
        ),
    });
    let audit = Arc::new(MemoryAudit::default());
    let dispatcher = Dispatcher::new(
        catalog,
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
    let mut visible: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    visible.sort_unstable();
    let mut expected_visible = contract.visible.clone();
    expected_visible.push(CAPABILITY_TOOL_NAME);
    expected_visible.sort_unstable();
    assert_eq!(visible, expected_visible);
    for tool in tools {
        let metadata = serde_json::to_value(tool).unwrap();
        assert_eq!(metadata["annotations"]["readOnlyHint"], true);
        assert_eq!(metadata["inputSchema"]["additionalProperties"], false);
    }
    assert!(backend.calls.lock().unwrap().is_empty());
    assert!(backend.probes.lock().unwrap().is_empty());
    assert!(audit.0.lock().unwrap().is_empty());

    // Discovery filtering is not the security boundary: direct calls also fail.
    for name in &hidden {
        let result = client.call_tool(request(name, json!({}))).await.unwrap();
        let response = serde_json::to_value(result).unwrap();
        assert_eq!(response["isError"], true);
        let error: Value =
            serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(error["error"], "permission_denied");
        assert!(backend.calls.lock().unwrap().is_empty());
        assert!(backend.probes.lock().unwrap().is_empty());
        assert_last_audit(&audit, name, "rejection", "denied");
    }

    for arguments in &contract.invalid_arguments {
        let result = client
            .call_tool(request(contract.name, arguments.clone()))
            .await
            .unwrap();
        let response = serde_json::to_value(result).unwrap();
        assert_eq!(response["isError"], true);
        assert!(!response.to_string().contains(PRIVATE));
        assert!(backend.calls.lock().unwrap().is_empty());
        assert!(backend.probes.lock().unwrap().is_empty());
        assert_last_audit(&audit, contract.name, "rejection", "invalid_arguments");
    }

    for (index, (_, expected)) in contract.responses.iter().enumerate() {
        let result = client
            .call_tool(request(contract.name, contract.arguments.clone()))
            .await
            .unwrap();
        let response = serde_json::to_value(result).unwrap();
        assert_ne!(response["isError"], true);
        assert_eq!(&response["structuredContent"], expected);
        let text: Value =
            serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(&text, expected);
        assert!(!response.to_string().contains(PRIVATE));
        {
            let calls = backend.calls.lock().unwrap();
            assert_eq!(calls.len(), index + 1);
            assert_eq!(calls.last(), Some(&contract.action));
        }
        assert_last_audit(&audit, contract.name, "finish", "success");
    }
    assert!(backend.responses.lock().unwrap().is_empty());
    assert_eq!(backend.probes.lock().unwrap().len(), 1);
    {
        let events = audit.0.lock().unwrap();
        assert_eq!(
            events.len(),
            hidden.len() + contract.invalid_arguments.len() + 2 * contract.responses.len()
        );
        assert!(!serde_json::to_string(&*events).unwrap().contains(PRIVATE));
        for event in &*events {
            let mut keys: Vec<&str> = event
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            keys.sort_unstable();
            assert_eq!(
                keys,
                [
                    "duration_ms",
                    "kind",
                    "operation",
                    "outcome",
                    "phase",
                    "request_sequence",
                    "timestamp_ms",
                ]
            );
            assert_eq!(event["kind"], "invocation");
        }
    }
    client.cancel().await.unwrap();
    server_task.await.unwrap();
}

#[tokio::test]
async fn wireless_read_mcp_contract_is_isolated_and_secret_free() {
    let device = format!("phy0; {PRIVATE} $(not-a-command)\n--stop");
    check_read_contract(ReadContract {
        name: "wireless_radio_info",
        category: Category::Wireless,
        visible: vec!["wireless_radio_info"],
        arguments: json!({"device": device}),
        action: PreparedAction::Ubus {
            object: "iwinfo".into(),
            method: "info".into(),
            arguments: json!({"device": device}),
        },
        invalid_arguments: vec![
            json!({}),
            json!({"device": true}),
            json!({"device": {"name": PRIVATE}}),
            json!({"device": "phy0", "method": PRIVATE}),
            json!({"device": "phy0", "object": PRIVATE}),
            json!({"device": "phy0", "execute": true}),
        ],
        responses: vec![
            (
                json!({
                    "phy": "phy0", "mode": "Master", "country": "KR",
                    "channel": 36, "frequency": 5180, "txpower": 20,
                    "quality": 60, "quality_max": 70, "signal": -48,
                    "noise": -95, "bitrate": 866700,
                    "encryption": {"enabled": true, "key": PRIVATE},
                    "ssid": PRIVATE, "bssid": PRIVATE,
                    "config": {"wifi_key": PRIVATE}, "password": PRIVATE
                }),
                json!({
                    "/phy": "phy0", "/mode": "Master", "/country": "KR",
                    "/channel": 36, "/frequency": 5180, "/txpower": 20,
                    "/quality": 60, "/quality_max": 70, "/signal": -48,
                    "/noise": -95, "/bitrate": 866700, "/encryption/enabled": true
                }),
            ),
            (
                json!({
                    "channel": {"value": 36, "password": PRIVATE},
                    "signal": [PRIVATE],
                    "encryption": {"enabled": {"secret": PRIVATE}},
                    "ssid": PRIVATE
                }),
                json!({}),
            ),
            (json!({"ssid": PRIVATE}), json!({})),
        ],
    })
    .await;
}

#[tokio::test]
async fn watchdog_read_mcp_contract_never_accepts_setter_arguments() {
    check_read_contract(ReadContract {
        name: "diagnostics_watchdog_status",
        category: Category::Diagnostics,
        visible: vec!["diagnostics_watchdog_status"],
        arguments: json!({}),
        action: PreparedAction::Ubus {
            object: "system".into(),
            method: "watchdog".into(),
            arguments: json!({}),
        },
        invalid_arguments: vec![
            json!({"stop": true}),
            json!({"stop": false}),
            json!({"frequency": 5}),
            json!({"timeout": 60}),
            json!({"magicclose": true}),
            json!({"password": PRIVATE}),
        ],
        responses: vec![
            (
                json!({
                    "status": "running", "timeout": 30, "frequency": 5,
                    "magicclose": false, "password": PRIVATE,
                    "config": {"key": PRIVATE}
                }),
                json!({
                    "/status": "running", "/timeout": 30,
                    "/frequency": 5, "/magicclose": false
                }),
            ),
            (
                json!({
                    "status": {"secret": PRIVATE}, "timeout": [PRIVATE],
                    "frequency": {"value": 5, "key": PRIVATE},
                    "magicclose": [false, PRIVATE]
                }),
                json!({}),
            ),
            (json!({}), json!({})),
        ],
    })
    .await;
}

#[tokio::test]
async fn logd_read_mcp_contract_never_exports_commands_or_invents_missing_state() {
    check_read_contract(ReadContract {
        name: "service_logd_status",
        category: Category::Services,
        visible: vec!["service_logd_status", "service_sysntpd_status"],
        arguments: json!({}),
        action: PreparedAction::Ubus {
            object: "service".into(),
            method: "list".into(),
            arguments: json!({"name": "log", "verbose": false}),
        },
        invalid_arguments: vec![
            json!({"name": PRIVATE}),
            json!({"verbose": true}),
            json!({"instance": PRIVATE}),
            json!({"command": PRIVATE}),
        ],
        responses: vec![
            (
                json!({"log": {
                    "data": {"password": PRIVATE},
                    "instances": {"logd": {
                        "running": true, "pid": 4242,
                        "command": [PRIVATE], "env": {"TOKEN": PRIVATE},
                        "data": {"key": PRIVATE}, "errors": [PRIVATE]
                    }, "logremote": {"running": true, "command": [PRIVATE]}}
                }}),
                json!({
                    "/log/instances/logd/running": true,
                    "/log/instances/logd/pid": 4242
                }),
            ),
            (
                json!({"log": {"instances": {"logd": {
                    "running": {"value": true, "secret": PRIVATE},
                    "pid": [PRIVATE], "exit_code": {"secret": PRIVATE}
                }}}}),
                json!({}),
            ),
            (
                json!({"log": {"instances": {"logremote": {"running": true}}}}),
                json!({}),
            ),
            (
                json!({"log": {"instances": {"logd": {
                    "running": false, "exit_code": 1, "command": [PRIVATE]
                }}}}),
                json!({
                    "/log/instances/logd/running": false,
                    "/log/instances/logd/exit_code": 1
                }),
            ),
        ],
    })
    .await;
}
