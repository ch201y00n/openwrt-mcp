use super::*;
const BANNER: &[u8] = b"opkg version 38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)\n";
const ROW: &[u8] = b"Package: libfixture-20260913\nVersion: 1:2~abc-r1\nArchitecture: aarch64_generic\nStatus: install hold,user unpacked\nConffiles:\n /fixture/secret excluded-hash\n\n";

#[tokio::test]
async fn opkg_capture_uses_two_literal_commands_one_session_and_selected_file_fields() {
    let mut fixture = Fixture::new(vec![described(BANNER), described(ROW)]).await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    let captured = backend
        .capture_opkg_status(&Limits::default())
        .await
        .unwrap();
    let page = captured.page(&[1; 16], 0).unwrap();
    assert_eq!(page["scope"], "opkg_root_status_file");
    assert_eq!(
        page["items"][0],
        json!({"name":"libfixture-20260913","version":"1:2~abc-r1","arch":"aarch64_generic","status":"install hold,user unpacked"})
    );
    assert!(!page.to_string().contains("secret"));
    assert_eq!(*fixture.observed.commands.lock().unwrap(), vec![
        b"exec '/usr/bin/env' '-i' 'PATH=/usr/sbin:/usr/bin:/sbin:/bin' 'LANG=C' '/bin/opkg' '--version'".to_vec(),
        b"exec '/usr/bin/env' '-i' 'PATH=/usr/sbin:/usr/bin:/sbin:/bin' 'LANG=C' '/bin/cat' '/usr/lib/opkg/status'".to_vec(),
    ]);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn opkg_unknown_version_or_failed_capture_never_falls_back_or_exposes_raw_text() {
    let mut late = ROW.to_vec();
    late.extend_from_slice(b"Package: late-bad\n\n");
    for (replies, expected, count) in [
        (
            vec![described(b"opkg version unreviewed\n")],
            "capability_unsupported",
            1,
        ),
        (
            vec![Reply::Complete {
                stdout: BANNER.to_vec(),
                stderr: b"private-backend-error".to_vec(),
                status: Some(1),
            }],
            "backend_failed",
            1,
        ),
        (
            vec![described(BANNER), described(&late)],
            "invalid_output",
            2,
        ),
        (
            vec![
                described(BANNER),
                Reply::Complete {
                    stdout: ROW.to_vec(),
                    stderr: b"private-backend-error".to_vec(),
                    status: Some(1),
                },
            ],
            "backend_failed",
            2,
        ),
        (
            vec![described(BANNER), described(&vec![b'x'; 65537])],
            "output_limit",
            2,
        ),
    ] {
        let mut fixture = Fixture::new(replies).await;
        let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
        let error = backend
            .capture_opkg_status(&Limits::default())
            .await
            .err()
            .unwrap();
        assert_eq!(error.code(), expected);
        assert!(!error.to_string().contains("private-backend-error"));
        assert_eq!(fixture.observed.commands.lock().unwrap().len(), count);
        drop(backend);
        fixture.closed().await;
    }
}

#[tokio::test]
async fn opkg_version_and_file_share_bounded_deadline_and_missing_exit_is_not_success() {
    for (replies, expected) in [
        (vec![Reply::Hang], "timeout"),
        (vec![described(BANNER), Reply::Hang], "timeout"),
        (
            vec![
                described(BANNER),
                Reply::Complete {
                    stdout: ROW.to_vec(),
                    stderr: vec![],
                    status: None,
                },
            ],
            "backend_failed",
        ),
    ] {
        let mut fixture = Fixture::new(replies).await;
        let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
        let error = backend
            .capture_opkg_status(&Limits {
                timeout_ms: 150,
                ..Default::default()
            })
            .await
            .err()
            .unwrap();
        assert_eq!(error.code(), expected);
        drop(backend);
        fixture.closed().await;
    }
}
