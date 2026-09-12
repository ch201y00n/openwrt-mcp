//! Linux-only private runner fixtures; they do not bypass the public target guard.

use std::time::{Duration, Instant};

use super::{LocalBackend, run};
use openwrt_mcp_core::{CapabilityObservation, PreparedAction, ProbeRequest, ReviewedObject};
use openwrt_mcp_device_codec::{compile_action, parse_ubus_describe};
use openwrt_mcp_runtime::{Backend, Limits};
use serde_json::json;

fn invocation(program: &str, args: &[&str]) -> PreparedAction {
    PreparedAction::Process {
        program: program.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    }
}

#[tokio::test]
async fn arguments_are_literal_and_output_is_json_only() {
    let backend = LocalBackend { _verified: () };
    let value = backend
        .execute(
            &invocation(
                "/usr/bin/printf",
                &["%s", "{\"literal\":\"$(echo unsafe); | &\"}"],
            ),
            &Limits::default(),
        )
        .await
        .unwrap();
    assert_eq!(value, json!({"literal": "$(echo unsafe); | &"}));
    let error = backend
        .execute(
            &invocation("/usr/bin/printf", &["fixture-secret"]),
            &Limits::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_output");
    assert!(!error.to_string().contains("fixture-secret"));
}

#[tokio::test]
async fn stderr_is_drained_bounded_and_never_exposed() {
    let backend = LocalBackend { _verified: () };
    let command = invocation("/usr/bin/ls", &["/openwrt-mcp-nonexistent-fixture-secret"]);
    let error = backend
        .execute(&command, &Limits::default())
        .await
        .unwrap_err();
    assert_eq!(error.code(), "backend_failed");
    assert!(!error.to_string().contains("fixture-secret"));
    let error = backend
        .execute(
            &command,
            &Limits {
                max_output_bytes: 1,
                ..Limits::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "output_limit");
}

#[tokio::test]
async fn endless_output_is_stopped_at_the_bound() {
    let started = Instant::now();
    let error = LocalBackend { _verified: () }
        .execute(
            &invocation("/usr/bin/yes", &["fixture-secret"]),
            &Limits {
                max_output_bytes: 1024,
                ..Limits::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "output_limit");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn timeout_kills_and_reaps_our_child() {
    let started = Instant::now();
    let running = tokio::spawn(async {
        LocalBackend { _verified: () }
            .execute(
                &invocation("/bin/sleep", &["29.875"]),
                &Limits {
                    timeout_ms: 1000,
                    ..Limits::default()
                },
            )
            .await
    });
    #[cfg(target_os = "linux")]
    let child_pid = tokio::time::timeout(Duration::from_millis(750), async {
        loop {
            if let Some(pid) = find_sleep_fixture() {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("synthetic sleep child should appear before its deadline");
    let error = running.await.unwrap().unwrap_err();
    assert_eq!(error.code(), "timeout");
    assert!(started.elapsed() < Duration::from_secs(3));
    #[cfg(target_os = "linux")]
    assert!(
        !std::path::Path::new(&format!("/proc/{child_pid}")).exists(),
        "child must be reaped, not left a zombie"
    );
    // This checks our direct child, not whole-process-tree termination.
}

#[cfg(target_os = "linux")]
fn find_sleep_fixture() -> Option<u32> {
    // Test threads own different Linux task IDs; inspect only this test process's
    // direct children and identify the synthetic, unique sleep argument.
    for task in std::fs::read_dir("/proc/self/task").ok()?.flatten() {
        let children = std::fs::read_to_string(task.path().join("children")).unwrap_or_default();
        for pid in children
            .split_whitespace()
            .filter_map(|value| value.parse::<u32>().ok())
        {
            if let Ok(cmdline) = std::fs::read(format!("/proc/{pid}/cmdline"))
                && cmdline.starts_with(b"/bin/sleep\0")
                && cmdline.ends_with(b"29.875\0")
            {
                return Some(pid);
            }
        }
    }
    None
}

#[tokio::test]
async fn stdin_is_closed_and_empty_output_is_null() {
    let value = LocalBackend { _verified: () }
        .execute(&invocation("/bin/cat", &[]), &Limits::default())
        .await
        .unwrap();
    assert!(value.is_null());
}

#[test]
fn limits_are_nonzero_and_have_hard_upper_bounds() {
    assert!(Limits::default().validate().is_ok());
    assert!(
        Limits {
            max_concurrent: 0,
            ..Limits::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Limits {
            timeout_ms: u64::MAX,
            ..Limits::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Limits {
            max_output_bytes: usize::MAX,
            ..Limits::default()
        }
        .validate()
        .is_err()
    );
}

#[tokio::test]
async fn completed_local_bytes_use_the_same_bounded_probe_decoder() {
    let bytes = "'network.interface.lan' @00000001\n\t\"status\":{}\n\t\"add_device\":{\"link-ext\":\"Boolean\"}\n";
    let command = compile_action(&invocation("/usr/bin/printf", &["%s", bytes])).unwrap();
    let stdout = run(
        &command,
        1024,
        tokio::time::Instant::now() + Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert_eq!(stdout, bytes.as_bytes());
    let CapabilityObservation::Ubus(observation) =
        parse_ubus_describe(ReviewedObject::NetworkInterfaceLan, &stdout, 1024).unwrap()
    else {
        panic!("expected metadata")
    };
    assert!(observation.methods.contains_key("status"));
    assert!(
        observation.methods["add_device"]
            .arguments
            .contains_key("link-ext")
    );
    assert_eq!(LocalBackend { _verified: () }.capability_epoch(), Some(1));
    assert_eq!(
        run(
            &command,
            8,
            tokio::time::Instant::now() + Duration::from_secs(2)
        )
        .await
        .unwrap_err()
        .code(),
        "output_limit"
    );
    let failed = compile_action(&invocation("/usr/bin/false", &[])).unwrap();
    assert_eq!(
        run(
            &failed,
            1024,
            tokio::time::Instant::now() + Duration::from_secs(2)
        )
        .await
        .unwrap_err()
        .code(),
        "backend_failed"
    );
}

#[tokio::test]
async fn invalid_local_probe_limits_are_rejected_before_any_program_is_started() {
    let error = LocalBackend { _verified: () }
        .probe(
            ProbeRequest::DescribeUbusObject(ReviewedObject::System),
            &Limits {
                timeout_ms: 0,
                ..Limits::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_config");
}
