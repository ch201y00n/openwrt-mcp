use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, Action, Catalog, Category, Grant, Operation, OutputMode, Parameter, ParameterKind,
    Permission, Policy, PreparedAction, Requirement,
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditOutcome, AuditPhase, AuditSink, Backend, Dispatcher, Limits, RuntimeError,
};
use serde_json::{Value, json};

#[derive(Default)]
struct RecordingAudit {
    events: Mutex<Vec<AuditEvent>>,
    fail_at: Option<usize>,
    count: AtomicUsize,
}

impl AuditSink for RecordingAudit {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        let index = self.count.fetch_add(1, Ordering::SeqCst);
        if self.fail_at == Some(index) {
            return Err(RuntimeError::AuditUnavailable);
        }
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}

#[derive(Default)]
struct FakeBackend {
    count: AtomicUsize,
    fail: bool,
}

#[async_trait]
impl Backend for FakeBackend {
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(RuntimeError::BackendFailed)
        } else {
            Ok(
                json!({"model": "synthetic-board", "password": "fixture-secret", "unexpected": "fixture-secret"}),
            )
        }
    }
}

fn fixture_catalog() -> Catalog {
    let read = Operation {
        name: "fixture_read".into(),
        description: "Synthetic read action for application-port tests.".into(),
        requirements: vec![Requirement {
            category: Category::System,
            permission: Permission::Read,
        }],
        parameters: Default::default(),
        action: Action::Ubus {
            object: "fixture".into(),
            method: "read".into(),
            arguments: Default::default(),
        },
        output_fields: vec!["/model".into()],
        output_mode: OutputMode::Scalars,
    };
    let mut query = read.clone();
    query.name = "fixture_query".into();
    query.requirements[0].category = Category::Network;
    query.parameters.insert(
        "name".into(),
        Parameter {
            kind: ParameterKind::String,
            required: true,
            allowed_values: vec![],
        },
    );
    query.action = Action::Ubus {
        object: "fixture".into(),
        method: "query".into(),
        arguments: [("name".into(), json!("{name}"))].into(),
    };
    Catalog::with_builtins(vec![read, query], vec![]).unwrap()
}

fn read_policy() -> Policy {
    Policy {
        categories: [
            (
                Category::System,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            ),
            (
                Category::Network,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            ),
        ]
        .into(),
        ..Policy::default()
    }
}

fn dispatcher(policy: Policy, backend: Arc<dyn Backend>, audit: Arc<dyn AuditSink>) -> Dispatcher {
    Dispatcher::new(fixture_catalog(), policy, backend, audit, Limits::default()).unwrap()
}

#[tokio::test]
async fn denied_is_hidden_audited_and_never_executed() {
    let backend = Arc::new(FakeBackend::default());
    let audit = Arc::new(RecordingAudit::default());
    let dispatcher = dispatcher(Policy::default(), backend.clone(), audit.clone());
    assert!(dispatcher.available_operations().is_empty());
    assert!(dispatcher.invoke("fixture_read", json!({})).await.is_err());
    assert_eq!(backend.count.load(Ordering::SeqCst), 0);
    let events = audit.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].outcome, AuditOutcome::Denied);
    assert_eq!(events[0].phase, AuditPhase::Rejection);
}

#[tokio::test]
async fn unknown_names_and_invalid_arguments_are_never_logged_or_executed() {
    let backend = Arc::new(FakeBackend::default());
    let audit = Arc::new(RecordingAudit::default());
    let dispatcher = dispatcher(read_policy(), backend.clone(), audit.clone());
    let unknown = dispatcher
        .invoke("fixture-secret\nforged log", json!({}))
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), "unknown_operation");
    let invalid = dispatcher
        .invoke("fixture_read", json!({"password": "fixture-secret"}))
        .await;
    assert!(invalid.is_err());
    assert!(dispatcher.invoke("fixture_query", json!({})).await.is_err());
    assert_eq!(backend.count.load(Ordering::SeqCst), 0);
    let serialized = serde_json::to_string(&*audit.events.lock().unwrap()).unwrap();
    assert!(!serialized.contains("fixture-secret"));
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("forged"));
}

#[tokio::test]
async fn successful_call_projects_output_and_links_audit_events() {
    let audit = Arc::new(RecordingAudit::default());
    let dispatcher = dispatcher(
        read_policy(),
        Arc::new(FakeBackend::default()),
        audit.clone(),
    );
    assert_eq!(
        dispatcher.invoke("fixture_read", json!({})).await.unwrap(),
        json!({"/model": "synthetic-board"})
    );
    let events = audit.events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].phase, AuditPhase::Start);
    assert_eq!(events[1].phase, AuditPhase::Finish);
    assert_eq!(events[1].outcome, AuditOutcome::Success);
    assert_eq!(events[0].request_sequence, events[1].request_sequence);
    assert!(events[1].duration_ms.is_some());
    assert!(
        !serde_json::to_string(&*events)
            .unwrap()
            .contains("fixture-secret")
    );
}

#[tokio::test]
async fn failed_backend_is_audited_without_device_data() {
    let audit = Arc::new(RecordingAudit::default());
    let backend = Arc::new(FakeBackend {
        fail: true,
        ..FakeBackend::default()
    });
    let dispatcher = dispatcher(read_policy(), backend, audit.clone());
    let error = dispatcher
        .invoke("fixture_read", json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "backend_failed");
    assert_eq!(
        audit.events.lock().unwrap()[1].outcome,
        AuditOutcome::Failed
    );
}

#[tokio::test]
async fn start_audit_failure_prevents_backend_execution() {
    let backend = Arc::new(FakeBackend::default());
    let audit = Arc::new(RecordingAudit {
        fail_at: Some(0),
        ..RecordingAudit::default()
    });
    let dispatcher = dispatcher(read_policy(), backend.clone(), audit);
    assert_eq!(
        dispatcher
            .invoke("fixture_read", json!({}))
            .await
            .unwrap_err()
            .code(),
        "audit_unavailable"
    );
    assert_eq!(backend.count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn completion_audit_failure_reports_uncertain_completed_work() {
    let backend = Arc::new(FakeBackend::default());
    let audit = Arc::new(RecordingAudit {
        fail_at: Some(1),
        ..RecordingAudit::default()
    });
    let dispatcher = dispatcher(read_policy(), backend.clone(), audit);
    assert_eq!(
        dispatcher
            .invoke("fixture_read", json!({}))
            .await
            .unwrap_err()
            .code(),
        "audit_completion_failed"
    );
    assert_eq!(backend.count.load(Ordering::SeqCst), 1);
}

struct BlockingBackend {
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait]
impl Backend for BlockingBackend {
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        self.entered.notify_one();
        self.release.notified().await;
        Ok(json!({}))
    }
}

#[tokio::test]
async fn saturation_rejects_immediately_instead_of_queuing() {
    let backend = Arc::new(BlockingBackend {
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let audit = Arc::new(RecordingAudit::default());
    let dispatcher = Arc::new(
        Dispatcher::new(
            fixture_catalog(),
            read_policy(),
            backend.clone(),
            audit.clone(),
            Limits {
                max_concurrent: 1,
                ..Limits::default()
            },
        )
        .unwrap(),
    );
    let first_dispatcher = dispatcher.clone();
    let first =
        tokio::spawn(async move { first_dispatcher.invoke("fixture_read", json!({})).await });
    backend.entered.notified().await;
    let second = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        dispatcher.invoke("fixture_read", json!({})),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(second.code(), "busy");
    backend.release.notify_one();
    first.await.unwrap().unwrap();
    assert!(
        audit
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.outcome == AuditOutcome::Busy)
    );
}

struct BlockingAudit {
    entered: tokio::sync::Notify,
    release: (Mutex<bool>, std::sync::Condvar),
    count: AtomicUsize,
}

impl BlockingAudit {
    fn release(&self) {
        *self.release.0.lock().unwrap() = true;
        self.release.1.notify_all();
    }
}

impl AuditSink for BlockingAudit {
    fn record(&self, _: &AuditEvent) -> Result<(), RuntimeError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        let _released = self
            .release
            .1
            .wait_while(self.release.0.lock().unwrap(), |released| !*released)
            .unwrap();
        Ok(())
    }
}

struct ReleaseOnDrop(Arc<BlockingAudit>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[tokio::test]
async fn blocked_audit_has_a_deadline_keeps_timers_live_and_never_spawns_more_workers() {
    let audit = Arc::new(BlockingAudit {
        entered: tokio::sync::Notify::new(),
        release: (Mutex::new(false), std::sync::Condvar::new()),
        count: AtomicUsize::new(0),
    });
    // Release even if an assertion fails, so a synthetic blocked writer never
    // strands the test runtime during its blocking-pool shutdown.
    let _release_on_drop = ReleaseOnDrop(audit.clone());
    let backend = Arc::new(FakeBackend::default());
    let dispatcher = Arc::new(
        Dispatcher::new(
            fixture_catalog(),
            read_policy(),
            backend.clone(),
            audit.clone(),
            Limits {
                timeout_ms: 100,
                ..Limits::default()
            },
        )
        .unwrap(),
    );
    let first_dispatcher = dispatcher.clone();
    let first =
        tokio::spawn(async move { first_dispatcher.invoke("fixture_read", json!({})).await });
    tokio::time::timeout(std::time::Duration::from_secs(1), audit.entered.notified())
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_millis(50),
        tokio::time::sleep(std::time::Duration::from_millis(5)),
    )
    .await
    .expect("audit must not block the current-thread async executor");
    let failure = tokio::time::timeout(std::time::Duration::from_secs(1), first)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(failure.code(), "audit_unavailable");
    for _ in 0..10 {
        assert_eq!(
            dispatcher
                .invoke("fixture_read", json!({}))
                .await
                .unwrap_err()
                .code(),
            "audit_unavailable"
        );
    }
    assert_eq!(audit.count.load(Ordering::SeqCst), 1);
    assert_eq!(backend.count.load(Ordering::SeqCst), 0);
}
