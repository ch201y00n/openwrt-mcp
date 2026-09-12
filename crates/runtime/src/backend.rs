use async_trait::async_trait;
use openwrt_mcp_core::PreparedAction;
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
    async fn execute(
        &self,
        invocation: &PreparedAction,
        limits: &Limits,
    ) -> Result<Value, RuntimeError>;
}
