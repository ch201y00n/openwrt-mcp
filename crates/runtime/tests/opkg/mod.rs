use super::*;

fn both(target: Arc<dyn Backend>, tokens: Arc<Tokens>, log: Arc<Log>) -> Arc<Dispatcher> {
    let mut apk = operation(PackageProfile::Apk3_0_5);
    apk.name = "apk".into();
    let mut opkg = operation(PackageProfile::Opkg38eccbb1RootStatus);
    opkg.name = "opkg".into();
    Arc::new(
        Dispatcher::new(
            Catalog::with_builtins(vec![apk, opkg], vec![]).unwrap(),
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
            },
            target,
            log,
            Limits::default(),
        )
        .unwrap()
        .with_snapshot_tokens(tokens),
    )
}

#[tokio::test]
async fn cross_manager_cursors_and_failed_refresh_share_one_slot_without_fallback() {
    let (_, target, tokens, log) = setup(PackageProfile::Apk3_0_5, true, 1, 10000);
    let d = both(target.clone(), tokens.clone(), log);
    for (first, second, scope) in [
        ("apk", "opkg", "opkg_root_status_file"),
        ("opkg", "apk", "apk_query_installed_visible"),
    ] {
        let calls = target.calls.load(Ordering::SeqCst);
        let old = d.invoke(first, json!({})).await.unwrap();
        let new = d.invoke(second, json!({})).await.unwrap();
        assert_eq!(new["scope"], scope);
        assert_eq!(
            d.invoke(first, json!({"cursor":old["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), calls + 2);
        let fresh = d.invoke(second, json!({})).await.unwrap();
        let page_args = json!({"cursor":fresh["next_cursor"]});
        let page = d.invoke(second, page_args.clone()).await.unwrap();
        assert_eq!(page, d.invoke(second, page_args.clone()).await.unwrap());
        assert_eq!(
            d.invoke(first, page_args).await.unwrap_err().code(),
            "invalid_cursor"
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), calls + 3);
        let old = d.invoke(first, json!({})).await.unwrap();
        target.fail.store(true, Ordering::SeqCst);
        assert_eq!(
            d.invoke(second, json!({})).await.unwrap_err().code(),
            "backend_failed"
        );
        target.fail.store(false, Ordering::SeqCst);
        assert_eq!(
            d.invoke(first, json!({"cursor":old["next_cursor"]}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
        assert_eq!(target.calls.load(Ordering::SeqCst), calls + 5);
    }
}

#[tokio::test]
async fn cross_manager_busy_and_cancelled_capture_cannot_resurrect_other_pages() {
    let (_, target, tokens, log) = setup(PackageProfile::Apk3_0_5, true, 1, 10000);
    let d = both(target.clone(), tokens, log);
    let first = d.invoke("apk", json!({})).await.unwrap();
    target.entered.notified().await;
    target.stall.store(true, Ordering::SeqCst);
    let copy = d.clone();
    let task = tokio::spawn(async move { copy.invoke("opkg", json!({})).await });
    target.entered.notified().await;
    assert_eq!(d.invoke("apk", json!({})).await.unwrap_err().code(), "busy");
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        d.invoke("apk", json!({"cursor":first["next_cursor"]}))
            .await
            .unwrap_err()
            .code(),
        "invalid_cursor"
    );
    assert_eq!(target.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn adapter_profile_mismatch_is_rejected_before_any_page() {
    struct Wrong;
    #[async_trait]
    impl Backend for Wrong {
        fn capability_epoch(&self) -> Option<u64> {
            Some(1)
        }
        async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
            panic!("no generic execute")
        }
        async fn capture_apk_installed(
            &self,
            _: &Limits,
        ) -> Result<PackageObservation, RuntimeError> {
            Ok(PackageObservation::new_opkg_status(vec![]).unwrap())
        }
        async fn capture_opkg_status(
            &self,
            _: &Limits,
        ) -> Result<PackageObservation, RuntimeError> {
            Ok(PackageObservation::new(vec![]).unwrap())
        }
    }
    let log = Arc::new(Log::default());
    let d = both(Arc::new(Wrong), Arc::new(Tokens::new(1)), log.clone());
    for name in ["apk", "opkg"] {
        assert_eq!(
            d.invoke(name, json!({})).await.unwrap_err().code(),
            "invalid_output"
        );
        assert_eq!(
            d.invoke(name, json!({"cursor":format!("{}.16", "01".repeat(16))}))
                .await
                .unwrap_err()
                .code(),
            "invalid_cursor"
        );
    }
    assert!(
        log.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.phase == AuditPhase::Finish)
            .all(|e| e.outcome != openwrt_mcp_runtime::AuditOutcome::Success)
    );
}
