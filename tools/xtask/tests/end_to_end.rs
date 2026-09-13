//! Real Cargo/Git fixtures exercise the assembled gate, not only its helpers.
//! Source snippets are inspected, never compiled or executed against a device.
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn cargo_roots_resolve_test_and_example_modules_without_exempting_production_descendants() {
    let fixture = Fixture::new();
    fixture.write(
        "crates/pure/tests/read_rows.rs",
        "mod supplied; #[test] fn rows() { supplied::check(); }\n",
    );
    fixture.write("crates/pure/tests/supplied/mod.rs", "pub fn check() {}\n");
    fixture.write(
        "crates/pure/examples/inspect.rs",
        "mod data; fn main() { data::check(); }\n",
    );
    fixture.write("crates/pure/examples/data/mod.rs", "pub fn check() {}\n");
    fixture.write("crates/pure/src/lib.rs", "mod nested;\n");
    fixture.write("crates/pure/src/nested.rs", "mod leaf;\n");
    fixture.write("crates/pure/src/nested/leaf.rs", "fn harmless() {}\n");
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/pure/src/nested/leaf.rs",
        "fn forbidden() { std::process::Command::new(\"fixture\"); }\n",
    );
    fixture.denied("forbidden API");
}

#[test]
fn cargo_root_resolution_rejects_wrong_missing_and_ambiguous_module_locations() {
    let wrong = Fixture::new();
    wrong.write("crates/pure/tests/read_rows.rs", "mod supplied;\n");
    wrong.write(
        "crates/pure/tests/read_rows/supplied.rs",
        "fn fixture() {}\n",
    );
    wrong.denied("external module must resolve to exactly one owned Rust file");
    let missing = Fixture::new();
    missing.write("crates/pure/tests/read_rows.rs", "mod supplied;\n");
    missing.denied("external module must resolve to exactly one owned Rust file");
    let ambiguous = Fixture::new();
    ambiguous.write("crates/pure/tests/read_rows.rs", "mod supplied;\n");
    ambiguous.write("crates/pure/tests/supplied.rs", "fn fixture() {}\n");
    ambiguous.write("crates/pure/tests/supplied/mod.rs", "fn fixture() {}\n");
    ambiguous.denied("external module must resolve to exactly one owned Rust file");
    let fake_root = Fixture::new();
    fake_root.write("crates/pure/src/nested/main.rs", "mod leaf;\n");
    fake_root.write("crates/pure/src/nested/leaf.rs", "fn fixture() {}\n");
    fake_root.denied("external module must resolve to exactly one owned Rust file");
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "openwrt-mcp-architecture-fixture-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create unique synthetic fixture directory");
        let fixture = Self { root };
        fixture.write(
            "Cargo.toml",
            "[workspace]\nresolver='3'\nmembers=['crates/pure','crates/tokio']\n",
        );
        fixture.write("Cargo.lock", "version = 4\n[[package]]\nname = 'pure'\nversion = '0.1.0'\ndependencies = ['tokio']\n[[package]]\nname = 'tokio'\nversion = '0.1.0'\n");
        fixture.write("crates/pure/Cargo.toml", "[package]\nname='pure'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nhidden={package='tokio',path='../tokio'}\n");
        fixture.write("crates/pure/src/lib.rs", "pub fn harmless() {}\n");
        fixture.write(
            "crates/tokio/Cargo.toml",
            "[package]\nname='tokio'\nversion='0.1.0'\nedition='2024'\n",
        );
        fixture.write("crates/tokio/src/lib.rs", "pub mod process { pub struct Command; impl Command { pub fn new(_: &str) -> Self { Self } } }\n");
        fixture.write("architecture/spec.toml", "version=1\ndecision='docs/adr/0001.md'\nrequirements='docs/requirements.md'\narchitecture='docs/architecture.md'\n[[crates]]\nname='pure'\npath='crates/pure'\nlayer='domain'\ndependencies=['tokio']\ndev_dependencies=[]\nbuild_dependencies=[]\nforbidden_paths=['std::fs','std::process','tokio::process']\nallowed_macros=['format']\n[[crates]]\nname='tokio'\npath='crates/tokio'\nlayer='infrastructure'\ndependencies=[]\ndev_dependencies=[]\nbuild_dependencies=[]\nforbidden_paths=[]\nallowed_macros=[]\n");
        fixture.write("docs/adr/0001.md", "Synthetic reviewed architecture.\n");
        fixture.write("docs/requirements.md", "Synthetic requirements.\n");
        fixture.write("docs/architecture.md", "Synthetic architecture.\n");
        fixture.git(&["init", "--quiet"]);
        fixture.git(&["add", "."]);
        fixture.git(&[
            "-c",
            "user.name=Architecture Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "synthetic baseline",
        ]);
        fixture
    }

    fn write(&self, path: &str, contents: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn git(&self, arguments: &[&str]) {
        let output = Command::new("git")
            // Synthetic commits must not run a contributor's hooks or signing agent.
            .arg("-c")
            .arg(format!(
                "core.hooksPath={}",
                self.root.join(".git/no-fixture-hooks").display()
            ))
            .args(["-c", "commit.gpgSign=false", "-c", "tag.gpgSign=false"])
            .args(arguments)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "synthetic Git fixture setup failed"
        );
    }

    fn denied(&self, expected: &str) {
        let error = xtask::architecture(&self.root, None)
            .expect_err("the assembled gate accepted an architecture violation");
        assert!(
            error.contains(expected),
            "wrong failure: {error}; expected {expected}"
        );
    }

    fn portability_checkpoint(&self) {
        let original = fs::read_to_string(self.root.join("architecture/spec.toml")).unwrap();
        let contract = original.replace("version=1", "version=4").replace("0001.md", "0004.md")
            .replacen("[[crates]]", "required_hosts=['windows','linux','macos']\nportable_crates=['pure']\nrequired_portable_tests=['crates/pure/tests/portable.rs']\nnative_ci='.github/workflows/ci.yml'\n[[crates]]", 1);
        self.write("architecture/spec.toml", &contract);
        self.write(
            "docs/adr/0004.md",
            "Synthetic native-host architecture checkpoint.\n",
        );
        self.write(
            "crates/pure/tests/portable.rs",
            "#[test] fn fixture() { assert_eq!(1,1); }\n",
        );
        let workflow = serde_json::json!({
            "on":{"push":null,"pull_request":null},
            "jobs":{"rust":{
                "runs-on":"${{ matrix.os }}",
                "strategy":{"fail-fast":false,"matrix":{"os":["ubuntu-latest","windows-latest","macos-latest"]}},
                "steps":[
                    {"shell":"pwsh","run":"./tools/Test-Repository.ps1 -BaseRef $env:ARCHITECTURE_BASE",
                     "env":{"ARCHITECTURE_BASE":"${{ github.event.pull_request.base.sha || github.event.before }}"}},
                    {"shell":"pwsh","run":"cargo test --locked -p pure --test portable"}
                ]
            }}
        });
        self.write(".github/workflows/ci.yml", &workflow.to_string());
        self.git(&["add", "."]);
        self.git(&[
            "-c",
            "user.name=Architecture Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "synthetic portability checkpoint",
        ]);
    }

    fn capability_checkpoint(&self, version: u64) {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut spec: toml::Value =
            toml::from_str(&fs::read_to_string(repository.join("architecture/spec.toml")).unwrap())
                .unwrap();
        assert!(matches!(
            version,
            5 | 6 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19
        ));
        spec["version"] = (version as i64).into();
        if version < 19 {
            spec.as_table_mut().unwrap().remove("windows_log_contract");
            spec["decision"] = "docs/adr/0018-validated-archive-sealing.md".into();
        }
        if version < 18 {
            spec.as_table_mut()
                .unwrap()
                .remove("archive_sealing_contract");
            spec["decision"] = "docs/adr/0017-bounded-gzip-archive-validation.md".into();
            for rule in spec["crates"].as_array_mut().unwrap() {
                rule["forbidden_paths"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|p| p.as_str() != Some("openwrt_mcp_runtime::sealing"));
                if rule["name"].as_str() == Some("openwrt-mcp") {
                    rule["dev_dependencies"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|d| d.as_str() != Some("openwrt-mcp-device-codec"));
                }
            }
            spec["required_portable_tests"]
                .as_array_mut()
                .unwrap()
                .retain(|p| p.as_str() != Some("crates/server/tests/protection_composition.rs"));
        }
        if version < 17 {
            spec.as_table_mut().unwrap().remove("gzip_archive_contract");
            spec["backup_archive_contract"]["consumers"] =
                "none_until_separate_integration_checkpoint".into();
            spec["decision"] = "docs/adr/0016-bounded-backup-archive-validation.md".into();
            for rule in spec["crates"].as_array_mut().unwrap() {
                rule["forbidden_paths"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|p| p.as_str() != Some("openwrt_mcp_device_codec::gzip"));
                if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
                    rule["dependencies"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|d| d.as_str() != Some("flate2"));
                }
            }
        }
        if version < 16 {
            spec.as_table_mut()
                .unwrap()
                .remove("backup_archive_contract");
            spec["decision"] = "docs/adr/0015-protected-resource-effects.md".into();
            for rule in spec["crates"].as_array_mut().unwrap() {
                rule["forbidden_paths"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|p| p.as_str() != Some("openwrt_mcp_device_codec::archive"));
                if rule["name"].as_str() == Some("openwrt-mcp-device-codec") {
                    rule["dependencies"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|d| d.as_str() != Some("zeroize"));
                }
            }
        }
        if version < 15 {
            spec.as_table_mut()
                .unwrap()
                .remove("management_effect_contract");
            spec["decision"] = "docs/adr/0014-opkg-status-observations.md".into();
            spec["required_portable_tests"]
                .as_array_mut()
                .unwrap()
                .retain(|suite| suite.as_str() != Some("crates/core/tests/security.rs"));
            for rule in spec["crates"].as_array_mut().unwrap() {
                rule["forbidden_paths"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|path| path.as_str() != Some("openwrt_mcp_core::management"));
            }
        }
        if version < 14 {
            spec.as_table_mut().unwrap().remove("opkg_status_contract");
            spec["decision"] = "docs/adr/0013-base-uci-families.md".into();
        }
        if version < 13 {
            spec["uci_read_contract"]["profiles"] = toml::Value::Array(
                [
                    "system:system",
                    "network:interface",
                    "wireless:wifi-device",
                    "firewall:defaults",
                    "dhcp:dnsmasq",
                    "fstab:mount",
                ]
                .into_iter()
                .map(Into::into)
                .collect(),
            );
            spec["decision"] = "docs/adr/0012-bounded-text-options.md".into();
        }
        if version < 12 {
            spec["uci_read_contract"]
                .as_table_mut()
                .unwrap()
                .remove("option_collections");
            let projection = spec["projection_contract"].as_table_mut().unwrap();
            for field in [
                "text_options",
                "text_option_output",
                "text_option_budget",
                "max_text_option_items",
            ] {
                projection.remove(field);
            }
            projection.insert("profile".into(), "typed_collections_v3".into());
            projection["node_forms"]
                .as_array_mut()
                .unwrap()
                .retain(|f| f.as_str() != Some("TextOption"));
            spec["decision"] = "docs/adr/0011-closed-uci-observations.md".into();
        }
        if version < 11 {
            spec.as_table_mut().unwrap().remove("uci_read_contract");
            spec["projection_contract"]
                .as_table_mut()
                .unwrap()
                .remove("text_enums");
            spec["projection_contract"]
                .as_table_mut()
                .unwrap()
                .remove("max_text_enum_values");
            spec["projection_contract"]["profile"] = "typed_collections_v2".into();
            spec["capability_contract"]["probe_profile"] = "base_luci_v2".into();
            spec["decision"] = "docs/adr/0010-typed-observation-rows.md".into();
            spec["required_portable_tests"]
                .as_array_mut()
                .unwrap()
                .retain(|suite| {
                    !matches!(
                        suite.as_str(),
                        Some(
                            "crates/core/tests/capabilities.rs"
                                | "crates/mcp/tests/read_contracts.rs"
                        )
                    )
                });
        }
        if version < 10 {
            let projection = spec["projection_contract"].as_table_mut().unwrap();
            for field in [
                "observation_rows",
                "scalar_union",
                "root_guards",
                "max_root_guards",
            ] {
                projection.remove(field);
            }
            projection.insert("profile".into(), "typed_collections_v1".into());
            projection["node_forms"]
                .as_array_mut()
                .unwrap()
                .retain(|form| form.as_str() != Some("RowArray"));
            spec["decision"] = "docs/adr/0009-reviewed-luci-observations.md".into();
        }
        if version < 9 {
            spec["capability_contract"]
                .as_table_mut()
                .unwrap()
                .remove("probe_profile");
            spec["decision"] = "docs/adr/0008-windows-protected-reads.md".into();
        }
        if version < 8 {
            spec.as_table_mut().unwrap().remove("windows_read_contract");
            if let Some(package) = spec.as_table_mut().unwrap().remove("package_contract") {
                let suites = package["required_tests"].as_array().unwrap();
                spec["required_portable_tests"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|suite| !suites.contains(suite));
            }
            spec["decision"] = if version == 5 {
                "docs/adr/0005-capability-observations.md"
            } else {
                "docs/adr/0006-bounded-read-projections.md"
            }
            .into();
        }
        if version == 5 {
            // Keep the v5 regression on its own contract rather than silently
            // inheriting every future architecture field from the working tree.
            spec["version"] = 5.into();
            spec["decision"] = "docs/adr/0005-capability-observations.md".into();
            for field in [
                "projection_contract",
                "action_response_contract",
                "mcp_result_contract",
            ] {
                if let Some(contract) = spec.as_table_mut().unwrap().remove(field) {
                    let suites = contract["required_tests"].as_array().unwrap();
                    spec["required_portable_tests"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|suite| !suites.contains(suite));
                }
            }
        }
        let original: toml::Value =
            toml::from_str(&fs::read_to_string(self.root.join("architecture/spec.toml")).unwrap())
                .unwrap();
        let rules = spec["crates"].as_array().unwrap().clone();
        let mut members = vec!["crates/pure".to_owned(), "crates/tokio".to_owned()];
        let mut lock = "version = 4\n[[package]]\nname = 'pure'\nversion = '0.1.0'\ndependencies = ['tokio']\n[[package]]\nname = 'tokio'\nversion = '0.1.0'\n".to_owned();
        for rule in rules {
            let path = rule["path"].as_str().unwrap();
            let name = rule["name"].as_str().unwrap();
            members.push(path.into());
            // Real Cargo metadata from inert standalone fixture packages; no production
            // code, downloaded dependencies, or device commands are used by this fixture.
            let lint = if version >= 8 {
                if name == "openwrt-mcp-host-platform" {
                    "[lints.rust]\nunsafe_code='deny'\n"
                } else {
                    "[lints]\nworkspace=true\n"
                }
            } else {
                ""
            };
            self.write(
                &format!("{path}/Cargo.toml"),
                &format!("[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n{lint}"),
            );
            self.write(
                &format!("{path}/src/lib.rs"),
                "//! Inert architecture fixture.\n",
            );
            lock.push_str(&format!("\n[[package]]\nname='{name}'\nversion='0.1.0'\n"));
        }
        spec["crates"].as_array_mut().unwrap().extend(
            original["crates"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|rule| matches!(rule["name"].as_str(), Some("pure" | "tokio")))
                .cloned(),
        );
        spec["portable_crates"]
            .as_array_mut()
            .unwrap()
            .push("pure".into());
        self.write("architecture/spec.toml", &toml::to_string(&spec).unwrap());
        self.write(
            "Cargo.toml",
            &format!(
                "[workspace]\nresolver='3'\nmembers={}\n[workspace.lints.rust]\nunsafe_code='forbid'\n",
                toml::Value::Array(members.into_iter().map(Into::into).collect())
            ),
        );
        self.write("Cargo.lock", &lock);
        if version >= 8 {
            for path in ["crates/pure/Cargo.toml", "crates/tokio/Cargo.toml"] {
                let original = fs::read_to_string(self.root.join(path)).unwrap();
                self.write(path, &format!("{original}\n[lints]\nworkspace=true\n"));
            }
            for file in [
                "crates/host-platform/src/windows/native.rs",
                "crates/host-platform/src/windows/policy.rs",
            ] {
                self.write(file, "//! Inert native boundary fixture.\n");
            }
            self.write(
                "crates/host-platform/tests/windows.rs",
                "#![cfg(target_os = \"windows\")]\n#[test] fn synthetic() { assert_eq!(1, 1); }\n",
            );
        }
        for path in [
            "architecture/capability-probes.toml",
            "compatibility/evidence.toml",
            ".github/workflows/ci.yml",
        ] {
            self.write(path, &fs::read_to_string(repository.join(path)).unwrap());
        }
        if version < 11 {
            let path = "architecture/capability-probes.toml";
            let mut registry: toml::Value =
                toml::from_str(&fs::read_to_string(self.root.join(path)).unwrap()).unwrap();
            registry["schema_version"] = 2.into();
            registry["probes"]
                .as_array_mut()
                .unwrap()
                .retain(|probe| probe["object"].as_str() != Some("uci"));
            self.write(path, &toml::to_string(&registry).unwrap());
        }
        if version < 9 {
            let path = "architecture/capability-probes.toml";
            let mut registry: toml::Value =
                toml::from_str(&fs::read_to_string(self.root.join(path)).unwrap()).unwrap();
            registry["schema_version"] = 1.into();
            registry["probes"]
                .as_array_mut()
                .unwrap()
                .retain(|probe| !matches!(probe["object"].as_str(), Some("luci" | "luci-rpc")));
            self.write(path, &toml::to_string(&registry).unwrap());
        }
        let evidence: toml::Value = toml::from_str(
            &fs::read_to_string(self.root.join("compatibility/evidence.toml")).unwrap(),
        )
        .unwrap();
        let mut artifacts = BTreeSet::from([spec["decision"].as_str().unwrap()]);
        for (collection, field) in [("targets", "sources"), ("records", "artifacts")] {
            for entry in evidence[collection].as_array().unwrap() {
                for path in entry[field].as_array().unwrap() {
                    artifacts.insert(path.as_str().unwrap());
                }
            }
        }
        for path in artifacts {
            // Seed only declared paths with inert text, never copy device evidence
            // or let an edited manifest write outside this isolated fixture.
            assert!(
                !path.is_empty()
                    && !path.contains(['\\', ':'])
                    && Path::new(path)
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                "fixture artifact must be a portable repository-relative path"
            );
            self.write(
                path,
                "Synthetic architecture/evidence fixture, not device acceptance.\n",
            );
        }
        for suite in spec["required_portable_tests"].as_array().unwrap() {
            self.write(
                suite.as_str().unwrap(),
                "#[test] fn synthetic_contract_fixture() { assert_eq!(2 + 2, 4); }\n",
            );
        }
        self.git(&["add", "."]);
        self.git(&[
            "-c",
            "user.name=Architecture Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "synthetic capability checkpoint",
        ]);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Delete only the exact, uniquely created synthetic test directory.
        let temp = std::env::temp_dir();
        if self.root.parent() == Some(temp.as_path())
            && self
                .root
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.starts_with("openwrt-mcp-architecture-fixture-"))
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn actual_cargo_rename_cannot_hide_io_from_the_assembled_gate() {
    let fixture = Fixture::new();
    xtask::architecture(&fixture.root, Some("HEAD")).unwrap();
    fixture.write(
        "crates/pure/src/lib.rs",
        "pub fn attempt() { hidden::process::Command::new(\"synthetic\"); }\n",
    );
    fixture.denied("forbidden API tokio::process");
}

#[test]
fn production_targets_cannot_borrow_test_or_example_exemptions() {
    for (kind, directory) in [
        ("lib", "tests"),
        ("lib", "examples"),
        ("bin", "tests"),
        ("bin", "examples"),
    ] {
        let fixture = Fixture::new();
        let target = if kind == "lib" {
            "[lib]"
        } else {
            "[[bin]]\nname='escape'"
        };
        fixture.write("crates/pure/Cargo.toml", &format!("[package]\nname='pure'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nhidden={{package='tokio',path='../tokio'}}\n{target}\npath='{directory}/escape.rs'\n"));
        fixture.write(
            &format!("crates/pure/{directory}/escape.rs"),
            "fn main() { std::fs::read(\"synthetic\"); }\n",
        );
        fixture.denied("standard src/ directory");
    }
}

#[test]
fn recursive_inventory_covers_inactive_nested_and_target_named_modules() {
    for module in ["nested", "target"] {
        let fixture = Fixture::new();
        fixture.write(
            "crates/pure/src/lib.rs",
            &format!("#[cfg(target_os=\"not_this_host\")] mod {module};\n"),
        );
        fixture.write(&format!("crates/pure/src/{module}.rs"), "mod deeper;\n");
        fixture.write(
            &format!("crates/pure/src/{module}/deeper.rs"),
            "fn attempt() { r#std::r#fs::read(\"synthetic\"); }\n",
        );
        fixture.denied("forbidden API std::fs");
    }
}

#[test]
fn actual_macro_and_attribute_expansion_inputs_cannot_hide_io() {
    let fixture = Fixture::new();
    for source in [
        "pub fn attempt() { format!(\"{:?}\", { use std::*; fs::read(\"synthetic\") }); }",
        "pub fn attempt() { format!(\"{:?}\", { extern crate std as os; os::fs::read(\"synthetic\") }); }",
        "#[derive(Debug,thiserror::Error)] #[error(\"{}\", r#std::r#fs::read_to_string(\"synthetic\").unwrap())] pub struct Failure;",
        "#[derive(serde::Deserialize)] #[serde(default=\"std::process::abort\")] pub struct Failure;",
    ] {
        fixture.write("crates/pure/src/lib.rs", source);
        let error = xtask::architecture(&fixture.root, None).unwrap_err();
        assert!(
            error.contains("imports inside macro") || error.contains("forbidden API"),
            "wrong failure: {error}"
        );
    }
}

#[test]
fn actual_unowned_sources_and_modules_are_rejected() {
    let fixture = Fixture::new();
    fixture.write("unowned/source.rs", "fn harmless() {}\n");
    fixture.denied("unowned Rust source");
    fs::remove_file(fixture.root.join("unowned/source.rs")).unwrap();
    fixture.write(
        "crates/pure/src/lib.rs",
        "#[path=\"../../tokio/src/lib.rs\"] mod escaped;\n",
    );
    fixture.denied("module remapping");
}

#[test]
fn actual_git_baseline_enforces_architecture_evolution_evidence() {
    let fixture = Fixture::new();
    let path = fixture.root.join("architecture/spec.toml");
    let original = fs::read_to_string(&path).unwrap();
    fixture.write(
        "architecture/spec.toml",
        &original.replace("std::fs", "std::io"),
    );
    fixture.denied("increased contract version");
    fixture.write(
        "architecture/spec.toml",
        &original.replace("version=1", "version=2"),
    );
    fixture.denied("new ADR path");
    fixture.write(
        "architecture/spec.toml",
        &original
            .replace("version=1", "version=2")
            .replace("0001.md", "0002.md"),
    );
    fixture.write("docs/adr/0002.md", "Synthetic revised requirement.\n");
    fixture.denied("changed evidence");
    fixture.write("docs/requirements.md", "Synthetic extended requirements.\n");
    fixture.write("docs/architecture.md", "Synthetic extended architecture.\n");
    fixture.denied("harness regression test change");
}

#[test]
fn no_test_fixture_points_into_the_real_repository() {
    let fixture = Fixture::new();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    assert!(!fixture.root.canonicalize().unwrap().starts_with(repo));
}

#[test]
fn forbidden_consumer_namespaces_cannot_escape_through_producer_root_aliases() {
    let fixture = Fixture::new();
    xtask::architecture(&fixture.root, None).unwrap();
    for source in [
        "pub mod process { pub struct Command; } pub use process::Command;",
        "pub mod process { pub struct Command; } use process::Command as Hidden; pub type Exposed = Hidden;",
        "pub mod process { pub struct Command; } pub fn expose() -> process::Command { process::Command }",
    ] {
        fixture.write("crates/tokio/src/lib.rs", source);
        fixture.denied("public surface flattens protected namespace");
    }
}

#[test]
fn assembled_v4_gate_rejects_os_leaks_and_hidden_required_tests() {
    let fixture = Fixture::new();
    fixture.portability_checkpoint();
    xtask::architecture(&fixture.root, Some("HEAD")).unwrap();
    fixture.write(
        "crates/pure/src/lib.rs",
        "#[cfg(windows)] pub fn harmless() {}\n",
    );
    fixture.denied("portable production cannot branch");
    fixture.write("crates/pure/src/lib.rs", "pub fn harmless() {}\n");
    for source in [
        "#![cfg(unix)] #[test] fn fixture() { assert_eq!(1,1); }",
        "#[test] #[ignore] fn fixture() { assert_eq!(1,1); }",
        "#[cfg(any())] mod absent { #[test] fn fixture() { assert_eq!(1,1); } }",
        "// removed portable tests",
    ] {
        fixture.write("crates/pure/tests/portable.rs", source);
        fixture.denied("required portable suite");
    }
}

#[test]
fn assembled_v4_gate_rejects_ci_exclusions_and_noop_test_targets() {
    let fixture = Fixture::new();
    fixture.portability_checkpoint();
    xtask::architecture(&fixture.root, Some("HEAD")).unwrap();
    let ci_path = fixture.root.join(".github/workflows/ci.yml");
    let original = fs::read_to_string(&ci_path).unwrap();
    let mut ci: serde_json::Value = serde_json::from_str(&original).unwrap();
    ci["jobs"]["rust"]["strategy"]["matrix"]["exclude"] =
        serde_json::json!([{"os":"windows-latest"}]);
    fixture.write(".github/workflows/ci.yml", &ci.to_string());
    fixture.denied("all three native runners");
    fixture.write(".github/workflows/ci.yml", &original);
    let manifest = fs::read_to_string(fixture.root.join("crates/pure/Cargo.toml")).unwrap();
    for extra in [
        "[[test]]\nname='portable'\nharness=false\n",
        "[[test]]\nname='portable'\ntest=false\n",
        "[[test]]\nname='portable'\nrequired-features=['never']\n",
    ] {
        fixture.write("crates/pure/Cargo.toml", &format!("{manifest}{extra}"));
        fixture.denied("required portable target");
    }
    fixture.write(
        "crates/pure/Cargo.toml",
        &manifest.replace("[package]", "[package]\nautotests=false"),
    );
    fixture.denied("autotests=false");
}

#[test]
fn assembled_v5_gate_rejects_unsafe_probe_and_overstated_inventory_evidence() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(5);
    xtask::architecture(&fixture.root, Some("HEAD")).unwrap();
    let registry =
        fs::read_to_string(fixture.root.join("architecture/capability-probes.toml")).unwrap();
    fixture.write(
        "architecture/capability-probes.toml",
        &registry.replace(
            "[\"-v\", \"list\", \"system\"]",
            "[\"call\", \"system\", \"reboot\"]",
        ),
    );
    fixture.denied("exact reviewed read-only");
    fixture.write("architecture/capability-probes.toml", &registry);
    let original = fs::read_to_string(fixture.root.join("compatibility/evidence.toml")).unwrap();
    let mut evidence: toml::Value = toml::from_str(&original).unwrap();
    assert!(
        evidence["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| {
                record["level"].as_str() == Some("emulated")
                    && evidence["targets"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|target| {
                            target["kind"].as_str() == Some("emulated")
                                && target["id"] == record["target"]
                        })
            }),
        "the assembled fixture must retain valid emulated evidence"
    );
    // Use a separate inventory-only physical target so this regression remains
    // meaningful if the original reference later gains real acceptance evidence.
    let mut inventory = evidence["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["kind"].as_str() == Some("physical"))
        .unwrap()
        .clone();
    inventory["id"] = "synthetic-physical-inventory".into();
    inventory["inventory_only"] = true.into();
    evidence["targets"].as_array_mut().unwrap().push(inventory);
    fixture.write(
        "docs/synthetic-acceptance.md",
        "Synthetic overclaim fixture, not device acceptance.\n",
    );
    let record: toml::Value = toml::from_str("id='fabricated'\nlevel='exact_device'\ntarget='synthetic-physical-inventory'\noperations=['system_info']\nartifacts=['docs/synthetic-acceptance.md']\nobserved_at='2026-09-12T19:30:00Z'\nhost='windows'\nenvironment='native'\n").unwrap();
    evidence["records"].as_array_mut().unwrap().push(record);
    fixture.write(
        "compatibility/evidence.toml",
        &toml::to_string(&evidence).unwrap(),
    );
    fixture.denied("inventory-only");
    fixture.write("compatibility/evidence.toml", &original);
    fixture.write(
        "crates/runtime/tests/capabilities.rs",
        "#[test] #[ignore] fn hidden() { assert_eq!(1,1); }\n",
    );
    fixture.denied("required portable suites");
}

#[test]
fn assembled_v6_gate_requires_reviewed_evolution_and_non_skipped_response_suites() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(5);
    fixture.capability_checkpoint(6);
    let error = xtask::architecture(&fixture.root, Some("HEAD^"))
        .expect_err("v6 evolution must require its changed documentation");
    assert!(error.contains("changed evidence"), "wrong failure: {error}");
    fixture.write(
        "docs/requirements.md",
        "Synthetic v6 bounded response requirements.\n",
    );
    fixture.write(
        "docs/architecture.md",
        "Synthetic v6 bounded response ownership.\n",
    );
    let error = xtask::architecture(&fixture.root, Some("HEAD^"))
        .expect_err("v6 evolution must require its harness regression");
    assert!(
        error.contains("harness regression"),
        "wrong failure: {error}"
    );
    fixture.write(
        "tools/xtask/tests/v6.rs",
        "#[test] fn synthetic_regression() { assert_eq!(1, 1); }\n",
    );
    let error = xtask::architecture(&fixture.root, Some("HEAD^"))
        .expect_err("v6 evolution must require its harness implementation");
    assert!(
        error.contains("harness implementation"),
        "wrong failure: {error}"
    );
    fixture.write(
        "tools/xtask/src/v6.rs",
        "//! Synthetic harness implementation marker.\n",
    );
    xtask::architecture(&fixture.root, Some("HEAD^")).unwrap();

    let original = fs::read_to_string(fixture.root.join("architecture/spec.toml")).unwrap();
    let mut missing: toml::Value = toml::from_str(&original).unwrap();
    missing
        .as_table_mut()
        .unwrap()
        .remove("projection_contract");
    fixture.write(
        "architecture/spec.toml",
        &toml::to_string(&missing).unwrap(),
    );
    fixture.denied("v6 requires a projection contract");
    fixture.write("architecture/spec.toml", &original);
    let suite = "crates/core/tests/collection_projection.rs";
    fs::remove_file(fixture.root.join(suite)).unwrap();
    fixture.denied("missing required portable suite");
    fixture.write(
        suite,
        "#![cfg(unix)] #[test] fn synthetic_only_on_one_host() { assert_eq!(1,1); }\n",
    );
    fixture.denied("required portable suite");
}

#[test]
fn assembled_v8_gate_confines_native_unsafe_and_lint_authority() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(8);
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/host-platform/src/windows/policy.rs",
        "fn escape() { unsafe {} }\n",
    );
    fixture.denied("unsafe expressions");
    fixture.write(
        "crates/host-platform/src/windows/policy.rs",
        "use windows_sys::Win32::Foundation::HANDLE;\n",
    );
    fixture.denied("forbidden API");
    fixture.write(
        "crates/host-platform/src/windows/policy.rs",
        "//! safe boundary\n",
    );
    fixture.write(
        "crates/host-platform/src/windows/native.rs",
        "#![allow(unsafe_code)] pub(super) fn read(path: &Path, max_bytes: usize, secret: bool) -> Result<Zeroizing<Vec<u8>>, HostError> { unsafe {} }\n",
    );
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/host-platform/src/windows/native.rs",
        "impl X { unsafe fn escape() {} }\n",
    );
    fixture.denied("unsafe/FFI methods");
    fixture.write("crates/host-platform/src/windows/native.rs", "//! reset\n");
    fixture.write("crates/host-platform/Cargo.toml", "[package]\nname='openwrt-mcp-host-platform'\nversion='0.1.0'\nedition='2024'\n[lints.rust]\nunsafe_code='allow'\n");
    fixture.denied("lint boundary");
}

#[test]
fn assembled_v9_gate_rejects_unreviewed_profile_and_luci_call_probe() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(9);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    fixture.write(path, &original.replace("base_luci_v2", "wildcard"));
    fixture.denied("probe profile");
    fixture.write(path, &original);
    let path = "architecture/capability-probes.toml";
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(fixture.root.join(path)).unwrap()).unwrap();
    registry["schema_version"] = 2.into();
    registry["probes"]
        .as_array_mut()
        .unwrap()
        .retain(|probe| !matches!(probe["object"].as_str(), Some("luci" | "luci-rpc")));
    for object in ["luci", "luci-rpc"] {
        let probe: toml::Value = toml::from_str(&format!("id='ubus.{object}'\nobject='{object}'\nprogram='/bin/ubus'\narguments=['-v','list','{object}']\neffect='read'\n")).unwrap();
        registry["probes"].as_array_mut().unwrap().push(probe);
    }
    fixture.write(path, &toml::to_string(&registry).unwrap());
    xtask::architecture(&fixture.root, None).unwrap();
    registry["probes"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap()["arguments"] = toml::Value::Array(
        ["call", "luci-rpc", "getDHCPLeases", "{}"]
            .map(Into::into)
            .to_vec(),
    );
    fixture.write(path, &toml::to_string(&registry).unwrap());
    fixture.denied("exact reviewed read-only");
}

#[test]
fn assembled_v10_gate_requires_finite_observation_and_guard_contracts() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(10);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let source = fs::read_to_string(fixture.root.join(path)).unwrap();
    fixture.write(
        path,
        &source.replace("ordered_duplicates_preserved_no_selection", "deduplicate"),
    );
    fixture.denied("v10 requires bounded observation");
    fixture.write(
        path,
        &source.replace("absent_pointers_before_projection", "ignore_errors"),
    );
    fixture.denied("v10 requires bounded observation");
    fixture.write(
        path,
        &source.replace("max_root_guards = 4", "max_root_guards = 400"),
    );
    fixture.denied("v10 requires bounded observation");
}

#[test]
fn assembled_v12_gate_requires_finite_nested_option_admission_and_shared_limits() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(12);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let source = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        (
            "nested_string_or_list_no_selection",
            "root_and_selection",
            "v12 requires bounded nested",
        ),
        (
            "kind_and_values_preserve_representation",
            "list_only",
            "v12 requires bounded nested",
        ),
        (
            "shared_scan_emit_bytes",
            "per_option_budget",
            "v12 requires bounded nested",
        ),
        (
            "max_text_option_items = 128",
            "max_text_option_items = 129",
            "v12 requires bounded nested",
        ),
        (
            "text_option_only",
            "any_collection",
            "v12 UCI nested collections",
        ),
        (
            "max_total_items = 256",
            "max_total_items = 4096",
            "projection hard ceilings",
        ),
    ] {
        assert!(source.contains(from));
        fixture.write(path, &source.replace(from, to));
        fixture.denied(expected);
    }
}

#[test]
fn assembled_v13_gate_requires_closed_base_profiles_without_new_authority_or_budgets() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(13);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let source = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        ("network:bridge-vlan", "network:*", "closed UCI profiles"),
        ("dhcp:host", "wireless:wifi-iface", "closed UCI profiles"),
        (
            "version = 13",
            "version = 12",
            "base family expansion requires architecture v13",
        ),
        (
            "custom_uci = \"deny\"",
            "custom_uci = \"allow\"",
            "closed UCI reads",
        ),
        (
            "profile_category_read",
            "extensions_execute",
            "closed UCI reads",
        ),
        (
            "max_total_items = 256",
            "max_total_items = 4096",
            "projection hard ceilings",
        ),
    ] {
        assert!(source.contains(from));
        fixture.write(path, &source.replace(from, to));
        fixture.denied(expected);
    }
}

#[test]
fn assembled_v14_gate_requires_closed_opkg_recipe_and_shared_bounds() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(14);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let source = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        ("version = 14", "version = 13", "opkg status expansion"),
        (
            "opkg_38eccbb1_root_status_v1",
            "opkg_any",
            "opkg status must retain",
        ),
        (
            "env_i_fixed_opkg_version_and_cat_root_status",
            "opkg_list_installed",
            "opkg status must retain",
        ),
        (
            "max_line_bytes = 8192",
            "max_line_bytes = 8193",
            "opkg status must retain",
        ),
        (
            "max_records = 4096",
            "max_records = 8192",
            "package contract must retain",
        ),
    ] {
        assert!(source.contains(from));
        fixture.write(path, &source.replace(from, to));
        fixture.denied(expected);
    }
}

#[test]
fn assembled_v11_gate_requires_closed_uci_admission_and_exact_text_enums() {
    let old_fixture = Fixture::new();
    old_fixture.capability_checkpoint(10);
    let old_source = fs::read_to_string(old_fixture.root.join("architecture/spec.toml")).unwrap();
    let mut old: toml::Value = toml::from_str(&old_source).unwrap();
    old["projection_contract"]
        .as_table_mut()
        .unwrap()
        .insert("text_enums".into(), "exact_finite_no_coercion".into());
    old["projection_contract"]
        .as_table_mut()
        .unwrap()
        .insert("max_text_enum_values".into(), 16.into());
    old_fixture.write("architecture/spec.toml", &toml::to_string(&old).unwrap());
    old_fixture.denied("text enum expansion");
    let fixture = Fixture::new();
    fixture.capability_checkpoint(11);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let source = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        (
            "fixed_uci_get_config_type_no_parameters",
            "arbitrary_uci",
            "closed UCI reads",
        ),
        (
            "custom_uci = \"deny\"",
            "custom_uci = \"allow\"",
            "closed UCI reads",
        ),
        (
            "rpcd_sessionless_shared_delta_non_atomic",
            "committed_only",
            "closed UCI reads",
        ),
        (
            "exact_finite_no_coercion",
            "coerce",
            "v11 requires bounded exact text enums",
        ),
        (
            "max_text_enum_values = 16",
            "max_text_enum_values = 17",
            "v11 requires bounded exact text enums",
        ),
        ("base_luci_uci_v3", "base_luci_v2", "probe profile"),
    ] {
        fixture.write(path, &source.replace(from, to));
        fixture.denied(expected);
    }
    fixture.write(path, &source);
    let path = "architecture/capability-probes.toml";
    let mut registry: toml::Value =
        toml::from_str(&fs::read_to_string(fixture.root.join(path)).unwrap()).unwrap();
    registry["schema_version"] = 3.into();
    registry["probes"]
        .as_array_mut()
        .unwrap()
        .retain(|p| p["object"].as_str() != Some("uci"));
    registry["probes"].as_array_mut().unwrap().push(toml::from_str("id='ubus.uci'\nobject='uci'\nprogram='/bin/ubus'\narguments=['call','uci','get','{}']\neffect='read'\n").unwrap());
    fixture.write(path, &toml::to_string(&registry).unwrap());
    fixture.denied("exact reviewed read-only");
}

#[test]
fn assembled_v15_gate_keeps_effect_analysis_private_bounded_and_non_authorizing() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(15);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        ("version = 15", "version = 14", "requires architecture v15"),
        (
            "max_nodes = 4096",
            "max_nodes = 4097",
            "non-authorizing bounded model",
        ),
        (
            "reachable_unknown_denies",
            "unknown_is_empty",
            "non-authorizing bounded model",
        ),
        (
            "conservative_before_after_union",
            "proposed_only",
            "non-authorizing bounded model",
        ),
        (
            "count_only_no_known_protected_impact",
            "mutation_allowed",
            "non-authorizing bounded model",
        ),
    ] {
        assert!(original.contains(from));
        fixture.write(path, &original.replace(from, to));
        fixture.denied(expected);
    }
    fixture.write(path, &original);
    fixture.write("crates/core/src/lib.rs", "pub mod management;\n");
    fixture.write(
        "crates/core/src/management/mod.rs",
        "pub struct EffectGraph;\n",
    );
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/core/src/management/mod.rs",
        "use serde::Serialize;\n",
    );
    fixture.denied("serde");
    fixture.write(
        "crates/core/src/management/mod.rs",
        "pub struct EffectGraph;\n",
    );
    fixture.write(
        "crates/mcp/src/lib.rs",
        "use openwrt_mcp_core::management::EffectGraph;\n",
    );
    fixture.denied("management");
    fixture.write("crates/mcp/src/lib.rs", "//! inert\n");
    fixture.write(
        "crates/core/src/lib.rs",
        "pub mod management; pub use management::EffectGraph;\n",
    );
    fixture.denied("public surface flattens protected namespace");
}

#[test]
fn assembled_v16_gate_rejects_archive_contract_source_and_consumer_escapes() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(16);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to, expected) in [
        ("version = 16", "version = 15", "codec must remain portable"),
        (
            "max_tail_bytes = 32768",
            "max_tail_bytes = 32769",
            "exact bounded non-authorizing profile",
        ),
        (
            "latched_first_error_no_reset",
            "skip_failed_entry",
            "exact bounded non-authorizing profile",
        ),
        (
            "regular_only_no_links_directories_extensions_or_special",
            "any_tar",
            "exact bounded non-authorizing profile",
        ),
    ] {
        assert!(original.contains(from));
        fixture.write(path, &original.replace(from, to));
        fixture.denied(expected);
    }
    fixture.write(path, &original);
    fixture.write("crates/device-codec/src/lib.rs", "pub mod archive;\n");
    fixture.write(
        "crates/device-codec/src/archive/mod.rs",
        "pub struct Validator;\n",
    );
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/device-codec/src/archive/mod.rs",
        "use std::io::Read;\n",
    );
    fixture.denied("std::io");
    fixture.write(
        "crates/device-codec/src/archive/mod.rs",
        "pub struct Validator;\n",
    );
    fixture.write(
        "crates/backend-ssh/src/lib.rs",
        "use openwrt_mcp_device_codec::archive::Validator;\n",
    );
    fixture.denied("archive");
    fixture.write("crates/backend-ssh/src/lib.rs", "//! inert\n");
    fixture.write(
        "crates/device-codec/src/lib.rs",
        "pub mod archive; pub use archive::Validator;\n",
    );
    fixture.denied("public surface flattens protected namespace");
}

#[test]
fn assembled_v17_gate_keeps_gzip_inside_its_reviewed_boundary() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(17);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to) in [
        ("max_header_bytes = 4096", "max_header_bytes = 4097"),
        (
            "one_member_exact_end_no_suffix_or_concatenation",
            "accept_suffix",
        ),
        (
            "final_complete_deflate_bytes_excluding_header_trailer",
            "ignore_expansion",
        ),
    ] {
        assert!(original.contains(from));
        fixture.write(path, &original.replace(from, to));
        fixture.denied("exact bounded single-member profile");
    }
    fixture.write(path, &original);
    fixture.write("crates/device-codec/src/lib.rs", "pub mod gzip;\n");
    fixture.write(
        "crates/device-codec/src/gzip/mod.rs",
        "pub struct Validator;\n",
    );
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        "crates/device-codec/src/gzip/mod.rs",
        "use std::io::Read;\n",
    );
    fixture.denied("std::io");
    fixture.write(
        "crates/device-codec/src/gzip/mod.rs",
        "pub struct Validator;\n",
    );
    fixture.write(
        "crates/device-codec/src/lib.rs",
        "pub mod gzip; use super::gzip::Validator;\n",
    );
    fixture.denied("sibling archive consumer");
    fixture.write("crates/device-codec/src/lib.rs", "pub mod gzip;\n");
    fixture.write(
        "crates/backend-ssh/src/lib.rs",
        "use openwrt_mcp_device_codec::gzip::Validator;\n",
    );
    fixture.denied("gzip");
}

#[test]
fn assembled_v18_gate_keeps_stream_sealing_out_of_device_authority() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(18);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to) in [
        (
            "max_ciphertext_bytes = 76546048",
            "max_ciphertext_bytes = 76546049",
        ),
        ("known_published_error_no_abort_or_retry", "abort_published"),
        (
            "true_eof_independent_counts_archive_and_producer_complete",
            "cipher_report_only",
        ),
    ] {
        assert!(original.contains(from));
        fixture.write(path, &original.replace(from, to));
        fixture.denied("exact bounded supplied-stream profile");
    }
    fixture.write(path, &original);
    fixture.write("crates/runtime/src/lib.rs", "pub mod sealing;\n");
    fixture.write("crates/runtime/src/sealing/mod.rs", "pub struct Sealer;\n");
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write("crates/runtime/src/sealing/mod.rs", "use std::fs::File;\n");
    fixture.denied("std::fs");
    fixture.write("crates/runtime/src/sealing/mod.rs", "pub struct Sealer;\n");
    fixture.write(
        "crates/runtime/src/lib.rs",
        "pub mod sealing; use crate::sealing::Sealer;\n",
    );
    fixture.denied("sibling authority");
    fixture.write("crates/runtime/src/lib.rs", "pub mod sealing;\n");
    fixture.write(
        "crates/mcp/src/lib.rs",
        "use openwrt_mcp_runtime::sealing::Sealer;\n",
    );
    fixture.denied("sealing");
}

#[test]
fn assembled_v19_gate_confines_windows_log_port_and_rotation_guarantees() {
    let fixture = Fixture::new();
    fixture.capability_checkpoint(19);
    xtask::architecture(&fixture.root, None).unwrap();
    let path = "architecture/spec.toml";
    let original = fs::read_to_string(fixture.root.join(path)).unwrap();
    for (from, to) in [
        (
            "preflight_held_generations_oldest_delete_descending_no_replace_create_new",
            "replace_by_path",
        ),
        (
            "terminal_latch_no_reopen_no_cleanup_unvalidated",
            "retry_reopen",
        ),
        ("max_component_units = 255", "max_component_units = 256"),
    ] {
        assert!(original.contains(from));
        fixture.write(path, &original.replace(from, to));
        fixture.denied("exact bounded NTFS profile");
    }
    fixture.write(path, &original);
    let native = "crates/host-platform/src/windows/native.rs";
    let original_native = fs::read_to_string(fixture.root.join(native)).unwrap();
    let entry = "pub(super) fn open_log(path: &Path, max_bytes: u64, retained: usize) -> Result<Box<dyn super::log::LogWriter>, HostError> {}";
    fixture.write(native, &format!("{original_native}\n{entry}"));
    xtask::architecture(&fixture.root, None).unwrap();
    fixture.write(
        native,
        &format!(
            "{original_native}\n{}",
            entry.replace("Box<dyn super::log::LogWriter>", "OwnedHandle")
        ),
    );
    fixture.denied("exact reviewed parent functions");
    fixture.write(native, &original_native);
    fixture.write("crates/host-platform/src/windows/log.rs", "pub(super) trait LogWriter: Send { fn write(&mut self, bytes: &[u8]) -> Result<(), HostError>; }\nuse std::fs::File;\n");
    fixture.denied("std::fs");
}
