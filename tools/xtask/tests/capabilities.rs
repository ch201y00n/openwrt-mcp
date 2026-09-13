//! ADR 0005 adversarial declarations. These tests never connect to a target.
use std::{fs, path::PathBuf};

use xtask::{
    Contract, check_capability_registry, check_compatibility_evidence, check_native_ci,
    check_source,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap()
}

fn contract() -> Contract {
    Contract::parse(&read("architecture/spec.toml")).unwrap()
}

#[test]
fn capability_contract_requires_codec_ownership_and_every_native_suite() {
    let original = contract();
    original.validate(&root()).unwrap();
    let mut bad = Vec::new();
    let mut value = original.clone();
    value.capability_contract = None;
    bad.push(value);
    let mut value = original.clone();
    value
        .capability_contract
        .as_mut()
        .unwrap()
        .required_tests
        .pop();
    bad.push(value);
    let mut value = original.clone();
    value.capability_contract.as_mut().unwrap().probe_registry = "../outside.toml".into();
    bad.push(value);
    let mut value = original.clone();
    value
        .portable_crates
        .retain(|name| name != "openwrt-mcp-device-codec");
    bad.push(value);
    for name in [
        "openwrt-mcp-device-codec",
        "openwrt-mcp-adapters",
        "openwrt-mcp-backend-ssh",
    ] {
        let mut value = original.clone();
        value
            .crates
            .iter_mut()
            .find(|rule| rule.name == name)
            .unwrap()
            .dependencies
            .clear();
        bad.push(value);
    }
    let mut value = original.clone();
    value
        .crates
        .iter_mut()
        .find(|rule| rule.name == "openwrt-mcp-device-codec")
        .unwrap()
        .dependencies
        .push("openwrt-mcp-runtime".into());
    bad.push(value);
    let mut value = original.clone();
    value
        .crates
        .iter_mut()
        .find(|rule| rule.name == "openwrt-mcp-transport")
        .unwrap()
        .dependencies
        .push("openwrt-mcp-device-codec".into());
    bad.push(value);
    let mut value = original;
    value
        .crates
        .iter_mut()
        .find(|rule| rule.name == "openwrt-mcp-device-codec")
        .unwrap()
        .forbidden_paths
        .retain(|path| path != "std::fs");
    bad.push(value);
    for value in bad {
        assert!(
            value.validate(&root()).is_err(),
            "accepted weakened v5 contract"
        );
    }
}

#[test]
fn probes_cannot_become_shells_method_calls_wildcards_or_signature_free_lists() {
    let source = read("architecture/capability-probes.toml");
    check_capability_registry(&source).unwrap();
    for (before, after) in [
        ("program = \"/bin/ubus\"", "program = \"/bin/sh\""),
        (
            "[\"-v\", \"list\", \"system\"]",
            "[\"call\", \"system\", \"reboot\", \"{}\"]",
        ),
        (
            "[\"-v\", \"list\", \"system\"]",
            "[\"-S\", \"-v\", \"list\", \"system\"]",
        ),
        ("[\"-v\", \"list\", \"system\"]", "[\"-v\", \"list\"]"),
        (
            "[\"-v\", \"list\", \"system\"]",
            "[\"-v\", \"list\", \"*\"]",
        ),
        ("object = \"system\"", "object = \"{client_object}\""),
        ("effect = \"read\"", "effect = \"execute\""),
        ("id = \"ubus.system\"", "id = \"caller_supplied\""),
        ("object = \"system\"", "object = \"uci\""),
    ] {
        assert!(check_capability_registry(&source.replacen(before, after, 1)).is_err());
    }
    let duplicate = source.split("[[probes]]").nth(1).unwrap();
    assert!(check_capability_registry(&format!("{source}\n[[probes]]{duplicate}")).is_err());
    let missing = source.rsplit_once("[[probes]]").unwrap().0;
    assert!(check_capability_registry(missing).is_err());
}

#[test]
fn luci_registry_requires_v9_and_only_two_additional_exact_descriptions() {
    let original = contract();
    original.validate(&root()).unwrap();
    for profile in [None, Some("wildcard".into()), Some("base_v1".into())] {
        let mut bad = original.clone();
        bad.capability_contract.as_mut().unwrap().probe_profile = profile;
        assert!(bad.validate(&root()).is_err());
    }
    let mut base: toml::Value =
        toml::from_str(&read("architecture/capability-probes.toml")).unwrap();
    base["schema_version"] = 1.into();
    base["probes"]
        .as_array_mut()
        .unwrap()
        .retain(|probe| !matches!(probe["object"].as_str(), Some("luci" | "luci-rpc")));
    let legacy = toml::to_string(&base).unwrap();
    xtask::check_capability_registry_for_version(&legacy, 8).unwrap();
    xtask::check_capability_registry_for_version(&legacy, 9).unwrap();
    base["schema_version"] = 2.into();
    for object in ["luci", "luci-rpc"] {
        let probe: toml::Value = toml::from_str(&format!("id='ubus.{object}'\nobject='{object}'\nprogram='/bin/ubus'\narguments=['-v','list','{object}']\neffect='read'\n")).unwrap();
        base["probes"].as_array_mut().unwrap().push(probe);
    }
    let reviewed = toml::to_string(&base).unwrap();
    xtask::check_capability_registry_for_version(&reviewed, 9).unwrap();
    assert!(xtask::check_capability_registry_for_version(&reviewed, 8).is_err());
    for object in ["luci.*", "luci-rpc.*", "dhcp", "file", "uci", "{object}"] {
        let mut bad = base.clone();
        bad["probes"].as_array_mut().unwrap().last_mut().unwrap()["object"] = object.into();
        assert!(check_capability_registry(&toml::to_string(&bad).unwrap()).is_err());
    }
    for version in [0, 3, 99] {
        let mut bad = base.clone();
        bad["schema_version"] = version.into();
        assert!(check_capability_registry(&toml::to_string(&bad).unwrap()).is_err());
    }
    base["probes"].as_array_mut().unwrap().pop();
    assert!(check_capability_registry(&toml::to_string(&base).unwrap()).is_err());
}

#[test]
fn registry_cannot_weaken_auth_binding_freshness_audit_or_fail_closed_behavior() {
    let source = read("architecture/capability-probes.toml");
    for (before, after) in [
        ("same_immutable_backend", "independent_prober"),
        ("private_dispatcher", "caller_supplied_snapshot"),
        ("opaque_backend_epoch", "release_string"),
        ("ttl_seconds = 30", "ttl_seconds = 0"),
        ("ttl_seconds = 30", "ttl_seconds = 3600"),
        ("fail_closed", "assume_supported"),
        ("start_before_cache_probe_execute", "start_after_probe"),
        (
            "\"check\", \"catalog\", \"tools_list\"",
            "\"check\", \"catalog\"",
        ),
        (
            "required_ubus_method_signature_response_contract",
            "optional",
        ),
        ("unverified_blocked", "extension_bypass"),
        ("capability_status", "execute"),
        ("operation_capability", "ubus_call"),
        (
            "metadata_audit_kind = \"capability\"",
            "metadata_audit_kind = \"invoke\"",
        ),
        ("shared_probe_execute", "independent_per_probe"),
    ] {
        assert!(check_capability_registry(&source.replace(before, after)).is_err());
    }
    assert!(check_capability_registry(&format!("{source}\nforce = true")).is_err());
}

fn evidence() -> toml::Value {
    // Isolate the original inventory fixture. New reviewed acceptance records may
    // be added to the repository without turning these adversarial inputs valid.
    let mut value: toml::Value = toml::from_str(&read("compatibility/evidence.toml")).unwrap();
    value["records"] = toml::Value::Array(vec![]);
    value["targets"]
        .as_array_mut()
        .unwrap()
        .retain(|target| target["id"].as_str() == Some("bpi-r4-openwrt-25.12.5-reference"));
    value["targets"][0]["inventory_only"] = true.into();
    value
}

fn valid_record(level: &str) -> toml::Value {
    toml::from_str(&format!("id='synthetic-review'\nlevel='{level}'\ntarget='bpi-r4-openwrt-25.12.5-reference'\noperations=['system_info']\nartifacts=['docs/adr/0005-capability-observations.md']\nobserved_at='2026-09-12T19:30:00Z'\n")).unwrap()
}

fn validate(value: &toml::Value) -> Result<(), String> {
    check_compatibility_evidence(&root(), &toml::to_string(value).unwrap())
}

#[test]
fn initial_reference_is_inventory_only_and_does_not_claim_operation_acceptance() {
    check_compatibility_evidence(&root(), &read("compatibility/evidence.toml")).unwrap();
    let value = evidence();
    validate(&value).unwrap();
    assert!(value["records"].as_array().unwrap().is_empty());
    assert_eq!(value["targets"][0]["inventory_only"].as_bool(), Some(true));
    let mut promoted = value.clone();
    promoted["targets"][0]["inventory_only"] = false.into();
    assert!(
        validate(&promoted)
            .unwrap_err()
            .contains("requires scoped target")
    );
    let mut fabricated = value;
    fabricated["records"] = toml::Value::Array(vec![valid_record("exact_device")]);
    assert!(
        validate(&fabricated)
            .unwrap_err()
            .contains("inventory-only")
    );
}

#[test]
fn evidence_levels_targets_operation_scope_and_package_fingerprints_are_required() {
    let mut value = evidence();
    value["records"] = toml::Value::Array(vec![valid_record("source"), {
        let mut record = valid_record("synthetic");
        record["id"] = "another-review".into();
        record
    }]);
    validate(&value).unwrap();
    for (field, replacement) in [
        ("level", "device"),
        ("target", "unknown"),
        ("observed_at", "yesterday"),
    ] {
        let mut bad = value.clone();
        bad["records"][0][field] = replacement.into();
        assert!(validate(&bad).is_err());
    }
    for operations in [
        vec![],
        vec!["*"],
        vec!["system_info", "system_info"],
        vec!["client chosen name"],
    ] {
        let mut bad = value.clone();
        bad["records"][0]["operations"] =
            toml::Value::Array(operations.into_iter().map(Into::into).collect());
        assert!(validate(&bad).is_err());
    }
    let mut bad = value.clone();
    bad["targets"][0]["packages"]
        .as_table_mut()
        .unwrap()
        .remove("rpcd");
    assert!(validate(&bad).is_err());
    let mut bad = value;
    bad["allowed_levels"].as_array_mut().unwrap().pop();
    assert!(validate(&bad).is_err());
}

#[test]
fn evidence_paths_cannot_escape_or_substitute_missing_files() {
    for path in [
        "../outside.md",
        "/absolute.md",
        "C:/outside.md",
        "D:relative.md",
        "docs\\reference-target.md",
        "compatibility/missing.md",
    ] {
        let mut source = evidence();
        source["targets"][0]["sources"] = toml::Value::Array(vec![path.into()]);
        assert!(validate(&source).is_err(), "accepted source {path}");
        let mut artifact = evidence();
        let mut record = valid_record("source");
        record["artifacts"] = toml::Value::Array(vec![path.into()]);
        artifact["records"] = toml::Value::Array(vec![record]);
        assert!(validate(&artifact).is_err(), "accepted artifact {path}");
    }
}

#[test]
fn native_evidence_cannot_relabel_wsl_or_synthetic_fixtures() {
    for host in ["windows", "linux", "macos"] {
        let mut value = evidence();
        let mut record = valid_record("native");
        record
            .as_table_mut()
            .unwrap()
            .insert("host".into(), host.into());
        record
            .as_table_mut()
            .unwrap()
            .insert("environment".into(), "native".into());
        value["records"] = toml::Value::Array(vec![record]);
        validate(&value).unwrap();
        value["records"][0]["environment"] = "wsl".into();
        assert!(validate(&value).is_err());
        value["records"][0]["level"] = "source".into();
        assert!(validate(&value).is_err());
    }
    let mut value = evidence();
    let mut record = valid_record("synthetic");
    record
        .as_table_mut()
        .unwrap()
        .insert("host".into(), "linux".into());
    record
        .as_table_mut()
        .unwrap()
        .insert("environment".into(), "wsl".into());
    value["records"] = toml::Value::Array(vec![record]);
    validate(&value).unwrap();
    value["records"][0]["host"] = "windows".into();
    assert!(validate(&value).is_err());
}

#[test]
fn emulated_target_acceptance_is_not_physical_device_evidence() {
    let mut value = evidence();
    let mut target = value["targets"][0].clone();
    target["id"] = "qemu-synthetic-target".into();
    target["kind"] = "emulated".into();
    target["inventory_only"] = false.into();
    let mut record = valid_record("emulated");
    record["target"] = "qemu-synthetic-target".into();
    record
        .as_table_mut()
        .unwrap()
        .insert("host".into(), "linux".into());
    record
        .as_table_mut()
        .unwrap()
        .insert("environment".into(), "wsl".into());
    value["targets"].as_array_mut().unwrap().push(target);
    value["records"] = toml::Value::Array(vec![record]);
    validate(&value).unwrap();
    let mut bad = value.clone();
    bad["records"][0]["level"] = "exact_device".into();
    assert!(
        validate(&bad)
            .unwrap_err()
            .contains("not exact physical-device")
    );
    value["targets"][1]["kind"] = "physical".into();
    assert!(validate(&value).is_err());
}

#[test]
fn package_fingerprints_allow_one_reviewed_manager_not_release_only() {
    let mut value = evidence();
    let packages = value["targets"][0]["packages"].as_table_mut().unwrap();
    packages.remove("apk");
    packages.insert("opkg".into(), "synthetic-version".into());
    validate(&value).unwrap();
    value["targets"][0]["packages"]
        .as_table_mut()
        .unwrap()
        .insert("apk".into(), "synthetic-version".into());
    assert!(validate(&value).is_err());
}

#[test]
fn codec_accepts_provided_stream_types_but_never_concrete_io_or_runtime() {
    let contract = contract();
    let codec = contract
        .crates
        .iter()
        .find(|rule| rule.name == "openwrt-mcp-device-codec")
        .unwrap();
    check_source(
        codec,
        "use std::io::{Error, Result, Write}; fn f(_: &mut dyn Write) -> Result<()> { Ok(()) }",
        false,
    )
    .unwrap();
    for source in [
        "use std::fs as bytes; fn f() { bytes::read(\"synthetic\"); }",
        "fn f() { std::io::stdin(); }",
        "fn f() { std::io::stdout(); }",
        "fn f() { std::io::stderr(); }",
        "fn f() { std::io::pipe(); }",
        "fn f() { std::env::var(\"synthetic\"); }",
        "fn f() { std::process::Command::new(\"synthetic\"); }",
        "fn f() { tokio::net::TcpStream::connect(\"synthetic\"); }",
    ] {
        assert!(check_source(codec, source, false).is_err());
    }
    assert!(
        !codec
            .dependencies
            .iter()
            .any(|name| name == "openwrt-mcp-runtime")
    );
}

#[test]
fn all_three_new_suites_are_mandatory_native_ci_steps() {
    let contract = contract();
    let original: serde_json::Value =
        serde_json::from_str(&read(".github/workflows/ci.yml")).unwrap();
    for suite in ["capability_contracts", "capabilities", "ubus_describe"] {
        let mut ci = original.clone();
        ci["jobs"]["rust"]["steps"]
            .as_array_mut()
            .unwrap()
            .retain(|step| {
                !step["run"]
                    .as_str()
                    .is_some_and(|run| run.ends_with(&format!("--test {suite}")))
            });
        assert!(check_native_ci(&contract, &ci.to_string()).is_err());
    }
}
