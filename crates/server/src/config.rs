use openwrt_mcp_adapters::AuditConfig;
use openwrt_mcp_core::{Catalog, Operation, Policy};
use openwrt_mcp_runtime::Limits;
use serde::Deserialize;
use std::{
    io::Write,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_CONFIG_BYTES: usize = 1_048_576;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub policy: Policy,
    pub limits: Limits,
    pub audit: AuditConfig,
    pub logging: Logging,
    pub actions: Vec<Operation>,
    pub protection: Option<crate::protection::ProtectionConfig>,
    pub target: crate::target::TargetConfig,
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
    /// Load only the explicit operator config through the host's protection profile.
    pub fn load(path: &Path) -> Result<Self, &'static str> {
        use openwrt_mcp_host_platform::HostError;
        let contents =
            openwrt_mcp_host_platform::read_config(path, MAX_CONFIG_BYTES).map_err(|error| {
                match error {
                    HostError::Unsupported => "config_protection_unsupported",
                    HostError::Limit => "config_too_large",
                    HostError::Insecure => "config_insecure",
                    HostError::InvalidPath => "config_path_invalid",
                    HostError::Unavailable => "config_unreadable",
                }
            })?;
        Self::parse(&contents)
    }

    /// No .env discovery or fallback. The original OS environment cannot be erased.
    pub fn load_from_environment(variable: &str) -> Result<Self, &'static str> {
        if variable.is_empty()
            || variable.len() > 128
            || !variable.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphabetic() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
            })
        {
            return Err("config_environment_invalid");
        }
        let value = std::env::var_os(variable).ok_or("config_unreadable")?;
        // var_os returns an owned OS copy before it can be bounded. No second
        // copy or parsing occurs for an oversized or non-Unicode value.
        let contents = value.to_str().ok_or("config_invalid")?;
        Self::parse(contents.as_bytes())
    }

    fn parse(contents: &[u8]) -> Result<Self, &'static str> {
        if contents.len() > MAX_CONFIG_BYTES {
            return Err("config_too_large");
        }
        let contents = std::str::from_utf8(contents).map_err(|_| "config_invalid")?;
        toml::from_str(contents).map_err(|_| "config_invalid")
    }

    pub fn catalog(&self) -> Result<Catalog, &'static str> {
        let catalog =
            openwrt_mcp_features::catalog(self.actions.clone()).map_err(|_| "catalog_invalid")?;
        self.policy
            .validate(&catalog)
            .map_err(|_| "policy_invalid")?;
        self.limits.validate().map_err(|_| "limits_invalid")?;
        self.audit.validate().map_err(|_| "audit_config_invalid")?;
        self.target
            .validate()
            .map_err(|_| "target_config_invalid")?;
        if let Some(protection) = &self.protection {
            protection
                .validate()
                .map_err(|_| "protection_config_invalid")?;
        }
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

    #[test]
    fn config_byte_bounds_and_encoding_fail_before_parsing_with_fixed_errors() {
        assert!(Config::parse(&vec![b' '; MAX_CONFIG_BYTES]).is_ok());
        assert_eq!(
            Config::parse(&vec![b' '; MAX_CONFIG_BYTES + 1]).err(),
            Some("config_too_large")
        );
        assert_eq!(Config::parse(&[0xff]).err(), Some("config_invalid"));
        assert_eq!(
            Config::parse(b"secret = 'synthetic-do-not-echo'").err(),
            Some("config_invalid")
        );
    }
}
