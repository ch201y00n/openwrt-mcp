use openwrt_mcp_host_platform::{
    PrivateLog, SystemLog, native_file_protection_supported, system_log_supported,
};
use openwrt_mcp_runtime::{AuditEvent, AuditSink, RuntimeError, safe_operation_name};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Mutex,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditFormat {
    #[default]
    Json,
    Text,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditDestination {
    #[default]
    Stderr,
    File,
    Syslog,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuditConfig {
    pub enabled: bool,
    pub format: AuditFormat,
    pub destination: AuditDestination,
    pub path: Option<PathBuf>,
    pub max_bytes: u64,
    pub retained_files: usize,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            format: AuditFormat::Json,
            destination: AuditDestination::Stderr,
            path: None,
            max_bytes: 10 * 1024 * 1024,
            retained_files: 5,
        }
    }
}

impl AuditConfig {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        if !(1024..=1_073_741_824).contains(&self.max_bytes)
            || !(1..=100).contains(&self.retained_files)
        {
            return Err(RuntimeError::InvalidConfig);
        }
        match self.destination {
            AuditDestination::File => {
                let path = self.path.as_ref().ok_or(RuntimeError::InvalidConfig)?;
                if !path.is_absolute() || path.file_name().is_none() {
                    return Err(RuntimeError::InvalidConfig);
                }
                if self.enabled && !native_file_protection_supported() {
                    return Err(RuntimeError::UnsupportedAuditDestination);
                }
            }
            AuditDestination::Stderr | AuditDestination::Syslog => {
                if self.path.is_some() {
                    return Err(RuntimeError::InvalidConfig);
                }
            }
        }
        if self.enabled
            && matches!(self.destination, AuditDestination::Syslog)
            && !system_log_supported()
        {
            return Err(RuntimeError::UnsupportedAuditDestination);
        }
        Ok(())
    }
}

enum DestinationState {
    Disabled,
    Stderr,
    File(PrivateLog),
    Syslog(SystemLog),
}

pub struct AuditWriter {
    config: AuditConfig,
    destination: Mutex<DestinationState>,
}

impl AuditWriter {
    pub fn new(config: AuditConfig) -> Result<Self, RuntimeError> {
        config.validate()?;
        let destination = if !config.enabled {
            DestinationState::Disabled
        } else {
            match config.destination {
                AuditDestination::Stderr => DestinationState::Stderr,
                AuditDestination::File => {
                    let path = config.path.as_ref().ok_or(RuntimeError::InvalidConfig)?;
                    DestinationState::File(
                        PrivateLog::open(path, config.max_bytes, config.retained_files)
                            .map_err(|_| RuntimeError::AuditUnavailable)?,
                    )
                }
                AuditDestination::Syslog => DestinationState::Syslog(
                    SystemLog::open().map_err(|_| RuntimeError::AuditUnavailable)?,
                ),
            }
        };
        Ok(Self {
            config,
            destination: Mutex::new(destination),
        })
    }

    fn render(&self, event: &AuditEvent) -> Result<Vec<u8>, RuntimeError> {
        // Defend direct AuditSink callers as well as dispatcher-created events.
        let mut safe = event.clone();
        safe.operation = safe_operation_name(&event.operation).to_owned();
        let mut line = match self.config.format {
            AuditFormat::Json => serde_json::to_vec(&safe).map_err(|_| RuntimeError::AuditUnavailable)?,
            AuditFormat::Text => format!(
                "timestamp_ms={} request_sequence={} phase={:?} operation={} outcome={:?} duration_ms={} ",
                safe.timestamp_ms, safe.request_sequence, safe.phase, safe.operation, safe.outcome,
                safe.duration_ms.map_or_else(|| "-".to_owned(), |value| value.to_string()),
            ).into_bytes(),
        };
        line.push(b'\n');
        Ok(line)
    }
}

impl AuditSink for AuditWriter {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError> {
        if !self.config.enabled {
            return Ok(());
        }
        let line = self.render(event)?;
        let mut destination = self
            .destination
            .lock()
            .map_err(|_| RuntimeError::AuditUnavailable)?;
        match &mut *destination {
            DestinationState::Disabled => Ok(()),
            DestinationState::Stderr => io::stderr()
                .lock()
                .write_all(&line)
                .map_err(|_| RuntimeError::AuditUnavailable),
            DestinationState::File(log) => {
                log.write(&line).map_err(|_| RuntimeError::AuditUnavailable)
            }
            DestinationState::Syslog(log) => {
                let mut message = b"<14>openwrt-mcp: ".to_vec();
                message.extend_from_slice(&line);
                log.send(&message)
                    .map_err(|_| RuntimeError::AuditUnavailable)
            }
        }
    }
}
