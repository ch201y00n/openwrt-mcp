//! Synthetic dispatcher contracts, not device acceptance or resource topology.
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, CapabilityObservation, Catalog, Category, Grant, MethodSignature, ObjectObservation,
    Operation, OutputMode, Policy, PreparedAction, ProbeRequest, ReviewedObject,
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditOutcome, AuditPhase, AuditSink, Backend, Dispatcher, Limits, RuntimeError,
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

const PRIVATE: &str = "synthetic-collection-private-never-export";

#[derive(Default)]
struct Log {
    fail: bool,
    events: Mutex<Vec<AuditEvent>>,
}
impl AuditSink for Log {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        if self.fail {
            return Err(RuntimeError::AuditUnavailable);
        }
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

struct Target {
    probes: AtomicUsize,
    calls: Mutex<Vec<PreparedAction>>,
    responses: Mutex<VecDeque<Value>>,
    audit: Arc<Log>,
}
impl Target {
    fn assert_start_recorded(&self) {
        assert_eq!(
            self.audit.events.lock().unwrap().last().unwrap().phase,
            AuditPhase::Start
        );
    }
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
        self.assert_start_recorded();
        assert_eq!(
            request,
            ProbeRequest::DescribeUbusObject(ReviewedObject::NetworkInterface)
        );
        self.probes.fetch_add(1, Ordering::SeqCst);
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object: ReviewedObject::NetworkInterface,
            methods: [(
                "dump".into(),
                MethodSignature {
                    arguments: Default::default(),
                },
            )]
            .into(),
        }))
    }
    async fn execute(&self, action: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        self.assert_start_recorded();
        self.calls.lock().unwrap().push(action.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(RuntimeError::BackendFailed)
    }
}

fn operation(select: bool) -> Operation {
    serde_json::from_value(json!({
        "name":"fixture_collection", "description":"Synthetic bounded collection contract.",
        "requirements":[{"category":"network","permission":"read"}],
        "parameters":if select { json!({"interface":{"kind":"string","required":true}}) } else { json!({}) },
        "action":{"kind":"ubus","object":"network.interface","method":"dump","arguments":{}},
        "capability":{"kind":"ubus_method","object":"network.interface","method":"dump","arguments":{},"response_contract":"fixture_collection.v1"},
        "output_fields":[],
        "output_mode":{"typed":{
            "kind":"collection",
            "selection":if select {json!({"kind":"exact_one","parameter":"interface"})} else {json!({"kind":"all"})},
            "collection":{
                "kind":"object_array","source":"/items","max_items":128,"identity":"name",
                "record":{"fields":[
                    {"name":"name","source":"/name","presence":"required","value":{"kind":"text","max_bytes":256}},
                    {"name":"up","source":"/up","presence":"required","value":{"kind":"boolean"}}
                ],"collections":[]}
            }
        }}
    })).unwrap()
}

fn fixture(
    operation: Operation,
    responses: Vec<Value>,
    allowed: bool,
    fail_audit: bool,
) -> (Dispatcher, Arc<Target>, Arc<Log>) {
    let audit = Arc::new(Log {
        fail: fail_audit,
        ..Log::default()
    });
    let target = Arc::new(Target {
        probes: AtomicUsize::new(0),
        calls: Mutex::new(Vec::new()),
        responses: Mutex::new(responses.into()),
        audit: audit.clone(),
    });
    let policy = Policy {
        categories: [(
            Category::Network,
            Grant {
                access: if allowed { Access::Read } else { Access::Deny },
                execute: false,
            },
        )]
        .into(),
        ..Policy::default()
    };
    let dispatcher = Dispatcher::new(
        Catalog::with_builtins(vec![operation], vec![]).unwrap(),
        policy,
        target.clone(),
        audit.clone(),
        Limits {
            max_output_bytes: 16_777_216,
            ..Limits::default()
        },
    )
    .unwrap();
    (dispatcher, target, audit)
}
fn row(name: &str) -> Value {
    json!({"name":name,"up":true,"password":PRIVATE,"data":{"key":PRIVATE},"undeclared":[PRIVATE]})
}
fn last_outcome(audit: &Log, expected: AuditOutcome) {
    let events = audit.events.lock().unwrap();
    let last = events.last().unwrap();
    assert_eq!(last.phase, AuditPhase::Finish);
    assert_eq!(last.outcome, expected);
    assert!(!serde_json::to_string(&*events).unwrap().contains(PRIVATE));
}

#[tokio::test]
async fn local_selector_is_bound_before_io_and_never_sent_as_a_device_argument() {
    let (dispatcher, target, audit) = fixture(
        operation(true),
        vec![json!({"items":[row("wan"),row("lan")]})],
        true,
        false,
    );
    let result = dispatcher
        .invoke("fixture_collection", json!({"interface":"lan"}))
        .await
        .unwrap();
    assert_eq!(result, json!({"name":"lan","up":true}));
    assert_eq!(target.probes.load(Ordering::SeqCst), 1);
    assert_eq!(
        *target.calls.lock().unwrap(),
        [PreparedAction::Ubus {
            object: "network.interface".into(),
            method: "dump".into(),
            arguments: json!({}),
        }]
    );
    last_outcome(&audit, AuditOutcome::Success);
}

#[tokio::test]
async fn invalid_selectors_policy_and_failed_audit_cause_zero_probe_and_execution() {
    for arguments in [
        json!({}),
        json!({"interface":false}),
        json!({"interface":""}),
        json!({"interface":"x".repeat(257)}),
        json!({"interface":"lan","source":PRIVATE}),
    ] {
        let (dispatcher, target, audit) = fixture(operation(true), vec![], true, false);
        assert!(
            dispatcher
                .invoke("fixture_collection", arguments)
                .await
                .is_err()
        );
        assert_eq!(target.probes.load(Ordering::SeqCst), 0);
        assert!(target.calls.lock().unwrap().is_empty());
        assert_eq!(
            audit.events.lock().unwrap().last().unwrap().phase,
            AuditPhase::Rejection
        );
    }
    for (allowed, audit_failure, expected) in [
        (false, false, "permission_denied"),
        (true, true, "audit_unavailable"),
    ] {
        let (dispatcher, target, _) = fixture(operation(true), vec![], allowed, audit_failure);
        assert_eq!(
            dispatcher
                .invoke("fixture_collection", json!({"interface":"lan"}))
                .await
                .unwrap_err()
                .code(),
            expected
        );
        assert_eq!(target.probes.load(Ordering::SeqCst), 0);
        assert!(target.calls.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn selection_absence_ambiguity_and_malformed_unselected_rows_fail_without_replay() {
    for (output, expected) in [
        (json!({"items":[row("wan")]}), "selection_not_observed"),
        (json!({"items":[row("lan"),row("lan")]}), "invalid_output"),
        (
            json!({"items":[row("lan"),{"name":"wan","up":{"secret":PRIVATE}}]}),
            "invalid_output",
        ),
        (json!({"items":null}), "invalid_output"),
        (json!({"other":PRIVATE}), "invalid_output"),
    ] {
        let (dispatcher, target, audit) = fixture(operation(true), vec![output], true, false);
        assert_eq!(
            dispatcher
                .invoke("fixture_collection", json!({"interface":"lan"}))
                .await
                .unwrap_err()
                .code(),
            expected
        );
        assert_eq!(target.calls.lock().unwrap().len(), 1);
        assert_eq!(audit.events.lock().unwrap().len(), 2);
        last_outcome(&audit, AuditOutcome::Failed);
    }
}

#[tokio::test]
async fn scan_limit_applies_to_all_source_rows_even_with_a_first_matching_item() {
    let rows: Vec<_> = (0..129).map(|index| row(&format!("item{index}"))).collect();
    let (dispatcher, target, audit) =
        fixture(operation(true), vec![json!({"items":rows})], true, false);
    assert_eq!(
        dispatcher
            .invoke("fixture_collection", json!({"interface":"item0"}))
            .await
            .unwrap_err()
            .code(),
        "output_limit"
    );
    assert_eq!(target.calls.lock().unwrap().len(), 1);
    last_outcome(&audit, AuditOutcome::Failed);
}

#[tokio::test]
async fn valid_empty_list_is_not_synthesized_from_missing_or_wrong_shape() {
    let (dispatcher, target, audit) = fixture(
        operation(false),
        vec![json!({"items":[]}), json!({"items":{}})],
        true,
        false,
    );
    assert_eq!(
        dispatcher
            .invoke("fixture_collection", json!({}))
            .await
            .unwrap(),
        json!({"items":[]})
    );
    assert_eq!(
        dispatcher
            .invoke("fixture_collection", json!({}))
            .await
            .unwrap_err()
            .code(),
        "invalid_output"
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 1);
    assert_eq!(target.calls.lock().unwrap().len(), 2);
    last_outcome(&audit, AuditOutcome::Failed);
}

#[tokio::test]
async fn legacy_normalized_limit_fails_before_success_audit_even_with_large_backend_limit() {
    let mut operation = operation(false);
    operation.output_mode = OutputMode::Scalars;
    operation.output_fields = vec!["/value".into()];
    let (dispatcher, target, audit) = fixture(
        operation,
        vec![json!({"value":"\n".repeat(40_000)})],
        true,
        false,
    );
    assert_eq!(
        dispatcher
            .invoke("fixture_collection", json!({}))
            .await
            .unwrap_err()
            .code(),
        "output_limit"
    );
    assert_eq!(target.calls.lock().unwrap().len(), 1);
    last_outcome(&audit, AuditOutcome::Failed);
}

#[test]
fn contract_keeps_normalized_limit_before_finish_audit() {
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
