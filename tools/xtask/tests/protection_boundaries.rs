//! ADR 0003 regressions use the checked-in contract and synthetic source/metadata.
//! No archive, environment source, key provider, cipher or router is executed.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde_json::{Value, json};
use xtask::{
    Contract, CrateRule, check_metadata, check_public_reexports, check_source,
    check_source_with_aliases,
};

fn contract() -> Contract {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Contract::parse(&fs::read_to_string(root.join("architecture/spec.toml")).unwrap()).unwrap()
}

fn rule(contract: &Contract, name: &str) -> CrateRule {
    contract
        .crates
        .iter()
        .find(|rule| rule.name == name)
        .unwrap()
        .clone()
}

fn metadata(contract: &Contract) -> Value {
    json!({
        "workspace_root":"/synthetic", "workspace_members":contract.crates.iter().map(|rule| &rule.name).collect::<Vec<_>>(),
        "packages":contract.crates.iter().map(|rule| json!({
            "id":rule.name, "name":rule.name, "manifest_path":format!("/synthetic/{}/Cargo.toml",rule.path),
            "dependencies":[], "targets":[{"kind":["lib"],"src_path":format!("/synthetic/{}/src/lib.rs",rule.path)}]
        })).collect::<Vec<_>>()
    })
}

fn edge(contract: &Contract, owner: &str, dependency: &str, kind: Value) -> Value {
    let mut value = metadata(contract);
    let package = value["packages"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|package| package["name"] == owner)
        .unwrap();
    let path = contract
        .crates
        .iter()
        .find(|rule| rule.name == dependency)
        .map(|rule| format!("/synthetic/{}", rule.path));
    let target = if matches!(dependency, "rustix" | "libc") {
        "cfg(unix)"
    } else {
        "cfg(target_os = \"not_this_host\")"
    };
    package["dependencies"] = json!([{"name":dependency,"rename":"innocent_alias","kind":kind,"target":target,"path":path}]);
    value
}

#[test]
fn application_stream_traits_do_not_authorize_opening_os_handles() {
    let contract = contract();
    let application = rule(&contract, "openwrt-mcp-runtime");
    assert!(check_source(&application, "use std::io::{Read, Write}; pub fn transfer(input: &mut dyn Read, output: &mut dyn Write) -> std::io::Result<u64> { std::io::copy(input, output) }", false).is_ok());
    for source in [
        "fn f() { std::fs::read(\"synthetic\"); }",
        "fn f() { std::env::var(\"SYNTHETIC_ONLY\"); }",
        "fn f() { std::process::Command::new(\"synthetic\"); }",
        "fn f() { std::io::stdin(); }",
        "fn f() { std::io::stdout(); }",
        "fn f() { std::io::stderr(); }",
        "fn f() { std::io::pipe(); }",
        "use std::io as streams; fn f() { streams::stdin(); }",
        "fn f() { r#std::r#io::r#stdout(); }",
        "fn f() { tokio::io::stdin(); }",
        "fn f() { std::os::unix::net::UnixStream::connect(\"synthetic\"); }",
    ] {
        assert!(
            check_source(&application, source, false).is_err(),
            "accepted application OS access: {source}"
        );
    }
}

#[test]
fn transport_cannot_name_or_reexport_protection_through_aliases() {
    let contract = contract();
    let transport = rule(&contract, "openwrt-mcp-transport");
    assert!(
        check_source(
            &transport,
            "use openwrt_mcp_runtime::{Dispatcher, Limits};",
            false
        )
        .is_ok()
    );
    for source in [
        "use openwrt_mcp_runtime::protection::Material;",
        "use openwrt_mcp_runtime::protection as keys;",
        "pub use openwrt_mcp_runtime::protection::*;",
        "use openwrt_mcp_runtime as app; pub use app::protection::Material;",
        "mod wrapper { pub use openwrt_mcp_runtime::protection as keys; }",
        "use r#openwrt_mcp_runtime::r#protection::Material;",
    ] {
        assert!(
            check_source(&transport, source, false).is_err(),
            "accepted protection access: {source}"
        );
    }
    let aliases = BTreeMap::from([("app".into(), "openwrt_mcp_runtime".into())]);
    assert!(
        check_source_with_aliases(
            &transport,
            "use app::protection::Material;",
            false,
            &aliases
        )
        .is_err()
    );
}

#[test]
fn protection_namespace_cannot_be_flattened_by_its_producer() {
    let protected = BTreeSet::from(["protection".into()]);
    for source in [
        "pub use protection::Material;",
        "pub use crate::protection as keys;",
        "pub use self::protection::*;",
        "use crate::protection::Material as Hidden; pub type Exposed = Hidden;",
        "mod wrapper { pub use super::protection::Material; } pub use wrapper::*;",
        "use crate::protection as p; pub struct Exposed { pub key: p::Material }",
        "pub fn expose() -> crate::protection::Material { unimplemented!() }",
        "pub use r#protection::Material;",
    ] {
        assert!(
            check_public_reexports(source, &protected, "openwrt_mcp_runtime::").is_err(),
            "accepted flattened namespace: {source}"
        );
    }
    for source in [
        "pub mod protection; pub use dispatcher::Dispatcher;",
        "mod protection { pub struct Material; }",
        "use crate::protection::Material; fn internal(_: Material) {}",
        "#[cfg(test)] mod tests { pub use crate::protection::Material; }",
    ] {
        assert!(
            check_public_reexports(source, &protected, "openwrt_mcp_runtime::").is_ok(),
            "rejected protected namespace definition: {source}"
        );
    }
}

#[test]
fn age_adapter_receives_streams_without_key_custody_io() {
    let contract = contract();
    let crypto = rule(&contract, "openwrt-mcp-crypto-age");
    assert!(check_source(&crypto, "use std::io::{Read, Write}; use openwrt_mcp_runtime::protection::Material; pub fn stream(_: &mut dyn Read, _: &mut dyn Write) {}", false).is_ok());
    for source in [
        "fn f() { std::fs::File::open(\"synthetic\"); }",
        "fn f() { std::env::var(\"SYNTHETIC_ONLY\"); }",
        "use std::fs as source;",
        "fn f() { std::io::stdin(); }",
        "fn f() { std::io::pipe(); }",
        "fn f() { std::thread::spawn(|| ()); }",
        "fn f() { std::process::Command::new(\"synthetic\"); }",
    ] {
        assert!(
            check_source(&crypto, source, false).is_err(),
            "accepted crypto custody/OS access: {source}"
        );
    }
}

#[test]
fn custody_and_cipher_edges_remain_independent_for_every_dependency_kind() {
    let contract = contract();
    for (owner, denied) in [
        ("openwrt-mcp-crypto-age", "openwrt-mcp-key-sources"),
        ("openwrt-mcp-crypto-age", "zip"),
        ("openwrt-mcp-crypto-age", "rustix"),
        ("openwrt-mcp-key-sources", "age"),
        ("openwrt-mcp-key-sources", "openwrt-mcp-crypto-age"),
        ("openwrt-mcp-runtime", "age"),
        ("openwrt-mcp-runtime", "zip"),
        ("openwrt-mcp-transport", "openwrt-mcp-key-sources"),
        ("openwrt-mcp-transport", "openwrt-mcp-crypto-age"),
    ] {
        for kind in [Value::Null, json!("dev"), json!("build")] {
            assert!(
                check_metadata(&contract, &edge(&contract, owner, denied, kind)).is_err(),
                "accepted edge {owner} -> {denied}"
            );
        }
    }
}

#[test]
fn only_approved_algorithm_custody_and_composition_dependencies_are_accepted() {
    let contract = contract();
    for (owner, allowed) in [
        ("openwrt-mcp-crypto-age", "age"),
        ("openwrt-mcp-crypto-age", "zeroize"),
        ("openwrt-mcp-key-sources", "zip"),
        ("openwrt-mcp-key-sources", "rustix"),
        ("openwrt-mcp-key-sources", "zeroize"),
        ("openwrt-mcp-runtime", "zeroize"),
        ("openwrt-mcp", "openwrt-mcp-key-sources"),
        ("openwrt-mcp", "openwrt-mcp-crypto-age"),
    ] {
        assert!(
            check_metadata(&contract, &edge(&contract, owner, allowed, Value::Null)).is_ok(),
            "rejected approved edge {owner} -> {allowed}"
        );
    }
    assert!(
        check_metadata(
            &contract,
            &edge(&contract, "openwrt-mcp", "age", Value::Null)
        )
        .is_err()
    );
    assert!(
        check_metadata(
            &contract,
            &edge(&contract, "openwrt-mcp", "age", json!("dev"))
        )
        .is_ok()
    );
    for (owner, future) in [
        ("openwrt-mcp-crypto-age", "future_cipher"),
        ("openwrt-mcp-key-sources", "future_archive"),
    ] {
        assert!(check_metadata(&contract, &edge(&contract, owner, future, Value::Null)).is_err());
    }
}
