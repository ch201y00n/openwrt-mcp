use openwrt_mcp_core::{Catalog, Operation, Policy};
use openwrt_mcp_runtime::{AuditConfig, Limits};
use serde::Deserialize;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_CONFIG_BYTES: u64 = 1_048_576;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub policy: Policy,
    pub limits: Limits,
    pub audit: AuditConfig,
    pub logging: Logging,
    pub actions: Vec<Operation>,
}

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    #[default]
    Json,
    Text,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Logging {
    pub level: LogLevel,
    pub format: LogFormat,
}

impl Logging {
    /// Only static application messages are accepted. SDK/raw payload logging is not enabled.
    pub async fn event(&self, level: LogLevel, code: &'static str) -> Result<(), &'static str> {
        if level == LogLevel::Off || level > self.level {
            return Ok(());
        }
        let level = match level {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let line = match self.format {
            LogFormat::Json => format!(
                "{}\n",
                serde_json::json!({"timestamp_ms":timestamp_ms,"level":level,"event":code})
            ),
            LogFormat::Text => format!("timestamp_ms={timestamp_ms} {level} {code}\n"),
        };
        tokio::time::timeout(
            Duration::from_secs(1),
            tokio::task::spawn_blocking(move || {
                std::io::stderr().lock().write_all(line.as_bytes())
            }),
        )
        .await
        .map_err(|_| "logging_unavailable")?
        .map_err(|_| "logging_unavailable")?
        .map_err(|_| "logging_unavailable")
    }
}

impl Config {
    /// Load only the explicitly selected local operator config; never print parse contents.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| "config_unreadable")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("config_not_regular_file");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o022 != 0 {
                return Err("config_writable_by_others");
            }
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|_| "config_unreadable")?;
        let opened = file.metadata().map_err(|_| "config_unreadable")?;
        if !opened.is_file() {
            return Err("config_not_regular_file");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if opened.permissions().mode() & 0o022 != 0 {
                return Err("config_writable_by_others");
            }
            if metadata.ino() != opened.ino() || metadata.dev() != opened.dev() {
                return Err("config_changed_during_open");
            }
        }
        let mut contents = String::new();
        file.take(MAX_CONFIG_BYTES + 1)
            .read_to_string(&mut contents)
            .map_err(|_| "config_unreadable")?;
        if contents.len() as u64 > MAX_CONFIG_BYTES {
            return Err("config_too_large");
        }
        toml::from_str(&contents).map_err(|_| "config_invalid")
    }

    pub fn catalog(&self) -> Result<Catalog, &'static str> {
        let catalog = Catalog::new(self.actions.clone()).map_err(|_| "catalog_invalid")?;
        self.policy
            .validate(&catalog)
            .map_err(|_| "policy_invalid")?;
        self.limits.validate().map_err(|_| "limits_invalid")?;
        self.audit.validate().map_err(|_| "audit_config_invalid")?;
        Ok(catalog)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_deny_and_unknown_config_keys_fail() {
        let config: Config = toml::from_str("").unwrap();
        let catalog = config.catalog().unwrap();
        assert!(
            catalog
                .operations()
                .iter()
                .all(|op| config.policy.authorize(op).is_err())
        );
        assert!(toml::from_str::<Config>("unknown = true").is_err());
        assert!(toml::from_str::<Config>("[policy.categories.typo]\naccess = 'read'").is_err());
        assert!(toml::from_str::<Config>("[logging]\nlevel = 'verbose'").is_err());
    }
}
