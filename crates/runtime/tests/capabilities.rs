//! Actual dispatcher lifecycle with synthetic observations, not device acceptance.
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, Action, CapabilityObservation, CapabilityRequirement, Catalog, Category, Grant,
    MethodSignature, ObjectObservation, Operation, OutputMode, Parameter, ParameterKind,
    Permission, Policy, PreparedAction, ProbeRequest, Requirement, ReviewedObject,
    UbusArgumentType, UnknownReason,
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditKind, AuditPhase, AuditSink, Backend, Dispatcher, Limits, RuntimeError,
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy)]
enum Mode {
    Present,
    Hidden,
    Incompatible,
    Incomplete,
    Foreign,
    UnknownType,
    Failure,
    ChangeEpoch,
}
struct Target {
    mode: Mutex<Mode>,
    epoch: AtomicU64,
    probes: AtomicUsize,
    calls: AtomicUsize,
    probe_delay_ms: AtomicU64,
    call_delay_ms: AtomicU64,
    order: Arc<Mutex<Vec<&'static str>>>,
}
#[async_trait]
impl Backend for Target {
    fn capability_epoch(&self) -> Option<u64> {
        match self.epoch.load(Ordering::SeqCst) {
            0 => None,
            epoch => Some(epoch),
        }
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
        self.probes.fetch_add(1, Ordering::SeqCst);
        self.order.lock().unwrap().push("probe");
        tokio::time::sleep(Duration::from_millis(
            self.probe_delay_ms.load(Ordering::SeqCst),
        ))
        .await;
        let mode = *self.mode.lock().unwrap();
        if matches!(mode, Mode::Failure) {
            return Err(RuntimeError::BackendFailed);
        }
        if matches!(mode, Mode::Hidden) {
            return Ok(CapabilityObservation::Unknown(
                UnknownReason::NotObservedOrHidden,
            ));
        }
        if matches!(mode, Mode::ChangeEpoch) {
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        let arguments = if matches!(mode, Mode::Incomplete) {
            Default::default()
        } else {
            [(
                "name".into(),
                match mode {
                    Mode::Incompatible => UbusArgumentType::Boolean,
                    Mode::UnknownType => UbusArgumentType::Unknown,
                    _ => UbusArgumentType::String,
                },
            )]
            .into()
        };
        Ok(CapabilityObservation::Ubus(ObjectObservation {
            object: if matches!(mode, Mode::Foreign) {
                ReviewedObject::Service
            } else {
                ReviewedObject::System
            },
            methods: [("query".into(), MethodSignature { arguments })].into(),
        }))
    }
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.order.lock().unwrap().push("execute");
        tokio::time::sleep(Duration::from_millis(
            self.call_delay_ms.load(Ordering::SeqCst),
        ))
        .await;
        Ok(json!({"uptime":42,"password":"synthetic-response-secret"}))
    }
}
#[derive(Default)]
struct Audit {
    events: Mutex<Vec<AuditEvent>>,
    order: Arc<Mutex<Vec<&'static str>>>,
    fail: bool,
}
impl AuditSink for Audit {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        if self.fail {
            return Err(RuntimeError::AuditUnavailable);
        }
        self.order.lock().unwrap().push(match event.phase {
            AuditPhase::Start => "start",
            AuditPhase::Finish => "finish",
            AuditPhase::Rejection => "reject",
        });
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}
fn operation() -> Operation {
    Operation {
        name: "fixture_query".into(),
        description: "Synthetic capability contract".into(),
        requirements: vec![Requirement {
            category: Category::System,
            permission: Permission::Read,
        }],
        parameters: [(
            "name".into(),
            Parameter {
                kind: ParameterKind::String,
                required: true,
                allowed_values: vec![],
            },
        )]
        .into(),
        action: Action::Ubus {
            object: "system".into(),
            method: "query".into(),
            arguments: [("name".into(), json!("{name}"))].into(),
        },
        capability: CapabilityRequirement::UbusMethod {
            object: "system".into(),
            method: "query".into(),
            arguments: [("name".into(), ParameterKind::String)].into(),
            response_contract: "fixture_query.v1".into(),
        },
        output_fields: vec!["/uptime".into()],
        output_mode: OutputMode::Scalars,
    }
}
fn policy() -> Policy {
    Policy {
        categories: [(
            Category::System,
            Grant {
                access: Access::Read,
                execute: false,
            },
        )]
        .into(),
        ..Default::default()
    }
}
fn dispatcher(
    target: Arc<dyn Backend>,
    audit: Arc<Audit>,
    allowed: bool,
    limits: Limits,
) -> Dispatcher {
    Dispatcher::new(
        Catalog::with_builtins(vec![operation()], vec![]).unwrap(),
        if allowed { policy() } else { Policy::default() },
        target,
        audit,
        limits,
    )
    .unwrap()
}
fn fixture(mode: Mode) -> (Arc<Target>, Arc<Audit>) {
    let order = Arc::new(Mutex::new(Vec::new()));
    (
        Arc::new(Target {
            mode: Mutex::new(mode),
            epoch: AtomicU64::new(1),
            probes: AtomicUsize::new(0),
            calls: AtomicUsize::new(0),
            probe_delay_ms: AtomicU64::new(0),
            call_delay_ms: AtomicU64::new(0),
            order: order.clone(),
        }),
        Arc::new(Audit {
            order,
            ..Default::default()
        }),
    )
}

#[tokio::test]
async fn offline_denied_invalid_and_unaudited_requests_have_zero_device_io() {
    let (target, audit) = fixture(Mode::Present);
    let denied = dispatcher(target.clone(), audit.clone(), false, Limits::default());
    assert!(denied.available_operations().is_empty());
    assert!(
        denied
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .is_err()
    );
    assert!(
        denied
            .capability_status(json!({"operation":"fixture_query","refresh":true}))
            .await
            .is_err()
    );
    let allowed = dispatcher(target.clone(), audit, true, Limits::default());
    assert_eq!(allowed.available_operations().len(), 1);
    for input in [
        json!({}),
        json!({"name":"test","force":true}),
        json!({"name":{}}),
    ] {
        assert!(allowed.invoke("fixture_query", input).await.is_err());
    }
    let blocked = dispatcher(
        target.clone(),
        Arc::new(Audit {
            fail: true,
            ..Default::default()
        }),
        true,
        Limits::default(),
    );
    assert_eq!(
        blocked
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .unwrap_err()
            .code(),
        "audit_unavailable"
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 0);
    assert_eq!(target.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn status_and_invocation_share_cache_but_audit_each_kind_before_io() {
    let (target, audit) = fixture(Mode::Present);
    let runtime = dispatcher(target.clone(), audit.clone(), true, Limits::default());
    let status = runtime
        .capability_status(json!({"operation":"fixture_query"}))
        .await
        .unwrap();
    assert_eq!(status.compatibility, "compatible");
    assert_eq!(status.scope, "input_signature_only");
    assert!(status.remaining_ttl_ms.is_some_and(|value| value <= 30_000));
    assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        runtime
            .invoke("fixture_query", json!({"name":"synthetic-argument-secret"}))
            .await
            .unwrap(),
        json!({"/uptime":42})
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 1);
    assert_eq!(
        *target.order.lock().unwrap(),
        vec!["start", "probe", "finish", "start", "execute", "finish"]
    );
    let events = audit.events.lock().unwrap();
    assert_eq!(events[0].kind, AuditKind::Capability);
    assert_eq!(events[2].kind, AuditKind::Invocation);
    assert!(
        !serde_json::to_string(&*events)
            .unwrap()
            .contains("synthetic")
    );
}
#[tokio::test]
async fn incomplete_hidden_foreign_unknown_and_conflicting_signatures_fail_closed() {
    for (mode, compatibility, reason, error) in [
        (
            Mode::Hidden,
            "unknown",
            "not_observed_or_hidden",
            "capability_unknown",
        ),
        (
            Mode::Incomplete,
            "unknown",
            "incomplete_signature",
            "capability_unknown",
        ),
        (
            Mode::Foreign,
            "unknown",
            "invalid_observation",
            "capability_unknown",
        ),
        (
            Mode::UnknownType,
            "unknown",
            "unrecognized_type",
            "capability_unknown",
        ),
        (
            Mode::Incompatible,
            "incompatible",
            "argument_type_mismatch",
            "capability_unsupported",
        ),
        (
            Mode::ChangeEpoch,
            "unknown",
            "stale_observation",
            "capability_unknown",
        ),
    ] {
        let (target, audit) = fixture(mode);
        let runtime = dispatcher(target.clone(), audit, true, Limits::default());
        let status = runtime
            .capability_status(json!({"operation":"fixture_query"}))
            .await
            .unwrap();
        assert_eq!(
            (status.compatibility, status.reason),
            (compatibility, reason)
        );
        assert_eq!(
            runtime
                .invoke("fixture_query", json!({"name":"test"}))
                .await
                .unwrap_err()
                .code(),
            error
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn failed_refresh_discards_success_and_cannot_be_overridden() {
    let (target, audit) = fixture(Mode::Present);
    let runtime = dispatcher(target.clone(), audit.clone(), true, Limits::default());
    runtime
        .capability_status(json!({"operation":"fixture_query"}))
        .await
        .unwrap();
    *target.mode.lock().unwrap() = Mode::Failure;
    assert_eq!(
        runtime
            .capability_status(json!({"operation":"fixture_query","refresh":true}))
            .await
            .unwrap_err()
            .code(),
        "backend_failed"
    );
    *target.mode.lock().unwrap() = Mode::Hidden;
    assert_eq!(
        runtime
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .unwrap_err()
            .code(),
        "capability_unknown"
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 3);
    for arguments in [
        json!({"operation":"fixture_query","force":true}),
        json!({"operation":"fixture_query","profile":"synthetic-secret"}),
        json!({"operation":"fixture_query","refresh":"synthetic-secret"}),
        json!({"operation":7}),
        json!([]),
    ] {
        assert_eq!(
            runtime
                .capability_status(arguments)
                .await
                .unwrap_err()
                .code(),
            "invalid_arguments"
        );
    }
    assert_eq!(target.probes.load(Ordering::SeqCst), 3);
    assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    assert!(
        !serde_json::to_string(&*audit.events.lock().unwrap())
            .unwrap()
            .contains("synthetic-secret")
    );
}
#[tokio::test]
async fn epochs_and_distinct_dispatchers_cannot_borrow_prior_evidence() {
    let (target, audit) = fixture(Mode::Present);
    let runtime = dispatcher(target.clone(), audit, true, Limits::default());
    runtime
        .capability_status(json!({"operation":"fixture_query"}))
        .await
        .unwrap();
    target.epoch.store(2, Ordering::SeqCst);
    *target.mode.lock().unwrap() = Mode::Hidden;
    assert!(
        runtime
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .is_err()
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 2);
    let (foreign, audit) = fixture(Mode::Hidden);
    let other = dispatcher(foreign.clone(), audit, true, Limits::default());
    assert!(
        other
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .is_err()
    );
    assert_eq!(foreign.probes.load(Ordering::SeqCst), 1);
    assert_eq!(foreign.calls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn probe_and_execution_share_one_total_device_deadline() {
    let (target, audit) = fixture(Mode::Present);
    target.probe_delay_ms.store(70, Ordering::SeqCst);
    target.call_delay_ms.store(70, Ordering::SeqCst);
    let runtime = dispatcher(
        target.clone(),
        audit,
        true,
        Limits {
            timeout_ms: 100,
            ..Default::default()
        },
    );
    assert_eq!(
        runtime
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .unwrap_err()
            .code(),
        "timeout"
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 1);
    assert_eq!(target.calls.load(Ordering::SeqCst), 1);
}
struct Legacy;
#[async_trait]
impl Backend for Legacy {
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        panic!("legacy execute must not bypass unavailable evidence")
    }
}
#[tokio::test]
async fn implementing_execute_only_does_not_grant_capabilities() {
    let runtime = dispatcher(
        Arc::new(Legacy),
        Arc::new(Audit::default()),
        true,
        Limits::default(),
    );
    assert_eq!(
        runtime
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .unwrap_err()
            .code(),
        "capability_unknown"
    );
}

#[tokio::test]
async fn unverified_process_and_unreviewed_objects_never_probe_even_with_operator_grants() {
    for process in [true, false] {
        let mut operation = operation();
        if process {
            operation.parameters.clear();
            operation.action = Action::Process {
                program: "/bin/true".into(),
                args: vec![],
            };
            operation.capability = CapabilityRequirement::Unverified {};
        } else {
            if let Action::Ubus { object, .. } = &mut operation.action {
                *object = "unreviewed.fixture".into();
            }
            if let CapabilityRequirement::UbusMethod { object, .. } = &mut operation.capability {
                *object = "unreviewed.fixture".into();
            }
        }
        let mut policy = policy();
        policy.categories.insert(
            Category::Extensions,
            Grant {
                access: Access::ReadWrite,
                execute: true,
            },
        );
        let (target, audit) = fixture(Mode::Present);
        let runtime = Dispatcher::new(
            Catalog::new(vec![operation]).unwrap(),
            policy,
            target.clone(),
            audit,
            Limits::default(),
        )
        .unwrap();
        let status = runtime
            .capability_status(json!({"operation":"fixture_query"}))
            .await
            .unwrap();
        assert_eq!(
            (status.compatibility, status.reason),
            ("unknown", "unreviewed_probe")
        );
        assert_eq!(
            runtime
                .invoke(
                    "fixture_query",
                    if process {
                        json!({})
                    } else {
                        json!({"name":"test"})
                    }
                )
                .await
                .unwrap_err()
                .code(),
            "capability_unknown"
        );
        assert_eq!(target.probes.load(Ordering::SeqCst), 0);
        assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn cancellation_of_refresh_discards_old_success_and_admission_stays_bounded() {
    let (target, audit) = fixture(Mode::Present);
    let runtime = Arc::new(dispatcher(
        target.clone(),
        audit,
        true,
        Limits {
            max_concurrent: 1,
            ..Default::default()
        },
    ));
    runtime
        .capability_status(json!({"operation":"fixture_query"}))
        .await
        .unwrap();
    target.probe_delay_ms.store(10_000, Ordering::SeqCst);
    let task = {
        let runtime = runtime.clone();
        tokio::spawn(async move {
            runtime
                .capability_status(json!({"operation":"fixture_query","refresh":true}))
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(1), async {
        while target.probes.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    for _ in 0..3 {
        assert_eq!(
            runtime
                .invoke("fixture_query", json!({"name":"test"}))
                .await
                .unwrap_err()
                .code(),
            "busy"
        );
    }
    assert_eq!(target.probes.load(Ordering::SeqCst), 2);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    target.probe_delay_ms.store(0, Ordering::SeqCst);
    *target.mode.lock().unwrap() = Mode::Hidden;
    assert_eq!(
        runtime
            .invoke("fixture_query", json!({"name":"test"}))
            .await
            .unwrap_err()
            .code(),
        "capability_unknown"
    );
    assert_eq!(target.probes.load(Ordering::SeqCst), 3);
    assert_eq!(target.calls.load(Ordering::SeqCst), 0);
}
