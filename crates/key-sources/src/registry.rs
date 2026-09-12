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

    /// Resolve a key-purpose source offline, retaining larger reads inside containers.
    pub fn resolve_key(&self, name: &str) -> Result<Arc<dyn KeySource>, ProtectionError> {
        Ok(Arc::new(PurposeSource {
            source: self.resolve(name)?,
            cap: self.limits.max_key_bytes,
        }))
    }
}

struct PurposeSource {
    source: Arc<dyn KeySource>,
    cap: usize,
}

impl KeySource for PurposeSource {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        let limit = max_bytes.min(self.cap);
        if limit == 0 {
            return Err(ProtectionError::ResourceLimit);
        }
        let material = self.source.read(limit)?;
        if material.expose_bytes().len() > limit {
            return Err(ProtectionError::ResourceLimit);
        }
        Ok(material)
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

#[cfg(test)]
mod tests {
    use super::PurposeSource;
    use openwrt_mcp_runtime::protection::{KeyMaterial, KeySource, ProtectionError};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct IgnoringLimitSource(AtomicUsize);

    impl KeySource for IgnoringLimitSource {
        fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
            self.0.store(max_bytes, Ordering::SeqCst);
            KeyMaterial::new(b"four".to_vec(), 4)
        }
    }

    #[test]
    fn purpose_source_checks_actual_material_and_rejects_zero_before_reading() {
        let inner = Arc::new(IgnoringLimitSource(AtomicUsize::new(0)));
        let source = PurposeSource {
            source: inner.clone(),
            cap: 3,
        };
        assert_eq!(
            source.read(1024).unwrap_err(),
            ProtectionError::ResourceLimit
        );
        assert_eq!(inner.0.load(Ordering::SeqCst), 3);
        assert_eq!(source.read(2).unwrap_err(), ProtectionError::ResourceLimit);
        assert_eq!(inner.0.load(Ordering::SeqCst), 2);
        assert_eq!(source.read(0).unwrap_err(), ProtectionError::ResourceLimit);
        assert_eq!(inner.0.load(Ordering::SeqCst), 2);
    }
}
