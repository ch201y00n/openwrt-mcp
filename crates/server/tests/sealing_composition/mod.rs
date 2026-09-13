//! Actual age + strict gzip validation with synthetic in-memory port models.
//! These fixtures do not prove native private staging, atomicity or durability.
use age::secrecy::ExposeSecret;
use openwrt_mcp_crypto_age::AgeX25519;
use openwrt_mcp_device_codec::{
    archive::{ExpectedArchive, ExpectedFile},
    gzip::GzipArchiveValidator,
};
use openwrt_mcp_runtime::{
    protection::{
        CryptoLimits, DecryptionSession, EncryptionSession, KeyLimits, KeyMaterial, KeySource,
        ProtectionError,
    },
    sealing::{
        ArchiveSealer, CheckCounts, CleanupOutcome, Publication, PublishOutcome, SealCheck,
        SealFailure, SealLimits, SealPortError, SealSource, SealStage, SealedCounts,
        StageCapabilities,
    },
};
use std::{
    io::{self, Cursor, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct Keys {
    material: KeyMaterial,
    reads: AtomicUsize,
}
impl KeySource for Keys {
    fn read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        KeyMaterial::new(self.material.expose_bytes().to_vec(), max_bytes)
    }
}
struct Sessions {
    encrypt: EncryptionSession,
    decrypt: DecryptionSession,
    public: Arc<Keys>,
    private: Arc<Keys>,
}
impl Sessions {
    fn new() -> Self {
        // Fresh identity remains in process memory; never files, logs or snapshots.
        let identity = age::x25519::Identity::generate();
        let public = Arc::new(Keys {
            material: KeyMaterial::new(identity.to_public().to_string().into_bytes(), 65536)
                .unwrap(),
            reads: AtomicUsize::new(0),
        });
        let private = Arc::new(Keys {
            material: KeyMaterial::new(
                identity.to_string().expose_secret().as_bytes().to_vec(),
                65536,
            )
            .unwrap(),
            reads: AtomicUsize::new(0),
        });
        Self {
            encrypt: EncryptionSession::new(
                public.clone(),
                Arc::new(AgeX25519),
                KeyLimits::default(),
                CryptoLimits::default(),
            )
            .unwrap(),
            decrypt: DecryptionSession::new(
                private.clone(),
                Arc::new(AgeX25519),
                KeyLimits::default(),
                CryptoLimits::default(),
            )
            .unwrap(),
            public,
            private,
        }
    }
}

fn fixture() -> Vec<u8> {
    // Independently generated Python tarfile USTAR + zlib 1.3 Z_FIXED, wbits=31.
    // One regular fixture/a containing b"public\n", expanded tar is 10240 bytes.
    let hex = "1f8b08000000000002034bcbac28292d4ad54f64a01d300002330303306d8049038139121b246e6e6a66cca060c04007505a5c925804b49261648282d2a49ccc642e8651300a46c1281805a360148c8251300a46c1281805a360148c8251300a46c1281805a360148c8251300a46c1281805a360148c8251300a46c1281805430b000044f1b73c00280000";
    hex.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

struct GzipCheck(GzipArchiveValidator);
impl GzipCheck {
    fn boxed() -> Box<dyn SealCheck> {
        Box::new(Self(GzipArchiveValidator::new(
            ExpectedArchive::new(&[ExpectedFile::new("fixture/a", 7).unwrap()]).unwrap(),
        )))
    }
}
impl SealCheck for GzipCheck {
    fn feed(&mut self, bytes: &[u8]) -> Result<(), SealPortError> {
        self.0.feed(bytes).map_err(|_| SealPortError)
    }
    fn finish(self: Box<Self>) -> Result<CheckCounts, SealPortError> {
        let summary = self.0.finish().map_err(|_| SealPortError)?;
        Ok(CheckCounts {
            source_bytes: summary.compressed_bytes,
            expanded_bytes: summary.archive_bytes,
            payload_bytes: summary.payload_bytes,
            files: summary.files as u64,
        })
    }
}

struct MemorySource {
    eof: Arc<AtomicBool>,
    data: Cursor<Vec<u8>>,
    chunk: usize,
    success: bool,
    completed: usize,
    cancelled: usize,
}
impl MemorySource {
    fn new(bytes: Vec<u8>, chunk: usize) -> Self {
        Self {
            eof: Arc::new(AtomicBool::new(false)),
            data: Cursor::new(bytes),
            chunk,
            success: true,
            completed: 0,
            cancelled: 0,
        }
    }
}
impl Read for MemorySource {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let size = bytes.len().min(self.chunk);
        let count = self.data.read(&mut bytes[..size])?;
        if size != 0 && count == 0 {
            self.eof.store(true, Ordering::SeqCst);
        }
        Ok(count)
    }
}
impl SealSource for MemorySource {
    fn complete(&mut self) -> Result<u64, SealPortError> {
        self.completed += 1;
        if self.success {
            Ok(self.data.position())
        } else {
            Err(SealPortError)
        }
    }
    fn cancel(&mut self) -> CleanupOutcome {
        self.cancelled += 1;
        CleanupOutcome::Cleaned
    }
}

struct MemoryStage {
    reject_after_eof: Option<Arc<AtomicBool>>,
    private: Vec<u8>,
    published: Option<Vec<u8>>,
    counts: Option<SealedCounts>,
    chunk: usize,
    fail_flush: bool,
    writes: usize,
    publishes: usize,
    aborts: usize,
}
impl MemoryStage {
    fn new(chunk: usize) -> Self {
        Self {
            reject_after_eof: None,
            private: Vec::new(),
            published: None,
            counts: None,
            chunk,
            fail_flush: false,
            writes: 0,
            publishes: 0,
            aborts: 0,
        }
    }
}
impl Write for MemoryStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self
            .reject_after_eof
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err(io::Error::other("synthetic final ciphertext write failure"));
        }
        let size = self.chunk.min(bytes.len());
        self.private.extend_from_slice(&bytes[..size]);
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("synthetic flush failure"))
        } else {
            Ok(())
        }
    }
}
impl SealStage for MemoryStage {
    fn capabilities(&self) -> StageCapabilities {
        // Trusted fixture declarations ONLY, not evidence of actual OS storage.
        StageCapabilities {
            private_staging: true,
            atomic_publication: true,
            durability: true,
        }
    }
    fn publish(&mut self, counts: SealedCounts) -> PublishOutcome {
        self.publishes += 1;
        assert!(self.published.is_none());
        assert!(self.private.starts_with(b"age-encryption.org/v1\n"));
        self.published = Some(std::mem::take(&mut self.private));
        self.counts = Some(counts);
        PublishOutcome::Published
    }
    fn abort(&mut self) -> CleanupOutcome {
        self.aborts += 1;
        self.private.clear();
        CleanupOutcome::Cleaned
    }
}

#[test]
fn complete_validated_gzip_is_age_sealed_and_authenticates_after_publication() {
    let sessions = Sessions::new();
    let bytes = fixture();
    assert_eq!(bytes.len(), 139);
    let mut operations = 0;
    for source_chunk in [1, 7, 65_536] {
        for stage_chunk in [1, 17, 65_536] {
            let mut source = MemorySource::new(bytes.clone(), source_chunk);
            let mut stage = MemoryStage::new(stage_chunk);
            let report = ArchiveSealer::new(SealLimits::default())
                .unwrap()
                .seal(
                    &mut source,
                    GzipCheck::boxed(),
                    &sessions.encrypt,
                    &mut stage,
                )
                .unwrap();
            assert_eq!(sessions.private.reads.load(Ordering::SeqCst), operations);
            assert_eq!(
                (
                    source.completed,
                    source.cancelled,
                    stage.publishes,
                    stage.aborts
                ),
                (1, 0, 1, 0)
            );
            assert_eq!(report.source_bytes, 139);
            assert_eq!(
                (report.expanded_bytes, report.payload_bytes, report.files),
                (10240, 7, 1)
            );
            let ciphertext = stage.published.as_ref().unwrap();
            assert_eq!(report.ciphertext_bytes, ciphertext.len() as u64);
            assert_eq!(stage.counts, Some(report));
            let mut restored = Vec::new();
            let decrypted = sessions
                .decrypt
                .decrypt_to_staging(&mut ciphertext.as_slice(), &mut restored)
                .unwrap();
            assert!(restored == bytes);
            assert_eq!(decrypted.output_bytes, 139);
            operations += 1;
            assert_eq!(sessions.public.reads.load(Ordering::SeqCst), operations);
            assert_eq!(sessions.private.reads.load(Ordering::SeqCst), operations);
        }
    }
}

#[test]
fn every_truncated_gzip_prefix_and_corrupt_trailer_is_never_published() {
    let sessions = Sessions::new();
    let original = fixture();
    let mut bad: Vec<Vec<u8>> = (0..original.len())
        .map(|n| original[..n].to_vec())
        .collect();
    for index in original.len() - 8..original.len() {
        for bit in 0..8 {
            let mut bytes = original.clone();
            bytes[index] ^= 1 << bit;
            bad.push(bytes);
        }
    }
    for suffix in [&[0_u8][..], original.as_slice()] {
        let mut bytes = original.clone();
        bytes.extend_from_slice(suffix);
        bad.push(bytes);
    }
    for (i, bytes) in bad.into_iter().enumerate() {
        let mut source = MemorySource::new(bytes, if i % 2 == 0 { 1 } else { 65_536 });
        let mut stage = MemoryStage::new(7);
        let report = ArchiveSealer::new(SealLimits::default())
            .unwrap()
            .seal(
                &mut source,
                GzipCheck::boxed(),
                &sessions.encrypt,
                &mut stage,
            )
            .unwrap_err();
        assert_eq!(report.publication, Publication::NotAttempted);
        assert!(matches!(
            report.cause,
            SealFailure::Check | SealFailure::EmptyStream
        ));
        assert_eq!(
            (
                source.completed,
                source.cancelled,
                stage.publishes,
                stage.aborts
            ),
            (0, 1, 0, 1)
        );
        assert!(stage.private.is_empty() && stage.published.is_none());
    }
    assert_eq!(sessions.private.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn valid_archive_with_failed_producer_or_ciphertext_flush_is_not_published() {
    let sessions = Sessions::new();
    for producer_failure in [false, true] {
        let mut source = MemorySource::new(fixture(), 17);
        source.success = !producer_failure;
        let mut stage = MemoryStage::new(3);
        stage.fail_flush = !producer_failure;
        let result = ArchiveSealer::new(SealLimits::default())
            .unwrap()
            .seal(
                &mut source,
                GzipCheck::boxed(),
                &sessions.encrypt,
                &mut stage,
            )
            .unwrap_err();
        assert_eq!(
            result.cause,
            if producer_failure {
                SealFailure::SourceCompletion
            } else {
                SealFailure::StageFlush
            }
        );
        assert_eq!((source.cancelled, stage.publishes, stage.aborts), (1, 0, 1));
        assert!(stage.private.is_empty() && stage.published.is_none());
    }
    assert_eq!(sessions.private.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn actual_age_ciphertext_ceiling_rejects_before_publication() {
    let sessions = Sessions::new();
    let sealer = ArchiveSealer::new(SealLimits::default()).unwrap();
    let mut source = MemorySource::new(fixture(), 7);
    let mut stage = MemoryStage::new(17);
    let size = sealer
        .seal(
            &mut source,
            GzipCheck::boxed(),
            &sessions.encrypt,
            &mut stage,
        )
        .unwrap()
        .ciphertext_bytes;
    // age stanza/header padding can vary, so measure and replay the same trusted
    // ciphertext transcript through a cipher model for exact counting elsewhere.
    // A strict tiny limit here always rejects actual age before publication.
    assert!(size > 139);
    let mut source = MemorySource::new(fixture(), 7);
    let mut stage = MemoryStage::new(17);
    let result = ArchiveSealer::new(SealLimits {
        ciphertext_bytes: 1,
        ..SealLimits::default()
    })
    .unwrap()
    .seal(
        &mut source,
        GzipCheck::boxed(),
        &sessions.encrypt,
        &mut stage,
    )
    .unwrap_err();
    assert_eq!(result.cause, SealFailure::CiphertextLimit);
    assert_eq!((stage.publishes, stage.aborts), (0, 1));
    assert!(stage.private.is_empty());
    assert_eq!(sessions.private.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn final_age_record_write_failure_after_eof_discards_private_ciphertext() {
    let sessions = Sessions::new();
    let mut source = MemorySource::new(fixture(), 7);
    let mut stage = MemoryStage::new(17);
    stage.reject_after_eof = Some(source.eof.clone());
    let result = ArchiveSealer::new(SealLimits::default())
        .unwrap()
        .seal(
            &mut source,
            GzipCheck::boxed(),
            &sessions.encrypt,
            &mut stage,
        )
        .unwrap_err();
    assert_eq!(result.cause, SealFailure::StageWrite);
    assert_eq!(result.publication, Publication::NotAttempted);
    assert!(source.eof.load(Ordering::SeqCst));
    assert_eq!(
        (
            source.completed,
            source.cancelled,
            stage.publishes,
            stage.aborts
        ),
        (0, 1, 0, 1)
    );
    assert!(stage.writes > 1);
    assert!(stage.private.is_empty() && stage.published.is_none());
    assert_eq!(sessions.private.reads.load(Ordering::SeqCst), 0);
}
