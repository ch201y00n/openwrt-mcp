use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use openwrt_mcp_runtime::{AuditEvent, AuditSink, RuntimeError, safe_operation_name};

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
                #[cfg(not(unix))]
                if self.enabled {
                    return Err(RuntimeError::UnsupportedAuditDestination);
                }
            }
            AuditDestination::Stderr | AuditDestination::Syslog => {
                if self.path.is_some() {
                    return Err(RuntimeError::InvalidConfig);
                }
            }
        }
        #[cfg(not(unix))]
        if self.enabled && matches!(self.destination, AuditDestination::Syslog) {
            return Err(RuntimeError::UnsupportedAuditDestination);
        }
        Ok(())
    }
}

enum DestinationState {
    Disabled,
    Stderr,
    File(FileState),
    #[cfg(unix)]
    Syslog(std::os::unix::net::UnixDatagram),
}

struct FileState {
    file: Option<File>,
    size: u64,
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
                    let file = open_restricted(path).map_err(|_| RuntimeError::AuditUnavailable)?;
                    let size = file
                        .metadata()
                        .map_err(|_| RuntimeError::AuditUnavailable)?
                        .len();
                    DestinationState::File(FileState {
                        file: Some(file),
                        size,
                    })
                }
                AuditDestination::Syslog => {
                    #[cfg(unix)]
                    {
                        let socket = std::os::unix::net::UnixDatagram::unbound()
                            .map_err(|_| RuntimeError::AuditUnavailable)?;
                        socket
                            .connect("/dev/log")
                            .map_err(|_| RuntimeError::AuditUnavailable)?;
                        // A stalled syslog daemon must fail closed, not block forever.
                        socket
                            .set_nonblocking(true)
                            .map_err(|_| RuntimeError::AuditUnavailable)?;
                        DestinationState::Syslog(socket)
                    }
                    #[cfg(not(unix))]
                    return Err(RuntimeError::UnsupportedAuditDestination);
                }
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
            DestinationState::File(state) => {
                let path = self
                    .config
                    .path
                    .as_ref()
                    .ok_or(RuntimeError::AuditUnavailable)?;
                // A partial write/rotation may leave an incomplete record. Fail
                // closed until an operator recreates the sink, rather than
                // silently appending a subsequent event to a damaged line.
                if state.file.is_none() {
                    return Err(RuntimeError::AuditUnavailable);
                }
                if line.len() as u64 > self.config.max_bytes {
                    return Err(RuntimeError::AuditUnavailable);
                }
                if state.size.saturating_add(line.len() as u64) > self.config.max_bytes {
                    // Failure leaves the sink closed: do not append to an old generation.
                    state.file.take();
                    rotate(path, self.config.retained_files)
                        .map_err(|_| RuntimeError::AuditUnavailable)?;
                    state.file =
                        Some(open_restricted(path).map_err(|_| RuntimeError::AuditUnavailable)?);
                    state.size = 0;
                }
                let file = state.file.as_mut().ok_or(RuntimeError::AuditUnavailable)?;
                if file.write_all(&line).is_err() {
                    state.file.take();
                    return Err(RuntimeError::AuditUnavailable);
                }
                state.size = state.size.saturating_add(line.len() as u64);
                // No userspace buffering. OS write completion is not durable fsync.
                Ok(())
            }
            #[cfg(unix)]
            DestinationState::Syslog(socket) => {
                // user.info PRI; RFC 3164-style daemon header with safe app name.
                let mut message = b"<14>openwrt-mcp: ".to_vec();
                message.extend_from_slice(&line);
                match socket.send(&message) {
                    Ok(count) if count == message.len() => Ok(()),
                    _ => Err(RuntimeError::AuditUnavailable),
                }
            }
        }
    }
}

fn reject_symlink_parents(path: &Path) -> io::Result<()> {
    for parent in path.ancestors().skip(1) {
        let metadata = fs::symlink_metadata(parent)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(io::Error::other("invalid audit directory"));
        }
    }
    Ok(())
}

fn regular_metadata(path: &Path) -> io::Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(io::Error::other("invalid audit file"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.nlink() != 1 {
                    return Err(io::Error::other("invalid audit links"));
                }
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn open_restricted(path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
        reject_symlink_parents(path)?;
        let before = regular_metadata(path)?;
        let mut options = OpenOptions::new();
        options
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        if before.is_none() {
            options.create_new(true);
        }
        let file = options.open(path)?;
        let after = file.metadata()?;
        if !after.is_file() || after.nlink() != 1 {
            return Err(io::Error::other("invalid audit file"));
        }
        if let Some(before) = before
            && (before.dev() != after.dev() || before.ino() != after.ino())
        {
            return Err(io::Error::other("audit file changed"));
        }
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        // POSIX mode bits do not enforce Windows ACLs. No false restriction claim.
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported audit filesystem",
        ))
    }
}

fn rotated_path(path: &Path, generation: usize) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(format!(".{generation}"));
    value.into()
}

fn rotate(path: &Path, retained_files: usize) -> io::Result<()> {
    // The parent must be operator-controlled. O_NOFOLLOW protects opened leaves;
    // portable std rename cannot make parent-directory replacement race-free.
    // Do not share a log path across processes or run an external log rotator.
    reject_symlink_parents(path)?;
    regular_metadata(path)?;
    for generation in 1..=retained_files {
        if let Some(metadata) = regular_metadata(&rotated_path(path, generation))? {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err(io::Error::other("insecure audit generation"));
                }
            }
            #[cfg(not(unix))]
            let _ = metadata;
        }
    }
    let last = rotated_path(path, retained_files);
    if last.try_exists()? {
        fs::remove_file(last)?;
    }
    for generation in (1..retained_files).rev() {
        let source = rotated_path(path, generation);
        if source.try_exists()? {
            fs::rename(source, rotated_path(path, generation + 1))?;
        }
    }
    fs::rename(path, rotated_path(path, 1))
}
