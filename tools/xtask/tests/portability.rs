//! ADR 0004 regressions inspect synthetic source/metadata/workflows only.
use std::{fs, path::PathBuf};

use serde_json::{Value, json};
use xtask::{
    Contract, check_metadata, check_native_ci, check_portable_source, check_portable_suite,
    check_source,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract() -> Contract {
    Contract::parse(&fs::read_to_string(root().join("architecture/spec.toml")).unwrap()).unwrap()
}

fn workflow() -> Value {
    serde_json::from_str(&fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap())
        .unwrap()
}

#[test]
fn v4_requires_all_hosts_owned_portable_layers_and_existing_suites() {
    let original = contract();
    original.validate(&root()).unwrap();
    let mut bad = Vec::new();
    let mut value = original.clone();
    value.required_hosts.retain(|host| host != "windows");
    bad.push(value);
    let mut value = original.clone();
    value.required_hosts[0] = "wsl".into();
    bad.push(value);
    let mut value = original.clone();
    value.required_hosts.push("linux".into());
    bad.push(value);
    let mut value = original.clone();
    value
        .portable_crates
        .retain(|name| name != "openwrt-mcp-runtime");
    bad.push(value);
    let mut value = original.clone();
    value.portable_crates.push("unowned".into());
    bad.push(value);
    let mut value = original.clone();
    value.required_portable_tests.clear();
    bad.push(value);
    let mut value = original.clone();
    value
        .required_portable_tests
        .push("crates/server/tests/missing.rs".into());
    bad.push(value);
    let mut value = original;
    value.required_portable_tests[0] = "crates/server/src/main.rs".into();
    bad.push(value);
    for value in bad {
        assert!(value.validate(&root()).is_err());
    }
}

#[test]
fn portable_production_rejects_os_conditions_even_in_inactive_branches_and_macros() {
    for source in [
        "#![cfg(unix)] pub fn f() {}",
        "#[cfg(windows)] fn f() {}",
        "#[cfg(not(any(target_os = \"linux\", target_os = \"macos\")))] fn f() {}",
        "#[cfg_attr(unix, inline)] fn f() {}",
        "#[cfg_attr(target_arch = \"aarch64\", cfg(windows))] fn f() {}",
        "fn f() { let _ = cfg!(target_family = \"unix\"); }",
        "fn f() { json!({\"active\": cfg!(windows)}); }",
        "fn f() { format!(\"{}\", cfg!(r#target_os = \"linux\")); }",
        "#[cfg(target_env = \"msvc\")] fn f() {}",
        "#[cfg(target_vendor = \"apple\")] fn f() {}",
    ] {
        assert!(check_portable_source(source).is_err(), "accepted {source}");
    }
    for source in [
        "fn f() { let _ = \"cfg!(windows)\"; }",
        "#[cfg(target_arch = \"aarch64\")] fn f() {}",
        "fn f() { let _ = cfg!(target_feature = \"neon\"); }",
        "#[cfg(test)] mod tests { #[cfg(unix)] fn fixture() {} }",
    ] {
        check_portable_source(source).unwrap();
    }
}

#[test]
fn every_declared_portable_crate_rejects_os_apis_as_well_as_cfg() {
    let contract = contract();
    for name in &contract.portable_crates {
        let rule = contract
            .crates
            .iter()
            .find(|rule| &rule.name == name)
            .unwrap();
        for source in [
            "use std::os::unix::fs::PermissionsExt;",
            "use std::env::consts as host; fn f() { let _ = host::OS; }",
            "fn f() { let _ = r#std::r#env::consts::FAMILY; }",
        ] {
            assert!(
                check_source(rule, source, false).is_err(),
                "accepted {name}: {source}"
            );
        }
    }
}

#[test]
fn required_portable_suites_cannot_be_empty_ignored_or_conditionally_absent() {
    for source in [
        "// no tests",
        "#[test] fn empty() {}",
        "#![cfg(unix)] #[test] fn native() { assert!(true); }",
        "#[cfg(any())] mod hidden { #[test] fn absent() { assert!(true); } }",
        "#[test] #[ignore] fn ignored() { assert!(true); }",
        "#[cfg_attr(windows, ignore)] #[test] fn absent() { assert!(true); }",
        "#[tokio::test] async fn f() { if cfg!(windows) { return; } assert!(true); }",
        "#[test] fn f() { assert!(cfg!(unix)); }",
    ] {
        assert!(check_portable_suite(source).is_err(), "accepted {source}");
    }
    check_portable_suite("#[test] fn fixture() { assert_eq!(1, 1); }").unwrap();
    check_portable_suite("#[tokio::test] async fn fixture() { assert_eq!(1, 1); }").unwrap();
}

fn metadata(contract: &Contract, dependency: &str, target: Value) -> Value {
    json!({
        "workspace_root":"/synthetic", "workspace_members":contract.crates.iter().map(|rule| &rule.name).collect::<Vec<_>>(),
        "packages":contract.crates.iter().map(|rule| json!({
            "id":rule.name, "name":rule.name, "manifest_path":format!("/synthetic/{}/Cargo.toml",rule.path),
            "dependencies":if rule.name == "openwrt-mcp-key-sources" && dependency == "rustix"
                || rule.name == "openwrt-mcp-adapters" && dependency == "libc" {
                json!([{"name":dependency,"rename":"host_helper","kind":null,"target":target}])
            } else { json!([]) },
            "targets":[{"kind":["lib"],"src_path":format!("/synthetic/{}/src/lib.rs",rule.path)}]
        })).collect::<Vec<_>>()
    })
}

#[test]
fn native_dependencies_keep_their_reviewed_target_constraints() {
    let contract = contract();
    for dependency in ["rustix", "libc"] {
        for target in [
            Value::Null,
            json!("cfg(windows)"),
            json!("cfg(any(unix, windows))"),
            json!("cfg(not(windows))"),
        ] {
            assert!(check_metadata(&contract, &metadata(&contract, dependency, target)).is_err());
        }
        for target in ["cfg(unix)", "cfg(target_os = \"linux\")"] {
            check_metadata(&contract, &metadata(&contract, dependency, json!(target))).unwrap();
        }
    }
}

#[test]
fn native_ci_rejects_missing_hosts_skip_switches_and_replaced_gate_commands() {
    let contract = contract();
    let original = workflow();
    check_native_ci(&contract, &original.to_string()).unwrap();
    let mut bad = Vec::new();
    let mut value = original.clone();
    value["jobs"]["rust"]["strategy"]["matrix"]["os"] = json!(["ubuntu-latest", "macos-latest"]);
    bad.push(value);
    let mut value = original.clone();
    value["jobs"]["rust"]["strategy"]["matrix"]["exclude"] = json!([{"os":"windows-latest"}]);
    bad.push(value);
    let mut value = original.clone();
    value["jobs"]["rust"]["runs-on"] = json!("ubuntu-latest");
    bad.push(value);
    for field in ["if", "continue-on-error"] {
        let mut value = original.clone();
        value["jobs"]["rust"][field] = json!(true);
        bad.push(value);
        let mut value = original.clone();
        value["jobs"]["rust"]["steps"][2][field] = json!(true);
        bad.push(value);
        let mut value = original.clone();
        value["jobs"]["rust"]["steps"][3][field] = json!(true);
        bad.push(value);
    }
    for replacement in [
        "./tools/Test-Repository.ps1 -UseWsl",
        "./tools/Test-Repository.ps1 -BaseRef $env:ARCHITECTURE_BASE; exit 0",
        "echo ./tools/Test-Repository.ps1 -BaseRef $env:ARCHITECTURE_BASE",
    ] {
        let mut value = original.clone();
        value["jobs"]["rust"]["steps"][2]["run"] = json!(replacement);
        bad.push(value);
    }
    let mut value = original.clone();
    value["jobs"]["rust"]["steps"].as_array_mut().unwrap().pop();
    bad.push(value);
    let mut value = original.clone();
    value["jobs"]["rust"]["steps"][3]["run"] =
        json!("cargo test --locked -p openwrt-mcp --test portable_stdio -- --ignored");
    bad.push(value);
    let mut value = original;
    value["on"]["push"] = json!({"paths":["never-match"]});
    bad.push(value);
    for value in bad {
        assert!(
            check_native_ci(&contract, &value.to_string()).is_err(),
            "accepted {value}"
        );
    }
}
