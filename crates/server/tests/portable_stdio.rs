//! Real executable pipes on every host; only child-local synthetic configuration.
use russh::{
    Channel, ChannelId,
    keys::{
        HashAlg, PrivateKey, PublicKey,
        ssh_key::{LineEnding, private::Ed25519Keypair},
    },
    server as ssh_server,
};
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    task::JoinHandle,
};

const CONFIG_ENV: &str = "OPENWRT_MCP_PORTABLE_STDIO_CONFIG";
const DEADLINE: Duration = Duration::from_secs(10);

struct Server {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    logs: JoinHandle<Vec<u8>>,
}

impl Server {
    fn spawn(config: &str) -> Self {
        Self::with_environment(config, &[])
    }

    fn with_environment(config: &str, environment: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"));
        command
            .args(["serve", "--config-env", CONFIG_ENV])
            .env_clear()
            .env(CONFIG_ENV, config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (name, value) in environment {
            command.env(name, value);
        }
        let mut child = command
            .spawn()
            .expect("start the actual native server executable");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let stderr = child.stderr.take().unwrap();
        // Drain concurrently; audit cannot deadlock behind the test's stderr pipe.
        let logs = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.take(65_537).read_to_end(&mut bytes).await.unwrap();
            assert!(
                bytes.len() <= 65_536,
                "fixture audit output exceeded its bound"
            );
            bytes
        });
        Self {
            child,
            input,
            output,
            logs,
        }
    }

    async fn send(&mut self, value: Value) {
        self.input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
    }

    async fn response(&mut self, id: u64) -> Value {
        tokio::time::timeout(DEADLINE, async {
            loop {
                let mut line = String::new();
                assert!(
                    self.output.read_line(&mut line).await.unwrap() > 0,
                    "server closed stdout before its response"
                );
                let value: Value =
                    serde_json::from_str(&line).expect("stdout must contain JSON-RPC only");
                assert_eq!(value["jsonrpc"], "2.0");
                if value["id"] == id {
                    return value;
                }
            }
        })
        .await
        .expect("bounded MCP response")
    }

    async fn initialize(&mut self) {
        self.send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
                "protocolVersion":"2025-11-25","capabilities":{},
                "clientInfo":{"name":"native-synthetic-fixture","version":"1"}
            }}),
        )
        .await;
        assert!(self.response(1).await.get("result").is_some());
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
    }

    async fn call(&mut self, id: u64, name: &str, arguments: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})).await;
        self.response(id).await
    }

    async fn finish(mut self) -> Vec<Value> {
        drop(self.input);
        drop(self.output);
        let status = tokio::time::timeout(DEADLINE, self.child.wait())
            .await
            .expect("bounded server shutdown")
            .unwrap();
        assert!(
            status.success(),
            "server must shut down successfully after stdin EOF"
        );
        let bytes = tokio::time::timeout(DEADLINE, self.logs)
            .await
            .unwrap()
            .unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("synthetic-client-secret"));
        assert!(!text.contains("synthetic-argument-secret"));
        assert!(!text.contains("payload"));
        let values: Vec<Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(
            values
                .iter()
                .any(|value| value["event"] == "server_starting")
        );
        assert!(
            values
                .iter()
                .any(|value| value["event"] == "server_stopped")
        );
        values
    }
}

fn error(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], true);
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn native_binary_default_deny_initializes_lists_rejects_and_shuts_down() {
    let mut server = Server::spawn("");
    server.initialize().await;
    server
        .send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .await;
    let list = server.response(2).await;
    assert!(list["result"]["tools"].as_array().unwrap().is_empty());
    let denied = server.call(3, "system_info", json!({})).await;
    assert_eq!(error(&denied), json!({"error":"permission_denied"}));
    let unknown = server
        .call(
            4,
            "synthetic-client-secret",
            json!({"payload":"synthetic-argument-secret"}),
        )
        .await;
    assert_eq!(error(&unknown), json!({"error":"unknown_operation"}));
    let logs = server.finish().await;
    assert!(logs.iter().any(|value| value["phase"] == "rejection"
        && value["outcome"] == "denied"
        && value["operation"] == "system_info"));
    assert!(logs.iter().any(|value| value["phase"] == "rejection"
        && value["outcome"] == "unknown_operation"
        && value["operation"] == "unknown"));
}

#[tokio::test]
async fn authorized_calls_without_target_do_not_execute_host_router_commands() {
    for target in ["", "[target]\nkind = 'unconfigured'\n"] {
        let config = format!("[policy.categories.system]\naccess = 'read'\n{target}");
        let mut server = Server::spawn(&config);
        server.initialize().await;
        server
            .send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
            .await;
        let list = server.response(2).await;
        let names: Vec<_> = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"system_info"));
        assert!(names.contains(&"system_board"));
        assert!(!names.contains(&"network_wan_status"));
        let allowed = server.call(3, "system_info", json!({})).await;
        assert_eq!(error(&allowed), json!({"error":"target_not_configured"}));
        let invalid = server
            .call(
                4,
                "system_info",
                json!({"payload":"synthetic-argument-secret"}),
            )
            .await;
        assert_eq!(error(&invalid), json!({"error":"unknown_argument"}));
        let logs = server.finish().await;
        assert!(logs.iter().any(|value| value["phase"] == "start"
            && value["outcome"] == "attempt"
            && value["operation"] == "system_info"));
        assert!(logs.iter().any(|value| value["phase"] == "finish"
            && value["outcome"] == "failed"
            && value["operation"] == "system_info"));
    }
}

#[tokio::test]
async fn missing_ssh_identity_fails_without_connecting_or_falling_back_to_local_execution() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let config = format!(
        "[policy.categories.system]\naccess='read'\n[target]\nkind='ssh'\nidentity_source='missing-key'\n[target.options]\nhost='127.0.0.1'\nport={port}\nusername='fixture'\nhost_key_sha256='SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'\n[target.sources.missing-key]\nkind='environment'\nvariable='OPENWRT_MCP_MISSING_SYNTHETIC_KEY'\n"
    );
    let mut server = Server::spawn(&config);
    server.initialize().await;
    let failed = server.call(2, "system_info", json!({})).await;
    assert_eq!(error(&failed), json!({"error":"authentication_failed"}));
    let logs = server.finish().await;
    assert!(
        logs.iter()
            .any(|value| value["phase"] == "finish" && value["outcome"] == "failed")
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

struct SyntheticSsh {
    identity: PublicKey,
    authentications: Arc<AtomicUsize>,
    commands: Arc<Mutex<Vec<Vec<u8>>>>,
    channels: Vec<Channel<ssh_server::Msg>>,
}

impl ssh_server::Handler for SyntheticSsh {
    type Error = russh::Error;

    async fn auth_publickey(
        &mut self,
        user: &str,
        key: &PublicKey,
    ) -> Result<ssh_server::Auth, Self::Error> {
        if user == "fixture" && key.key_data() == self.identity.key_data() {
            self.authentications.fetch_add(1, Ordering::SeqCst);
            Ok(ssh_server::Auth::Accept)
        } else {
            Ok(ssh_server::Auth::reject())
        }
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<ssh_server::Msg>,
        reply: ssh_server::ChannelOpenHandle,
        _: &mut ssh_server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        self.channels.push(channel);
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        command: &[u8],
        session: &mut ssh_server::Session,
    ) -> Result<(), Self::Error> {
        self.commands.lock().unwrap().push(command.to_vec());
        session.channel_success(channel)?;
        // These are fabricated responses, not output from a workstation command.
        session.data(channel, b"{\"uptime\":123,\"memory\":{\"total\":1024},\"private_key\":\"synthetic-response-secret\"}".as_slice())?;
        session.extended_data(channel, 1, b"synthetic-stderr-secret".as_slice())?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
}

fn synthetic_key(seed: u8) -> PrivateKey {
    // Deterministic public test seeds, never deployment credentials or files.
    PrivateKey::new(
        Ed25519Keypair::from_seed(&[seed; 32]).into(),
        "synthetic-only",
    )
    .unwrap()
}

#[tokio::test]
async fn actual_binary_uses_pinned_loopback_ssh_and_preserves_policy_projection_and_audit() {
    let host_key = synthetic_key(31);
    let identity = synthetic_key(47);
    let pin = host_key
        .public_key()
        .fingerprint(HashAlg::Sha256)
        .to_string();
    let pem = identity.to_openssh(LineEnding::LF).unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let authentications = Arc::new(AtomicUsize::new(0));
    let commands = Arc::new(Mutex::new(Vec::new()));
    let handler = SyntheticSsh {
        identity: identity.public_key().clone(),
        authentications: authentications.clone(),
        commands: commands.clone(),
        channels: Vec::new(),
    };
    let remote = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        drop(listener); // Exactly one connection: subsequent calls must reuse it.
        let config = ssh_server::Config {
            keys: vec![host_key],
            auth_rejection_time: Duration::ZERO,
            auth_rejection_time_initial: Some(Duration::ZERO),
            ..Default::default()
        };
        let running = ssh_server::run_stream(Arc::new(config), stream, handler)
            .await
            .expect("synthetic SSH handshake starts");
        // EOF/socket shutdown on normal client teardown can be an SSH I/O error.
        let _ = running.await;
    });
    let config = format!(
        "[policy.categories.system]\naccess='read'\n[target]\nkind='ssh'\nidentity_source='ssh-key'\n[target.options]\nhost='127.0.0.1'\nport={port}\nusername='fixture'\nhost_key_sha256='{pin}'\n[target.sources.ssh-key]\nkind='environment'\nvariable='OPENWRT_MCP_SYNTHETIC_SSH_IDENTITY'\n"
    );
    let mut server = Server::with_environment(
        &config,
        &[("OPENWRT_MCP_SYNTHETIC_SSH_IDENTITY", pem.as_str())],
    );
    server.initialize().await;
    let denied = server.call(2, "network_wan_status", json!({})).await;
    assert_eq!(error(&denied), json!({"error":"permission_denied"}));
    assert_eq!(authentications.load(Ordering::SeqCst), 0);
    for id in [3, 4] {
        let response = server.call(id, "system_info", json!({})).await;
        assert_ne!(response["result"]["isError"], true);
        assert_eq!(response["result"]["structuredContent"]["/uptime"], 123);
        assert_eq!(
            response["result"]["structuredContent"]["/memory/total"],
            1024
        );
        assert!(!response.to_string().contains("synthetic-response-secret"));
        assert!(!response.to_string().contains("synthetic-stderr-secret"));
    }
    let invalid = server
        .call(
            5,
            "system_info",
            json!({"payload":"synthetic-argument-secret"}),
        )
        .await;
    assert_eq!(error(&invalid), json!({"error":"unknown_argument"}));
    let logs = server.finish().await;
    tokio::time::timeout(DEADLINE, remote)
        .await
        .expect("bounded fake SSH shutdown")
        .unwrap();
    assert_eq!(authentications.load(Ordering::SeqCst), 1);
    let commands = commands.lock().unwrap();
    assert_eq!(commands.len(), 2);
    for command in commands.iter() {
        assert_eq!(
            command.as_slice(),
            b"exec '/bin/ubus' '-S' 'call' 'system' 'info' '{}'"
        );
    }
    assert_eq!(
        logs.iter()
            .filter(|value| value["phase"] == "finish"
                && value["outcome"] == "success"
                && value["operation"] == "system_info")
            .count(),
        2
    );
    let text = serde_json::to_string(&logs).unwrap();
    for prohibited in [
        "BEGIN OPENSSH PRIVATE KEY",
        "synthetic-response-secret",
        "synthetic-stderr-secret",
        pem.as_str(),
    ] {
        assert!(
            !text.contains(prohibited),
            "audit output must contain no key, raw response or raw stderr"
        );
    }
}
