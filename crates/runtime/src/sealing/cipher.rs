use super::{CipherCounts, SealCipher, SealPortError};
use crate::protection::EncryptionSession;
use std::io::{Read, Write};

impl SealCipher for EncryptionSession {
    fn encrypt(
        &self,
        input: &mut dyn Read,
        output: &mut dyn Write,
    ) -> Result<CipherCounts, SealPortError> {
        let report = EncryptionSession::encrypt(self, input, output).map_err(|_| SealPortError)?;
        Ok(CipherCounts {
            input_bytes: report.input_bytes,
            output_bytes: report.output_bytes,
        })
    }
}
