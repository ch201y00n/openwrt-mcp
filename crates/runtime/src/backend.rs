use std::{path::Path, process::Stdio, time::Duration};

use async_trait::async_trait;
use openwrt_mcp_core::Invocation;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{io::AsyncReadExt, process::Command};

use crate::RuntimeError;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Limits {
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub max_concurrent: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout_ms: 10_000,
            max_output_bytes: 65_536,
            max_concurrent: 2,
        }
    }
}

impl Limits {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !(1..=300_000).contains(&self.timeout_ms)
            || !(1..=16_777_216).contains(&self.max_output_bytes)
            || !(1..=64).contains(&self.max_concurrent)
        {
            return Err(RuntimeError::InvalidConfig);
        }
        Ok(())
    }
}

#[async_trait]
pub trait Backend: Send + Sync {
    async fn execute(
        &self,
        invocation: &Invocation,
        limits: &Limits,
    ) -> Result<Value, RuntimeError>;
}

#[derive(Debug, Default)]
pub struct LocalBackend;

#[async_trait]
impl Backend for LocalBackend {
    async fn execute(
        &self,
        invocation: &Invocation,
        limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        limits.validate()?;
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
