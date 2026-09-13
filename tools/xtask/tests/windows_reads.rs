use serde_json::json;
use std::{collections::BTreeMap, fs, path::PathBuf};
use xtask::{
    Contract, check_native_ci, check_owned_source, check_windows_dependency, check_windows_lints,
    check_windows_suite,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn declaration() -> toml::Value {
    toml::from_str(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}
fn contract() -> Contract {
    Contract::parse(&toml::to_string(&declaration()).unwrap()).unwrap()
}
fn rejected(value: &toml::Value) {
    assert!(
        Contract::parse(&toml::to_string(value).unwrap())
            .and_then(|c| c.validate(&root()))
            .is_err()
    );
}

#[test]
fn windows_profile_cannot_be_removed_or_weakened() {
    let original = declaration();
    let mut missing = original.clone();
    missing
        .as_table_mut()
        .unwrap()
        .remove("windows_read_contract");
    rejected(&missing);
    for (key, value) in original["windows_read_contract"].as_table().unwrap() {
        let mut removed = original.clone();
        removed["windows_read_contract"]
            .as_table_mut()
            .unwrap()
            .remove(key);
        rejected(&removed);
        let mut changed = original.clone();
        changed["windows_read_contract"][key] = match value {
            toml::Value::String(_) => "unreviewed".into(),
            toml::Value::Integer(n) => (n + 1).into(),
            _ => unreachable!(),
        };
        rejected(&changed);
    }
}

#[test]
fn unsafe_and_sdk_are_confined_to_exact_owned_native_file() {
    let c = contract();
    let aliases = BTreeMap::new();
    let native = "crates/host-platform/src/windows/native.rs";
    for file in [
        "crates/host-platform/src/windows/policy.rs",
        "crates/host-platform/src/windows.rs",
        "crates/host-platform/src/windows/extra.rs",
        "crates/runtime/src/lib.rs",
    ] {
        for source in [
            "fn f() { unsafe {} }",
            "#![allow(unsafe_code)] fn f() {}",
            "#[cfg_attr(windows, allow(unsafe_code))] fn f() {}",
            "use windows_sys::Win32::Foundation::HANDLE;",
            "use renamed::Win32::Foundation::HANDLE;",
            "fn f() { let _ = format!(\"{:?}\", unsafe { 1 }); }",
        ] {
            let mut aliases = aliases.clone();
            aliases.insert("renamed".into(), "windows_sys".into());
            assert!(
                check_owned_source(&c, file, source, &aliases).is_err(),
                "{file}: {source}"
            );
        }
    }
    check_owned_source(
        &c,
        native,
        "#![allow(unsafe_code)] fn f() { unsafe {} }",
        &aliases,
    )
    .unwrap();
    for source in [
        "unsafe fn f() {}",
        "unsafe extern \"C\" { fn f(); }",
        "pub use windows_sys::Win32::Foundation::HANDLE;",
        "pub fn read() {}",
        "pub(crate) fn read() {}",
        "pub(super) fn arbitrary() {}",
        "mod nested { fn f() { unsafe {} } }",
    ] {
        assert!(
            check_owned_source(&c, native, source, &aliases).is_err(),
            "{source}"
        );
    }
    assert!(
        check_owned_source(&c, native, "pub(super) fn read() -> *mut u8 {}", &aliases).is_err()
    );
    assert!(check_owned_source(&c, native, "pub(super) struct Handle;", &aliases).is_err());
    check_owned_source(&c, native, "pub(super) fn read(path: &Path, max_bytes: usize, secret: bool,) -> Result<Zeroizing<Vec<u8>>, HostError> {}", &aliases).unwrap();
    check_owned_source(&c, native, "pub(super) fn read(path: &Path, max_bytes: usize, secret: bool) -> Result<Zeroizing<Vec<u8>>, HostError> {}", &aliases).unwrap();
}

#[test]
fn sdk_cannot_change_owner_target_version_or_feature_surface() {
    let d = json!({"name":"windows-sys","target":"cfg(target_os = \"windows\")","req":"=0.61.2","uses_default_features":false,"kind":null,"path":null,"features":["Wdk_Foundation","Wdk_Storage_FileSystem","Win32_Foundation","Win32_Security","Win32_Storage_FileSystem","Win32_System_IO","Win32_System_Threading"]});
    let owner = "openwrt-mcp-host-platform";
    check_windows_dependency(owner, &d).unwrap();
    assert!(check_windows_dependency("openwrt-mcp-runtime", &d).is_err());
    for (key, value) in [
        ("target", json!(null)),
        ("req", json!("^0.61.2")),
        ("uses_default_features", json!(true)),
        ("features", json!([])),
        ("path", json!("/unowned")),
        ("kind", json!("build")),
    ] {
        let mut changed = d.clone();
        changed[key] = value;
        assert!(check_windows_dependency(owner, &changed).is_err());
    }
    for owner in ["openwrt-mcp-host-platform", "openwrt-mcp-core"] {
        let changed: toml::Value = toml::from_str("[lints.rust]\nunsafe_code='allow'").unwrap();
        assert!(check_windows_lints(owner, &changed).is_err());
    }
}

#[test]
fn native_windows_suite_cannot_disappear_or_skip_cases() {
    let c = contract();
    c.validate(&root()).unwrap();
    let good = "#![cfg(target_os = \"windows\")] #[test] fn case() { assert_eq!(1, 1); }";
    check_windows_suite(good).unwrap();
    for source in [
        "#[test] fn case() {}",
        "#![cfg(target_os = \"linux\")] #[test] fn case() {}",
        "#![cfg(target_os = \"windows\")] #[test] #[ignore] fn case() {}",
        "#![cfg(target_os = \"windows\")] #[test] #[cfg(target_env = \"gnu\")] fn case() {}",
    ] {
        assert!(check_windows_suite(source).is_err());
    }
    let mut ci: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root().join(&c.native_ci)).unwrap()).unwrap();
    ci["jobs"]["rust"]["steps"]
        .as_array_mut()
        .unwrap()
        .retain(|step| {
            step["run"].as_str()
                != Some("cargo test --locked -p openwrt-mcp-host-platform --test windows")
        });
    assert!(check_native_ci(&c, &ci.to_string()).is_err());
}
