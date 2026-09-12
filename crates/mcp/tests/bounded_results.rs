//! Actual synthetic MCP serialization, not real-router acceptance.
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, CapabilityObservation, Catalog, Category, Grant, MAX_NORMALIZED_BYTES, MethodSignature,
    ObjectObservation, Operation, Policy, PreparedAction, ProbeRequest, ReviewedObject,
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditOutcome, AuditPhase, AuditSink, Backend, Dispatcher, Limits, RuntimeError,
};
use openwrt_mcp_transport::{MAX_CALL_RESULT_BYTES, McpServer};
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

const PRIVATE: &str = "synthetic-bounded-reply-secret";
struct Target {
    output: Value,
    calls: AtomicUsize,
}
#[async_trait]
impl Backend for Target {
    fn capability_epoch(&self) -> Option<u64> {
        Some(1)
    }
    async fn probe(
        &self,
        request: ProbeRequest,
        _: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        assert_eq!(
            request,
            ProbeRequest::DescribeUbusObject(ReviewedObject::System)
        );
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object: ReviewedObject::System,
            methods: [(
                "info".into(),
                MethodSignature {
                    arguments: Default::default(),
                },
            )]
            .into(),
        }))
    }
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.output.clone())
    }
}
#[derive(Default)]
struct Log(Mutex<Vec<AuditEvent>>);
impl AuditSink for Log {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        self.0.lock().unwrap().push(event.clone());
        Ok(())
    }
}

async fn call(output: Value) -> (Value, Vec<AuditEvent>) {
    tokio::time::timeout(Duration::from_secs(10),async{
        let operation:Operation=serde_json::from_value(json!({
            "name":"fixture_result","description":"Synthetic result-bound fixture.",
            "requirements":[{"category":"system","permission":"read"}],
            "action":{"kind":"ubus","object":"system","method":"info","arguments":{}},
            "capability":{"kind":"ubus_method","object":"system","method":"info","arguments":{},"response_contract":"fixture_result.v1"},
            "output_fields":["/value"]
        })).unwrap();
        let policy=Policy{categories:[(Category::System,Grant{access:Access::Read,execute:false})].into(),..Policy::default()};
        let target=Arc::new(Target{output,calls:AtomicUsize::new(0)});
        let audit=Arc::new(Log::default());
        let dispatcher=Dispatcher::new(Catalog::with_builtins(vec![operation],vec![]).unwrap(),policy,target.clone(),audit.clone(),Limits{max_output_bytes:16_777_216,..Limits::default()}).unwrap();
        let (client_io,server_io)=tokio::io::duplex(65_536);
        let server=tokio::spawn(async move{McpServer::new(dispatcher).serve(server_io).await.unwrap().waiting().await.unwrap()});
        let client=().serve(client_io).await.unwrap();
        let request:CallToolRequestParams=serde_json::from_value(json!({"name":"fixture_result","arguments":{}})).unwrap();
        let response=serde_json::to_value(client.call_tool(request).await.unwrap()).unwrap();
        client.cancel().await.unwrap();server.await.unwrap();
        assert_eq!(target.calls.load(Ordering::SeqCst),1);
        let events=audit.0.lock().unwrap().clone();
        assert_eq!(events.len(),2);
        assert_eq!(events[0].phase,AuditPhase::Start);
        assert_eq!(events[1].phase,AuditPhase::Finish);
        assert!(!serde_json::to_string(&events).unwrap().contains(PRIVATE));
        assert!(!response.to_string().contains(PRIVATE));
        (response,events)
    }).await.expect("bounded synthetic MCP lifecycle")
}

#[tokio::test]
async fn exact_normalized_limit_preserves_text_and_structured_copies_within_result_limit() {
    let overhead = serde_json::to_vec(&json!({"/value":""})).unwrap().len();
    let text = "x".repeat(MAX_NORMALIZED_BYTES - overhead);
    let expected = json!({"/value":text});
    assert_eq!(
        serde_json::to_vec(&expected).unwrap().len(),
        MAX_NORMALIZED_BYTES
    );
    let (result, events) = call(json!({"value":text,"password":PRIVATE})).await;
    assert_ne!(result["isError"], true);
    assert_eq!(result["structuredContent"], expected);
    assert_eq!(
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        expected
    );
    let encoded = serde_json::to_vec(&result).unwrap();
    assert!(encoded.len() > MAX_NORMALIZED_BYTES);
    assert!(encoded.len() <= MAX_CALL_RESULT_BYTES);
    assert_eq!(events[1].outcome, AuditOutcome::Success);
}

#[tokio::test]
async fn json_escaping_unicode_and_both_result_copies_are_accounted_for() {
    let unit = "\u{1}\n\t\"\\한글";
    let encoded_unit = serde_json::to_vec(&json!(unit)).unwrap().len() - 2;
    let overhead = serde_json::to_vec(&json!({"/value":""})).unwrap().len();
    let text = unit.repeat((MAX_NORMALIZED_BYTES - overhead) / encoded_unit);
    let expected = json!({"/value":text});
    let normalized = serde_json::to_vec(&expected).unwrap().len();
    assert!(normalized <= MAX_NORMALIZED_BYTES);
    assert!(normalized > MAX_NORMALIZED_BYTES - encoded_unit);
    let (result, events) = call(json!({"value":text,"private_key":PRIVATE})).await;
    assert_ne!(result["isError"], true);
    assert_eq!(result["structuredContent"], expected);
    assert_eq!(
        serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        expected
    );
    assert!(serde_json::to_vec(&result).unwrap().len() <= MAX_CALL_RESULT_BYTES);
    assert_eq!(events[1].outcome, AuditOutcome::Success);
}

#[tokio::test]
async fn oversized_normalized_response_is_a_small_error_and_failed_completion_audit() {
    let overhead = serde_json::to_vec(&json!({"/value":""})).unwrap().len();
    for text in [
        "x".repeat(MAX_NORMALIZED_BYTES - overhead + 1),
        "\n".repeat(40_000),
    ] {
        let (result, events) = call(json!({"value":text,"password":PRIVATE})).await;
        assert_eq!(result["isError"], true);
        assert!(result.get("structuredContent").is_none());
        assert_eq!(
            serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
            json!({"error":"output_limit"})
        );
        assert!(serde_json::to_vec(&result).unwrap().len() < 256);
        assert_eq!(events[1].outcome, AuditOutcome::Failed);
    }
}

#[test]
fn actual_result_limits_match_the_architecture_contract() {
    let contract: toml::Value =
        toml::from_str(include_str!("../../../architecture/spec.toml")).unwrap();
    let result = &contract["mcp_result_contract"];
    assert_eq!(result["owner"].as_str(), Some("openwrt-mcp-transport"));
    assert_eq!(
        result["max_serialized_bytes"].as_integer(),
        Some(MAX_CALL_RESULT_BYTES as i64)
    );
    assert_eq!(
        contract["projection_contract"]["max_normalized_bytes"].as_integer(),
        Some(MAX_NORMALIZED_BYTES as i64)
    );
    assert_eq!(
        result["serialized_scope"].as_str(),
        Some("entire_call_tool_result")
    );
}
