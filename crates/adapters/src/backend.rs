//! Local process implementation of the application backend port.
#[cfg(all(test, target_os = "linux"))]
mod fixtures;

use std::{path::Path, process::Stdio, time::Duration};

use async_trait::async_trait;
use openwrt_mcp_core::PreparedAction;
use openwrt_mcp_runtime::{Backend, Limits, RuntimeError};
use serde_json::Value;
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Debug, PartialEq, Eq)]
struct Invocation {
    program: String,
    args: Vec<String>,
}

fn compile(action: &PreparedAction) -> Invocation {
    match action {
        PreparedAction::Ubus {
            object,
            method,
            arguments,
        } => Invocation {
            program: "/bin/ubus".to_owned(),
            args: vec![
                "-S".to_owned(),
                "call".to_owned(),
                object.clone(),
                method.clone(),
                arguments.to_string(),
            ],
        },
        PreparedAction::Process { program, args } => Invocation {
            program: program.clone(),
            args: args.clone(),
        },
    }
}

/// This constructor only accepts a verified local OpenWrt host. The private
/// state deliberately prevents a public unit/default constructor bypass.
///
/// ```compile_fail
/// let backend = openwrt_mcp_adapters::LocalBackend;
/// ```
#[derive(Debug)]
pub struct LocalBackend {
    _verified: (),
}

impl LocalBackend {
    pub fn new() -> Result<Self, RuntimeError> {
        openwrt_mcp_host_platform::verify_openwrt_local()
            .map_err(|_| RuntimeError::UnsupportedTarget)?;
        Ok(Self { _verified: () })
    }
}

/// Default target: no host process, router connection, or key access.
pub struct UnconfiguredBackend;

#[async_trait]
impl Backend for UnconfiguredBackend {
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TargetNotConfigured)
    }
}

#[async_trait]
impl Backend for LocalBackend {
    async fn execute(
        &self,
        action: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        limits.validate()?;
        let invocation = compile(action);
        if !Path::new(&invocation.program).is_absolute() {
            return Err(RuntimeError::BackendFailed);
        }
        // No shell, inherited stdin, or environment-controlled loader/program lookup.
        let mut child = Command::new(&invocation.program)
            .args(&invocation.args)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| RuntimeError::BackendFailed)?;
        let stdout = child.stdout.take().ok_or(RuntimeError::BackendFailed)?;
        let stderr = child.stderr.take().ok_or(RuntimeError::BackendFailed)?;

        // Read both pipes while waiting: neither full stderr nor a inherited pipe
        // held by a descendant can make a normal invocation wait past its deadline.
        let completion = tokio::time::timeout(Duration::from_millis(limits.timeout_ms), async {
            tokio::try_join!(
                read_bounded(stdout, limits.max_output_bytes, true),
                read_bounded(stderr, limits.max_output_bytes, false),
                async { child.wait().await.map_err(|_| RuntimeError::BackendFailed) },
            )
        })
        .await;

        let (stdout, _, status) = match completion {
            Ok(Ok(result)) => result,
            failed => {
                // Explicitly kill AND reap our child on errors. kill_on_drop is
                // the cancellation backstop; no process-tree sandbox is claimed.
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(match failed {
                    Err(_) => RuntimeError::Timeout,
                    Ok(Err(error)) => error,
                    Ok(Ok(_)) => unreachable!(),
                });
            }
        };

        if !status.success() {
            return Err(RuntimeError::BackendFailed);
        }
        if stdout.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&stdout).map_err(|_| RuntimeError::InvalidOutput)
    }
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    mut stream: R,
    max_bytes: usize,
    retain: bool,
) -> Result<Vec<u8>, RuntimeError> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut total = 0_usize;
    loop {
        let count = stream
            .read(&mut buffer)
            .await
            .map_err(|_| RuntimeError::BackendFailed)?;
        if count == 0 {
            return Ok(output);
        }
        total = total.checked_add(count).ok_or(RuntimeError::OutputLimit)?;
        if total > max_bytes {
            return Err(RuntimeError::OutputLimit);
        }
        if retain {
            output.extend_from_slice(&buffer[..count]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ubus_compilation_is_fixed_argv_and_preserves_json_types() {
        let invocation = compile(&PreparedAction::Ubus {
            object: "fixture.object".into(),
            method: "query".into(),
            arguments: json!({"name":"quote \" ; literal", "count":4, "enabled":true}),
        });
        assert_eq!(invocation.program, "/bin/ubus");
        assert_eq!(
            &invocation.args[..4],
            &["-S", "call", "fixture.object", "query"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&invocation.args[4]).unwrap(),
            json!({"name":"quote \" ; literal", "count":4, "enabled":true})
        );
    }
}
