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
            ReviewedObject::NetworkInterface => ("dump", vec![]),
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
        let mut methods = BTreeMap::from([(
            method.into(),
            MethodSignature {
                arguments: arguments
                    .into_iter()
                    .map(|(name, kind)| (name.into(), kind))
                    .collect(),
            },
        )]);
        if object == ReviewedObject::Iwinfo {
            for method in ["assoclist", "countrylist"] {
                methods.insert(
                    method.into(),
                    MethodSignature {
                        arguments: [("device".into(), UbusArgumentType::String)].into(),
                    },
                );
            }
            methods.insert(
                "devices".into(),
                MethodSignature {
                    arguments: BTreeMap::new(),
                },
            );
        }
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object,
            methods,
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
    invalid_outputs: Vec<(Value, &'static str)>,
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
                .chain(
                    contract
                        .invalid_outputs
                        .iter()
                        .map(|(response, _)| response.clone()),
                )
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
    for (index, (_, expected_error)) in contract.invalid_outputs.iter().enumerate() {
        let result = client
            .call_tool(request(contract.name, contract.arguments.clone()))
            .await
            .unwrap();
        let response = serde_json::to_value(result).unwrap();
        assert_eq!(response["isError"], true);
        assert!(response.get("structuredContent").is_none());
        let error: Value =
            serde_json::from_str(response["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(error, json!({"error": expected_error}));
        assert!(!response.to_string().contains(PRIVATE));
        let calls = backend.calls.lock().unwrap();
        assert_eq!(calls.len(), contract.responses.len() + index + 1);
        assert_eq!(calls.last(), Some(&contract.action));
        assert_last_audit(&audit, contract.name, "finish", "failed");
    }
    assert!(backend.responses.lock().unwrap().is_empty());
    assert_eq!(backend.probes.lock().unwrap().len(), 1);
    {
        let events = audit.0.lock().unwrap();
        assert_eq!(
            events.len(),
            hidden.len()
                + contract.invalid_arguments.len()
                + 2 * (contract.responses.len() + contract.invalid_outputs.len())
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
        visible: wireless_tools(),
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
        invalid_outputs: vec![],
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
        invalid_outputs: vec![],
    })
    .await;
}

#[tokio::test]
async fn logd_read_mcp_contract_never_exports_commands_or_invents_missing_state() {
    check_read_contract(ReadContract {
        name: "service_logd_status",
        category: Category::Services,
        visible: vec![
            "service_logd_status",
            "service_sysntpd_status",
            "service_status",
            "service_status_list",
        ],
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
        invalid_outputs: vec![],
    })
    .await;
}

fn interface(name: &str) -> Value {
    json!({"interface": name, "up": true, "pending": false,
        "available": true, "autostart": true, "dynamic": false})
}

fn network_tools() -> Vec<&'static str> {
    vec![
        "network_device_status",
        "network_lan_status",
        "network_wan_status",
        "network_interface_status",
        "network_interfaces",
    ]
}

fn service_tools() -> Vec<&'static str> {
    vec![
        "service_logd_status",
        "service_sysntpd_status",
        "service_status",
        "service_status_list",
    ]
}

#[tokio::test]
async fn network_list_mcp_uses_fixed_dump_and_rejects_partial_or_oversized_results() {
    let clean = interface("lan");
    let mut raw = clean.clone();
    raw["ipv4-address"] = json!([PRIVATE]);
    raw["data"] = json!({"password": PRIVATE});
    check_read_contract(ReadContract {
        name: "network_interfaces", category: Category::Network, visible: network_tools(),
        arguments: json!({}),
        action: PreparedAction::Ubus { object: "network.interface".into(), method: "dump".into(), arguments: json!({}) },
        invalid_arguments: vec![json!({"interface": "lan"}), json!({"method": "up"}), json!({"execute": true})],
        responses: vec![(json!({"interface": [raw]}), json!({"items": [clean]})),
            (json!({"interface": []}), json!({"items": []}))],
        invalid_outputs: vec![(json!({}), "invalid_output"), (json!({"interface": null}), "invalid_output"),
            (json!({"interface": (0..129).map(|i| interface(&format!("if{i}"))).collect::<Vec<_>>()}), "output_limit")],
    }).await;
}

#[tokio::test]
async fn interface_mcp_selector_is_local_exact_and_validates_unselected_rows() {
    let name = "lan; $(not-a-command) / ' 한글";
    let clean = interface(name);
    let mut raw = clean.clone();
    raw["route"] = json!([PRIVATE]);
    let mut malformed = interface("other");
    malformed["up"] = json!({"secret": PRIVATE});
    check_read_contract(ReadContract {
        name: "network_interface_status",
        category: Category::Network,
        visible: network_tools(),
        arguments: json!({"interface": name}),
        action: PreparedAction::Ubus {
            object: "network.interface".into(),
            method: "dump".into(),
            arguments: json!({}),
        },
        invalid_arguments: vec![
            json!({}),
            json!({"interface": ""}),
            json!({"interface": true}),
            json!({"interface": "한".repeat(86)}),
            json!({"interface": name, "method": "down"}),
        ],
        responses: vec![(
            json!({"interface": [interface("other"), raw]}),
            clean.clone(),
        )],
        invalid_outputs: vec![
            (json!({"interface": []}), "selection_not_observed"),
            (
                json!({"interface": [clean.clone(), clean.clone()]}),
                "invalid_output",
            ),
            (json!({"interface": [clean, malformed]}), "invalid_output"),
        ],
    })
    .await;
}

#[tokio::test]
async fn wireless_device_list_mcp_preserves_empty_observation_and_rejects_duplicates() {
    check_read_contract(ReadContract {
        name: "wireless_devices",
        category: Category::Wireless,
        visible: wireless_tools(),
        arguments: json!({}),
        action: PreparedAction::Ubus {
            object: "iwinfo".into(),
            method: "devices".into(),
            arguments: json!({}),
        },
        invalid_arguments: vec![json!({"device": "phy0"}), json!({"method": "scan"})],
        responses: vec![
            (
                json!({"devices": ["phy0-ap0"], "ssid": PRIVATE}),
                json!({"items": [{"device": "phy0-ap0"}]}),
            ),
            (json!({"devices": []}), json!({"items": []})),
        ],
        invalid_outputs: vec![
            (json!({}), "invalid_output"),
            (json!({"devices": ["dup", "dup"]}), "invalid_output"),
            (json!({"devices": [""]}), "invalid_output"),
            (json!({"devices": [{"key": PRIVATE}]}), "invalid_output"),
        ],
    })
    .await;
}

fn raw_service() -> Value {
    json!({"data": {"password": PRIVATE}, "instances": {
        "primary": {"running": true, "pid": 42, "command": [PRIVATE], "env": {"TOKEN": PRIVATE}},
        "stopped": {"running": false, "exit_code": 1, "errors": [PRIVATE]}}})
}

fn wireless_tools() -> Vec<&'static str> {
    vec![
        "wireless_radio_info",
        "wireless_devices",
        "wireless_stations",
        "wireless_station_status",
        "wireless_countries",
    ]
}

fn station_source(mac: &str) -> Value {
    json!({"mac":mac,"signal":-48,"noise":-95,"authorized":true,
        "rx":{"bytes":9007199254740993_u64,"rate":866700,"mhz":80,"drop_misc":2,"secret":PRIVATE},
        "tx":{"bytes":42,"rate":144400,"mhz":20},
        "ssid":PRIVATE,"bssid":PRIVATE,"key":PRIVATE,"mesh local PS":PRIVATE})
}

fn station_result(mac: &str) -> Value {
    json!({"mac":mac,"signal_dbm":-48,"noise_dbm":-95,"authorized":true,
        "rx_bytes":"9007199254740993","rx_rate_kbps":866700,"rx_bandwidth_mhz":80,"rx_dropped":"2",
        "tx_bytes":"42","tx_rate_kbps":144400,"tx_bandwidth_mhz":20})
}

#[tokio::test]
async fn passive_station_list_mcp_preserves_counters_without_configuration_or_raw_audit() {
    let device = format!("phy0; {PRIVATE} $(not-a-command)");
    let mac = "02:00:00:00:00:01";
    let clean = station_result(mac);
    let mut malformed = station_source(mac);
    malformed["rx"]["bytes"] = json!(-1);
    check_read_contract(ReadContract {
        name: "wireless_stations",
        category: Category::Wireless,
        visible: wireless_tools(),
        arguments: json!({"device":device}),
        action: PreparedAction::Ubus {
            object: "iwinfo".into(),
            method: "assoclist".into(),
            arguments: json!({"device":device}),
        },
        invalid_arguments: vec![
            json!({}),
            json!({"device":true}),
            json!({"device":"phy0","mac":mac}),
            json!({"device":"phy0","method":"scan"}),
        ],
        responses: vec![
            (
                json!({"results":[station_source(mac)]}),
                json!({"items":[clean]}),
            ),
            (json!({"results":[]}), json!({"items":[]})),
        ],
        invalid_outputs: vec![
            (json!({}), "invalid_output"),
            (json!({"results":[malformed]}), "invalid_output"),
            (
                json!({"results":[station_source(mac),station_source(mac)]}),
                "invalid_output",
            ),
        ],
    })
    .await;
}

#[tokio::test]
async fn station_exact_mcp_validates_other_rows_and_never_transmits_the_mac_selector() {
    let mac = "02:00:00:00:00:AB";
    let mut malformed = station_source("02:00:00:00:00:02");
    malformed["noise"] = json!(PRIVATE);
    check_read_contract(ReadContract {
        name: "wireless_station_status",
        category: Category::Wireless,
        visible: wireless_tools(),
        arguments: json!({"device":"phy0-ap0","mac":mac}),
        action: PreparedAction::Ubus {
            object: "iwinfo".into(),
            method: "assoclist".into(),
            arguments: json!({"device":"phy0-ap0"}),
        },
        invalid_arguments: vec![
            json!({"device":"phy0"}),
            json!({"device":"phy0","mac":""}),
            json!({"device":"phy0","mac":"x".repeat(18)}),
            json!({"device":"phy0","mac":mac,"disconnect":true}),
        ],
        responses: vec![(
            json!({"results":[station_source("02:00:00:00:00:02"),station_source(mac)]}),
            station_result(mac),
        )],
        invalid_outputs: vec![
            (json!({"results":[]}), "selection_not_observed"),
            (
                json!({"results":[station_source("02:00:00:00:00:ab")]}),
                "selection_not_observed",
            ),
            (
                json!({"results":[station_source(mac),malformed]}),
                "invalid_output",
            ),
        ],
    })
    .await;
}

#[tokio::test]
async fn country_list_mcp_is_a_passive_metadata_read_not_a_regulatory_setter() {
    let clean = json!({"iso3166":"KR","code":"KR","country":"South Korea","active":true});
    let mut raw = clean.clone();
    raw["config"] = json!({"key":PRIVATE});
    check_read_contract(ReadContract {
        name: "wireless_countries",
        category: Category::Wireless,
        visible: wireless_tools(),
        arguments: json!({"device":"phy0"}),
        action: PreparedAction::Ubus {
            object: "iwinfo".into(),
            method: "countrylist".into(),
            arguments: json!({"device":"phy0"}),
        },
        invalid_arguments: vec![
            json!({}),
            json!({"device":null}),
            json!({"device":"phy0","country":"KR"}),
            json!({"device":"phy0","execute":true}),
        ],
        responses: vec![
            (json!({"results":[raw]}), json!({"items":[clean.clone()]})),
            (json!({"results":[]}), json!({"items":[]})),
        ],
        invalid_outputs: vec![
            (json!({}), "invalid_output"),
            (json!({"results":[clean.clone(),clean]}), "invalid_output"),
            (
                json!({"results":[{"iso3166":"KR","code":"KR","country":PRIVATE,"active":1}]}),
                "invalid_output",
            ),
        ],
    })
    .await;
}

fn clean_service(name: &str) -> Value {
    json!({"name": name, "instances": [{"name": "primary", "running": true, "pid": 42},
        {"name": "stopped", "running": false, "exit_code": 1}]})
}

#[tokio::test]
async fn service_exact_mcp_sends_fixed_read_arguments_and_filters_instance_secrets() {
    let name = "daemon; $(not-a-command) ' 한글";
    check_read_contract(ReadContract {
        name: "service_status",
        category: Category::Services,
        visible: service_tools(),
        arguments: json!({"name": name}),
        action: PreparedAction::Ubus {
            object: "service".into(),
            method: "list".into(),
            arguments: json!({"name": name, "verbose": false}),
        },
        invalid_arguments: vec![
            json!({}),
            json!({"name": ""}),
            json!({"name": name, "verbose": true}),
            json!({"name": name, "method": "delete"}),
            json!({"name": "x".repeat(257)}),
        ],
        responses: vec![
            (
                json!({name: raw_service(), "unselected": {"instances": {}}}),
                clean_service(name),
            ),
            (json!({name: {}}), json!({"name": name})),
        ],
        invalid_outputs: vec![
            (json!({}), "selection_not_observed"),
            (json!({name: {"instances": null}}), "invalid_output"),
            (
                json!({name: raw_service(), "unselected": {"instances": {"bad": {"running": 1}}}}),
                "invalid_output",
            ),
        ],
    })
    .await;
}

#[tokio::test]
async fn service_list_mcp_exposes_only_generic_metadata_and_checks_global_nested_budget() {
    let too_many: serde_json::Map<String, Value> = (0..3)
        .map(|index| {
            let instances: serde_json::Map<String, Value> = (0..85)
                .map(|i| (format!("instance{i}"), json!({"running": true})))
                .collect();
            (format!("service{index}"), json!({"instances": instances}))
        })
        .collect();
    check_read_contract(ReadContract {
        name: "service_status_list",
        category: Category::Services,
        visible: service_tools(),
        arguments: json!({}),
        action: PreparedAction::Ubus {
            object: "service".into(),
            method: "list".into(),
            arguments: json!({"verbose": false}),
        },
        invalid_arguments: vec![
            json!({"name": "firewall"}),
            json!({"verbose": true}),
            json!({"method": "delete"}),
        ],
        responses: vec![
            (
                json!({"firewall": raw_service(), "vpn": {}}),
                json!({"items": [clean_service("firewall"), {"name": "vpn"}]}),
            ),
            (json!({}), json!({"items": []})),
        ],
        invalid_outputs: vec![
            (json!([]), "invalid_output"),
            (
                json!({"daemon": {"instances": {"bad": {"pid": 42}}}}),
                "invalid_output",
            ),
            (Value::Object(too_many), "output_limit"),
        ],
    })
    .await;
}
