//! All identities are freshly generated synthetic fixtures held only in memory.
use age::secrecy::ExposeSecret;
use openwrt_mcp_crypto_age::AgeX25519;
use openwrt_mcp_runtime::protection::{
    CryptoLimits, DecryptionProvider, EncryptionProvider, IdentityMaterial, KeyMaterial,
    ProtectionError, RecipientMaterial,
};
use std::{
    cell::Cell,
    io::{Error, ErrorKind, Read, Result as IoResult, Write},
    rc::Rc,
    time::Duration,
};

fn material(bytes: &[u8]) -> KeyMaterial {
    KeyMaterial::new(bytes.to_vec(), 65_536).unwrap()
}

fn recipient(identity: &age::x25519::Identity) -> RecipientMaterial {
    RecipientMaterial::new(material(identity.to_public().to_string().as_bytes()))
}

fn private(identity: &age::x25519::Identity) -> IdentityMaterial {
    IdentityMaterial::new(material(identity.to_string().expose_secret().as_bytes()))
}

fn ciphertext(identity: &age::x25519::Identity, plaintext: &[u8]) -> Vec<u8> {
    let mut encrypted = Vec::new();
    AgeX25519
        .encrypt(
            &recipient(identity),
            &mut &plaintext[..],
            &mut encrypted,
            &CryptoLimits::default(),
        )
        .unwrap();
    encrypted
}

#[test]
fn actual_age_round_trips_empty_and_chunk_boundary_payloads_with_exact_counts() {
    let identity = age::x25519::Identity::generate();
    assert_eq!(EncryptionProvider::format_id(&AgeX25519), "age-x25519-v1");
    assert_eq!(DecryptionProvider::format_id(&AgeX25519), "age-x25519-v1");
    for length in [0, 1, 65_535, 65_536, 65_537, 131_072] {
        let plaintext = vec![42; length];
        let mut encrypted = Vec::new();
        let report = AgeX25519
            .encrypt(
                &recipient(&identity),
                &mut plaintext.as_slice(),
                &mut encrypted,
                &CryptoLimits::default(),
            )
            .unwrap();
        assert_eq!(report.input_bytes, length as u64);
        assert_eq!(report.output_bytes, encrypted.len() as u64);
        assert!(encrypted.starts_with(b"age-encryption.org/v1\n"));
        let mut staging = Vec::new();
        let report = AgeX25519
            .decrypt_to_staging(
                &private(&identity),
                &mut encrypted.as_slice(),
                &mut staging,
                &CryptoLimits::default(),
            )
            .unwrap();
        assert_eq!(report.input_bytes, encrypted.len() as u64);
        assert_eq!(report.output_bytes, length as u64);
        assert_eq!(staging, plaintext);
    }
}

#[test]
fn provider_output_and_input_interoperate_with_upstream_age() {
    let identity = age::x25519::Identity::generate();
    let plaintext = b"synthetic protection interoperability fixture";
    let encrypted = ciphertext(&identity, plaintext);
    assert_eq!(age::decrypt(&identity, &encrypted).unwrap(), plaintext);
    let upstream = age::encrypt(&identity.to_public(), plaintext).unwrap();
    let mut staging = Vec::new();
    AgeX25519
        .decrypt_to_staging(
            &private(&identity),
            &mut upstream.as_slice(),
            &mut staging,
            &CryptoLimits::default(),
        )
        .unwrap();
    assert_eq!(staging, plaintext);
}

#[test]
fn multiple_native_recipients_identities_comments_and_crlf_are_supported() {
    let first = age::x25519::Identity::generate();
    let second = age::x25519::Identity::generate();
    let recipients = RecipientMaterial::new(material(
        format!(
            "# Synthetic recipients\r\n\r\n{}\r\n {} \n",
            first.to_public(),
            second.to_public()
        )
        .as_bytes(),
    ));
    let mut encrypted = Vec::new();
    AgeX25519
        .encrypt(
            &recipients,
            &mut &b"fixture"[..],
            &mut encrypted,
            &CryptoLimits::default(),
        )
        .unwrap();
    for identity in [&first, &second] {
        let mut staging = Vec::new();
        AgeX25519
            .decrypt_to_staging(
                &private(identity),
                &mut encrypted.as_slice(),
                &mut staging,
                &CryptoLimits::default(),
            )
            .unwrap();
        assert_eq!(staging, b"fixture");
    }
    let unrelated = age::x25519::Identity::generate();
    let document = zeroize::Zeroizing::new(format!(
        "# Synthetic identities\n{}\n{}\n",
        unrelated.to_string().expose_secret(),
        second.to_string().expose_secret()
    ));
    let identities = IdentityMaterial::new(material(document.as_bytes()));
    AgeX25519
        .decrypt_to_staging(
            &identities,
            &mut encrypted.as_slice(),
            &mut Vec::new(),
            &CryptoLimits::default(),
        )
        .unwrap();
}

#[test]
fn wrong_identity_fails_before_releasing_plaintext() {
    let identity = age::x25519::Identity::generate();
    let wrong = age::x25519::Identity::generate();
    let encrypted = ciphertext(&identity, b"synthetic secret");
    let mut staging = Vec::new();
    assert_eq!(
        AgeX25519
            .decrypt_to_staging(
                &private(&wrong),
                &mut encrypted.as_slice(),
                &mut staging,
                &CryptoLimits::default()
            )
            .unwrap_err(),
        ProtectionError::DecryptionFailed
    );
    assert!(staging.is_empty());
}

#[test]
fn late_tampering_truncation_and_appended_data_never_report_success() {
    let identity = age::x25519::Identity::generate();
    let encrypted = ciphertext(&identity, &vec![42; 131_073]);
    let mut tampered = encrypted.clone();
    *tampered.last_mut().unwrap() ^= 1;
    let mut appended = encrypted.clone();
    appended.push(42);
    let truncated = encrypted[..encrypted.len() - 1].to_vec();
    for altered in [tampered, appended, truncated] {
        let mut staging = Vec::new();
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &private(&identity),
                    &mut altered.as_slice(),
                    &mut staging,
                    &CryptoLimits::default()
                )
                .unwrap_err(),
            ProtectionError::DecryptionFailed
        );
        // Earlier authenticated chunks can exist in staging. They are not a complete backup.
        assert!(!staging.is_empty());
        assert!(staging.len() < 131_073);
    }
    for cut in [0, 1, 20, 100] {
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &private(&identity),
                    &mut &encrypted[..cut],
                    &mut Vec::new(),
                    &CryptoLimits::default()
                )
                .unwrap_err(),
            ProtectionError::DecryptionFailed
        );
    }
}

#[test]
fn private_and_public_material_are_not_interchangeable() {
    let identity = age::x25519::Identity::generate();
    let bad_recipients =
        RecipientMaterial::new(material(identity.to_string().expose_secret().as_bytes()));
    let mut encrypted = Vec::new();
    assert_eq!(
        AgeX25519
            .encrypt(
                &bad_recipients,
                &mut &b"fixture"[..],
                &mut encrypted,
                &CryptoLimits::default()
            )
            .unwrap_err(),
        ProtectionError::InvalidMaterial
    );
    assert!(encrypted.is_empty());
    let bad_identity = IdentityMaterial::new(material(identity.to_public().to_string().as_bytes()));
    assert_eq!(
        AgeX25519
            .decrypt_to_staging(
                &bad_identity,
                &mut &b"fixture"[..],
                &mut Vec::new(),
                &CryptoLimits::default()
            )
            .unwrap_err(),
        ProtectionError::InvalidMaterial
    );
}

#[test]
fn other_key_schemes_empty_documents_and_bad_encoding_fail_closed() {
    for bytes in [
        &b"# comments only\n \r\n"[..],
        &b"ssh-ed25519 synthetic-not-a-key"[..],
        &b"AGE-PLUGIN-synthetic-not-a-key"[..],
        &b"synthetic-passphrase"[..],
        &b"\xff\xfe"[..],
    ] {
        let mut output = Vec::new();
        let error = AgeX25519
            .encrypt(
                &RecipientMaterial::new(material(bytes)),
                &mut &b"fixture"[..],
                &mut output,
                &CryptoLimits::default(),
            )
            .unwrap_err();
        assert_eq!(error, ProtectionError::InvalidMaterial);
        assert!(!error.to_string().contains("synthetic"));
        assert!(output.is_empty());
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &IdentityMaterial::new(material(bytes)),
                    &mut &b"fixture"[..],
                    &mut Vec::new(),
                    &CryptoLimits::default()
                )
                .unwrap_err(),
            ProtectionError::InvalidMaterial
        );
    }
}

#[test]
fn oversized_native_key_lines_are_rejected_before_stream_access() {
    for length in [129, 1024 * 1024] {
        let mut bytes = vec![b'A'; length];
        bytes[..4].copy_from_slice(b"age1");
        let recipients = RecipientMaterial::new(KeyMaterial::new(bytes, length).unwrap());
        assert_eq!(
            AgeX25519
                .encrypt(
                    &recipients,
                    &mut FailingStream,
                    &mut FailingStream,
                    &CryptoLimits::default()
                )
                .unwrap_err(),
            ProtectionError::InvalidMaterial
        );
        let mut bytes = vec![b'A'; length];
        bytes[..15].copy_from_slice(b"AGE-SECRET-KEY-");
        let identity = IdentityMaterial::new(KeyMaterial::new(bytes, length).unwrap());
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &identity,
                    &mut FailingStream,
                    &mut FailingStream,
                    &CryptoLimits::default()
                )
                .unwrap_err(),
            ProtectionError::InvalidMaterial
        );
    }
}

#[test]
fn oversized_unauthenticated_header_is_capped_before_payload_processing() {
    let identity = age::x25519::Identity::generate();
    let mut malformed = b"age-encryption.org/v1\n-> X25519 ".to_vec();
    malformed.resize(1024 * 1024, b'A');
    for maximum in [1024, 1024 * 1024] {
        let mut input = malformed.as_slice();
        let mut staging = Vec::new();
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &private(&identity),
                    &mut input,
                    &mut staging,
                    &CryptoLimits {
                        max_input_bytes: maximum,
                        ..CryptoLimits::default()
                    }
                )
                .unwrap_err(),
            ProtectionError::ResourceLimit
        );
        // The one-byte overflow probe is counted here as source consumption,
        // but it never reaches age's unauthenticated metadata parser.
        assert_eq!(
            malformed.len() - input.len(),
            maximum.min(256 * 1024) as usize + 1
        );
        assert!(staging.is_empty());
    }
}

#[test]
fn key_count_bounds_apply_to_recipients_and_identities_before_stream_io() {
    let identity = age::x25519::Identity::generate();
    let limits = CryptoLimits {
        max_recipients: 1,
        ..CryptoLimits::default()
    };
    let recipients = RecipientMaterial::new(material(
        format!("{}\n{}", identity.to_public(), identity.to_public()).as_bytes(),
    ));
    let mut output = Vec::new();
    assert_eq!(
        AgeX25519
            .encrypt(&recipients, &mut &b"fixture"[..], &mut output, &limits)
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
    assert!(output.is_empty());
    let identities = zeroize::Zeroizing::new(format!(
        "{}\n{}",
        identity.to_string().expose_secret(),
        identity.to_string().expose_secret()
    ));
    assert_eq!(
        AgeX25519
            .decrypt_to_staging(
                &IdentityMaterial::new(material(identities.as_bytes())),
                &mut &b"fixture"[..],
                &mut output,
                &limits
            )
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
}

#[test]
fn encrypt_input_limit_accepts_exact_eof_and_rejects_overflow() {
    let identity = age::x25519::Identity::generate();
    let limits = CryptoLimits {
        max_input_bytes: 7,
        ..CryptoLimits::default()
    };
    let mut encrypted = Vec::new();
    let report = AgeX25519
        .encrypt(
            &recipient(&identity),
            &mut &b"1234567"[..],
            &mut encrypted,
            &limits,
        )
        .unwrap();
    assert_eq!(report.input_bytes, 7);
    assert_eq!(
        AgeX25519
            .encrypt(
                &recipient(&identity),
                &mut &b"12345678"[..],
                &mut Vec::new(),
                &limits
            )
            .unwrap_err(),
        ProtectionError::ResourceLimit
    );
}

#[test]
fn encrypt_output_limit_includes_header_and_final_authentication_tag() {
    let identity = age::x25519::Identity::generate();
    for maximum in [1, 16, 100] {
        let mut output = Vec::new();
        let limits = CryptoLimits {
            max_output_bytes: maximum,
            ..CryptoLimits::default()
        };
        assert_eq!(
            AgeX25519
                .encrypt(
                    &recipient(&identity),
                    &mut &b"fixture"[..],
                    &mut output,
                    &limits
                )
                .unwrap_err(),
            ProtectionError::ResourceLimit
        );
        assert!(output.len() as u64 <= maximum);
    }
}

#[test]
fn decrypt_limits_apply_to_ciphertext_and_staging_without_false_eof() {
    let identity = age::x25519::Identity::generate();
    let encrypted = ciphertext(&identity, b"fixture");
    let exact = CryptoLimits {
        max_input_bytes: encrypted.len() as u64,
        max_output_bytes: 7,
        ..CryptoLimits::default()
    };
    AgeX25519
        .decrypt_to_staging(
            &private(&identity),
            &mut encrypted.as_slice(),
            &mut Vec::new(),
            &exact,
        )
        .unwrap();
    for limits in [
        CryptoLimits {
            max_input_bytes: encrypted.len() as u64 - 1,
            ..exact
        },
        CryptoLimits {
            max_output_bytes: 6,
            ..exact
        },
    ] {
        let mut staging = Vec::new();
        assert_eq!(
            AgeX25519
                .decrypt_to_staging(
                    &private(&identity),
                    &mut encrypted.as_slice(),
                    &mut staging,
                    &limits
                )
                .unwrap_err(),
            ProtectionError::ResourceLimit
        );
        assert!(staging.len() as u64 <= limits.max_output_bytes);
    }
}

struct MarkEof<'a> {
    input: &'a [u8],
    eof: Rc<Cell<bool>>,
}
impl Read for MarkEof<'_> {
    fn read(&mut self, output: &mut [u8]) -> IoResult<usize> {
        let count = self.input.read(output)?;
        if count == 0 {
            self.eof.set(true);
        }
        Ok(count)
    }
}
struct RejectAfterEof {
    written: Vec<u8>,
    eof: Rc<Cell<bool>>,
    rejected: bool,
}
impl Write for RejectAfterEof {
    fn write(&mut self, input: &[u8]) -> IoResult<usize> {
        if self.eof.get() {
            self.rejected = true;
            Err(Error::other("synthetic-secret-final-write-error"))
        } else {
            self.written.write(input)
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

#[test]
fn finalization_failure_is_not_success_even_after_all_plaintext_was_read() {
    let identity = age::x25519::Identity::generate();
    let eof = Rc::new(Cell::new(false));
    let mut input = MarkEof {
        input: b"small synthetic fixture",
        eof: eof.clone(),
    };
    let mut output = RejectAfterEof {
        written: Vec::new(),
        eof,
        rejected: false,
    };
    let error = AgeX25519
        .encrypt(
            &recipient(&identity),
            &mut input,
            &mut output,
            &CryptoLimits::default(),
        )
        .unwrap_err();
    assert_eq!(error, ProtectionError::StreamFailed);
    assert!(output.rejected);
    assert!(!output.written.is_empty());
    assert!(!error.to_string().contains("synthetic"));
    assert!(age::decrypt(&identity, &output.written).is_err());
}

struct FailingStream;
impl Read for FailingStream {
    fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
        Err(Error::other("synthetic-secret-read"))
    }
}
impl Write for FailingStream {
    fn write(&mut self, _: &[u8]) -> IoResult<usize> {
        Err(Error::other("synthetic-secret-write"))
    }
    fn flush(&mut self) -> IoResult<()> {
        Err(Error::other("synthetic-secret-flush"))
    }
}
struct FailFlush(Vec<u8>);
impl Write for FailFlush {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        self.0.write(bytes)
    }
    fn flush(&mut self) -> IoResult<()> {
        Err(Error::other("synthetic-secret-flush"))
    }
}

#[test]
fn input_output_and_flush_failures_are_sanitized_in_both_directions() {
    let identity = age::x25519::Identity::generate();
    let encrypted = ciphertext(&identity, b"fixture");
    let limits = CryptoLimits::default();
    for result in [
        AgeX25519.encrypt(
            &recipient(&identity),
            &mut FailingStream,
            &mut Vec::new(),
            &limits,
        ),
        AgeX25519.encrypt(
            &recipient(&identity),
            &mut &b"fixture"[..],
            &mut FailingStream,
            &limits,
        ),
        AgeX25519.encrypt(
            &recipient(&identity),
            &mut &b"fixture"[..],
            &mut FailFlush(Vec::new()),
            &limits,
        ),
        AgeX25519.decrypt_to_staging(
            &private(&identity),
            &mut FailingStream,
            &mut Vec::new(),
            &limits,
        ),
        AgeX25519.decrypt_to_staging(
            &private(&identity),
            &mut encrypted.as_slice(),
            &mut FailingStream,
            &limits,
        ),
        AgeX25519.decrypt_to_staging(
            &private(&identity),
            &mut encrypted.as_slice(),
            &mut FailFlush(Vec::new()),
            &limits,
        ),
    ] {
        let error = result.unwrap_err();
        assert_eq!(error, ProtectionError::StreamFailed);
        assert!(!error.to_string().contains("synthetic"));
    }
}

struct ShortIo<T>(T);
impl<T: Read> Read for ShortIo<T> {
    fn read(&mut self, output: &mut [u8]) -> IoResult<usize> {
        let capacity = output.len().min(3);
        self.0.read(&mut output[..capacity])
    }
}
impl<T: Write> Write for ShortIo<T> {
    fn write(&mut self, input: &[u8]) -> IoResult<usize> {
        self.0.write(&input[..input.len().min(3)])
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flush()
    }
}

#[test]
fn short_reads_and_writes_do_not_truncate_data_or_inflate_counts() {
    let identity = age::x25519::Identity::generate();
    let plaintext = b"a synthetic stream with short reads and writes";
    let mut output = ShortIo(Vec::new());
    let report = AgeX25519
        .encrypt(
            &recipient(&identity),
            &mut ShortIo(&plaintext[..]),
            &mut output,
            &CryptoLimits::default(),
        )
        .unwrap();
    assert_eq!(report.output_bytes, output.0.len() as u64);
    let mut staging = ShortIo(Vec::new());
    AgeX25519
        .decrypt_to_staging(
            &private(&identity),
            &mut ShortIo(output.0.as_slice()),
            &mut staging,
            &CryptoLimits::default(),
        )
        .unwrap();
    assert_eq!(staging.0, plaintext);
}

struct AlwaysInterrupted;
impl Read for AlwaysInterrupted {
    fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
        Err(ErrorKind::Interrupted.into())
    }
}
struct DelayedRead;
impl Read for DelayedRead {
    fn read(&mut self, _: &mut [u8]) -> IoResult<usize> {
        std::thread::sleep(Duration::from_millis(50));
        Ok(0)
    }
}

#[test]
fn cooperative_deadline_covers_repeated_interrupts_and_return_from_blocking_input() {
    let identity = age::x25519::Identity::generate();
    let limits = CryptoLimits {
        timeout_ms: 20,
        ..CryptoLimits::default()
    };
    for input in [
        &mut AlwaysInterrupted as &mut dyn Read,
        &mut DelayedRead as &mut dyn Read,
    ] {
        assert_eq!(
            AgeX25519
                .encrypt(&recipient(&identity), input, &mut Vec::new(), &limits)
                .unwrap_err(),
            ProtectionError::DeadlineExceeded
        );
    }
    assert_eq!(
        AgeX25519
            .decrypt_to_staging(
                &private(&identity),
                &mut AlwaysInterrupted,
                &mut Vec::new(),
                &limits
            )
            .unwrap_err(),
        ProtectionError::DeadlineExceeded
    );
}

#[test]
fn invalid_limits_are_rejected_before_key_parsing_or_stream_access() {
    let invalid = CryptoLimits {
        max_recipients: 0,
        ..CryptoLimits::default()
    };
    assert_eq!(
        AgeX25519
            .encrypt(
                &RecipientMaterial::new(material(b"not-a-key")),
                &mut FailingStream,
                &mut FailingStream,
                &invalid
            )
            .unwrap_err(),
        ProtectionError::InvalidConfig
    );
    assert_eq!(
        AgeX25519
            .decrypt_to_staging(
                &IdentityMaterial::new(material(b"not-a-key")),
                &mut FailingStream,
                &mut FailingStream,
                &invalid
            )
            .unwrap_err(),
        ProtectionError::InvalidConfig
    );
}
