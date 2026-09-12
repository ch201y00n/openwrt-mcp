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
    CommandSpec, MAX_PROBE_BYTES, compile_action, compile_probe, encode_remote, parse_ubus_describe,
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
        if stdout.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&stdout).map_err(|_| RuntimeError::InvalidOutput)
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
