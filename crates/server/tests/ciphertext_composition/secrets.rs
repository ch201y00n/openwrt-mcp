use super::*;
use openwrt_mcp_key_sources::{
    ContainerFormat, FileProtection, ProtectedFileAccess, SourceConfig, SourceRegistry,
    secrets::ServiceSecrets,
};
use openwrt_mcp_runtime::mutation_ports::{
    MutationError, SecretPurpose, SecretSource, secrets::SecretReference,
};
use std::{collections::BTreeMap, io::Cursor, path::Path};

struct Files {
    zip: Vec<u8>,
    reads: AtomicU64,
}
impl ProtectedFileAccess for Files {
    fn read(
        &self,
        _: &Path,
        _: FileProtection,
        maximum: usize,
    ) -> Result<KeyMaterial, ProtectionError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        KeyMaterial::new(self.zip.clone(), maximum)
    }
}
#[tokio::test]
async fn purpose_specific_service_secret_reuses_zip_source_without_extraction_or_eager_reads() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        "keys/service",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    zip.write_all(b"synthetic-service-value").unwrap();
    let files = Arc::new(Files {
        zip: zip.finish().unwrap().into_inner(),
        reads: AtomicU64::new(0),
    });
    let configs = BTreeMap::from([
        (
            "archive".into(),
            SourceConfig::RestrictedFile {
                path: std::env::temp_dir().join("not-a-real-p3-key-file"),
            },
        ),
        (
            "entry".into(),
            SourceConfig::ArchiveEntry {
                source: "archive".into(),
                format: ContainerFormat::Zip,
                entry: "keys/service".into(),
            },
        ),
    ]);
    let registry =
        SourceRegistry::with_file_access(configs, KeyLimits::default(), files.clone()).unwrap();
    let reference = || SecretReference::new("service".into()).unwrap();
    let resolver = ServiceSecrets::new(
        &registry,
        vec![(reference(), SecretPurpose::ServiceToken, "entry".into())],
    )
    .unwrap();
    assert_eq!(files.reads.load(Ordering::SeqCst), 0);
    assert_eq!(
        resolver
            .resolve(&reference(), SecretPurpose::WirelessPassword, budget())
            .await
            .err(),
        Some(MutationError::Invalid)
    );
    assert_eq!(files.reads.load(Ordering::SeqCst), 0);
    let value = resolver
        .resolve(&reference(), SecretPurpose::ServiceToken, budget())
        .await
        .unwrap();
    assert!(value.expose_for(SecretPurpose::ServiceToken).unwrap() == b"synthetic-service-value");
    assert!(
        value
            .expose_for(SecretPurpose::WireGuardPrivateKey)
            .is_err()
    );
    assert_eq!(files.reads.load(Ordering::SeqCst), 1);
    let cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        resolver
            .resolve(&reference(), SecretPurpose::ServiceToken, cancelled)
            .await
            .err(),
        Some(MutationError::Cancelled)
    );
    assert_eq!(files.reads.load(Ordering::SeqCst), 1);
    assert!(
        ServiceSecrets::new(
            &registry,
            vec![
                (reference(), SecretPurpose::ServiceToken, "entry".into()),
                (reference(), SecretPurpose::ServiceToken, "entry".into())
            ]
        )
        .is_err()
    );
    assert!(SecretReference::new("../path".into()).is_err());
}

#[test]
fn explicit_environment_secret_uses_isolated_child_process_not_global_mutation() {
    const FLAG: &str = "OPENWRT_MCP_P3_TEST_CHILD";
    const VALUE: &str = "OPENWRT_MCP_P3_TEST_SYNTHETIC_SECRET";
    if std::env::var_os(FLAG).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","ciphertext_composition::secrets::explicit_environment_secret_uses_isolated_child_process_not_global_mutation"])
            .env(FLAG,"1").env(VALUE,"synthetic-only-env-value").output().unwrap();
        assert!(
            output.status.success(),
            "synthetic environment child failed"
        );
        for stream in [&output.stdout, &output.stderr] {
            assert!(!stream.windows(24).any(|w| w == b"synthetic-only-env-value"));
        }
        return;
    }
    let registry = SourceRegistry::new(
        BTreeMap::from([(
            "source".into(),
            SourceConfig::Environment {
                variable: VALUE.into(),
            },
        )]),
        KeyLimits::default(),
    )
    .unwrap();
    let reference = SecretReference::new("service".into()).unwrap();
    let resolver = ServiceSecrets::new(
        &registry,
        vec![(
            SecretReference::new("service".into()).unwrap(),
            SecretPurpose::ServiceToken,
            "source".into(),
        )],
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let value = runtime
        .block_on(resolver.resolve(&reference, SecretPurpose::ServiceToken, budget()))
        .unwrap();
    assert!(value.expose_for(SecretPurpose::ServiceToken).unwrap() == b"synthetic-only-env-value");
}
