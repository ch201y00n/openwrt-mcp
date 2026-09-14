//! Child of the mandatory native remote suite, not a standalone Cargo target.
use super::*;
use openwrt_mcp_runtime::{
    backups::{ArchiveFormat, ArtifactBinding, BackupError},
    mutation_ports::WorkBudget,
};
fn binding() -> ArtifactBinding {
    ArtifactBinding::system(
        [1; 16],
        [2; 16],
        [3; 16],
        [4; 16],
        9,
        32,
        ArchiveFormat::Tar,
    )
    .unwrap()
}
fn budget() -> WorkBudget {
    WorkBudget::new(Duration::from_secs(2)).unwrap()
}
fn envelope() -> Vec<u8> {
    let mut data = binding().encoded().to_vec();
    data.extend_from_slice(&[0x71; 32]);
    data
}

#[tokio::test]
async fn binary_capture_requires_existing_pinned_target_and_exact_request_echo() {
    let mut fixture = Fixture::new(vec![
        success(),
        described(&envelope()),
        described(&envelope()),
    ])
    .await;
    let source = MemoryKey::new(2);
    let backend =
        SshBackend::new_for_capture(fixture.options.clone(), source.clone(), [2; 16]).unwrap();
    assert!(backend.capture_archive(binding(), budget()).await.is_err());
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    backend
        .execute(&action(), &Limits::default())
        .await
        .unwrap();
    for _ in 0..2 {
        let captured = backend.capture_archive(binding(), budget()).await.unwrap();
        assert_eq!(captured.into_parts().1.len(), 32);
    }
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    {
        let requests = fixture.observed.commands.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(requests[1].as_slice() == binding().encoded());
    }
    let wrong = ArtifactBinding::system(
        [1; 16],
        [9; 16],
        [3; 16],
        [4; 16],
        9,
        32,
        ArchiveFormat::Tar,
    )
    .unwrap();
    assert_eq!(
        backend.capture_archive(wrong, budget()).await.err(),
        Some(BackupError::Invalid)
    );
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn binary_capture_rejects_missing_eof_exit_short_long_or_foreign_data_without_reconnect() {
    let mut foreign = envelope();
    foreign[0] ^= 1;
    let mut extra = envelope();
    extra.push(0);
    for reply in [
        Reply::CaptureNoEof(envelope()),
        Reply::Complete {
            stdout: envelope(),
            stderr: vec![],
            status: None,
        },
        Reply::Complete {
            stdout: envelope(),
            stderr: vec![],
            status: Some(1),
        },
        Reply::Complete {
            stdout: envelope(),
            stderr: vec![b'x'; 16385],
            status: Some(0),
        },
        described(&envelope()[..119]),
        described(&extra),
        described(&foreign),
        Reply::Reject,
    ] {
        let mut fixture = Fixture::new(vec![success(), reply]).await;
        let source = MemoryKey::new(2);
        let backend =
            SshBackend::new_for_capture(fixture.options.clone(), source.clone(), [2; 16]).unwrap();
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap();
        assert!(backend.capture_archive(binding(), budget()).await.is_err());
        assert!(backend.capability_epoch().is_none());
        assert!(backend.capture_archive(binding(), budget()).await.is_err());
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        drop(backend);
        fixture.closed().await;
    }
}

#[tokio::test]
async fn hung_capture_observes_shared_cancellation_and_absolute_deadline() {
    for cancelled in [false, true] {
        let mut fixture = Fixture::new(vec![success(), Reply::Hang]).await;
        let backend =
            SshBackend::new_for_capture(fixture.options.clone(), MemoryKey::new(2), [2; 16])
                .unwrap();
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap();
        let budget = WorkBudget::new(Duration::from_millis(150)).unwrap();
        let signal = budget.clone();
        let cancel = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(40)).await;
            if cancelled {
                signal.cancel();
            }
        });
        let error = backend
            .capture_archive(binding(), budget)
            .await
            .err()
            .unwrap();
        assert_eq!(
            error,
            if cancelled {
                BackupError::Cancelled
            } else {
                BackupError::Deadline
            }
        );
        cancel.await.unwrap();
        assert!(backend.capability_epoch().is_none());
        drop(backend);
        fixture.closed().await;
    }
}
