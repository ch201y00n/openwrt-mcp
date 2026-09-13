#![cfg(target_os = "windows")]
//! Native audit composition only; no target, real log, Vault or credential access.
mod windows_audit_fixture;
use openwrt_mcp_adapters::{AuditConfig, AuditDestination, AuditFormat, AuditWriter};
use openwrt_mcp_host_platform::read_secret;
use openwrt_mcp_runtime::{AuditEvent, AuditOutcome, AuditPhase, AuditSink};
use serde_json::{Value, json};
use std::{fs, process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use windows_audit_fixture::Fixture;

fn config(f: &Fixture) -> AuditConfig {
    AuditConfig {
        destination: AuditDestination::File,
        path: Some(f.path.join("audit.log")),
        max_bytes: 1024,
        retained_files: 2,
        ..AuditConfig::default()
    }
}

#[test]
fn native_text_audit_sanitizes_names_appends_and_disabled_audit_creates_nothing() {
    let f = Fixture::new();
    let mut settings = config(&f);
    settings.format = AuditFormat::Text;
    let event = AuditEvent::new(
        1,
        AuditPhase::Start,
        "synthetic-secret\nforged-record",
        AuditOutcome::Attempt,
        None,
    );
    let writer = AuditWriter::new(settings.clone()).unwrap();
    writer.record(&event).unwrap();
    drop(writer);
    AuditWriter::new(settings.clone())
        .unwrap()
        .record(&event)
        .unwrap();
    let bytes = read_secret(settings.path.as_ref().unwrap(), 1024).unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(text.lines().count(), 2);
    assert!(!text.contains("synthetic-secret"));
    assert!(!text.contains("forged-record"));
    assert_eq!(text.matches("operation=unknown").count(), 2);
    let disabled = f.path.join("disabled.log");
    settings.path = Some(disabled.clone());
    settings.enabled = false;
    AuditWriter::new(settings).unwrap().record(&event).unwrap();
    assert!(!disabled.exists());
}

#[test]
fn native_rotation_failure_propagates_as_safe_audit_error_without_reopening() {
    let f = Fixture::new();
    let writer = AuditWriter::new(config(&f)).unwrap();
    let event = AuditEvent::new(
        1,
        AuditPhase::Start,
        "system_info",
        AuditOutcome::Attempt,
        None,
    );
    let retained = f.path.join("audit.log.1");
    fs::write(&retained, b"synthetic retained record").unwrap();
    fs::hard_link(&retained, f.path.join("alias.log")).unwrap();
    let error = (0..20)
        .find_map(|_| writer.record(&event).err())
        .expect("rotation must reject a hard-linked generation");
    assert_eq!(error.code(), "audit_unavailable");
    fs::remove_file(f.path.join("alias.log")).unwrap();
    assert_eq!(
        writer.record(&event).unwrap_err().code(),
        "audit_unavailable"
    );
    assert_eq!(fs::read(&retained).unwrap(), b"synthetic retained record");
    assert!(!f.path.join("audit.log.2").exists());
}

async fn response(reader: &mut BufReader<tokio::process::ChildStdout>, id: u64) -> Value {
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).await.unwrap() > 0);
        let value: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        if value["id"] == id {
            return value;
        }
    }
}

#[tokio::test]
async fn actual_windows_mcp_binary_uses_private_rotating_audit_without_payload_disclosure() {
    let f = Fixture::new();
    let audit_path = f.path.join("audit.log");
    let configuration = format!(
        "[policy]\nallow_operations=['system_info']\n[policy.categories.system]\naccess='read'\n[audit]\ndestination='file'\npath={}\nmax_bytes=1024\nretained_files=2\n",
        toml::Value::String(audit_path.to_str().unwrap().into())
    );
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"))
            .args(["serve", "--config-env", "OPENWRT_MCP_AUDIT_FIXTURE_CONFIG"])
            .env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
            .env("OPENWRT_MCP_AUDIT_FIXTURE_CONFIG", configuration)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .kill_on_drop(true).spawn().unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut stderr = child.stderr.take().unwrap().take(65_537);
        let logs = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.len() <= 65_536);
            bytes
        });
        let initialize = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"synthetic-audit-fixture","version":"1"}}});
        input.write_all(format!("{initialize}\n").as_bytes()).await.unwrap();
        assert!(response(&mut output, 1).await.get("result").is_some());
        input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n").await.unwrap();
        for id in 2..42 {
            let (name, arguments, expected) = match id % 4 {
                0 => ("system_info", json!({}), "target_not_configured"),
                1 => ("system_info", json!({"payload":"synthetic-argument-secret"}), "unknown_argument"),
                2 => ("system_board", json!({}), "permission_denied"),
                _ => ("synthetic-client-secret", json!({"payload":"synthetic-argument-secret"}), "unknown_operation"),
            };
            let request = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}});
            input.write_all(format!("{request}\n").as_bytes()).await.unwrap();
            let value = response(&mut output, id).await;
            assert_eq!(value["result"]["isError"], true);
            let error: Value = serde_json::from_str(value["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(error, json!({"error":expected}));
        }
        drop(input);
        drop(output);
        assert!(child.wait().await.unwrap().success());
        let bytes = logs.await.unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("synthetic-client-secret"));
        assert!(!text.contains("synthetic-argument-secret"));
        assert!(!text.contains("request_sequence")); // Audit stays in the configured file.
    }).await.expect("bounded actual native MCP audit fixture");
    let mut previous = 0;
    let mut records = 0;
    for name in ["audit.log.2", "audit.log.1", "audit.log"] {
        let bytes = read_secret(&f.path.join(name), 1024).unwrap();
        assert!(!bytes.is_empty());
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.ends_with('\n'));
        assert!(!text.contains("synthetic-client-secret"));
        assert!(!text.contains("synthetic-argument-secret"));
        assert!(!text.contains("payload"));
        for line in text.lines() {
            let value: Value = serde_json::from_str(line).unwrap();
            assert!(matches!(
                value["operation"].as_str().unwrap(),
                "unknown" | "system_info" | "system_board"
            ));
            let sequence = value["request_sequence"].as_u64().unwrap();
            assert!(sequence >= previous);
            previous = sequence;
            records += 1;
        }
    }
    assert!(records > 2);
    assert!(!f.path.join("audit.log.3").exists());
}
