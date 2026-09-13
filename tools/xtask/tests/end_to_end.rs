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
        assert!(matches!(version, 5 | 6 | 8 | 9 | 10));
        spec["version"] = (version as i64).into();
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
