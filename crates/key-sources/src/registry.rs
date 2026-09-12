use crate::config::{entry_name, environment_name, file_path, identifier};
use crate::{
    ContainerFormat, FileProtection, NativeFileAccess, ProtectedFileAccess, SourceConfig,
    ZipContainer,
};
use openwrt_mcp_runtime::protection::{
    KeyContainer, KeyLimits, KeyMaterial, KeySource, ProtectionError,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use zeroize::Zeroizing;

pub struct SourceRegistry {
    configs: BTreeMap<String, SourceConfig>,
    limits: KeyLimits,
    files: Arc<dyn ProtectedFileAccess>,
}

impl SourceRegistry {
    pub fn new(
        configs: BTreeMap<String, SourceConfig>,
        limits: KeyLimits,
    ) -> Result<Self, ProtectionError> {
        Self::with_file_access(configs, limits, Arc::new(NativeFileAccess))
    }

    pub fn with_file_access(
        configs: BTreeMap<String, SourceConfig>,
        limits: KeyLimits,
        files: Arc<dyn ProtectedFileAccess>,
    ) -> Result<Self, ProtectionError> {
        limits.validate()?;
        if configs.len() > 128 {
            return Err(ProtectionError::ResourceLimit);
        }
        for (alias, config) in &configs {
            if !identifier(alias) {
                return Err(ProtectionError::InvalidConfig);
            }
            match config {
                SourceConfig::RestrictedFile { path }
                | SourceConfig::PersonalVaultFile { path } => file_path(path)?,
                SourceConfig::Environment { variable } if !environment_name(variable) => {
                    return Err(ProtectionError::InvalidConfig);
                }
                SourceConfig::ArchiveEntry { source, entry, .. } => {
                    if !identifier(source) || !entry_name(entry, false) {
                        return Err(ProtectionError::InvalidConfig);
                    }
                    match configs.get(source) {
                        Some(SourceConfig::ArchiveEntry { .. }) | None => {
                            return Err(ProtectionError::InvalidConfig);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            configs,
            limits,
            files,
        })
    }

    /// Resolve performs no key access. Aliases are operator configuration, never client paths.
    pub fn resolve(&self, name: &str) -> Result<Arc<dyn KeySource>, ProtectionError> {
        let config = self
            .configs
            .get(name)
            .ok_or(ProtectionError::InvalidConfig)?;
        match config {
            SourceConfig::RestrictedFile { path } | SourceConfig::PersonalVaultFile { path } => {
                Ok(Arc::new(FileSource {
                    path: path.clone(),
                    protection: if matches!(config, SourceConfig::PersonalVaultFile { .. }) {
                        FileProtection::PersonalVault
                    } else {
                        FileProtection::Restricted
                    },
                    files: Arc::clone(&self.files),
                    cap: self.limits.max_container_bytes,
                }))
            }
            SourceConfig::Environment { variable } => Ok(Arc::new(EnvironmentSource {
                variable: variable.clone(),
                cap: self.limits.max_container_bytes,
            })),
            SourceConfig::ArchiveEntry {
                source,
                format,
                entry,
            } => Ok(Arc::new(ArchiveSource {
                source: self.resolve(source)?,
                container: match format {
                    ContainerFormat::Zip => Arc::new(ZipContainer),
                },
                entry: entry.clone(),
                limits: self.limits,
            })),
        }
    }
}

struct FileSource {
    path: std::path::PathBuf,
    protection: FileProtection,
    files: Arc<dyn ProtectedFileAccess>,
    cap: usize,
}

impl KeySource for FileSource {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        let limit = max_bytes.min(self.cap);
        if limit == 0 {
            return Err(ProtectionError::ResourceLimit);
        }
        let material = self.files.read(&self.path, self.protection, limit)?;
        if material.expose_bytes().len() > limit {
            return Err(ProtectionError::ResourceLimit);
        }
        Ok(material)
    }
}

struct EnvironmentSource {
    variable: String,
    cap: usize,
}

impl KeySource for EnvironmentSource {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        let limit = max_bytes.min(self.cap);
        if limit == 0 {
            return Err(ProtectionError::ResourceLimit);
        }
        let value = std::env::var_os(&self.variable).ok_or(ProtectionError::SourceUnavailable)?;
        KeyMaterial::from_zeroizing(Zeroizing::new(value.into_encoded_bytes()), limit)
    }
}

struct ArchiveSource {
    source: Arc<dyn KeySource>,
    container: Arc<dyn KeyContainer>,
    entry: String,
    limits: KeyLimits,
}

impl KeySource for ArchiveSource {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        let mut limits = self.limits;
        limits.max_key_bytes = limits.max_key_bytes.min(max_bytes);
        limits.validate()?;
        let container = self.source.read(limits.max_container_bytes)?;
        self.container.read_entry(&container, &self.entry, &limits)
    }
}
