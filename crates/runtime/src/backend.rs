use async_trait::async_trait;
use openwrt_mcp_core::{CapabilityObservation, PreparedAction, ProbeRequest, UnknownReason};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// An opaque authentication/connection generation, never a release number.
    /// None prevents cached evidence reuse. Reconnection must change this value.
    fn capability_epoch(&self) -> Option<u64> {
        None
    }

    /// Probe and execution belong to this same immutable target authority.
    /// Implementing execute alone never grants a positive capability assertion.
    async fn probe(
        &self,
        _request: ProbeRequest,
        _limits: &Limits,
    ) -> Result<CapabilityObservation, RuntimeError> {
        Ok(CapabilityObservation::Unknown(
            UnknownReason::ProbeUnavailable,
        ))
    }

    async fn execute(
        &self,
        invocation: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError>;
}
