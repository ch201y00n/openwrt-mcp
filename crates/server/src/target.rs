//! Operator-owned target selection; construction and validation never read SSH keys.

use openwrt_mcp_adapters::{LocalBackend, UnconfiguredBackend};
use openwrt_mcp_backend_ssh::{SshBackend, SshOptions};
use openwrt_mcp_key_sources::{SourceConfig, SourceRegistry};
use openwrt_mcp_runtime::{Backend, RuntimeError, protection::KeyLimits};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetConfig {
    Unconfigured {},
    OpenwrtLocal {},
    Ssh {
        options: SshOptions,
        identity_source: String,
        sources: BTreeMap<String, SourceConfig>,
        #[serde(default)]
        key_limits: KeyLimits,
    },
}

impl Default for TargetConfig {
    fn default() -> Self {
        Self::Unconfigured {}
    }
}

impl TargetConfig {
    /// Offline: no host marker, key source or network access.
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if let Self::Ssh {
            options,
            identity_source,
            sources,
            key_limits,
        } = self
        {
            options.validate()?;
            SourceRegistry::new(sources.clone(), *key_limits)
                .and_then(|registry| registry.resolve_key(identity_source))
                .map_err(|_| RuntimeError::InvalidConfig)?;
        }
        Ok(())
    }

    /// Local verification is performed only for the explicitly selected local target.
    pub fn backend(&self) -> Result<Arc<dyn Backend>, RuntimeError> {
        self.validate()?;
        match self {
            Self::Unconfigured {} => Ok(Arc::new(UnconfiguredBackend)),
            Self::OpenwrtLocal {} => Ok(Arc::new(LocalBackend::new()?)),
            Self::Ssh {
                options,
                identity_source,
                sources,
                key_limits,
            } => {
                let source = SourceRegistry::new(sources.clone(), *key_limits)
                    .and_then(|registry| registry.resolve_key(identity_source))
                    .map_err(|_| RuntimeError::InvalidConfig)?;
                Ok(Arc::new(SshBackend::new(options.clone(), source)?))
            }
        }
    }
}
