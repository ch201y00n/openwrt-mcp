use openwrt_mcp_runtime::RuntimeError;
use serde::Deserialize;
use std::net::IpAddr;

/// Operator-owned endpoint and exact host-key pin. No implicit SSH configuration.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshOptions {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub host_key_sha256: String,
}

impl SshOptions {
    pub fn validate(&self) -> Result<(), RuntimeError> {
        let hostname = !self.host.is_empty()
            && self.host.len() <= 253
            && (self.host.parse::<IpAddr>().is_ok()
                || self.host.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && label.as_bytes()[0].is_ascii_alphanumeric()
                        && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                        && label
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                }));
        if !hostname
            || self.port == 0
            || self.username.is_empty()
            || self.username.len() > 64
            || !self
                .username
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
            || !self.host_key_sha256.starts_with("SHA256:")
            || self.host_key_sha256.len() != 50
        {
            return Err(RuntimeError::InvalidConfig);
        }
        let pin = self
            .host_key_sha256
            .parse::<russh::keys::ssh_key::Fingerprint>()
            .map_err(|_| RuntimeError::InvalidConfig)?;
        if pin.to_string() != self.host_key_sha256 {
            return Err(RuntimeError::InvalidConfig);
        }
        Ok(())
    }
}
