//! Synthetic lifecycle tests; no router state or installed-package claims.
mod opkg;
use async_trait::async_trait;
use openwrt_mcp_core::{
    Access, Catalog, Category, Grant, Operation, Policy, PreparedAction,
    packages::{OpkgStatusRecord, PackageObservation, PackageProfile, PackageRecord},
};
use openwrt_mcp_runtime::{
    AuditEvent, AuditPhase, AuditSink, Backend, Dispatcher, Limits, RuntimeError,
    packages::SnapshotTokens,
};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::time::Duration;
#[derive(Default)]
struct Log {
    events: Mutex<Vec<AuditEvent>>,
    fail_start: AtomicBool,
    fail_finish: AtomicBool,
}
impl AuditSink for Log {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        if (event.phase == AuditPhase::Start && self.fail_start.load(Ordering::SeqCst))
            || (event.phase == AuditPhase::Finish && self.fail_finish.load(Ordering::SeqCst))
        {
            return Err(RuntimeError::AuditUnavailable);
        }
        self.events.lock().unwrap().push(event.clone());
        Ok(())
    }
}
struct Tokens {
    next: AtomicU64,
    calls: AtomicUsize,
    fail: AtomicBool,
}
impl Tokens {
    fn new(seed: u64) -> Self {
        Self {
            next: AtomicU64::new(seed),
            calls: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
        }
    }
}
#[async_trait]
impl SnapshotTokens for Tokens {
    async fn nonce(&self) -> Result<[u8; 16], RuntimeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return Err(RuntimeError::EntropyUnavailable);
        }
        let mut nonce = [1; 16];
        nonce[..8].copy_from_slice(&self.next.fetch_add(1, Ordering::SeqCst).to_be_bytes());
        Ok(nonce)
    }
}
struct Target {
    calls: AtomicUsize,
    epoch: AtomicU64,
    fail: AtomicBool,
    change_epoch: AtomicBool,
    stall: AtomicBool,
    entered: tokio::sync::Notify,
    log: Arc<Log>,
}
impl Target {
    async fn before_capture(&self) -> Result<(), RuntimeError> {
        assert_eq!(
            self.log.events.lock().unwrap().last().unwrap().phase,
            AuditPhase::Start
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.stall.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
        if self.fail.load(Ordering::SeqCst) {
            return Err(RuntimeError::BackendFailed);
        }
        if self.change_epoch.load(Ordering::SeqCst) {
            self.epoch.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
#[async_trait]
impl Backend for Target {
    fn capability_epoch(&self) -> Option<u64> {
        Some(self.epoch.load(Ordering::SeqCst))
    }
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        panic!("package workflow must not use generic execute")
    }
    async fn capture_opkg_status(&self, _: &Limits) -> Result<PackageObservation, RuntimeError> {
        self.before_capture().await?;
        Ok(PackageObservation::new_opkg_status(
            (0..281)
                .map(|i| OpkgStatusRecord {
                    name: format!("fixture{i:04}"),
                    version: "1".into(),
                    arch: "aarch64".into(),
                    status: "install hold,user unpacked".into(),
                })
                .collect(),
        )
        .unwrap())
    }
    async fn capture_apk_installed(&self, _: &Limits) -> Result<PackageObservation, RuntimeError> {
        self.before_capture().await?;
        Ok(PackageObservation::new(
            (0..281)
                .map(|i| PackageRecord {
                    name: format!("fixture{i:04}"),
                    version: "1".into(),
                    arch: "aarch64".into(),
                    layer: 0,
                })
                .collect(),
        )
        .unwrap())
    }
}
fn operation(profile: PackageProfile) -> Operation {
    serde_json::from_value(
        json!({"name":"fixture_packages","description":"Synthetic package fixture",
        "requirements":[{"category":"packages","permission":"read"}],
        "parameters":{"cursor":{"kind":"string","required":false}},
        "action":{"kind":if profile == PackageProfile::Apk3_0_5 { "apk_installed_page" } else { "opkg_status_page" }},"capability":{"kind":if profile == PackageProfile::Apk3_0_5 { "apk_installed_query" } else { "opkg_root_status_file" }}}),
    )
    .unwrap()
}
fn setup(
    profile: PackageProfile,
    allow: bool,
    seed: u64,
    timeout: u64,
) -> (Arc<Dispatcher>, Arc<Target>, Arc<Tokens>, Arc<Log>) {
    let log = Arc::new(Log::default());
    let target = Arc::new(Target {
        calls: AtomicUsize::new(0),
        epoch: AtomicU64::new(1),
        fail: AtomicBool::new(false),
        change_epoch: AtomicBool::new(false),
        stall: AtomicBool::new(false),
        entered: tokio::sync::Notify::new(),
        log: log.clone(),
    });
    let tokens = Arc::new(Tokens::new(seed));
    let policy = if allow {
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
    let dispatcher = Dispatcher::new(
        Catalog::with_builtins(vec![operation(profile)], vec![]).unwrap(),
        policy,
        target.clone(),
        log.clone(),
        Limits {
            timeout_ms: timeout,
            ..Default::default()
        },
    )
    .unwrap()
    .with_snapshot_tokens(tokens.clone());
    (Arc::new(dispatcher), target, tokens, log)
}
async fn invoke(dispatcher: &Dispatcher, args: Value) -> Result<Value, RuntimeError> {
    dispatcher.invoke("fixture_packages", args).await
}
#[tokio::test]
async fn every_page_is_audited_but_capture_and_entropy_happen_only_once() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, target, tokens, log) = setup(profile, true, 1, 10000);
        let mut page = invoke(&d, json!({})).await.unwrap();
        let mut count = page["items"].as_array().unwrap().len();
        let mut pages = 1;
        while let Some(cursor) = page.get("next_cursor") {
            page = invoke(&d, json!({"cursor":cursor})).await.unwrap();
            count += page["items"].as_array().unwrap().len();
            pages += 1;
        }
        assert_eq!(count, 281);
        assert_eq!(pages, 18);
        assert_eq!(target.calls.load(Ordering::SeqCst), 1);
        assert_eq!(tokens.calls.load(Ordering::SeqCst), 1);
        let events = log.events.lock().unwrap();
        assert_eq!(events.len(), 36);
        let encoded = serde_json::to_string(&*events).unwrap();
        for prohibited in [
            "fixture0000",
            "aarch64",
            "next_cursor",
            "version",
            "arguments",
        ] {
            assert!(!encoded.contains(prohibited));
        }
    }
}
#[tokio::test]
async fn denied_invalid_and_start_audit_failure_do_zero_capture_or_entropy() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (denied, t, entropy, _) = setup(profile, false, 1, 10000);
        assert_eq!(
            invoke(&denied, json!({})).await.unwrap_err().code(),
            "permission_denied"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 0);
        assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
        let (d, t, entropy, log) = setup(profile, true, 1, 10000);
        for args in [
            json!({"program":"/bin/sh"}),
            json!({"cursor":""}),
            json!({"cursor":null}),
        ] {
            assert!(invoke(&d, args).await.is_err());
        }
        log.fail_start.store(true, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "audit_unavailable"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 0);
        assert_eq!(entropy.calls.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn metadata_remains_unknown_without_capturing_or_inventing_a_signature() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, target, tokens, _) = setup(profile, true, 1, 10000);
        let status = d
            .capability_status(json!({"operation":"fixture_packages","refresh":true}))
            .await
            .unwrap();
        assert_eq!(status.compatibility, "unknown");
        assert_eq!(status.reason, "capture_required");
        assert_eq!(
            status.scope,
            if profile == PackageProfile::Apk3_0_5 {
                "closed_query_response"
            } else {
                "closed_file_response"
            }
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), 0);
        assert_eq!(tokens.calls.load(Ordering::SeqCst), 0);
    }
}
#[tokio::test]
async fn replay_is_identical_and_foreign_or_invalid_offsets_never_recapture() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, t, _, _) = setup(profile, true, 1, 10000);
        let first = invoke(&d, json!({})).await.unwrap();
        let args = json!({"cursor":first["next_cursor"]});
        let page = invoke(&d, args.clone()).await.unwrap();
        assert_eq!(page, invoke(&d, args.clone()).await.unwrap());
        let (other, other_target, _, _) = setup(profile, true, 50, 10000);
        invoke(&other, json!({})).await.unwrap();
        assert_eq!(
            invoke(&other, args).await.unwrap_err().code(),
            "invalid_cursor"
        );
        assert_eq!(other_target.calls.load(Ordering::SeqCst), 1);
        let cursor = first["next_cursor"]
            .as_str()
            .unwrap()
            .replace(".16", ".4080");
        assert!(invoke(&d, json!({"cursor":cursor})).await.is_err());
        assert_eq!(t.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn failed_refresh_and_entropy_failure_cannot_restore_the_previous_snapshot() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, t, entropy, _) = setup(profile, true, 1, 10000);
        let first = invoke(&d, json!({})).await.unwrap();
        t.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "backend_failed"
        );
        assert_eq!(
            invoke(&d, json!({"cursor":first["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 2);
        t.fail.store(false, Ordering::SeqCst);
        let fresh = invoke(&d, json!({})).await.unwrap();
        entropy.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "entropy_unavailable"
        );
        assert_eq!(
            invoke(&d, json!({"cursor":fresh["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 3);
    }
}
#[tokio::test]
async fn epoch_change_during_capture_or_before_continuation_fails_closed() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, t, _, _) = setup(profile, true, 1, 10000);
        t.change_epoch.store(true, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "invalid_cursor"
        );
        t.change_epoch.store(false, Ordering::SeqCst);
        let page = invoke(&d, json!({})).await.unwrap();
        t.epoch.fetch_add(1, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({"cursor":page["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 2);
    }
}
#[tokio::test]
async fn cancelled_capture_invalidates_old_pages_and_concurrent_capture_is_busy() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, t, _, _) = setup(profile, true, 1, 10000);
        let old = invoke(&d, json!({})).await.unwrap();
        // Consume the initial notification before waiting for the next capture.
        t.entered.notified().await;
        t.stall.store(true, Ordering::SeqCst);
        let copy = d.clone();
        let task = tokio::spawn(async move { invoke(&copy, json!({})).await });
        t.entered.notified().await;
        assert_eq!(invoke(&d, json!({})).await.unwrap_err().code(), "busy");
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(
            invoke(&d, json!({"cursor":old["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 2);
    }
}
#[tokio::test]
async fn timeout_and_completion_audit_failure_never_return_success() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        let (d, t, _, _) = setup(profile, true, 1, 100);
        t.stall.store(true, Ordering::SeqCst);
        assert_eq!(invoke(&d, json!({})).await.unwrap_err().code(), "timeout");
        let (d, t, _, log) = setup(profile, true, 1, 10000);
        log.fail_finish.store(true, Ordering::SeqCst);
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "audit_completion_failed"
        );
        assert_eq!(
            invoke(&d, json!({})).await.unwrap_err().code(),
            "audit_unavailable"
        );
        assert_eq!(t.calls.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn absent_or_zero_entropy_cannot_start_capture() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        struct Zero;
        #[async_trait]
        impl SnapshotTokens for Zero {
            async fn nonce(&self) -> Result<[u8; 16], RuntimeError> {
                Ok([0; 16])
            }
        }
        let (_, target, _, log) = setup(profile, true, 1, 10000);
        let policy = Policy {
            categories: [(
                Category::Packages,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            )]
            .into(),
            ..Default::default()
        };
        let dispatcher = Dispatcher::new(
            Catalog::with_builtins(vec![operation(profile)], vec![]).unwrap(),
            policy,
            target.clone(),
            log,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            invoke(&dispatcher, json!({})).await.unwrap_err().code(),
            "entropy_unavailable"
        );
        let dispatcher = dispatcher.with_snapshot_tokens(Arc::new(Zero));
        assert_eq!(
            invoke(&dispatcher, json!({})).await.unwrap_err().code(),
            "entropy_unavailable"
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn late_non_yielding_entropy_is_rejected_before_backend_capture() {
    for profile in [
        PackageProfile::Apk3_0_5,
        PackageProfile::Opkg38eccbb1RootStatus,
    ] {
        struct Late;
        #[async_trait]
        impl SnapshotTokens for Late {
            async fn nonce(&self) -> Result<[u8; 16], RuntimeError> {
                std::thread::sleep(Duration::from_millis(200));
                Ok([1; 16])
            }
        }
        let (_, target, _, log) = setup(profile, true, 1, 100);
        let policy = Policy {
            categories: [(
                Category::Packages,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            )]
            .into(),
            ..Default::default()
        };
        let dispatcher = Dispatcher::new(
            Catalog::with_builtins(vec![operation(profile)], vec![]).unwrap(),
            policy,
            target.clone(),
            log,
            Limits {
                timeout_ms: 100,
                ..Default::default()
            },
        )
        .unwrap()
        .with_snapshot_tokens(Arc::new(Late));
        assert_eq!(
            invoke(&dispatcher, json!({})).await.unwrap_err().code(),
            "timeout"
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), 0);
    }
}
