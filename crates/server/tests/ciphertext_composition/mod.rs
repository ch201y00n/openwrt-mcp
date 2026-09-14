//! Real native ciphertext files, synthetic keys in RAM, no router or Vault.
mod faults;
mod remote;
mod secrets;
use age::secrecy::ExposeSecret;
use openwrt_mcp_adapters::backups::{
    RecordStore, provisioning_header, restore_inspect, seal_capture,
};
use openwrt_mcp_crypto_age::{AgeX25519, provenance::HmacSha256};
use openwrt_mcp_runtime::{
    backups::{
        ArchiveFormat, ArtifactBinding, BackupError, CapturedArchive, RecordAuthenticator,
        RecordPublication,
    },
    mutation_ports::WorkBudget,
    protection::{
        CryptoLimits, DecryptionSession, EncryptionSession, KeyLimits, KeyMaterial, KeySource,
        ProtectionError,
    },
};
use std::{
    fs::{self, File},
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn budget() -> WorkBudget {
    WorkBudget::new(Duration::from_secs(30)).unwrap()
}
struct Keys(KeyMaterial);
impl KeySource for Keys {
    fn read(&self, max: usize) -> Result<KeyMaterial, ProtectionError> {
        KeyMaterial::new(self.0.expose_bytes().to_vec(), max)
    }
}
struct Crypto {
    encrypt: EncryptionSession,
    decrypt: DecryptionSession,
    auth: Arc<dyn RecordAuthenticator>,
}
impl Crypto {
    fn new() -> Self {
        let identity = age::x25519::Identity::generate();
        let public = Arc::new(Keys(
            KeyMaterial::new(identity.to_public().to_string().into_bytes(), 65536).unwrap(),
        ));
        let private = Arc::new(Keys(
            KeyMaterial::new(
                identity.to_string().expose_secret().as_bytes().to_vec(),
                65536,
            )
            .unwrap(),
        ));
        Self {
            encrypt: EncryptionSession::new(
                public,
                Arc::new(AgeX25519),
                KeyLimits::default(),
                CryptoLimits::default(),
            )
            .unwrap(),
            decrypt: DecryptionSession::new(
                private,
                Arc::new(AgeX25519),
                KeyLimits::default(),
                CryptoLimits::default(),
            )
            .unwrap(),
            auth: authenticator(41),
        }
    }
}
fn authenticator(seed: u8) -> Arc<dyn RecordAuthenticator> {
    Arc::new(HmacSha256::new(KeyMaterial::new(vec![seed; 32], 32).unwrap()).unwrap())
}
struct StoreFile {
    root: PathBuf,
    path: PathBuf,
}
impl StoreFile {
    fn new(auth: &dyn RecordAuthenticator) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let root = temp.join(format!(
            "openwrt-mcp-p3-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        assert!(root.starts_with(&temp));
        assert!(!root.starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR"))));
        fs::create_dir(&root).unwrap();
        let path = root.join("ciphertext.records");
        let mut file = File::create_new(&path).unwrap();
        file.write_all(&provisioning_header([1; 16], auth).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        Self { root, path }
    }
    fn open(&self, crypto: &Crypto) -> RecordStore {
        RecordStore::open(&self.path, [1; 16], crypto.auth.clone(), &budget()).unwrap()
    }
}
impl Drop for StoreFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir(&self.root);
    }
}

fn tar(payload: &[u8]) -> Vec<u8> {
    let mut header = [0u8; 512];
    header[..17].copy_from_slice(b"etc/config/system");
    header[100..108].copy_from_slice(b"0000600\0");
    header[108..116].copy_from_slice(b"0000000\0");
    header[116..124].copy_from_slice(b"0000000\0");
    header[124..136].copy_from_slice(format!("{:011o}\0", payload.len()).as_bytes());
    header[136..148].copy_from_slice(b"00000000000\0");
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum: u64 = header.iter().map(|b| *b as u64).sum();
    header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
    let mut data = header.to_vec();
    data.extend_from_slice(payload);
    data.resize(data.len().div_ceil(512) * 512 + 1024, 0);
    data
}
fn gzip(bytes: &[u8]) -> Vec<u8> {
    // Independent stored-block DEFLATE fixture; no production compression helper.
    let mut output = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
    let chunks: Vec<_> = bytes.chunks(65535).collect();
    for (index, block) in chunks.iter().enumerate() {
        output.push(u8::from(index + 1 == chunks.len()));
        let length = block.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(block);
    }
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    output.extend_from_slice(&(!crc).to_le_bytes());
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output
}
fn binding(payload: usize, source: usize, format: ArchiveFormat, job: u8) -> ArtifactBinding {
    ArtifactBinding::system(
        [1; 16],
        [2; 16],
        [3; 16],
        [job; 16],
        payload as u64,
        source as u64,
        format,
    )
    .unwrap()
}
fn capture(binding: &ArtifactBinding, bytes: Vec<u8>) -> CapturedArchive {
    let count = bytes.len() as u64;
    CapturedArchive::completed(binding.clone(), bytes.into(), count).unwrap()
}

#[test]
fn native_store_age_tar_and_gzip_roundtrip_reopen_and_no_secret_file() {
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let marker = b"P3-SYNTHETIC-SYSTEM-NOT-A-ROUTER";
    for (index, (format, payload)) in [
        (ArchiveFormat::Tar, marker.to_vec()),
        (ArchiveFormat::Gzip, vec![0x6b; 65536]),
        (ArchiveFormat::Tar, vec![]),
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = tar(&payload);
        let bytes = if format == ArchiveFormat::Gzip {
            gzip(&bytes)
        } else {
            bytes
        };
        let binding = binding(payload.len(), bytes.len(), format, index as u8 + 10);
        let mut store = file.open(&crypto);
        assert!(RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget()).is_err());
        let sealed = seal_capture(
            capture(&binding, bytes.clone()),
            &crypto.encrypt,
            &mut store,
            budget(),
        )
        .unwrap();
        assert_eq!(sealed.payload_bytes, payload.len() as u64);
        assert_eq!(sealed.files, 1);
        let duplicate = seal_capture(
            capture(&binding, bytes),
            &crypto.encrypt,
            &mut store,
            budget(),
        )
        .unwrap_err();
        assert_eq!(duplicate.publication, RecordPublication::NotPublished);
        drop(store);
        let mut store = file.open(&crypto);
        assert_eq!(
            store.reconcile(&binding, &budget()),
            RecordPublication::Durable
        );
        assert_eq!(
            restore_inspect(&mut store, &binding, &crypto.decrypt, budget()).unwrap(),
            sealed
        );
        drop(store);
        let on_disk = fs::read(&file.path).unwrap();
        assert!(!on_disk.windows(marker.len()).any(|w| w == marker));
        assert_eq!(fs::read_dir(&file.root).unwrap().count(), 1);
    }
}

#[test]
fn invalid_source_manifest_and_cipher_finalization_never_append() {
    use openwrt_mcp_runtime::sealing::{CipherCounts, SealCipher, SealPortError};
    struct FailFinalize;
    impl SealCipher for FailFinalize {
        fn encrypt(
            &self,
            input: &mut dyn std::io::Read,
            output: &mut dyn std::io::Write,
        ) -> Result<CipherCounts, SealPortError> {
            let mut bytes = [0; 4096];
            while input.read(&mut bytes).map_err(|_| SealPortError)? != 0 {}
            output
                .write_all(b"age-encryption.org/v1\nunfinished")
                .map_err(|_| SealPortError)?;
            Err(SealPortError)
        }
    }
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let bytes = tar(b"synthetic");
    let good = binding(9, bytes.len(), ArchiveFormat::Tar, 4);
    assert!(
        CapturedArchive::completed(
            good.clone(),
            bytes[..bytes.len() - 1].to_vec().into(),
            bytes.len() as u64
        )
        .is_err()
    );
    let cases: [(ArtifactBinding, Vec<u8>, &dyn SealCipher); 3] = [
        (
            binding(10, bytes.len(), ArchiveFormat::Tar, 4),
            bytes.clone(),
            &crypto.encrypt,
        ),
        (
            binding(9, bytes.len() - 1, ArchiveFormat::Tar, 4),
            bytes[..bytes.len() - 1].to_vec(),
            &crypto.encrypt,
        ),
        (good.clone(), bytes.clone(), &FailFinalize),
    ];
    for (bound, data, cipher) in cases {
        let mut store = file.open(&crypto);
        let failure =
            seal_capture(capture(&bound, data), cipher, &mut store, budget()).unwrap_err();
        assert_eq!(failure.publication, RecordPublication::NotPublished);
        drop(store);
        assert_eq!(fs::metadata(&file.path).unwrap().len(), 64);
    }
}

#[test]
fn wrong_provenance_key_binding_header_and_tampering_fail_closed() {
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let data = tar(b"synthetic");
    let bound = binding(9, data.len(), ArchiveFormat::Tar, 4);
    let mut store = file.open(&crypto);
    seal_capture(capture(&bound, data), &crypto.encrypt, &mut store, budget()).unwrap();
    drop(store);
    let original = fs::read(&file.path).unwrap();
    assert!(RecordStore::open(&file.path, [1; 16], authenticator(42), &budget()).is_err());
    assert!(RecordStore::open(&file.path, [9; 16], crypto.auth.clone(), &budget()).is_err());
    let mut store = file.open(&crypto);
    let wrong = ArtifactBinding::system(
        [1; 16],
        [5; 16],
        [3; 16],
        [4; 16],
        9,
        bound.source_bytes(),
        ArchiveFormat::Tar,
    )
    .unwrap();
    assert!(restore_inspect(&mut store, &wrong, &crypto.decrypt, budget()).is_err());
    let other = Crypto::new();
    assert!(restore_inspect(&mut store, &bound, &other.decrypt, budget()).is_err());
    drop(store);
    for index in [0, 8, 24, 32, 64, 72, 136, 168, original.len() - 1] {
        let mut damaged = original.clone();
        damaged[index] ^= 1;
        fs::write(&file.path, damaged).unwrap();
        assert!(
            RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget()).is_err(),
            "tamper offset {index}"
        );
    }
    for end in 0..original.len() {
        fs::write(&file.path, &original[..end]).unwrap();
        let opened = RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget());
        if end == 64 {
            assert_eq!(
                opened.unwrap().reconcile(&bound, &budget()),
                RecordPublication::Unknown
            );
        } else {
            assert!(opened.is_err(), "truncation {end}");
        }
    }
}
