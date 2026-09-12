//! Real Cargo/Git fixtures exercise the assembled gate, not only its helpers.
//! Source snippets are inspected, never compiled or executed against a device.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
