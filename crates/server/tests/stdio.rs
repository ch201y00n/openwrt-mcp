//! Test the shipped binary's actual pipes. Only synthetic workstation programs run.
#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

struct PrivateConfig(PathBuf);
impl PrivateConfig {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("openwrt-mcp-stdio-{}-{unique}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let path = directory.join("fixture.toml");
        fs::write(
            &path,
            include_str!("../../../config/example-extension.toml"),
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        Self(path)
    }
}
impl Drop for PrivateConfig {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

async fn response(reader: &mut BufReader<tokio::process::ChildStdout>, expected_id: u64) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let mut line = String::new();
            assert!(
                reader.read_line(&mut line).await.unwrap() > 0,
                "server closed stdout"
            );
            let value: Value =
                serde_json::from_str(&line).expect("stdout must contain only JSON-RPC");
            assert_eq!(value["jsonrpc"], "2.0");
            if value["id"] == expected_id {
                return value;
            }
        }
    })
    .await
    .expect("protocol response deadline")
}

#[tokio::test]
async fn executable_stdio_honors_policy_executes_exact_fixture_and_audits_without_payload() {
    let config = PrivateConfig::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
        .arg("serve")
        .arg("--config")
        .arg(&config.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    input.write_all(format!("{}\n", json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"protocolVersion":"2025-11-25", "capabilities":{}, "clientInfo":{"name":"fixture-client", "version":"1"}}})).as_bytes()).await.unwrap();
    assert!(response(&mut output, 1).await.get("result").is_some());
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await
        .unwrap();
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
        .await
        .unwrap();
    let listed = response(&mut output, 2).await;
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 1);
    assert_eq!(listed["result"]["tools"][0]["name"], "fixture_echo");
    input.write_all(format!("{}\n", json!({"jsonrpc":"2.0", "id":3, "method":"tools/call", "params":{"name":"fixture_echo", "arguments":{"payload":"{\"ok\":true}"}}})).as_bytes()).await.unwrap();
    let called = response(&mut output, 3).await;
    assert_eq!(called["result"]["structuredContent"]["/ok"], true);
    input.write_all(format!("{}\n", json!({"jsonrpc":"2.0", "id":4, "method":"tools/call", "params":{"name":"fixture_echo", "arguments":{"payload":"synthetic-argument-secret"}}})).as_bytes()).await.unwrap();
    assert_eq!(response(&mut output, 4).await["result"]["isError"], true);
    drop(input);
    drop(output);
    let exited = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(exited.status.success());
    let logs = String::from_utf8(exited.stderr).unwrap();
    assert!(logs.contains("fixture_echo"));
    assert!(!logs.contains("synthetic-argument-secret"));
    assert!(!logs.contains("payload"));
    for line in logs.lines() {
        serde_json::from_str::<Value>(line).unwrap();
    }
}

#[tokio::test]
async fn check_is_offline_and_rejects_invalid_audit_config_without_echoing_contents() {
    let config = PrivateConfig::new();
    let valid = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
        .arg("check")
        .arg("--config")
        .arg(&config.0)
        .output()
        .await
        .unwrap();
    assert!(valid.status.success());
    fs::write(
        &config.0,
        "[audit]\ndestination = 'file'\npath = 'synthetic-private-relative-path'\n",
    )
    .unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
        .arg("check")
        .arg("--config")
        .arg(&config.0)
        .output()
        .await
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(
        !String::from_utf8(invalid.stderr)
            .unwrap()
            .contains("synthetic-private")
    );
}
