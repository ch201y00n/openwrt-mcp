//! Declaration and source-boundary regressions, not native log behavior.
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{Contract, check_native_ci, check_owned_source, check_windows_log_source};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

mod profiles;
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn parse(v: &toml::Value) -> Contract {
    Contract::parse(&toml::to_string(v).unwrap()).unwrap()
}
fn denied(v: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(v).unwrap())
            .and_then(|c| c.validate(&root()))
            .is_err()
    );
}
const NATIVE: &str = "crates/host-platform/src/windows/native.rs";
const FACADE: &str = "crates/host-platform/src/windows/log.rs";
const ENTRY: &str = "pub(super) fn open_log(path: &Path, max_bytes: u64, retained: usize) -> Result<Box<dyn super::log::LogWriter>, HostError> {}";
const PORT: &str = "pub(super) trait LogWriter: Send { fn write(&mut self, bytes: &[u8]) -> Result<(), HostError>; }";

#[test]
fn log_profile_requires_every_exact_field_limit_and_v19() {
    let original = declaration();
    parse(&original).validate(&root()).unwrap();
    for (key, value) in original["windows_log_contract"].as_table().unwrap() {
        let mut missing = original.clone();
        missing["windows_log_contract"]
            .as_table_mut()
            .unwrap()
            .remove(key);
        denied(&missing);
        let mut changed = original.clone();
        changed["windows_log_contract"][key] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(n) => (n + 1).into(),
            _ => unreachable!(),
        };
        denied(&changed);
    }
    let mut v = original.clone();
    v["windows_log_contract"]
        .as_table_mut()
        .unwrap()
        .insert("repair_acl".into(), true.into());
    denied(&v);
    let mut v = original.clone();
    v.as_table_mut().unwrap().remove("windows_log_contract");
    denied(&v);
    v["version"] = 18.into();
    profiles::before_v20(&mut v);
    parse(&v).validate(&root()).unwrap();
    v.as_table_mut().unwrap().insert(
        "windows_log_contract".into(),
        original["windows_log_contract"].clone(),
    );
    denied(&v);
}

#[test]
fn only_the_exact_opaque_parent_factory_is_admitted_after_v19() {
    let original = declaration();
    let c = parse(&original);
    let aliases = BTreeMap::new();
    check_owned_source(&c, NATIVE, ENTRY, &aliases).unwrap();
    check_owned_source(
        &c,
        NATIVE,
        &ENTRY.replace("retained: usize)", "retained: usize,)"),
        &aliases,
    )
    .unwrap();
    for invalid in [
        ENTRY.replace("pub(super)", "pub(crate)"),
        ENTRY.replace("pub(super)", "pub"),
        ENTRY.replace("open_log", "open_file"),
        ENTRY.replace("u64", "usize"),
        ENTRY.replace("Box<dyn super::log::LogWriter>", "std::fs::File"),
        ENTRY.replace("Box<dyn super::log::LogWriter>", "OwnedHandle"),
        ENTRY.replace("Box<dyn super::log::LogWriter>", "Box<dyn std::io::Write>"),
        "pub(super) struct NativeLog { pub handle: HANDLE }".into(),
        "pub(super) trait Leaked { fn handle(&self) -> HANDLE; }".into(),
        "pub(super) type Hidden = HANDLE;".into(),
        "pub(super) union Hidden { handle: HANDLE }".into(),
    ] {
        assert!(check_owned_source(&c, NATIVE, &invalid, &aliases).is_err());
    }
    let mut older = original.clone();
    older["version"] = 18.into();
    older.as_table_mut().unwrap().remove("windows_log_contract");
    assert!(check_owned_source(&parse(&older), NATIVE, ENTRY, &aliases).is_err());
    assert!(check_windows_log_source(&parse(&older), FACADE, PORT, &aliases).is_err());
}

#[test]
fn safe_facade_cannot_gain_handles_io_or_a_wider_port() {
    let c = parse(&declaration());
    let aliases = BTreeMap::new();
    check_windows_log_source(&c, FACADE, PORT, &aliases).unwrap();
    for code in [
        "use std::fs::File;",
        "use std::os::windows::io::OwnedHandle;",
        "use windows_sys::Win32::Foundation::HANDLE;",
        "use std::env as hidden;",
        "use std::io::Write;",
        "use std::process::Command;",
        "fn leak() { let _ = unsafe { 1 }; }",
        "fn leak() { let _ = format!(\"{}\", std::env::var(\"fixture\").unwrap()); }",
    ] {
        assert!(
            check_windows_log_source(&c, FACADE, &format!("{PORT}\n{code}"), &aliases).is_err()
        );
    }
    for port in [
        "".to_owned(),
        PORT.replace(": Send", ""),
        PORT.replace("pub(super)", "pub"),
        PORT.replace("bytes: &[u8]", "bytes: Vec<u8>"),
        PORT.replace("write", "read"),
        PORT.replace("; }", "; fn path(&self) -> &Path; }"),
        PORT.replace("Result<(), HostError>", "Result<u64, HostError>"),
        format!("#[cfg(target_os = \"windows\")] {PORT}"),
        PORT.replace("fn write", "#[cfg(target_os = \"windows\")] fn write"),
        format!("{PORT}\n{PORT}"),
    ] {
        assert!(check_windows_log_source(&c, FACADE, &port, &aliases).is_err());
    }
    // The only new facade still cannot host SDK calls through the main scanner.
    assert!(
        check_owned_source(
            &c,
            FACADE,
            "use windows_sys::Win32::Foundation::HANDLE;",
            &aliases
        )
        .is_err()
    );
}

#[test]
fn log_support_cannot_drop_windows_native_suite_or_existing_host_matrix() {
    let c = parse(&declaration());
    let original = fs::read_to_string(root().join(&c.native_ci)).unwrap();
    check_native_ci(&c, &original).unwrap();
    let no_suite = original.replace(
        "cargo test --locked -p openwrt-mcp-host-platform --test windows",
        "cargo check",
    );
    assert!(check_native_ci(&c, &no_suite).is_err());
    let no_host = original.replace("\"windows-latest\",", "");
    assert!(check_native_ci(&c, &no_host).is_err());
    for source in [
        "#![cfg(target_os = \"linux\")]\n#[test] fn hidden() {}",
        "#![cfg(target_os = \"windows\")]\n#[test] #[ignore] fn hidden() {}",
    ] {
        assert!(xtask::check_windows_suite(source).is_err());
    }
}
