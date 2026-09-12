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
