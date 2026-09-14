use age::secrecy::ExposeSecret;
mod ciphertext_composition;
mod sealing_composition;
use openwrt_mcp_crypto_age::AgeX25519;
use openwrt_mcp_key_sources::{
    ContainerFormat, FileProtection, ProtectedFileAccess, SourceConfig, SourceRegistry,
};
use openwrt_mcp_runtime::protection::{
    CryptoLimits, DecryptionSession, EncryptionSession, KeyLimits, KeyMaterial, ProtectionError,
};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

// Entirely memory-backed: no real key, Vault, archive, or plaintext file is opened.
struct MemoryFiles {
    public_path: PathBuf,
    private_path: PathBuf,
    public: Vec<u8>,
    archive: Vec<u8>,
    reads: Mutex<Vec<bool>>,
}

impl ProtectedFileAccess for MemoryFiles {
    fn read(
        &self,
        path: &Path,
        protection: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError> {
        if path == self.public_path && protection == FileProtection::Restricted {
            self.reads.lock().unwrap().push(false);
            KeyMaterial::new(self.public.clone(), max_bytes)
        } else if path == self.private_path && protection == FileProtection::PersonalVault {
            self.reads.lock().unwrap().push(true);
            KeyMaterial::new(self.archive.clone(), max_bytes)
        } else {
            Err(ProtectionError::SourceUnavailable)
        }
    }
}

#[test]
fn separate_sources_archive_and_age_compose_without_private_access_during_encryption() {
    let identity = age::x25519::Identity::generate();
    let private = identity.to_string();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "router/identity.txt",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(private.expose_secret().as_bytes()).unwrap();
    zip.start_file(
        "unrelated/key.txt",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.write_all(b"synthetic-unrelated-not-age").unwrap();
    let root = std::env::temp_dir(); // Paths identify memory fixtures; never read.
    let public_path = root.join("openwrt-mcp-not-a-real-recipient-file");
    let private_path = root.join("openwrt-mcp-not-a-real-vault-archive");
    let files = Arc::new(MemoryFiles {
        public_path: public_path.clone(),
        private_path: private_path.clone(),
        public: identity.to_public().to_string().into_bytes(),
        archive: zip.finish().unwrap().into_inner(),
        reads: Mutex::new(Vec::new()),
    });
    let sources = BTreeMap::from([
        (
            "public".into(),
            SourceConfig::RestrictedFile { path: public_path },
        ),
        (
            "vault".into(),
            SourceConfig::PersonalVaultFile { path: private_path },
        ),
        (
            "identity".into(),
            SourceConfig::ArchiveEntry {
                source: "vault".into(),
                format: ContainerFormat::Zip,
                entry: "router/identity.txt".into(),
            },
        ),
    ]);
    let registry =
        SourceRegistry::with_file_access(sources, KeyLimits::default(), files.clone()).unwrap();
    let encryption = EncryptionSession::new(
        registry.resolve("public").unwrap(),
        Arc::new(AgeX25519),
        KeyLimits::default(),
        CryptoLimits::default(),
    )
    .unwrap();
    let decryption = DecryptionSession::new(
        registry.resolve("identity").unwrap(),
        Arc::new(AgeX25519),
        KeyLimits::default(),
        CryptoLimits::default(),
    )
    .unwrap();
    assert!(files.reads.lock().unwrap().is_empty());
    let plaintext = b"synthetic backup bytes, not router configuration";
    let mut ciphertext = Vec::new();
    let report = encryption
        .encrypt(&mut plaintext.as_slice(), &mut ciphertext)
        .unwrap();
    assert_eq!(*files.reads.lock().unwrap(), vec![false]);
    assert_eq!(report.input_bytes, plaintext.len() as u64);
    assert_eq!(report.output_bytes, ciphertext.len() as u64);
    assert!(ciphertext.starts_with(b"age-encryption.org/v1\n"));
    let mut staging = Vec::new();
    let report = decryption
        .decrypt_to_staging(&mut ciphertext.as_slice(), &mut staging)
        .unwrap();
    assert_eq!(*files.reads.lock().unwrap(), vec![false, true]);
    assert!(staging == plaintext);
    assert_eq!(report.input_bytes, ciphertext.len() as u64);
    assert_eq!(report.output_bytes, plaintext.len() as u64);
}
