//! Fixed binary capture on an already-authenticated, independently observed target.
//! No raw execution, reconnect, key loading, JSON or device admission capability.
use crate::{SshBackend, SshOptions, session::FailureGuard};
use openwrt_mcp_runtime::{
    RuntimeError,
    backups::{ArtifactBinding, BINDING_BYTES, BackupError, CapturedArchive, MAX_PLAINTEXT},
    mutation_ports::WorkBudget,
    protection::KeySource,
};
use russh::ChannelMsg;
use std::{sync::Arc, time::Duration};
use zeroize::Zeroizing;

impl SshBackend {
    pub fn new_for_capture(
        options: SshOptions,
        identity: Arc<dyn KeySource>,
        target: [u8; 16],
    ) -> Result<Self, RuntimeError> {
        if target == [0; 16] {
            return Err(RuntimeError::BackendFailed);
        }
        let mut backend = Self::new(options, identity)?;
        backend.capture_target = Some(target);
        Ok(backend)
    }

    pub async fn capture_archive(
        &self,
        binding: ArtifactBinding,
        budget: WorkBudget,
    ) -> Result<CapturedArchive, BackupError> {
        budget.check()?;
        if self
            .capture_target
            .as_ref()
            .is_none_or(|id| binding.target() != id)
            || self.control.epoch().is_none()
        {
            return Err(BackupError::Invalid);
        }
        let state = self.state.try_lock().map_err(|_| BackupError::Busy)?;
        self.control.check().map_err(|_| BackupError::Unavailable)?;
        let mut guard = FailureGuard::new(&self.control);
        let deadline = tokio::time::Instant::from_std(budget.deadline());
        let mut channel = tokio::time::timeout_at(deadline, state.capture_channel())
            .await
            .map_err(|_| BackupError::Deadline)?
            .map_err(|_| BackupError::Unavailable)?;
        tokio::time::timeout_at(deadline, async {
            channel
                .request_subsystem(true, "openwrt-mcp-capture-v1")
                .await?;
            channel.data(binding.encoded().as_slice()).await?;
            channel.eof().await
        })
        .await
        .map_err(|_| BackupError::Deadline)?
        .map_err(|_| BackupError::Unavailable)?;
        let maximum = binding.source_bytes() as usize + BINDING_BYTES;
        if maximum > MAX_PLAINTEXT + BINDING_BYTES {
            return Err(BackupError::Limit);
        }
        let mut bytes = Zeroizing::new(Vec::with_capacity(binding.source_bytes() as usize));
        let mut prefix = [0; BINDING_BYTES];
        let mut prefix_bytes = 0;
        let mut accepted = false;
        let mut eof = false;
        let mut status = None;
        let mut stderr_bytes = 0;
        loop {
            budget.check()?;
            let message =
                match tokio::time::timeout(Duration::from_millis(20), channel.wait()).await {
                    Ok(Some(message)) => message,
                    Ok(None) => return Err(BackupError::Unavailable),
                    Err(_) => continue,
                };
            budget.check()?;
            match message {
                ChannelMsg::WindowAdjusted { .. } => {}
                ChannelMsg::Success if !accepted => accepted = true,
                ChannelMsg::Data { data } if accepted && !eof => {
                    if data.len() > 65536 || data.len() > maximum - prefix_bytes - bytes.len() {
                        return Err(BackupError::Limit);
                    }
                    let count = (BINDING_BYTES - prefix_bytes).min(data.len());
                    prefix[prefix_bytes..prefix_bytes + count].copy_from_slice(&data[..count]);
                    prefix_bytes += count;
                    if prefix_bytes == BINDING_BYTES && &prefix != binding.encoded() {
                        return Err(BackupError::Integrity);
                    }
                    bytes.extend_from_slice(&data[count..]);
                }
                ChannelMsg::ExtendedData { data, ext: 1 } if accepted && !eof => {
                    if data.len() > 16384 - stderr_bytes {
                        return Err(BackupError::Limit);
                    }
                    stderr_bytes += data.len();
                }
                ChannelMsg::ExitStatus { exit_status } if status.is_none() => {
                    status = Some(exit_status)
                }
                ChannelMsg::Eof if accepted && !eof => eof = true,
                ChannelMsg::Close
                    if accepted
                        && eof
                        && status == Some(0)
                        && prefix_bytes + bytes.len() == maximum =>
                {
                    tokio::time::timeout_at(deadline, channel.close())
                        .await
                        .map_err(|_| BackupError::Deadline)?
                        .map_err(|_| BackupError::Unavailable)?;
                    budget.check()?;
                    let count = bytes.len() as u64;
                    let captured = CapturedArchive::completed(binding, bytes, count)?;
                    guard.complete();
                    return Ok(captured);
                }
                _ => return Err(BackupError::Integrity),
            }
        }
    }
}
