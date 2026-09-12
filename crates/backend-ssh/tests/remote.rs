//! Synthetic keys and in-process loopback SSH only; no real router or key files.
use openwrt_mcp_backend_ssh::{SshBackend, SshOptions};
use openwrt_mcp_core::{
    CapabilityObservation, PreparedAction, ProbeRequest, ReviewedObject, UnknownReason,
};
use openwrt_mcp_runtime::{
    Backend, Limits, RuntimeError,
    protection::{KeyMaterial, KeySource, ProtectionError},
};
use russh::{Channel, ChannelId, server};
use serde_json::json;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::Notify, task::JoinHandle};

fn key(seed: u8) -> russh::keys::PrivateKey {
    russh::keys::PrivateKey::new(
        russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]).into(),
        "synthetic",
    )
    .unwrap()
}

struct MemoryKey {
    key: russh::keys::PrivateKey,
    reads: AtomicUsize,
}
impl MemoryKey {
    fn new(seed: u8) -> Arc<Self> {
        Arc::new(Self {
            key: key(seed),
            reads: AtomicUsize::new(0),
        })
    }
}
impl KeySource for MemoryKey {
    fn read(&self, maximum: usize) -> Result<KeyMaterial, ProtectionError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let pem = self
            .key
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .unwrap();
        KeyMaterial::new(pem.as_bytes().to_vec(), maximum)
    }
}

enum Reply {
    Complete {
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        status: Option<u32>,
    },
    Hang,
    StatusWithoutClose,
    Reject,
}
fn success() -> Reply {
    Reply::Complete {
        stdout: br#"{"ok":true}"#.to_vec(),
        stderr: Vec::new(),
        status: Some(0),
    }
}

fn described(stdout: &[u8]) -> Reply {
    Reply::Complete {
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
        status: Some(0),
    }
}

fn probe_request() -> ProbeRequest {
    ProbeRequest::DescribeUbusObject(ReviewedObject::System)
}

#[derive(Default)]
struct Observed {
    commands: Mutex<Vec<Vec<u8>>>,
    submitted: Notify,
    connections: AtomicUsize,
}
struct FakeServer {
    allowed: russh::keys::PublicKey,
    replies: VecDeque<Reply>,
    observed: Arc<Observed>,
    channels: Vec<Channel<server::Msg>>,
}
impl server::Handler for FakeServer {
    type Error = russh::Error;
    async fn auth_publickey(
        &mut self,
        user: &str,
        offered: &russh::keys::PublicKey,
    ) -> Result<server::Auth, Self::Error> {
        Ok(
            if user == "fixture" && offered.key_data() == self.allowed.key_data() {
                server::Auth::Accept
            } else {
                server::Auth::reject()
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        self.channels.push(channel);
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        command: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        self.observed
            .commands
            .lock()
            .unwrap()
            .push(command.to_vec());
        self.observed.submitted.notify_one();
        match self.replies.pop_front().unwrap_or_else(success) {
            Reply::Reject => session.channel_failure(channel)?,
            Reply::Hang => session.channel_success(channel)?,
            Reply::StatusWithoutClose => {
                session.channel_success(channel)?;
                session.exit_status_request(channel, 0)?;
            }
            Reply::Complete {
                stdout,
                stderr,
                status,
            } => {
                session.channel_success(channel)?;
                for chunk in stdout.chunks(8192) {
                    session.data(channel, chunk.to_vec())?;
                }
                for chunk in stderr.chunks(8192) {
                    session.extended_data(channel, 1, chunk.to_vec())?;
                }
                if let Some(status) = status {
                    session.exit_status_request(channel, status)?;
                }
                session.eof(channel)?;
                session.close(channel)?;
            }
        }
        Ok(())
    }
}

struct Fixture {
    options: SshOptions,
    observed: Arc<Observed>,
    task: JoinHandle<()>,
}
impl Fixture {
    async fn new(replies: Vec<Reply>) -> Self {
        let host = key(1);
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let options = SshOptions {
            host: "127.0.0.1".into(),
            port: listener.local_addr().unwrap().port(),
            username: "fixture".into(),
            host_key_sha256: host
                .public_key()
                .fingerprint(russh::keys::HashAlg::Sha256)
                .to_string(),
        };
        let observed = Arc::new(Observed::default());
        let events = observed.clone();
        let config = Arc::new(server::Config {
            keys: vec![host],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        });
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            events.connections.fetch_add(1, Ordering::SeqCst);
            let handler = FakeServer {
                allowed: key(2).public_key().clone(),
                replies: replies.into(),
                observed: events,
                channels: Vec::new(),
            };
            if let Ok(running) = server::run_stream(config, stream, handler).await {
                let _ = running.await;
            }
        });
        Self {
            options,
            observed,
            task,
        }
    }
    async fn closed(&mut self) {
        tokio::time::timeout(Duration::from_secs(2), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn action() -> PreparedAction {
    PreparedAction::Ubus {
        object: "system".into(),
        method: "info".into(),
        arguments: json!({}),
    }
}

#[tokio::test]
async fn offline_construction_and_invalid_options_never_read_source() {
    let fixture = Fixture::new(vec![]).await;
    let source = MemoryKey::new(2);
    let _backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 0);
    for host in [
        "",
        "user@host",
        "-option",
        "host name",
        "host/command",
        "host\0",
        "..",
    ] {
        let mut options = fixture.options.clone();
        options.host = host.into();
        assert!(matches!(
            options.validate(),
            Err(RuntimeError::InvalidConfig)
        ));
    }
    for pin in [
        "",
        "SHA1:bad",
        "SHA256:bad",
        "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n",
    ] {
        let mut options = fixture.options.clone();
        options.host_key_sha256 = pin.into();
        assert!(options.validate().is_err());
    }
    for host in ["localhost", "router.example", "::1"] {
        let mut options = fixture.options.clone();
        options.host = host.into();
        options.validate().unwrap();
    }
    assert!(serde_json::from_value::<SshOptions>(json!({"host":"localhost","port":22,"username":"fixture","host_key_sha256":fixture.options.host_key_sha256,"command":"forbidden"})).is_err());
}

#[tokio::test]
async fn authenticated_calls_reuse_connection_and_quote_router_arguments_exactly() {
    let mut fixture = Fixture::new(vec![success(), success()]).await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    let action = PreparedAction::Process {
        program: "/usr/bin/fixture".into(),
        args: vec!["a'b;$(literal)\n\"".into(), "".into()],
    };
    assert_eq!(
        backend.execute(&action, &Limits::default()).await.unwrap(),
        json!({"ok":true})
    );
    let ubus = PreparedAction::Ubus {
        object: "network.interface.wan".into(),
        method: "status".into(),
        arguments: json!({"count":3,"enabled":true,"name":"x'y"}),
    };
    backend.execute(&ubus, &Limits::default()).await.unwrap();
    let commands = fixture.observed.commands.lock().unwrap().clone();
    assert_eq!(
        commands[0],
        b"exec '/usr/bin/fixture' 'a'\\''b;$(literal)\n\"' ''"
    );
    assert_eq!(commands[1], b"exec '/bin/ubus' '-S' 'call' 'network.interface.wan' 'status' '{\"count\":3,\"enabled\":true,\"name\":\"x'\\''y\"}'");
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn wrong_host_key_and_wrong_identity_fail_closed_without_exec_or_retry() {
    for wrong_host in [true, false] {
        let mut fixture = Fixture::new(vec![success()]).await;
        let source = MemoryKey::new(if wrong_host { 2 } else { 3 });
        let mut options = fixture.options.clone();
        if wrong_host {
            options.host_key_sha256 = key(3)
                .public_key()
                .fingerprint(russh::keys::HashAlg::Sha256)
                .to_string();
        }
        let backend = SshBackend::new(options, source.clone()).unwrap();
        let error = backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap_err();
        assert_eq!(
            error.code(),
            if wrong_host {
                "host_key_rejected"
            } else {
                "authentication_failed"
            }
        );
        assert!(matches!(
            backend.execute(&action(), &Limits::default()).await,
            Err(RuntimeError::BackendFailed)
        ));
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        assert!(fixture.observed.commands.lock().unwrap().is_empty());
        fixture.closed().await;
    }
}

#[tokio::test]
async fn complete_nonzero_and_invalid_json_results_leave_connection_reusable() {
    let mut fixture = Fixture::new(vec![
        Reply::Complete {
            stdout: b"{\"secret\":true}".to_vec(),
            stderr: b"synthetic-secret-stderr".to_vec(),
            status: Some(7),
        },
        Reply::Complete {
            stdout: b"synthetic-not-json".to_vec(),
            stderr: Vec::new(),
            status: Some(0),
        },
        Reply::Complete {
            stdout: b" \n\t".to_vec(),
            stderr: Vec::new(),
            status: Some(0),
        },
        success(),
    ])
    .await;
    let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
    assert_eq!(
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap_err()
            .code(),
        "backend_failed"
    );
    assert_eq!(
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap_err()
            .code(),
        "invalid_output"
    );
    assert_eq!(
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap(),
        json!(null)
    );
    backend
        .execute(&action(), &Limits::default())
        .await
        .unwrap();
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn strict_action_decoder_rejects_completed_bad_json_without_replay_or_reconnect() {
    let too_deep = format!("{}null{}", "[".repeat(33), "]".repeat(33)).into_bytes();
    let too_many = format!("[{}]", vec!["0"; 65_536].join(",")).into_bytes();
    let outputs = [
        (br#"{"name":1,"\u006eame":2}"#.to_vec(), "invalid_output"),
        (
            br#"{"public":true,"foreign":{"fixture-secret":1,"fixture-secret":2}}"#.to_vec(),
            "invalid_output",
        ),
        (br#"{"fixture-secret":"#.to_vec(), "invalid_output"),
        (b"{} {}".to_vec(), "invalid_output"),
        (b"\"\xff\"".to_vec(), "invalid_output"),
        (b"1e400".to_vec(), "invalid_output"),
        (too_deep, "output_limit"),
        (too_many, "output_limit"),
    ];
    let mut replies = outputs
        .iter()
        .map(|(bytes, _)| described(bytes))
        .collect::<Vec<_>>();
    replies.push(success());
    let mut fixture = Fixture::new(replies).await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    let limits = Limits {
        max_output_bytes: 256 * 1024,
        ..Limits::default()
    };
    for (index, (_, code)) in outputs.iter().enumerate() {
        let error = backend.execute(&action(), &limits).await.unwrap_err();
        assert_eq!(error.code(), *code);
        assert!(!error.to_string().contains("fixture-secret"));
        assert_eq!(backend.capability_epoch(), Some(1));
        assert_eq!(fixture.observed.commands.lock().unwrap().len(), index + 1);
    }
    assert_eq!(
        backend.execute(&action(), &limits).await.unwrap(),
        json!({"ok":true})
    );
    assert_eq!(
        fixture.observed.commands.lock().unwrap().len(),
        outputs.len() + 1
    );
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn stdout_and_stderr_bounds_fail_and_close_without_replay() {
    for stderr in [false, true] {
        let mut fixture = Fixture::new(vec![Reply::Complete {
            stdout: if stderr {
                b"{}".to_vec()
            } else {
                vec![b'x'; 33]
            },
            stderr: if stderr { vec![b'x'; 33] } else { Vec::new() },
            status: Some(0),
        }])
        .await;
        let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
        let limits = Limits {
            max_output_bytes: 32,
            ..Limits::default()
        };
        assert!(matches!(
            backend.execute(&action(), &limits).await,
            Err(RuntimeError::OutputLimit)
        ));
        assert!(matches!(
            backend.execute(&action(), &limits).await,
            Err(RuntimeError::BackendFailed)
        ));
        assert_eq!(fixture.observed.commands.lock().unwrap().len(), 1);
        fixture.closed().await;
    }
}

#[tokio::test]
async fn exact_output_limits_and_multiple_channel_windows_are_accepted() {
    let maximum = 128 * 1024;
    let mut stdout = vec![b' '; maximum];
    stdout[..2].copy_from_slice(b"{}");
    let mut fixture = Fixture::new(vec![
        Reply::Complete {
            stdout,
            stderr: vec![b'x'; maximum],
            status: Some(0),
        },
        success(),
    ])
    .await;
    let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
    let limits = Limits {
        max_output_bytes: maximum,
        ..Limits::default()
    };
    assert_eq!(
        backend.execute(&action(), &limits).await.unwrap(),
        json!({})
    );
    backend.execute(&action(), &limits).await.unwrap();
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn missing_exit_status_and_rejected_exec_are_not_success() {
    for reply in [
        Reply::Complete {
            stdout: b"{}".to_vec(),
            stderr: Vec::new(),
            status: None,
        },
        Reply::Reject,
    ] {
        let mut fixture = Fixture::new(vec![reply]).await;
        let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
        assert!(matches!(
            backend.execute(&action(), &Limits::default()).await,
            Err(RuntimeError::BackendFailed)
        ));
        assert!(matches!(
            backend.execute(&action(), &Limits::default()).await,
            Err(RuntimeError::BackendFailed)
        ));
        fixture.closed().await;
    }
}

#[tokio::test]
async fn deadline_includes_channel_completion_and_poisons_session() {
    let mut fixture = Fixture::new(vec![Reply::StatusWithoutClose]).await;
    let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
    let limits = Limits {
        timeout_ms: 200,
        ..Limits::default()
    };
    assert!(matches!(
        backend.execute(&action(), &limits).await,
        Err(RuntimeError::Timeout)
    ));
    assert!(matches!(
        backend.execute(&action(), &limits).await,
        Err(RuntimeError::BackendFailed)
    ));
    fixture.closed().await;
}

#[tokio::test]
async fn overlap_is_busy_and_caller_cancellation_closes_session_without_retry() {
    let mut fixture = Fixture::new(vec![Reply::Hang]).await;
    let source = MemoryKey::new(2);
    let backend = Arc::new(SshBackend::new(fixture.options.clone(), source.clone()).unwrap());
    let running_backend = backend.clone();
    let request =
        tokio::spawn(async move { running_backend.execute(&action(), &Limits::default()).await });
    tokio::time::timeout(
        Duration::from_secs(2),
        fixture.observed.submitted.notified(),
    )
    .await
    .unwrap();
    assert!(matches!(
        backend.execute(&action(), &Limits::default()).await,
        Err(RuntimeError::Busy)
    ));
    request.abort();
    let _ = request.await;
    assert!(matches!(
        backend.execute(&action(), &Limits::default()).await,
        Err(RuntimeError::BackendFailed)
    ));
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    fixture.closed().await;
}

struct SlowSource {
    reads: AtomicUsize,
}
impl KeySource for SlowSource {
    fn read(&self, _: usize) -> Result<KeyMaterial, ProtectionError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(80));
        Err(ProtectionError::SourceUnavailable)
    }
}
#[tokio::test]
async fn timed_out_source_creates_only_one_blocking_worker() {
    let fixture = Fixture::new(vec![]).await;
    let source = Arc::new(SlowSource {
        reads: AtomicUsize::new(0),
    });
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    let limits = Limits {
        timeout_ms: 20,
        ..Limits::default()
    };
    assert!(matches!(
        backend.execute(&action(), &limits).await,
        Err(RuntimeError::Timeout)
    ));
    for _ in 0..5 {
        assert!(matches!(
            backend.execute(&action(), &limits).await,
            Err(RuntimeError::BackendFailed)
        ));
    }
    tokio::time::sleep(Duration::from_millis(90)).await;
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_prepared_actions_and_limits_do_not_read_key_or_connect() {
    let fixture = Fixture::new(vec![]).await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    for action in [
        PreparedAction::Process {
            program: "C:\\Windows\\host.exe".into(),
            args: vec![],
        },
        PreparedAction::Process {
            program: "/bin/../host".into(),
            args: vec![],
        },
        PreparedAction::Process {
            program: "/bin/fixture".into(),
            args: vec!["nul\0".into()],
        },
        PreparedAction::Process {
            program: "/bin/fixture".into(),
            args: vec!["x".repeat(1025)],
        },
        PreparedAction::Process {
            program: "/bin/fixture".into(),
            args: vec!["'".repeat(1024); 64],
        },
        PreparedAction::Ubus {
            object: "system;injected".into(),
            method: "info".into(),
            arguments: json!({}),
        },
        PreparedAction::Ubus {
            object: "system".into(),
            method: "info".into(),
            arguments: json!({"nested":{"x":42}}),
        },
    ] {
        assert!(backend.execute(&action, &Limits::default()).await.is_err());
    }
    assert!(matches!(
        backend
            .execute(
                &action(),
                &Limits {
                    timeout_ms: 0,
                    ..Limits::default()
                }
            )
            .await,
        Err(RuntimeError::InvalidConfig)
    ));
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 0);
}

struct InvalidSource {
    bytes: Vec<u8>,
    reads: AtomicUsize,
}
impl KeySource for InvalidSource {
    fn read(&self, maximum: usize) -> Result<KeyMaterial, ProtectionError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        assert_eq!(maximum, 65_536);
        // Deliberately violate the source contract for the oversized fixture:
        // the backend must independently reject it before parsing or networking.
        KeyMaterial::new(self.bytes.clone(), 1024 * 1024)
    }
}

#[tokio::test]
async fn malformed_and_oversized_identity_fail_before_connection_and_never_retry() {
    for bytes in [
        b"synthetic-not-an-openssh-private-key".to_vec(),
        vec![b'x'; 65_537],
    ] {
        let fixture = Fixture::new(vec![]).await;
        let source = Arc::new(InvalidSource {
            bytes,
            reads: AtomicUsize::new(0),
        });
        let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
        assert!(matches!(
            backend.execute(&action(), &Limits::default()).await,
            Err(RuntimeError::AuthenticationFailed)
        ));
        assert!(matches!(
            backend.execute(&action(), &Limits::default()).await,
            Err(RuntimeError::BackendFailed)
        ));
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn probe_and_execute_share_one_authenticated_session_and_epoch() {
    let mut fixture = Fixture::new(vec![
        described(b"'system' @00000001\n\t\"info\":{}\n"),
        success(),
    ])
    .await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    assert_eq!(backend.capability_epoch(), None);
    let CapabilityObservation::Ubus(observed) = backend
        .probe(probe_request(), &Limits::default())
        .await
        .unwrap()
    else {
        panic!("expected metadata")
    };
    assert!(observed.methods.contains_key("info"));
    assert_eq!(backend.capability_epoch(), Some(1));
    assert_eq!(
        backend
            .execute(&action(), &Limits::default())
            .await
            .unwrap(),
        json!({"ok":true})
    );
    assert_eq!(backend.capability_epoch(), Some(1));
    assert_eq!(source.reads.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    assert_eq!(
        *fixture.observed.commands.lock().unwrap(),
        vec![
            b"exec '/bin/ubus' '-v' 'list' 'system'".to_vec(),
            b"exec '/bin/ubus' '-S' 'call' 'system' 'info' '{}'".to_vec()
        ]
    );
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn complete_invalid_empty_and_nonzero_probes_never_create_positive_evidence() {
    let mut fixture = Fixture::new(vec![
        described(b"'service' @00000001\n\t\"info\":{}\n"),
        described(b"'system' @00000001\n\t\"info\":{}\n\t\"info\":{}\n"),
        described(b"'system' @00000001\n\t\"info\":{}"),
        described(b""),
        Reply::Complete {
            stdout: Vec::new(),
            stderr: b"synthetic-private-error".to_vec(),
            status: Some(4),
        },
        success(),
    ])
    .await;
    let backend = SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap();
    for _ in 0..3 {
        assert_eq!(
            backend
                .probe(probe_request(), &Limits::default())
                .await
                .unwrap(),
            CapabilityObservation::Unknown(UnknownReason::InvalidObservation)
        );
        assert_eq!(backend.capability_epoch(), Some(1));
    }
    assert_eq!(
        backend
            .probe(probe_request(), &Limits::default())
            .await
            .unwrap(),
        CapabilityObservation::Unknown(UnknownReason::NotObservedOrHidden)
    );
    let error = backend
        .probe(
            ProbeRequest::DescribeUbusObject(ReviewedObject::NetworkInterfaceWan),
            &Limits::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "backend_failed");
    assert!(!error.to_string().contains("private"));
    backend
        .execute(&action(), &Limits::default())
        .await
        .unwrap();
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 1);
    drop(backend);
    fixture.closed().await;
}

#[tokio::test]
async fn probe_bounds_missing_completion_and_timeout_revoke_epoch_without_replay() {
    for (reply, maximum, code) in [
        (Reply::Hang, 65_536, "timeout"),
        (Reply::StatusWithoutClose, 65_536, "timeout"),
        (described(&[b'x'; 33]), 32, "output_limit"),
        (described(&vec![b'x'; 65_537]), 131_072, "output_limit"),
        (
            Reply::Complete {
                stdout: b"'system' @00000001\n".to_vec(),
                stderr: Vec::new(),
                status: None,
            },
            65_536,
            "backend_failed",
        ),
    ] {
        let mut fixture = Fixture::new(vec![
            described(b"'system' @00000001\n\t\"info\":{}\n"),
            reply,
        ])
        .await;
        let source = MemoryKey::new(2);
        let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
        backend
            .probe(probe_request(), &Limits::default())
            .await
            .unwrap();
        assert_eq!(backend.capability_epoch(), Some(1));
        let limits = Limits {
            timeout_ms: 200,
            max_output_bytes: maximum,
            ..Limits::default()
        };
        assert_eq!(
            backend
                .probe(probe_request(), &limits)
                .await
                .unwrap_err()
                .code(),
            code
        );
        assert_eq!(backend.capability_epoch(), None);
        assert_eq!(
            backend
                .probe(probe_request(), &Limits::default())
                .await
                .unwrap_err()
                .code(),
            "backend_failed"
        );
        assert_eq!(fixture.observed.commands.lock().unwrap().len(), 2);
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        fixture.closed().await;
    }
}

#[tokio::test]
async fn cancelled_probe_revokes_epoch_and_cannot_restart_or_overlap_with_execute() {
    let mut fixture = Fixture::new(vec![Reply::Hang]).await;
    let backend = Arc::new(SshBackend::new(fixture.options.clone(), MemoryKey::new(2)).unwrap());
    let running = backend.clone();
    let task =
        tokio::spawn(async move { running.probe(probe_request(), &Limits::default()).await });
    tokio::time::timeout(
        Duration::from_secs(2),
        fixture.observed.submitted.notified(),
    )
    .await
    .unwrap();
    assert_eq!(backend.capability_epoch(), Some(1));
    assert!(matches!(
        backend.execute(&action(), &Limits::default()).await,
        Err(RuntimeError::Busy)
    ));
    task.abort();
    let _ = task.await;
    assert_eq!(backend.capability_epoch(), None);
    assert!(matches!(
        backend.probe(probe_request(), &Limits::default()).await,
        Err(RuntimeError::BackendFailed)
    ));
    assert_eq!(fixture.observed.commands.lock().unwrap().len(), 1);
    fixture.closed().await;
}

#[tokio::test]
async fn invalid_probe_limits_leave_source_and_connection_untouched() {
    let fixture = Fixture::new(vec![]).await;
    let source = MemoryKey::new(2);
    let backend = SshBackend::new(fixture.options.clone(), source.clone()).unwrap();
    for limits in [
        Limits {
            timeout_ms: 0,
            ..Limits::default()
        },
        Limits {
            max_output_bytes: usize::MAX,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            backend.probe(probe_request(), &limits).await,
            Err(RuntimeError::InvalidConfig)
        ));
    }
    assert_eq!(backend.capability_epoch(), None);
    assert_eq!(source.reads.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.observed.connections.load(Ordering::SeqCst), 0);
}
