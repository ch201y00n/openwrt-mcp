//! Native age X25519 over caller-owned streams; no key-location or OS access.
//!
//! Decryption emits staging bytes before final authentication. Discard staging
//! on every error and do not apply it merely because this provider returned success.
//! Deadlines are cooperative: a caller's blocking Read/Write cannot be interrupted.
//! Header construction has an additional fixed 256-KiB read budget, including
//! the nonce and buffered read-ahead, to bound unauthenticated metadata parsing.
//! Native key lines longer than 128 bytes are rejected before key decoding.

mod keys;
pub mod provenance;
mod stream;

use openwrt_mcp_runtime::protection::{
    CryptoLimits, CryptoReport, DecryptionProvider, EncryptionProvider, IdentityMaterial,
    ProtectionError, RecipientMaterial,
};
use std::io::{BufReader, Read, Write};
use stream::{BoundedRead, BoundedWrite, Budget, copy};

const MAX_HEADER_READ_BYTES: u64 = 256 * 1024;

#[derive(Debug, Default)]
pub struct AgeX25519;

impl EncryptionProvider for AgeX25519 {
    fn format_id(&self) -> &'static str {
        "age-x25519-v1"
    }

    fn encrypt(
        &self,
        recipients: &RecipientMaterial,
        input: &mut dyn Read,
        output: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError> {
        limits.validate()?;
        let budget = Budget::new(limits.timeout_ms);
        let recipients =
            keys::recipients(recipients.expose_bytes(), limits.max_recipients, &budget)?;
        let encryptor = age::Encryptor::with_recipients(
            recipients
                .iter()
                .map(|recipient| recipient as &dyn age::Recipient),
        )
        .map_err(|_| budget.error(ProtectionError::EncryptionFailed))?;
        budget.check()?;
        let mut input = BoundedRead::new(input, limits.max_input_bytes, &budget);
        let mut output = BoundedWrite::new(output, limits.max_output_bytes, &budget);
        let mut writer = encryptor
            .wrap_output(&mut output)
            .map_err(|_| budget.error(ProtectionError::EncryptionFailed))?;
        copy(
            &mut input,
            &mut writer,
            &budget,
            ProtectionError::EncryptionFailed,
        )?;
        // finish writes the authenticated final chunk; dropping writer is insufficient.
        writer
            .finish()
            .map_err(|_| budget.error(ProtectionError::EncryptionFailed))?;
        output
            .flush()
            .map_err(|_| budget.error(ProtectionError::StreamFailed))?;
        budget.check()?;
        Ok(CryptoReport {
            input_bytes: input.count(),
            output_bytes: output.count(),
        })
    }
}

impl DecryptionProvider for AgeX25519 {
    fn format_id(&self) -> &'static str {
        "age-x25519-v1"
    }

    fn decrypt_to_staging(
        &self,
        identity: &IdentityMaterial,
        input: &mut dyn Read,
        staging: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError> {
        limits.validate()?;
        let budget = Budget::new(limits.timeout_ms);
        // The configured key-count limit bounds native identities as well as recipients.
        let identities = keys::identities(identity.expose_bytes(), limits.max_recipients, &budget)?;
        let mut input = BoundedRead::new(input, limits.max_input_bytes, &budget);
        let mut output = BoundedWrite::new(staging, limits.max_output_bytes, &budget);
        budget.begin_header(MAX_HEADER_READ_BYTES);
        // The buffered API parses lines without repeatedly requesting individual
        // bytes. Its bounded read-ahead counts toward this constructor budget.
        let decryptor = age::Decryptor::new_buffered(BufReader::new(&mut input))
            .map_err(|_| budget.error(ProtectionError::DecryptionFailed))?;
        budget.finish_header();
        budget.check()?;
        if decryptor.is_scrypt() {
            return Err(ProtectionError::UnsupportedProtection);
        }
        let mut reader = decryptor
            .decrypt(
                identities
                    .iter()
                    .map(|identity| identity as &dyn age::Identity),
            )
            .map_err(|_| budget.error(ProtectionError::DecryptionFailed))?;
        // Read through authenticated EOF, including exact 64-KiB last chunks.
        copy(
            &mut reader,
            &mut output,
            &budget,
            ProtectionError::DecryptionFailed,
        )?;
        drop(reader);
        output
            .flush()
            .map_err(|_| budget.error(ProtectionError::StreamFailed))?;
        budget.check()?;
        Ok(CryptoReport {
            input_bytes: input.count(),
            output_bytes: output.count(),
        })
    }
}
