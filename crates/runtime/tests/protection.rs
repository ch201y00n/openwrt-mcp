//! Synthetic application-port tests. No key provider, archive, cipher algorithm,
//! filesystem, environment secret or router is used by these fixtures.
mod sealing;
use std::{
    error::Error,
    io::{Cursor, Read, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use openwrt_mcp_runtime::protection::{
    CryptoLimits, CryptoReport, DecryptionProvider, DecryptionSession, EncryptionProvider,
    EncryptionSession, IdentityMaterial, KeyLimits, KeyMaterial, KeySource, ProtectionError,
    RecipientMaterial,
};
use serde_json::json;
use zeroize::Zeroizing;

struct Source {
    bytes: Mutex<Vec<u8>>,
    requests: Mutex<Vec<usize>>,
    error: Option<ProtectionError>,
    ignore_bound: bool,
}

impl Source {
    fn new(bytes: &[u8]) -> Self {
        Self {
            bytes: Mutex::new(bytes.to_vec()),
            requests: Mutex::new(Vec::new()),
            error: None,
            ignore_bound: false,
        }
    }

    fn requests(&self) -> Vec<usize> {
        self.requests.lock().unwrap().clone()
    }
}

impl KeySource for Source {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        self.requests.lock().unwrap().push(max_bytes);
        if let Some(error) = self.error {
            return Err(error);
        }
        let bytes = self.bytes.lock().unwrap().clone();
        let bound = if self.ignore_bound {
            bytes.len()
        } else {
            max_bytes
        };
        KeyMaterial::new(bytes, bound)
    }
}

struct EncryptOnly {
    name: &'static str,
    seen: Mutex<Vec<Vec<u8>>>,
    calls: AtomicUsize,
    fail: Option<ProtectionError>,
}

impl EncryptOnly {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            seen: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
            fail: None,
        }
    }
}

impl EncryptionProvider for EncryptOnly {
    fn format_id(&self) -> &'static str {
        self.name
    }

    fn encrypt(
        &self,
        recipients: &RecipientMaterial,
        input: &mut dyn Read,
        output: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap()
            .push(recipients.expose_bytes().to_vec());
        limits.validate()?;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|_| ProtectionError::StreamFailed)?;
        output
            .write_all(self.name.as_bytes())
            .map_err(|_| ProtectionError::StreamFailed)?;
        if let Some(error) = self.fail {
            return Err(error);
        }
        Ok(CryptoReport {
            input_bytes: bytes.len() as u64,
            output_bytes: self.name.len() as u64,
        })
    }
}

struct DecryptOnly {
    seen: Mutex<Vec<Vec<u8>>>,
    calls: AtomicUsize,
    fail: Option<ProtectionError>,
}

impl DecryptOnly {
    fn new() -> Self {
        Self {
            seen: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
            fail: None,
        }
    }
}

impl DecryptionProvider for DecryptOnly {
    fn format_id(&self) -> &'static str {
        "synthetic-decrypt-only"
    }

    fn decrypt_to_staging(
        &self,
        identity: &IdentityMaterial,
        input: &mut dyn Read,
        staging: &mut dyn Write,
        limits: &CryptoLimits,
    ) -> Result<CryptoReport, ProtectionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap()
            .push(identity.expose_bytes().to_vec());
        limits.validate()?;
        let mut bytes = Vec::new();
        input
            .read_to_end(&mut bytes)
            .map_err(|_| ProtectionError::StreamFailed)?;
        staging
            .write_all(b"synthetic-staging")
            .map_err(|_| ProtectionError::StreamFailed)?;
        if let Some(error) = self.fail {
            return Err(error);
        }
        Ok(CryptoReport {
            input_bytes: bytes.len() as u64,
            output_bytes: 17,
        })
    }
}

fn key_limits() -> KeyLimits {
    KeyLimits {
        max_key_bytes: 64,
        ..KeyLimits::default()
    }
}

#[test]
fn encryption_provider_is_replaceable_without_a_decryption_provider_or_identity() {
    let source = Arc::new(Source::new(b"synthetic-public-recipient"));
    // EncryptOnly does not implement DecryptionProvider. No identity source is
    // supplied, constructed or required by either encryption session.
    for name in ["synthetic-format-one", "synthetic-format-two"] {
        let provider = Arc::new(EncryptOnly::new(name));
        let session = EncryptionSession::new(
            source.clone(),
            provider.clone(),
            key_limits(),
            CryptoLimits::default(),
        )
        .unwrap();
        assert_eq!(session.format_id(), name);
        let mut output = Vec::new();
        assert_eq!(
            session
                .encrypt(&mut b"synthetic-input".as_slice(), &mut output)
                .unwrap(),
            CryptoReport {
                input_bytes: 15,
                output_bytes: name.len() as u64
            }
        );
        assert_eq!(output, name.as_bytes());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            *provider.seen.lock().unwrap(),
            vec![b"synthetic-public-recipient".to_vec()]
        );
    }
    assert_eq!(source.requests(), vec![64, 64]);
}

#[test]
fn encryption_and_decryption_sessions_load_only_their_purpose_binding() {
    let recipient = Arc::new(Source::new(b"synthetic-recipient"));
    let identity = Arc::new(Source::new(b"synthetic-identity"));
    let encryption = Arc::new(EncryptOnly::new("synthetic-encryption"));
    let decryption = Arc::new(DecryptOnly::new());
    let encrypt = EncryptionSession::new(
        recipient.clone(),
        encryption.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    let decrypt = DecryptionSession::new(
        identity.clone(),
        decryption.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    assert_eq!(decrypt.format_id(), "synthetic-decrypt-only");
    assert!(recipient.requests().is_empty());
    assert!(identity.requests().is_empty());
    encrypt
        .encrypt(&mut b"synthetic-input".as_slice(), &mut Vec::new())
        .unwrap();
    assert_eq!(recipient.requests(), vec![64]);
    assert!(identity.requests().is_empty());
    decrypt
        .decrypt_to_staging(&mut b"synthetic-ciphertext".as_slice(), &mut Vec::new())
        .unwrap();
    assert_eq!(recipient.requests(), vec![64]);
    assert_eq!(identity.requests(), vec![64]);
    assert_eq!(
        *encryption.seen.lock().unwrap(),
        vec![b"synthetic-recipient".to_vec()]
    );
    assert_eq!(
        *decryption.seen.lock().unwrap(),
        vec![b"synthetic-identity".to_vec()]
    );
}

#[test]
fn every_operation_reads_exactly_one_fresh_bounded_material() {
    let source = Arc::new(Source::new(b"synthetic-first"));
    let encrypt_provider = Arc::new(EncryptOnly::new("synthetic-encryption"));
    let decrypt_provider = Arc::new(DecryptOnly::new());
    let encrypt = EncryptionSession::new(
        source.clone(),
        encrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    let decrypt = DecryptionSession::new(
        source.clone(),
        decrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    for value in [
        b"synthetic-first".as_slice(),
        b"synthetic-second".as_slice(),
    ] {
        *source.bytes.lock().unwrap() = value.to_vec();
        encrypt
            .encrypt(&mut b"input".as_slice(), &mut Vec::new())
            .unwrap();
        decrypt
            .decrypt_to_staging(&mut b"input".as_slice(), &mut Vec::new())
            .unwrap();
    }
    let expected = vec![b"synthetic-first".to_vec(), b"synthetic-second".to_vec()];
    assert_eq!(*encrypt_provider.seen.lock().unwrap(), expected);
    assert_eq!(*decrypt_provider.seen.lock().unwrap(), expected);
    assert_eq!(source.requests(), vec![64, 64, 64, 64]);
}

#[test]
fn source_failures_stop_before_any_cipher_or_stream_use_without_retry() {
    for error in [
        ProtectionError::SourceUnavailable,
        ProtectionError::InteractionRequired,
        ProtectionError::UnsupportedProtection,
        ProtectionError::InvalidContainer,
        ProtectionError::ResourceLimit,
    ] {
        let source = Arc::new(Source {
            error: Some(error),
            ..Source::new(b"synthetic")
        });
        let encrypt_provider = Arc::new(EncryptOnly::new("synthetic"));
        let decrypt_provider = Arc::new(DecryptOnly::new());
        let encrypt = EncryptionSession::new(
            source.clone(),
            encrypt_provider.clone(),
            key_limits(),
            CryptoLimits::default(),
        )
        .unwrap();
        let decrypt = DecryptionSession::new(
            source.clone(),
            decrypt_provider.clone(),
            key_limits(),
            CryptoLimits::default(),
        )
        .unwrap();
        let mut encryption_output = Vec::new();
        let mut staging = Vec::new();
        let mut plaintext = Cursor::new(b"untouched");
        let mut ciphertext = Cursor::new(b"untouched");
        assert_eq!(
            encrypt.encrypt(&mut plaintext, &mut encryption_output),
            Err(error)
        );
        assert_eq!(
            decrypt.decrypt_to_staging(&mut ciphertext, &mut staging),
            Err(error)
        );
        assert_eq!(source.requests(), vec![64, 64]);
        assert_eq!(encrypt_provider.calls.load(Ordering::SeqCst), 0);
        assert_eq!(decrypt_provider.calls.load(Ordering::SeqCst), 0);
        assert!(encryption_output.is_empty());
        assert!(staging.is_empty());
        assert_eq!(plaintext.position(), 0);
        assert_eq!(ciphertext.position(), 0);
    }
}

#[test]
fn a_source_that_ignores_the_requested_bound_is_rejected_before_the_cipher() {
    let source = Arc::new(Source {
        ignore_bound: true,
        ..Source::new(&[7; 65])
    });
    let encrypt_provider = Arc::new(EncryptOnly::new("synthetic"));
    let decrypt_provider = Arc::new(DecryptOnly::new());
    let encrypt = EncryptionSession::new(
        source.clone(),
        encrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    let decrypt = DecryptionSession::new(
        source.clone(),
        decrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    assert_eq!(
        encrypt.encrypt(&mut b"input".as_slice(), &mut Vec::new()),
        Err(ProtectionError::ResourceLimit)
    );
    assert_eq!(
        decrypt.decrypt_to_staging(&mut b"input".as_slice(), &mut Vec::new()),
        Err(ProtectionError::ResourceLimit)
    );
    assert_eq!(source.requests(), vec![64, 64]);
    assert_eq!(encrypt_provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(decrypt_provider.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn provider_failure_is_returned_without_retry_and_partial_staging_stays_uncommitted() {
    let source = Arc::new(Source::new(b"synthetic"));
    let encrypt_provider = Arc::new(EncryptOnly {
        fail: Some(ProtectionError::EncryptionFailed),
        ..EncryptOnly::new("synthetic-partial")
    });
    let decrypt_provider = Arc::new(DecryptOnly {
        fail: Some(ProtectionError::DecryptionFailed),
        ..DecryptOnly::new()
    });
    let encrypt = EncryptionSession::new(
        source.clone(),
        encrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    let decrypt = DecryptionSession::new(
        source.clone(),
        decrypt_provider.clone(),
        key_limits(),
        CryptoLimits::default(),
    )
    .unwrap();
    let mut output = Vec::new();
    let mut staging = Vec::new();
    assert_eq!(
        encrypt.encrypt(&mut b"input".as_slice(), &mut output),
        Err(ProtectionError::EncryptionFailed)
    );
    assert_eq!(
        decrypt.decrypt_to_staging(&mut b"input".as_slice(), &mut staging),
        Err(ProtectionError::DecryptionFailed)
    );
    assert_eq!(output, b"synthetic-partial");
    assert_eq!(staging, b"synthetic-staging");
    assert_eq!(source.requests(), vec![64, 64]);
    assert_eq!(encrypt_provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(decrypt_provider.calls.load(Ordering::SeqCst), 1);
    // The API returns no success report. Discarding staging/publication belongs
    // to the future backup transaction caller, not this stream-port primitive.
}

#[test]
fn material_construction_is_bounded_and_debug_is_redacted_for_every_purpose() {
    let marker = b"synthetic-sensitive-material";
    let key = KeyMaterial::new(marker.to_vec(), marker.len()).unwrap();
    assert_eq!(key.expose_bytes(), marker);
    assert_eq!(format!("{key:?}"), "KeyMaterial([REDACTED])");
    let recipient =
        RecipientMaterial::new(KeyMaterial::new(marker.to_vec(), marker.len()).unwrap());
    let identity = IdentityMaterial::new(
        KeyMaterial::from_zeroizing(Zeroizing::new(marker.to_vec()), marker.len()).unwrap(),
    );
    assert_eq!(recipient.expose_bytes(), marker);
    assert_eq!(identity.expose_bytes(), marker);
    for rendered in [
        format!("{key:#?}"),
        format!("{recipient:?}"),
        format!("{recipient:#?}"),
        format!("{identity:?}"),
        format!("{identity:#?}"),
    ] {
        assert!(rendered.contains("REDACTED"));
        assert!(!rendered.contains("synthetic"));
    }
    assert_eq!(
        KeyMaterial::new(Vec::new(), 1).err(),
        Some(ProtectionError::InvalidMaterial)
    );
    assert_eq!(
        KeyMaterial::new(vec![1], 0).err(),
        Some(ProtectionError::InvalidConfig)
    );
    assert_eq!(
        KeyMaterial::new(vec![1], 16 * 1024 * 1024 + 1).err(),
        Some(ProtectionError::InvalidConfig)
    );
    assert_eq!(
        KeyMaterial::new(vec![1, 2], 1).err(),
        Some(ProtectionError::ResourceLimit)
    );
    assert!(KeyMaterial::new(vec![1], 16 * 1024 * 1024).is_ok());
}

#[test]
fn secret_material_cannot_acquire_clone_or_serialization_implicitly() {
    // Exactly one blanket implementation is inferable only while $type does
    // NOT implement $trait. Adding the forbidden trait makes this fail to compile.
    macro_rules! assert_not_impl {
        ($type:ty, $trait:path) => {{
            trait AmbiguousIfImpl<A> {
                fn marker() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            struct Forbidden;
            impl<T: ?Sized + $trait> AmbiguousIfImpl<Forbidden> for T {}
            let _ = <$type as AmbiguousIfImpl<_>>::marker;
        }};
    }
    assert_not_impl!(KeyMaterial, Clone);
    assert_not_impl!(RecipientMaterial, Clone);
    assert_not_impl!(IdentityMaterial, Clone);
    assert_not_impl!(KeyMaterial, serde::Serialize);
    assert_not_impl!(RecipientMaterial, serde::Serialize);
    assert_not_impl!(IdentityMaterial, serde::Serialize);
    assert_not_impl!(KeyMaterial, serde::de::DeserializeOwned);
    assert_not_impl!(RecipientMaterial, serde::de::DeserializeOwned);
    assert_not_impl!(IdentityMaterial, serde::de::DeserializeOwned);
    assert_not_impl!(KeyMaterial, serde::Deserialize<'static>);
    assert_not_impl!(RecipientMaterial, serde::Deserialize<'static>);
    assert_not_impl!(IdentityMaterial, serde::Deserialize<'static>);
    assert_not_impl!(KeyMaterial, std::fmt::Display);
}

#[test]
fn protection_errors_have_only_fixed_codes_and_no_upstream_error_chain() {
    for (error, code) in [
        (ProtectionError::InvalidConfig, "protection_config_invalid"),
        (ProtectionError::InvalidMaterial, "key_material_invalid"),
        (ProtectionError::SourceUnavailable, "key_source_unavailable"),
        (
            ProtectionError::InteractionRequired,
            "key_source_interaction_required",
        ),
        (
            ProtectionError::UnsupportedProtection,
            "key_source_protection_unsupported",
        ),
        (
            ProtectionError::UnsupportedContainer,
            "key_container_unsupported",
        ),
        (ProtectionError::InvalidContainer, "key_container_invalid"),
        (ProtectionError::ResourceLimit, "protection_resource_limit"),
        (ProtectionError::EncryptionFailed, "encryption_failed"),
        (ProtectionError::DecryptionFailed, "decryption_failed"),
        (ProtectionError::StreamFailed, "protection_stream_failed"),
        (
            ProtectionError::DeadlineExceeded,
            "protection_deadline_exceeded",
        ),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
        assert!(error.source().is_none());
        assert!(!format!("{error:?}").contains("synthetic"));
    }
}

fn invalid_keys() -> Vec<KeyLimits> {
    let base = KeyLimits::default();
    vec![
        KeyLimits {
            max_key_bytes: 0,
            ..base
        },
        KeyLimits {
            max_key_bytes: 1024 * 1024 + 1,
            ..base
        },
        KeyLimits {
            max_container_bytes: 0,
            ..base
        },
        KeyLimits {
            max_container_bytes: 16 * 1024 * 1024 + 1,
            ..base
        },
        KeyLimits {
            max_entries: 0,
            ..base
        },
        KeyLimits {
            max_entries: 4097,
            ..base
        },
        KeyLimits {
            max_directory_bytes: 0,
            ..base
        },
        KeyLimits {
            max_directory_bytes: 1024 * 1024 + 1,
            ..base
        },
        KeyLimits {
            max_expansion_ratio: 0,
            ..base
        },
        KeyLimits {
            max_expansion_ratio: 1001,
            ..base
        },
        KeyLimits {
            max_container_bytes: 1,
            max_directory_bytes: 2,
            ..base
        },
    ]
}

fn invalid_crypto() -> Vec<CryptoLimits> {
    let base = CryptoLimits::default();
    vec![
        CryptoLimits {
            max_input_bytes: 0,
            ..base
        },
        CryptoLimits {
            max_input_bytes: 1024 * 1024 * 1024 + 1,
            ..base
        },
        CryptoLimits {
            max_output_bytes: 0,
            ..base
        },
        CryptoLimits {
            max_output_bytes: 1024 * 1024 * 1024 + 1,
            ..base
        },
        CryptoLimits {
            timeout_ms: 0,
            ..base
        },
        CryptoLimits {
            timeout_ms: 300_001,
            ..base
        },
        CryptoLimits {
            max_recipients: 0,
            ..base
        },
        CryptoLimits {
            max_recipients: 65,
            ..base
        },
    ]
}

#[test]
fn invalid_limits_are_rejected_before_any_source_or_provider_is_touched() {
    let source = Arc::new(Source::new(b"synthetic"));
    let encrypt = Arc::new(EncryptOnly::new("synthetic"));
    let decrypt = Arc::new(DecryptOnly::new());
    for (keys, crypto) in invalid_keys()
        .into_iter()
        .map(|keys| (keys, CryptoLimits::default()))
        .chain(
            invalid_crypto()
                .into_iter()
                .map(|crypto| (KeyLimits::default(), crypto)),
        )
    {
        assert_eq!(
            EncryptionSession::new(source.clone(), encrypt.clone(), keys, crypto).err(),
            Some(ProtectionError::InvalidConfig)
        );
        assert_eq!(
            DecryptionSession::new(source.clone(), decrypt.clone(), keys, crypto).err(),
            Some(ProtectionError::InvalidConfig)
        );
    }
    assert!(source.requests().is_empty());
    assert_eq!(encrypt.calls.load(Ordering::SeqCst), 0);
    assert_eq!(decrypt.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn documented_limit_defaults_and_hard_cap_boundaries_validate_without_allocating() {
    let keys = KeyLimits::default();
    assert_eq!(
        (
            keys.max_key_bytes,
            keys.max_container_bytes,
            keys.max_entries,
            keys.max_directory_bytes,
            keys.max_expansion_ratio
        ),
        (64 * 1024, 4 * 1024 * 1024, 128, 256 * 1024, 100)
    );
    let crypto = CryptoLimits::default();
    assert_eq!(
        (
            crypto.max_input_bytes,
            crypto.max_output_bytes,
            crypto.timeout_ms,
            crypto.max_recipients
        ),
        (64 * 1024 * 1024, 65 * 1024 * 1024, 30_000, 16)
    );
    assert!(keys.validate().is_ok());
    assert!(crypto.validate().is_ok());
    assert!(
        KeyLimits {
            max_key_bytes: 1,
            max_container_bytes: 1,
            max_entries: 1,
            max_directory_bytes: 1,
            max_expansion_ratio: 1
        }
        .validate()
        .is_ok()
    );
    assert!(
        KeyLimits {
            max_key_bytes: 1024 * 1024,
            max_container_bytes: 16 * 1024 * 1024,
            max_entries: 4096,
            max_directory_bytes: 1024 * 1024,
            max_expansion_ratio: 1000
        }
        .validate()
        .is_ok()
    );
    assert!(
        CryptoLimits {
            max_input_bytes: 1,
            max_output_bytes: 1,
            timeout_ms: 1,
            max_recipients: 1
        }
        .validate()
        .is_ok()
    );
    assert!(
        CryptoLimits {
            max_input_bytes: 1024 * 1024 * 1024,
            max_output_bytes: 1024 * 1024 * 1024,
            timeout_ms: 300_000,
            max_recipients: 64
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn limit_deserialization_rejects_unknown_fields_wrong_types_and_negative_values() {
    let _: KeyLimits = serde_json::from_value(json!({})).unwrap();
    let _: CryptoLimits = serde_json::from_value(json!({})).unwrap();
    for value in [
        json!({"unknown":1}),
        json!({"max_key_bytes":"64"}),
        json!({"max_entries":-1}),
        json!({"max_expansion_ratio":null}),
        json!([]),
    ] {
        assert!(
            serde_json::from_value::<KeyLimits>(value.clone()).is_err(),
            "accepted malformed key-limit fixture: {value}"
        );
    }
    for value in [
        json!({"unknown":1}),
        json!({"timeout_ms":"30"}),
        json!({"max_recipients":-1}),
        json!({"max_output_bytes":null}),
        json!([]),
    ] {
        assert!(
            serde_json::from_value::<CryptoLimits>(value.clone()).is_err(),
            "accepted malformed crypto-limit fixture: {value}"
        );
    }
    // Deserialization is structural; session construction applies numeric policy.
    let zero: CryptoLimits = serde_json::from_value(json!({"timeout_ms":0})).unwrap();
    assert_eq!(zero.validate(), Err(ProtectionError::InvalidConfig));
    assert!(
        serde_json::from_str::<KeyLimits>(r#"{"max_key_bytes":10,"max_key_bytes":20}"#).is_err()
    );
    assert!(serde_json::from_str::<CryptoLimits>(r#"{"timeout_ms":10,"timeout_ms":20}"#).is_err());
    let overridden: KeyLimits = serde_json::from_value(json!({"max_key_bytes":128})).unwrap();
    assert_eq!(overridden.max_key_bytes, 128);
    assert_eq!(overridden.max_entries, KeyLimits::default().max_entries);
    let overridden: CryptoLimits = serde_json::from_value(json!({"timeout_ms":60_000})).unwrap();
    assert_eq!(overridden.timeout_ms, 60_000);
    assert_eq!(
        overridden.max_recipients,
        CryptoLimits::default().max_recipients
    );
}
