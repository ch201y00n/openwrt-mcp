use openwrt_mcp_key_sources::{
    ContainerFormat, FileProtection, NativeFileAccess, ProtectedFileAccess, SourceConfig,
    SourceRegistry, ZipContainer,
};
use openwrt_mcp_runtime::protection::{KeyContainer, KeyLimits, KeyMaterial, ProtectionError};
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn zip_data(entries: &[(&str, &[u8])], method: zip::CompressionMethod) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(
                *name,
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn read_zip(
    bytes: Vec<u8>,
    entry: &str,
    limits: KeyLimits,
) -> Result<KeyMaterial, ProtectionError> {
    let material = KeyMaterial::new(bytes, 16 * 1024 * 1024).unwrap();
    ZipContainer.read_entry(&material, entry, &limits)
}

fn offset(bytes: &[u8], signature: &[u8]) -> usize {
    bytes
        .windows(signature.len())
        .position(|v| v == signature)
        .unwrap()
}

fn u16_at(bytes: &mut [u8], position: usize, value: u16) {
    bytes[position..position + 2].copy_from_slice(&value.to_le_bytes());
}
fn u32_at(bytes: &mut [u8], position: usize, value: u32) {
    bytes[position..position + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn stored_and_deflated_exact_selection_without_extracting() {
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let bytes = zip_data(
            &[
                ("keys/first.txt", b"synthetic-first"),
                ("keys/second.txt", b"synthetic-second"),
            ],
            method,
        );
        let output = read_zip(bytes, "keys/second.txt", KeyLimits::default()).unwrap();
        assert_eq!(output.expose_bytes(), b"synthetic-second");
    }
}

#[test]
fn archive_missing_entry_and_unsafe_requested_paths_fail() {
    let bytes = zip_data(&[("key.txt", b"synthetic")], zip::CompressionMethod::Stored);
    assert_eq!(
        read_zip(bytes.clone(), "absent", KeyLimits::default()).unwrap_err(),
        ProtectionError::SourceUnavailable
    );
    for entry in [
        "../key.txt",
        "/key.txt",
        "a\\key.txt",
        "C:key.txt",
        "a//key.txt",
        "key.txt/",
    ] {
        assert_eq!(
            read_zip(bytes.clone(), entry, KeyLimits::default()).unwrap_err(),
            ProtectionError::InvalidConfig
        );
    }
}

#[test]
fn malformed_truncated_encrypted_and_zip64_archives_fail() {
    let bytes = zip_data(&[("key.txt", b"synthetic")], zip::CompressionMethod::Stored);
    assert!(
        read_zip(
            bytes[..bytes.len() - 1].to_vec(),
            "key.txt",
            KeyLimits::default()
        )
        .is_err()
    );
    let central = offset(&bytes, b"PK\x01\x02");
    let mut encrypted = bytes.clone();
    u16_at(&mut encrypted, central + 8, 1);
    assert_eq!(
        read_zip(encrypted, "key.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::UnsupportedContainer
    );
    let mut large = bytes.clone();
    let end = offset(&large, b"PK\x05\x06");
    u16_at(&mut large, end + 10, u16::MAX);
    u16_at(&mut large, end + 8, u16::MAX);
    assert_eq!(
        read_zip(large, "key.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::UnsupportedContainer
    );
}

#[test]
fn alternate_end_record_inside_comment_cannot_change_parser_view() {
    let mut bytes = zip_data(&[("key.txt", b"synthetic")], zip::CompressionMethod::Stored);
    let end = offset(&bytes, b"PK\x05\x06");
    let mut fake = bytes[end..end + 22].to_vec();
    u16_at(&mut fake, 8, 65_534);
    u16_at(&mut fake, 10, 65_534);
    u16_at(&mut bytes, end + 20, 23);
    bytes.extend_from_slice(&fake);
    bytes.push(0);
    assert_eq!(
        read_zip(bytes, "key.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
}

#[test]
fn archive_metadata_count_size_and_actual_output_are_bounded() {
    let bytes = zip_data(
        &[("one.txt", b"one"), ("two.txt", b"two")],
        zip::CompressionMethod::Stored,
    );
    let limits = KeyLimits {
        max_entries: 1,
        ..KeyLimits::default()
    };
    assert_eq!(
        read_zip(bytes.clone(), "one.txt", limits).unwrap_err(),
        ProtectionError::ResourceLimit
    );
    let limits = KeyLimits {
        max_directory_bytes: 1,
        ..KeyLimits::default()
    };
    assert_eq!(
        read_zip(bytes.clone(), "one.txt", limits).unwrap_err(),
        ProtectionError::ResourceLimit
    );
    let limits = KeyLimits {
        max_key_bytes: 2,
        ..KeyLimits::default()
    };
    assert_eq!(
        read_zip(bytes, "one.txt", limits).unwrap_err(),
        ProtectionError::ResourceLimit
    );
    let bomb = zip_data(
        &[("key.txt", &[b'x'; 16_384])],
        zip::CompressionMethod::Deflated,
    );
    let limits = KeyLimits {
        max_expansion_ratio: 2,
        ..KeyLimits::default()
    };
    assert_eq!(
        read_zip(bomb, "key.txt", limits).unwrap_err(),
        ProtectionError::ResourceLimit
    );
}

#[test]
fn dishonest_inflated_size_cannot_bypass_actual_expansion_ratio() {
    let mut bomb = zip_data(
        &[("key.txt", &[b'x'; 16_384])],
        zip::CompressionMethod::Deflated,
    );
    let central = offset(&bomb, b"PK\x01\x02");
    u32_at(&mut bomb, central + 24, 1);
    u32_at(&mut bomb, 22, 1);
    let limits = KeyLimits {
        max_expansion_ratio: 2,
        ..KeyLimits::default()
    };
    assert_eq!(
        read_zip(bomb, "key.txt", limits).unwrap_err(),
        ProtectionError::ResourceLimit
    );
}

#[test]
fn duplicate_traversal_symlink_overlap_and_disagreeing_names_fail() {
    let bytes = zip_data(
        &[("one.txt", b"one"), ("two.txt", b"two")],
        zip::CompressionMethod::Stored,
    );
    let central1 = offset(&bytes, b"PK\x01\x02");
    let central2 = central1 + 4 + offset(&bytes[central1 + 4..], b"PK\x01\x02");
    let mut duplicate = bytes.clone();
    duplicate[central2 + 46..central2 + 53].copy_from_slice(b"one.txt");
    assert_eq!(
        read_zip(duplicate, "one.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
    let mut traversal = bytes.clone();
    traversal[central1 + 46..central1 + 53].copy_from_slice(b"../.txt");
    assert_eq!(
        read_zip(traversal, "one.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
    let mut symlink = bytes.clone();
    u32_at(&mut symlink, central1 + 38, 0o120777 << 16);
    assert_eq!(
        read_zip(symlink, "one.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
    let mut overlap = bytes.clone();
    u32_at(&mut overlap, central2 + 42, 0);
    assert_eq!(
        read_zip(overlap, "one.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
    let mut disagree = bytes;
    disagree[30] = b'X';
    assert_eq!(
        read_zip(disagree, "one.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
}

#[test]
fn crc_corruption_and_nested_archive_are_rejected() {
    let mut bytes = zip_data(&[("key.txt", b"synthetic")], zip::CompressionMethod::Stored);
    let data = offset(&bytes, b"synthetic");
    bytes[data] ^= 1;
    assert_eq!(
        read_zip(bytes, "key.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
    let inner = zip_data(&[("inner", b"synthetic")], zip::CompressionMethod::Stored);
    let outer = zip_data(&[("key.zip", &inner)], zip::CompressionMethod::Stored);
    assert_eq!(
        read_zip(outer, "key.zip", KeyLimits::default()).unwrap_err(),
        ProtectionError::UnsupportedContainer
    );
}

#[test]
fn structurally_valid_local_records_cannot_overlap_payloads() {
    let inner = zip_data(&[("two.txt", b"two")], zip::CompressionMethod::Stored);
    let inner_central = offset(&inner, b"PK\x01\x02");
    let inner_end = offset(&inner, b"PK\x05\x06");
    let outer = zip_data(
        &[("one.txt", &inner[..inner_central])],
        zip::CompressionMethod::Stored,
    );
    let outer_central = offset(&outer, b"PK\x01\x02");
    let outer_end = offset(&outer, b"PK\x05\x06");
    let inner_local_in_outer = 4 + offset(&outer[4..], b"PK\x03\x04");
    let mut second_record = inner[inner_central..inner_end].to_vec();
    u32_at(&mut second_record, 42, inner_local_in_outer as u32);
    let mut overlapped = outer[..outer_end].to_vec();
    overlapped.extend_from_slice(&second_record);
    let new_end = overlapped.len();
    overlapped.extend_from_slice(&outer[outer_end..]);
    u16_at(&mut overlapped, new_end + 8, 2);
    u16_at(&mut overlapped, new_end + 10, 2);
    u32_at(
        &mut overlapped,
        new_end + 12,
        (new_end - outer_central) as u32,
    );
    assert_eq!(
        read_zip(overlapped, "two.txt", KeyLimits::default()).unwrap_err(),
        ProtectionError::InvalidContainer
    );
}

struct FakeFile {
    calls: AtomicUsize,
    result: Result<Vec<u8>, ProtectionError>,
}
impl ProtectedFileAccess for FakeFile {
    fn read(
        &self,
        _: &Path,
        _: FileProtection,
        max_bytes: usize,
    ) -> Result<KeyMaterial, ProtectionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.result {
            Ok(bytes) => KeyMaterial::new(bytes.clone(), max_bytes),
            Err(error) => Err(*error),
        }
    }
}

fn absolute_fixture_path() -> std::path::PathBuf {
    std::env::temp_dir().join("openwrt-mcp-synthetic-not-read")
}

#[test]
fn registry_and_resolution_are_offline_and_locked_vault_never_falls_back() {
    let files = Arc::new(FakeFile {
        calls: AtomicUsize::new(0),
        result: Err(ProtectionError::InteractionRequired),
    });
    let configs = BTreeMap::from([(
        "vault".to_owned(),
        SourceConfig::PersonalVaultFile {
            path: absolute_fixture_path(),
        },
    )]);
    let registry =
        SourceRegistry::with_file_access(configs, KeyLimits::default(), files.clone()).unwrap();
    let source = registry.resolve("vault").unwrap();
    assert_eq!(files.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        source.read(1024).unwrap_err(),
        ProtectionError::InteractionRequired
    );
    assert_eq!(files.calls.load(Ordering::SeqCst), 1);
    assert!(registry.resolve("unknown").is_err());
}

#[test]
fn archive_decorator_selects_only_configured_member() {
    let files = Arc::new(FakeFile {
        calls: AtomicUsize::new(0),
        result: Ok(zip_data(
            &[("keys/selected", b"synthetic")],
            zip::CompressionMethod::Stored,
        )),
    });
    let configs = BTreeMap::from([
        (
            "vault".to_owned(),
            SourceConfig::PersonalVaultFile {
                path: absolute_fixture_path(),
            },
        ),
        (
            "key".to_owned(),
            SourceConfig::ArchiveEntry {
                source: "vault".to_owned(),
                format: ContainerFormat::Zip,
                entry: "keys/selected".to_owned(),
            },
        ),
    ]);
    let registry =
        SourceRegistry::with_file_access(configs, KeyLimits::default(), files.clone()).unwrap();
    assert_eq!(
        registry
            .resolve("key")
            .unwrap()
            .read(1024)
            .unwrap()
            .expose_bytes(),
        b"synthetic"
    );
    assert_eq!(files.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn purpose_resolution_bounds_direct_files_and_archive_entries_without_limiting_containers() {
    for archived in [false, true] {
        for bytes in [b"four".as_slice(), b"large".as_slice()] {
            let files = Arc::new(FakeFile {
                calls: AtomicUsize::new(0),
                result: Ok(if archived {
                    zip_data(&[("key", bytes)], zip::CompressionMethod::Stored)
                } else {
                    bytes.to_vec()
                }),
            });
            let mut configs = BTreeMap::from([(
                "file".to_owned(),
                SourceConfig::RestrictedFile {
                    path: absolute_fixture_path(),
                },
            )]);
            let alias = if archived {
                configs.insert(
                    "entry".to_owned(),
                    SourceConfig::ArchiveEntry {
                        source: "file".to_owned(),
                        format: ContainerFormat::Zip,
                        entry: "key".to_owned(),
                    },
                );
                "entry"
            } else {
                "file"
            };
            let registry = SourceRegistry::with_file_access(
                configs,
                KeyLimits {
                    max_key_bytes: 4,
                    ..KeyLimits::default()
                },
                files.clone(),
            )
            .unwrap();
            let source = registry.resolve_key(alias).unwrap();
            assert_eq!(files.calls.load(Ordering::SeqCst), 0);
            assert_eq!(source.read(0).unwrap_err(), ProtectionError::ResourceLimit);
            assert_eq!(files.calls.load(Ordering::SeqCst), 0);
            assert!(matches!(
                registry.resolve_key("unknown"),
                Err(ProtectionError::InvalidConfig)
            ));
            if bytes.len() == 4 {
                assert_eq!(source.read(1024).unwrap().expose_bytes(), bytes);
            } else {
                assert_eq!(
                    source.read(1024).unwrap_err(),
                    ProtectionError::ResourceLimit
                );
            }
            assert_eq!(source.read(3).unwrap_err(), ProtectionError::ResourceLimit);
            // Raw sources may carry an entire container, unlike purpose-bound keys.
            let raw = registry.resolve("file").unwrap().read(1024).unwrap();
            if archived {
                assert!(raw.expose_bytes().len() > 4);
            } else {
                assert_eq!(raw.expose_bytes(), bytes);
            }
        }
    }
}

#[test]
fn container_format_selection_is_explicit_and_unknown_formats_fail() {
    use serde::Deserialize;
    use serde::de::value::{Error, StrDeserializer};
    let approved = ContainerFormat::deserialize(StrDeserializer::<Error>::new("zip")).unwrap();
    assert!(matches!(approved, ContainerFormat::Zip));
    assert!(ContainerFormat::deserialize(StrDeserializer::<Error>::new("tar")).is_err());
}

#[test]
fn missing_alias_cycles_nested_containers_and_bad_environment_names_fail_offline() {
    for configs in [
        BTreeMap::from([(
            "a".to_owned(),
            SourceConfig::ArchiveEntry {
                source: "missing".to_owned(),
                format: ContainerFormat::Zip,
                entry: "key".to_owned(),
            },
        )]),
        BTreeMap::from([(
            "a".to_owned(),
            SourceConfig::ArchiveEntry {
                source: "a".to_owned(),
                format: ContainerFormat::Zip,
                entry: "key".to_owned(),
            },
        )]),
        BTreeMap::from([(
            "a".to_owned(),
            SourceConfig::Environment {
                variable: "bad-name".to_owned(),
            },
        )]),
        BTreeMap::from([(
            "a".to_owned(),
            SourceConfig::RestrictedFile {
                path: "relative.key".into(),
            },
        )]),
    ] {
        assert!(SourceRegistry::new(configs, KeyLimits::default()).is_err());
    }
}

#[test]
fn native_vault_protection_is_explicitly_unsupported() {
    assert_eq!(
        NativeFileAccess
            .read(
                &absolute_fixture_path(),
                FileProtection::PersonalVault,
                1024
            )
            .unwrap_err(),
        ProtectionError::UnsupportedProtection
    );
}

#[test]
fn environment_source_child() {
    if std::env::var_os("OPENWRT_MCP_SYNTHETIC_CHILD").is_none() {
        return;
    }
    let registry = SourceRegistry::new(
        BTreeMap::from([(
            "env".to_owned(),
            SourceConfig::Environment {
                variable: "OPENWRT_MCP_SYNTHETIC_KEY".to_owned(),
            },
        )]),
        KeyLimits::default(),
    )
    .unwrap();
    let source = registry.resolve("env").unwrap();
    assert_eq!(source.read(1024).unwrap().expose_bytes(), b"synthetic-only");
    assert_eq!(source.read(1).unwrap_err(), ProtectionError::ResourceLimit);
    for max_key_bytes in [b"synthetic-only".len(), b"synthetic-only".len() - 1] {
        let purpose_registry = SourceRegistry::new(
            BTreeMap::from([(
                "env".to_owned(),
                SourceConfig::Environment {
                    variable: "OPENWRT_MCP_SYNTHETIC_KEY".to_owned(),
                },
            )]),
            KeyLimits {
                max_key_bytes,
                ..KeyLimits::default()
            },
        )
        .unwrap();
        let purpose = purpose_registry.resolve_key("env").unwrap();
        assert_eq!(purpose.read(0).unwrap_err(), ProtectionError::ResourceLimit);
        assert_eq!(purpose.read(1).unwrap_err(), ProtectionError::ResourceLimit);
        if max_key_bytes == b"synthetic-only".len() {
            assert_eq!(
                purpose.read(1024).unwrap().expose_bytes(),
                b"synthetic-only"
            );
        } else {
            assert_eq!(
                purpose.read(1024).unwrap_err(),
                ProtectionError::ResourceLimit
            );
        }
    }
    let missing = SourceRegistry::new(
        BTreeMap::from([(
            "missing".to_owned(),
            SourceConfig::Environment {
                variable: "OPENWRT_MCP_SYNTHETIC_MISSING".to_owned(),
            },
        )]),
        KeyLimits::default(),
    )
    .unwrap();
    assert_eq!(
        missing.resolve("missing").unwrap().read(1024).unwrap_err(),
        ProtectionError::SourceUnavailable
    );
}

#[test]
fn environment_lookup_uses_injected_child_environment_without_global_mutation() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "environment_source_child"])
        .env_clear()
        .env("OPENWRT_MCP_SYNTHETIC_CHILD", "1")
        .env("OPENWRT_MCP_SYNTHETIC_KEY", "synthetic-only")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "synthetic environment child failed"
    );
}
