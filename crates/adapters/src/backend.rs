//! Local process implementation of the application backend port.
#[cfg(all(test, target_os = "linux"))]
mod fixtures;

use std::{process::Stdio, time::Duration};

use async_trait::async_trait;
use openwrt_mcp_core::{CapabilityObservation, PreparedAction, ProbeRequest, UnknownReason};
use openwrt_mcp_device_codec::{
    CodecError, CommandSpec, MAX_PROBE_BYTES, compile_action, compile_probe, parse_action_response,
    parse_ubus_describe,
};
use openwrt_mcp_runtime::{Backend, Limits, RuntimeError};
use serde_json::Value;
use tokio::{io::AsyncReadExt, process::Command};

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

    async fn probe(
        &self,
        _: ProbeRequest,
        limits: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        limits.validate()?;
        Err(RuntimeError::TargetNotConfigured)
    }
}

#[async_trait]
impl Backend for LocalBackend {
    async fn capture_apk_installed(
        &self,
        limits: &Limits,
    ) -> Result<openwrt_mcp_core::packages::PackageObservation, RuntimeError> {
        use openwrt_mcp_device_codec::packages::{
            MAX_APK_VERSION_BYTES, apk_installed_command, apk_version_command, parse_apk_installed,
            validate_apk_version,
        };
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        let maximum = limits
            .max_output_bytes
            .min(openwrt_mcp_core::packages::MAX_PACKAGE_SOURCE_BYTES);
        let version = run(
            &apk_version_command(),
            maximum.min(MAX_APK_VERSION_BYTES),
            deadline,
        )
        .await?;
        validate_apk_version(&version).map_err(|_| RuntimeError::CapabilityUnsupported)?;
        if tokio::time::Instant::now() >= deadline {
            return Err(RuntimeError::Timeout);
        }
        let bytes = run(&apk_installed_command(), maximum, deadline).await?;
        let result = parse_apk_installed(&bytes, maximum).map_err(|error| match error {
            CodecError::OutputLimit => RuntimeError::OutputLimit,
            _ => RuntimeError::InvalidOutput,
        });
        if tokio::time::Instant::now() >= deadline {
            return Err(RuntimeError::Timeout);
        }
        result
    }
    fn capability_epoch(&self) -> Option<u64> {
        Some(1)
    }

    async fn execute(
        &self,
        action: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        let invocation = compile_action(action).map_err(|_| RuntimeError::BackendFailed)?;
        let stdout = run(&invocation, limits.max_output_bytes, deadline).await?;
        parse_action_response(&stdout, limits.max_output_bytes).map_err(|error| match error {
            CodecError::OutputLimit => RuntimeError::OutputLimit,
            _ => RuntimeError::InvalidOutput,
        })
    }

    async fn probe(
        &self,
        request: ProbeRequest,
        limits: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        let maximum = limits.max_output_bytes.min(MAX_PROBE_BYTES);
        let stdout = run(&compile_probe(request), maximum, deadline).await?;
        let ProbeRequest::DescribeUbusObject(object) = request;
        Ok(
            parse_ubus_describe(object, &stdout, maximum).unwrap_or(
                CapabilityObservation::Unknown(UnknownReason::InvalidObservation),
            ),
        )
    }
}

/// Only complete bounded successful process output leaves this private runner.
async fn run(
    invocation: &CommandSpec,
    maximum: usize,
    deadline: tokio::time::Instant,
) -> Result<Vec<u8>, RuntimeError> {
    // No shell, inherited stdin, or environment-controlled loader/program lookup.
    let mut child = Command::new(invocation.program())
        .args(invocation.arguments())
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
    let completion = tokio::time::timeout_at(deadline, async {
        tokio::try_join!(
            read_bounded(stdout, maximum, true),
            read_bounded(stderr, maximum, false),
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
    Ok(stdout)
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
        let invocation = compile_action(&PreparedAction::Ubus {
            object: "fixture.object".into(),
            method: "query".into(),
            arguments: json!({"name":"quote \" ; literal", "count":4, "enabled":true}),
        })
        .unwrap();
        assert_eq!(invocation.program(), "/bin/ubus");
        assert_eq!(
            &invocation.arguments()[..4],
            &["-S", "call", "fixture.object", "query"]
        );
        assert_eq!(
            serde_json::from_str::<Value>(&invocation.arguments()[4]).unwrap(),
            json!({"name":"quote \" ; literal", "count":4, "enabled":true})
        );
    }

    #[tokio::test]
    async fn unconfigured_probe_has_no_epoch_and_never_falls_back_to_host_execution() {
        let backend = UnconfiguredBackend;
        assert_eq!(backend.capability_epoch(), None);
        let error = backend
            .probe(
                ProbeRequest::DescribeUbusObject(openwrt_mcp_core::ReviewedObject::System),
                &Limits::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), "target_not_configured");
    }
}
