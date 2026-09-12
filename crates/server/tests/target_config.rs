//! Typed target/configuration boundaries; child environment never mutates the parent.
use openwrt_mcp::config::Config;
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

const CONFIG_ENV: &str = "OPENWRT_MCP_TARGET_CONFIG_FIXTURE";
const PIN: &str = "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

fn ssh_config(port: u16) -> String {
    format!(
        "[policy.categories.system]\naccess='read'\n[target]\nkind='ssh'\nidentity_source='ssh-key'\n[target.options]\nhost='127.0.0.1'\nport={port}\nusername='fixture'\nhost_key_sha256='{PIN}'\n[target.sources.ssh-key]\nkind='environment'\nvariable='OPENWRT_MCP_NEVER_SET_SYNTHETIC_IDENTITY'\n"
    )
}

async fn run(command: &str, config: &str) -> std::process::Output {
    run_arguments(&[command, "--config-env", CONFIG_ENV], config).await
}

async fn run_arguments(arguments: &[&str], config: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openwrt-mcp"));
    child
        .args(arguments)
        .env_clear()
        .env(CONFIG_ENV, config)
        .stdin(Stdio::null())
        .kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(10), child.output())
        .await
        .expect("bounded offline command")
        .unwrap()
}

#[test]
fn target_variants_and_nested_options_reject_unreviewed_fields() {
    for config in [
        "",
        "[target]\nkind='unconfigured'",
        "[target]\nkind='openwrt_local'",
    ] {
        toml::from_str::<Config>(config).unwrap().catalog().unwrap();
    }
    let valid = ssh_config(22);
    toml::from_str::<Config>(&valid).unwrap().catalog().unwrap();
    for config in [
        "[target]\nkind='automatic'".to_owned(),
        "[target]\nkind='unconfigured'\nfallback='local'".to_owned(),
        "[target]\nkind='openwrt_local'\nprogram='/bin/sh'".to_owned(),
        "target=[]".to_owned(),
        valid.replace("kind='ssh'", "kind='ssh'\nfallback='openwrt_local'"),
        valid.replace(
            "[target.options]",
            "[target.options]\ntrust_on_first_use=true",
        ),
        valid.replace(
            "kind='environment'",
            "kind='environment'\ninline_key='synthetic-never-accept'",
        ),
    ] {
        assert!(
            toml::from_str::<Config>(&config).is_err(),
            "unreviewed target options must fail schema validation"
        );
    }
}

#[test]
fn invalid_ssh_selection_is_rejected_before_backend_construction() {
    let valid = ssh_config(22);
    for config in [
        valid.replace("identity_source='ssh-key'", "identity_source='missing'"),
        valid.replace("port=22", "port=0"),
        valid.replace("host='127.0.0.1'", "host='bad host; synthetic'"),
        valid.replace(PIN, "synthetic-unpinned-host"),
        format!("{valid}\n[target.key_limits]\nmax_key_bytes=0\n"),
    ] {
        let parsed: Config = toml::from_str(&config).unwrap();
        assert!(parsed.catalog().is_err());
    }
}

#[tokio::test]
async fn check_and_catalog_do_not_read_identity_or_connect_to_the_selected_ssh_target() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let config = ssh_config(listener.local_addr().unwrap().port());
    let checked = run("check", &config).await;
    assert!(
        checked.status.success(),
        "offline check must not resolve missing key material"
    );
    assert!(
        String::from_utf8(checked.stdout)
            .unwrap()
            .starts_with("configuration valid;")
    );
    assert!(checked.stderr.is_empty());
    let catalog = run("catalog", &config).await;
    assert!(catalog.status.success());
    let value: Value = serde_json::from_slice(&catalog.stdout).unwrap();
    assert!(
        value["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation["name"] == "system_info")
    );
    assert!(catalog.stderr.is_empty());
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err(),
        "offline operations must not open the configured SSH connection"
    );
}

#[tokio::test]
async fn explicit_local_target_can_be_checked_offline_without_probing_this_host() {
    let output = run("check", "[target]\nkind='openwrt_local'\n").await;
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[tokio::test]
async fn environment_config_cli_is_exclusive_and_failures_do_not_echo_config_contents() {
    for args in [
        vec![
            "check",
            "--config-env",
            CONFIG_ENV,
            "--config",
            "synthetic-private-path",
        ],
        vec![
            "check",
            "--config",
            "synthetic-private-path",
            "--config-env",
            CONFIG_ENV,
        ],
        vec!["check", "--config-env", "INVALID-NAME"],
        vec!["check", "--config-env", "OPENWRT_MCP_MISSING_CONFIG"],
    ] {
        let output = run_arguments(&args, "# synthetic-private-config\n").await;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(!text.contains("synthetic-private"));
        assert!(!text.contains("INVALID-NAME"));
    }
    let invalid = run("check", "unknown='synthetic-private-config'\n").await;
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    let logs = String::from_utf8(invalid.stderr).unwrap();
    assert!(!logs.contains("synthetic-private-config"));
    assert!(logs.contains("config_invalid"));
}
