//! Portable remote OpenWrt execution with one persistent, pinned SSH session.
//! No host commands, key files, environment lookup or automatic reconnect.
//! Timeout/cancellation closes the TCP connection, not a guaranteed remote
//! process-tree kill or rollback. Restart the backend after such failures.

mod command;
mod options;
mod session;

pub use options::SshOptions;

use async_trait::async_trait;
use openwrt_mcp_core::PreparedAction;
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
}

impl Drop for SshBackend {
    fn drop(&mut self) {
        self.control.invalidate();
    }
}

#[async_trait]
impl Backend for SshBackend {
    async fn execute(
        &self,
        action: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError> {
        limits.validate()?;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(limits.timeout_ms);
        self.control.check()?;
        let mut state = self.state.try_lock().map_err(|_| RuntimeError::Busy)?;
        self.control.check()?;
        let command = command::encode(action)?;
        let mut guard = session::FailureGuard::new(&self.control);
        let outcome = tokio::time::timeout_at(deadline, async {
            state.connect(&self.options, &self.control).await?;
            state.execute(command, limits.max_output_bytes).await
        })
        .await
        .map_err(|_| RuntimeError::Timeout)?;
        match outcome {
            Ok(completed) => {
                // A known completed channel remains reusable even if its exit
                // status or returned JSON was unsuccessful.
                guard.complete();
                completed
            }
            Err(error) => Err(error),
        }
    }
}
