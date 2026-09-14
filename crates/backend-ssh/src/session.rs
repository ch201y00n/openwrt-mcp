use crate::SshOptions;
use openwrt_mcp_runtime::{RuntimeError, protection::KeySource};
use russh::{ChannelMsg, client::Handle};
use std::{
    net::{Shutdown, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const MAX_IDENTITY_BYTES: usize = 65_536;

fn is_flow_control(message: &ChannelMsg) -> bool {
    matches!(message, ChannelMsg::WindowAdjusted { .. })
}

#[derive(Default)]
pub(crate) struct Control {
    failed: AtomicBool,
    authenticated: Arc<AtomicBool>,
    socket: Mutex<Option<TcpStream>>,
}

impl Control {
    pub(crate) fn epoch(&self) -> Option<u64> {
        if !self.failed.load(Ordering::Acquire) && self.authenticated.load(Ordering::Acquire) {
            Some(1)
        } else {
            None
        }
    }

    pub(crate) fn check(&self) -> Result<(), RuntimeError> {
        if self.failed.load(Ordering::Acquire) {
            Err(RuntimeError::BackendFailed)
        } else {
            Ok(())
        }
    }

    pub(crate) fn invalidate(&self) {
        self.failed.store(true, Ordering::Release);
        self.authenticated.store(false, Ordering::Release);
        // Only assignment/shutdown hold this lock, never an await or callback.
        // Recover poison so cancellation still closes the network connection.
        let mut socket = self
            .socket
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(socket) = socket.take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }

    fn install(&self, socket: TcpStream) {
        *self
            .socket
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(socket);
    }
}

pub(crate) struct FailureGuard<'a> {
    control: &'a Control,
    armed: bool,
}
impl<'a> FailureGuard<'a> {
    pub(crate) fn new(control: &'a Control) -> Self {
        Self {
            control,
            armed: true,
        }
    }
    pub(crate) fn complete(&mut self) {
        self.armed = false;
    }
}
impl Drop for FailureGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.control.invalidate();
        }
    }
}

struct PinnedHost {
    fingerprint: String,
    rejected: Arc<AtomicBool>,
    authenticated: Arc<AtomicBool>,
}

impl Drop for PinnedHost {
    fn drop(&mut self) {
        // Driver shutdown, including an idle remote disconnect, revokes the
        // observation epoch without exposing a handle or opening another session.
        self.authenticated.store(false, Ordering::Release);
    }
}

impl russh::client::Handler for PinnedHost {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        presented: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let accepted = match presented {
            russh::keys::PublicKeyOrCertificate::PublicKey { key, .. } => {
                key.algorithm() == russh::keys::Algorithm::Ed25519
                    && key.fingerprint(russh::keys::HashAlg::Sha256).to_string() == self.fingerprint
            }
            russh::keys::PublicKeyOrCertificate::Certificate(_) => false,
        };
        if !accepted {
            self.rejected.store(true, Ordering::Release);
        }
        Ok(accepted)
    }
}

pub(crate) struct State {
    identity: Option<Arc<dyn KeySource>>,
    handle: Option<Handle<PinnedHost>>,
}

impl State {
    /// Existing authenticated connection only; no key loading or reconnect.
    pub(crate) async fn capture_channel(
        &self,
    ) -> Result<russh::Channel<russh::client::Msg>, RuntimeError> {
        self.handle
            .as_ref()
            .ok_or(RuntimeError::BackendFailed)?
            .channel_open_session()
            .await
            .map_err(|_| RuntimeError::BackendFailed)
    }
    pub(crate) fn new(identity: Arc<dyn KeySource>) -> Self {
        Self {
            identity: Some(identity),
            handle: None,
        }
    }

    pub(crate) async fn connect(
        &mut self,
        options: &SshOptions,
        control: &Control,
    ) -> Result<(), RuntimeError> {
        if let Some(handle) = &self.handle {
            return if handle.is_closed() {
                Err(RuntimeError::BackendFailed)
            } else {
                Ok(())
            };
        }
        let source = self.identity.take().ok_or(RuntimeError::BackendFailed)?;
        // At most one blocking worker per backend. A timed-out source cannot
        // accumulate retries because the failure guard permanently invalidates.
        let material = tokio::task::spawn_blocking(move || source.read(MAX_IDENTITY_BYTES))
            .await
            .map_err(|_| RuntimeError::AuthenticationFailed)?
            .map_err(|_| RuntimeError::AuthenticationFailed)?;
        if material.expose_bytes().len() > MAX_IDENTITY_BYTES {
            return Err(RuntimeError::AuthenticationFailed);
        }
        let key = russh::keys::PrivateKey::from_openssh(material.expose_bytes())
            .map_err(|_| RuntimeError::AuthenticationFailed)?;
        drop(material);
        if key.is_encrypted() || key.algorithm() != russh::keys::Algorithm::Ed25519 {
            return Err(RuntimeError::AuthenticationFailed);
        }
        let stream = tokio::net::TcpStream::connect((options.host.as_str(), options.port))
            .await
            .map_err(|_| RuntimeError::BackendFailed)?;
        stream
            .set_nodelay(true)
            .map_err(|_| RuntimeError::BackendFailed)?;
        let stream = stream.into_std().map_err(|_| RuntimeError::BackendFailed)?;
        control.install(
            stream
                .try_clone()
                .map_err(|_| RuntimeError::BackendFailed)?,
        );
        let stream =
            tokio::net::TcpStream::from_std(stream).map_err(|_| RuntimeError::BackendFailed)?;
        let rejected = Arc::new(AtomicBool::new(false));
        let config = russh::client::Config {
            window_size: 65_536,
            maximum_packet_size: 32_768,
            channel_buffer_size: 2,
            keepalive_interval: Some(Duration::from_secs(15)),
            keepalive_max: 3,
            nodelay: true,
            ..Default::default()
        };
        let mut handle = russh::client::connect_stream(
            Arc::new(config),
            stream,
            PinnedHost {
                fingerprint: options.host_key_sha256.clone(),
                rejected: rejected.clone(),
                authenticated: control.authenticated.clone(),
            },
        )
        .await
        .map_err(|_| {
            if rejected.load(Ordering::Acquire) {
                RuntimeError::HostKeyRejected
            } else {
                RuntimeError::BackendFailed
            }
        })?;
        let authenticated = handle
            .authenticate_publickey(
                &options.username,
                russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None),
            )
            .await
            .map_err(|_| RuntimeError::AuthenticationFailed)?;
        if !authenticated.success() {
            return Err(RuntimeError::AuthenticationFailed);
        }
        control.authenticated.store(true, Ordering::Release);
        if handle.is_closed() {
            control.invalidate();
            return Err(RuntimeError::BackendFailed);
        }
        self.handle = Some(handle);
        Ok(())
    }

    /// Outer errors mean uncertain connection/completion; inner errors mean a
    /// fully completed channel with unsuccessful exit status. Parsing is separate.
    pub(crate) async fn execute(
        &self,
        command: Vec<u8>,
        maximum: usize,
    ) -> Result<Result<Vec<u8>, RuntimeError>, RuntimeError> {
        let handle = self.handle.as_ref().ok_or(RuntimeError::BackendFailed)?;
        let mut channel = handle
            .channel_open_session()
            .await
            .map_err(|_| RuntimeError::BackendFailed)?;
        channel
            .exec(true, command)
            .await
            .map_err(|_| RuntimeError::BackendFailed)?;
        channel
            .eof()
            .await
            .map_err(|_| RuntimeError::BackendFailed)?;
        let mut stdout = Vec::new();
        let mut stderr_count = 0_usize;
        let mut exit_status = None;
        let mut accepted = false;
        let mut eof = false;
        while let Some(message) = channel.wait().await {
            // The driver has already applied the send-window update. This
            // informational event does not affect remote completion state.
            if is_flow_control(&message) {
                continue;
            }
            match message {
                ChannelMsg::Success if !accepted => accepted = true,
                ChannelMsg::Data { data } if !eof => {
                    if data.len() > maximum - stdout.len() {
                        return Err(RuntimeError::OutputLimit);
                    }
                    stdout.extend_from_slice(&data);
                }
                ChannelMsg::ExtendedData { data, ext: 1 } if !eof => {
                    if data.len() > maximum - stderr_count {
                        return Err(RuntimeError::OutputLimit);
                    }
                    stderr_count += data.len();
                }
                ChannelMsg::ExitStatus { exit_status: code } if exit_status.is_none() => {
                    exit_status = Some(code)
                }
                ChannelMsg::Eof if !eof => eof = true,
                ChannelMsg::Close => {
                    if !accepted {
                        return Err(RuntimeError::BackendFailed);
                    }
                    let status = exit_status.ok_or(RuntimeError::BackendFailed)?;
                    channel
                        .close()
                        .await
                        .map_err(|_| RuntimeError::BackendFailed)?;
                    if status != 0 {
                        return Ok(Err(RuntimeError::BackendFailed));
                    }
                    return Ok(Ok(stdout));
                }
                _ => return Err(RuntimeError::BackendFailed),
            }
        }
        Err(RuntimeError::BackendFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::{Control, PinnedHost, is_flow_control};
    use russh::ChannelMsg;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn only_window_adjustment_bypasses_response_completion_state() {
        for new_size in [0, 32_768, u32::MAX] {
            assert!(is_flow_control(&ChannelMsg::WindowAdjusted { new_size }));
        }
        for message in [
            ChannelMsg::Data {
                data: b"{}".to_vec().into(),
            },
            ChannelMsg::ExtendedData {
                data: b"synthetic".to_vec().into(),
                ext: 1,
            },
            ChannelMsg::Success,
            ChannelMsg::Failure,
            ChannelMsg::ExitStatus { exit_status: 0 },
            ChannelMsg::Eof,
            ChannelMsg::Close,
        ] {
            assert!(!is_flow_control(&message));
        }
    }

    #[test]
    fn driver_drop_revokes_an_authenticated_epoch_even_when_idle() {
        let control = Control::default();
        assert_eq!(control.epoch(), None);
        let handler = PinnedHost {
            fingerprint: "synthetic".into(),
            rejected: Arc::new(AtomicBool::new(false)),
            authenticated: control.authenticated.clone(),
        };
        control.authenticated.store(true, Ordering::Release);
        assert_eq!(control.epoch(), Some(1));
        drop(handler);
        assert_eq!(control.epoch(), None);
    }
}
