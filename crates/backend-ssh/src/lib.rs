//! Portable remote OpenWrt execution with one persistent, pinned SSH session.
//! No host commands, key files, environment lookup or automatic reconnect.
//! Timeout/cancellation closes the TCP connection, not a guaranteed remote
//! process-tree kill or rollback. Restart the backend after such failures.

mod options;
mod session;

pub use options::SshOptions;

use async_trait::async_trait;
use openwrt_mcp_core::{CapabilityObservation, PreparedAction, ProbeRequest, UnknownReason};
use openwrt_mcp_device_codec::{
    CodecError, CommandSpec, MAX_PROBE_BYTES, compile_action, compile_probe, encode_remote,
    parse_action_response, parse_ubus_describe,
};
use openwrt_mcp_runtime::{Backend, Limits, RuntimeError, protection::KeySource};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct SshBackend {
    options: SshOptions,
    state: Mutex<session::State>,
    control: session::Control,
}

impl SshBackend {
    /// Validate trusted options without reading a key or opening a connection.
    pub fn new(options: SshOptions, identity: Arc<dyn KeySource>) -> Result<Self, RuntimeError> {
        options.validate()?;
        Ok(Self {
            options,
            state: Mutex::new(session::State::new(identity)),
            control: session::Control::default(),
        })
    }

    async fn run(
        &self,
        command: &CommandSpec,
        maximum: usize,
        deadline: tokio::time::Instant,
    ) -> Result<Vec<u8>, RuntimeError> {
        self.control.check()?;
        let mut state = self.state.try_lock().map_err(|_| RuntimeError::Busy)?;
        self.control.check()?;
        let command = encode_remote(command).map_err(|_| RuntimeError::BackendFailed)?;
        let mut guard = session::FailureGuard::new(&self.control);
        let outcome = tokio::time::timeout_at(deadline, async {
            state.connect(&self.options, &self.control).await?;
            state.execute(command, maximum).await
        })
        .await
        .map_err(|_| RuntimeError::Timeout)?;
        match outcome {
            Ok(completed) => {
                // A completed channel remains reusable after nonzero exit or
                // malformed response. Decoding is deliberately outside the guard.
                guard.complete();
                completed
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for SshBackend {
    fn drop(&mut self) {
        self.control.invalidate();
    }
}

#[async_trait]
impl Backend for SshBackend {
    async fn capture_opkg_status(
        &self,
        limits: &Limits,
    ) -> Result<openwrt_mcp_core::packages::PackageObservation, RuntimeError> {
        use openwrt_mcp_device_codec::packages::{
            MAX_OPKG_VERSION_BYTES, opkg_status_command, opkg_version_command, parse_opkg_status,
            validate_opkg_version,
        };
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        let maximum = limits
            .max_output_bytes
            .min(openwrt_mcp_core::packages::MAX_PACKAGE_SOURCE_BYTES);
        let version = self
            .run(
                &opkg_version_command(),
                maximum.min(MAX_OPKG_VERSION_BYTES),
                deadline,
            )
            .await?;
        validate_opkg_version(&version).map_err(|_| RuntimeError::CapabilityUnsupported)?;
        if tokio::time::Instant::now() >= deadline {
            return Err(RuntimeError::Timeout);
        }
        let bytes = self.run(&opkg_status_command(), maximum, deadline).await?;
        let result = parse_opkg_status(&bytes, maximum).map_err(|error| match error {
            CodecError::OutputLimit => RuntimeError::OutputLimit,
            _ => RuntimeError::InvalidOutput,
        });
        if tokio::time::Instant::now() >= deadline {
            return Err(RuntimeError::Timeout);
        }
        result
    }
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
        let version = self
            .run(
                &apk_version_command(),
                maximum.min(MAX_APK_VERSION_BYTES),
                deadline,
            )
            .await?;
        validate_apk_version(&version).map_err(|_| RuntimeError::CapabilityUnsupported)?;
        if tokio::time::Instant::now() >= deadline {
            return Err(RuntimeError::Timeout);
        }
        let bytes = self
            .run(&apk_installed_command(), maximum, deadline)
            .await?;
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
        self.control.epoch()
    }

    async fn execute(
        &self,
        action: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        let command = compile_action(action).map_err(|_| RuntimeError::BackendFailed)?;
        let stdout = self
            .run(&command, limits.max_output_bytes, deadline)
            .await?;
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
        let stdout = self.run(&compile_probe(request), maximum, deadline).await?;
        let ProbeRequest::DescribeUbusObject(object) = request;
        Ok(
            parse_ubus_describe(object, &stdout, maximum).unwrap_or(
                CapabilityObservation::Unknown(UnknownReason::InvalidObservation),
            ),
        )
    }
}
