//! ADR 0005 checks architecture declarations and reviewable evidence scope.
//! These checks do not attest that a router ran a test or implement runtime gating.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use serde::Deserialize;

use crate::{CheckResult, Contract, spec::safe_path};

const CODEC: &str = "openwrt-mcp-device-codec";
const OBJECTS: [&str; 7] = [
    "system",
    "network.device",
    "network.interface",
    "network.interface.lan",
    "network.interface.wan",
    "iwinfo",
    "service",
];
const SUITES: [&str; 3] = [
    "crates/features/tests/capability_contracts.rs",
    "crates/runtime/tests/capabilities.rs",
    "crates/device-codec/tests/ubus_describe.rs",
];
const LEVELS: [&str; 5] = ["source", "synthetic", "native", "emulated", "exact_device"];
const REFERENCE: &str = "bpi-r4-openwrt-25.12.5-reference";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CapabilityContract {
    #[serde(default)]
    pub probe_profile: Option<String>,
    pub probe_registry: String,
    pub evidence_manifest: String,
    pub required_tests: Vec<String>,
}

impl Contract {
    pub(crate) fn validate_capabilities(&self, root: &Path) -> CheckResult {
        let capability = self
            .capability_contract
            .as_ref()
            .ok_or("v5 requires a capability contract")?;
        let profile = if self.version >= 11 {
            "base_luci_uci_v3"
        } else {
            "base_luci_v2"
        };
        if (self.version >= 9 && capability.probe_profile.as_deref() != Some(profile))
            || (self.version < 9 && capability.probe_profile.is_some())
        {
            return Err(
                "capability probe profile requires its reviewed architecture version".into(),
            );
        }
        if capability.probe_registry != "architecture/capability-probes.toml"
            || capability.evidence_manifest != "compatibility/evidence.toml"
            || !exact(&capability.required_tests, &SUITES)
            || capability
                .required_tests
                .iter()
                .any(|suite| !self.required_portable_tests.contains(suite))
        {
            return Err(
                "capability contract requires reviewed registries and all portable suites".into(),
            );
        }
        let codec = self
            .crates
            .iter()
            .find(|rule| rule.name == CODEC)
            .ok_or("capability codec boundary is missing")?;
        if codec.path != "crates/device-codec"
            || codec.layer != "infrastructure"
            || !self.portable_crates.iter().any(|name| name == CODEC)
            || !exact(
                &codec.dependencies,
                &["openwrt-mcp-core", "serde", "serde_json"],
            )
            || !codec.dev_dependencies.is_empty()
            || !codec.build_dependencies.is_empty()
        {
            return Err(
                "capability codec must remain portable and core-only with reviewed parsers".into(),
            );
        }
        for path in [
            "std::fs",
            "std::io::stdin",
            "std::io::stdout",
            "std::io::stderr",
            "std::io::pipe",
            "std::net",
            "std::os",
            "std::process",
            "std::thread",
            "std::env",
            "tokio",
        ] {
            if !codec.forbidden_paths.iter().any(|ban| ban == path) {
                return Err("capability codec cannot acquire concrete I/O".into());
            }
        }
        for rule in &self.crates {
            if rule.dependencies.iter().any(|name| name == CODEC)
                && !matches!(
                    rule.name.as_str(),
                    "openwrt-mcp-adapters" | "openwrt-mcp-backend-ssh"
                )
            {
                return Err("only target infrastructure may consume the capability codec".into());
            }
        }
        for name in ["openwrt-mcp-adapters", "openwrt-mcp-backend-ssh"] {
            if !self.crates.iter().any(|rule| {
                rule.name == name
                    && rule
                        .dependencies
                        .iter()
                        .any(|dependency| dependency == CODEC)
            }) {
                return Err("both target adapters require the shared codec boundary".into());
            }
        }
        check_capability_registry_for_version(
            &read_artifact(root, &capability.probe_registry)?,
            self.version,
        )?;
        check_compatibility_evidence(root, &read_artifact(root, &capability.evidence_manifest)?)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    schema_version: u64,
    runtime: RuntimeContract,
    probes: Vec<Probe>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeContract {
    backend_ownership: String,
    cache_ownership: String,
    epoch_binding: String,
    ttl_seconds: u64,
    unknown_behavior: String,
    audit_order: String,
    offline_surfaces: Vec<String>,
    operation_metadata: String,
    process_capability: String,
    metadata_use_case: String,
    metadata_tool: String,
    metadata_audit_kind: String,
    device_deadline: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    id: String,
    object: String,
    program: String,
    arguments: Vec<String>,
    effect: String,
}

/// Reject any expansion of the reviewed probe surface or runtime trust contract.
pub fn check_capability_registry(source: &str) -> CheckResult {
    check_capability_registry_for_version(source, 11)
}

/// Prior schemas are closed checkpoint subsets; v3 requires the v11 checkpoint.
pub fn check_capability_registry_for_version(
    source: &str,
    architecture_version: u64,
) -> CheckResult {
    let registry: Registry =
        toml::from_str(source).map_err(|_| "invalid capability probe registry or unknown field")?;
    let mut expected = BTreeSet::from(OBJECTS);
    match registry.schema_version {
        1 => {}
        2 if architecture_version >= 9 => {
            expected.extend(["luci", "luci-rpc"]);
        }
        3 if architecture_version >= 11 => {
            expected.extend(["luci", "luci-rpc", "uci"]);
        }
        _ => {
            return Err(
                "capability registry profile requires reviewed architecture evolution".into(),
            );
        }
    }
    let runtime = &registry.runtime;
    if runtime.backend_ownership != "same_immutable_backend"
        || runtime.cache_ownership != "private_dispatcher"
        || runtime.epoch_binding != "opaque_backend_epoch"
        || runtime.ttl_seconds != 30
        || runtime.unknown_behavior != "fail_closed"
        || runtime.audit_order != "start_before_cache_probe_execute"
        || !exact(
            &runtime.offline_surfaces,
            &["check", "catalog", "tools_list"],
        )
        || runtime.operation_metadata != "required_ubus_method_signature_response_contract"
        || runtime.process_capability != "unverified_blocked"
        || runtime.metadata_use_case != "capability_status"
        || runtime.metadata_tool != "operation_capability"
        || runtime.metadata_audit_kind != "capability"
        || runtime.device_deadline != "shared_probe_execute"
    {
        return Err("capability registry weakens the reviewed runtime lifecycle".into());
    }
    let mut objects = BTreeSet::new();
    for probe in &registry.probes {
        if !expected.contains(probe.object.as_str())
            || !objects.insert(probe.object.as_str())
            || probe.id != format!("ubus.{}", probe.object)
            || probe.program != "/bin/ubus"
            || probe.arguments != ["-v", "list", probe.object.as_str()]
            || probe.effect != "read"
        {
            return Err("capability probe must be exact reviewed read-only ubus -v list".into());
        }
    }
    if objects != expected {
        return Err("capability registry is missing reviewed object probes".into());
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema_version: u64,
    allowed_levels: Vec<String>,
    targets: Vec<Target>,
    records: Vec<Record>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Target {
    id: String,
    kind: String,
    release: String,
    revision: String,
    kernel: String,
    target: String,
    board: String,
    observed_at: String,
    inventory_observed_at: String,
    inventory_only: bool,
    sources: Vec<String>,
    packages: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: String,
    level: String,
    target: String,
    operations: Vec<String>,
    artifacts: Vec<String>,
    observed_at: String,
    host: Option<String>,
    environment: Option<String>,
}

/// Reviewable evidence declarations are not proof of execution or runtime authority.
pub fn check_compatibility_evidence(root: &Path, source: &str) -> CheckResult {
    let evidence: Evidence =
        toml::from_str(source).map_err(|_| "invalid compatibility evidence or unknown field")?;
    if evidence.schema_version != 1 || !exact(&evidence.allowed_levels, &LEVELS) {
        return Err(
            "compatibility evidence must distinguish all reviewed validation levels".into(),
        );
    }
    let mut targets = BTreeMap::new();
    for target in &evidence.targets {
        if !identifier(&target.id)
            || !matches!(target.kind.as_str(), "physical" | "emulated")
            || targets.insert(target.id.as_str(), target).is_some()
            || [
                &target.release,
                &target.revision,
                &target.kernel,
                &target.target,
                &target.board,
            ]
            .iter()
            .any(|value| !bounded_label(value))
            || !timestamp(&target.observed_at)
            || !timestamp(&target.inventory_observed_at)
            || target.packages.len() > 128
            || target
                .packages
                .iter()
                .any(|(name, version)| !identifier(name) || !bounded_label(version))
            || ["ubus", "rpcd", "netifd", "procd"]
                .iter()
                .any(|name| !target.packages.contains_key(*name))
            || target.packages.contains_key("apk") == target.packages.contains_key("opkg")
        {
            return Err(
                "compatibility target needs unique identity, timestamps and package fingerprints"
                    .into(),
            );
        }
        artifacts(root, &target.sources)?;
    }
    if targets
        .get(REFERENCE)
        .is_none_or(|target| target.kind != "physical")
    {
        return Err("compatibility evidence is missing the initial reference scope".into());
    }
    let mut ids = BTreeSet::new();
    let mut device_accepted = BTreeSet::new();
    for record in &evidence.records {
        let target = targets
            .get(record.target.as_str())
            .ok_or("compatibility record references an unknown target")?;
        if !identifier(&record.id)
            || !ids.insert(&record.id)
            || !LEVELS.contains(&record.level.as_str())
            || !timestamp(&record.observed_at)
            || record.operations.is_empty()
            || record.operations.iter().collect::<BTreeSet<_>>().len() != record.operations.len()
            || record
                .operations
                .iter()
                .any(|operation| !identifier(operation))
        {
            return Err(
                "compatibility record has invalid level, identity or operation scope".into(),
            );
        }
        artifacts(root, &record.artifacts)?;
        if matches!(record.level.as_str(), "exact_device" | "emulated") {
            if target.inventory_only {
                return Err("inventory-only target is not target operation acceptance".into());
            }
            if (record.level == "exact_device" && target.kind != "physical")
                || (record.level == "emulated" && target.kind != "emulated")
            {
                return Err(
                    "emulated userspace evidence is not exact physical-device acceptance".into(),
                );
            }
            if record
                .artifacts
                .iter()
                .any(|path| target.sources.contains(path))
            {
                return Err(
                    "inventory provenance cannot substitute for operation acceptance artifacts"
                        .into(),
                );
            }
            device_accepted.insert(target.id.as_str());
        }
        let host = (record.host.as_deref(), record.environment.as_deref());
        let native = matches!(host, (Some("windows" | "linux" | "macos"), Some("native")));
        let executed = native || matches!(host, (Some("linux"), Some("wsl")));
        match record.level.as_str() {
            "source" if host != (None, None) => {
                return Err("source review is not host execution evidence".into());
            }
            "synthetic" if host != (None, None) && !executed => {
                return Err("synthetic execution needs a valid host/environment pair".into());
            }
            "native" if !native => {
                return Err(
                    "native evidence needs an actual native host; WSL is Linux only".into(),
                );
            }
            "exact_device" | "emulated" if !executed => {
                return Err("target acceptance needs a valid host/environment pair".into());
            }
            _ => {}
        }
    }
    if evidence
        .targets
        .iter()
        .any(|target| !target.inventory_only && !device_accepted.contains(target.id.as_str()))
    {
        return Err("non-inventory target requires scoped target acceptance evidence".into());
    }
    Ok(())
}

fn artifacts(root: &Path, paths: &[String]) -> CheckResult {
    if paths.is_empty()
        || paths.len() > 64
        || paths.iter().collect::<BTreeSet<_>>().len() != paths.len()
    {
        return Err("evidence requires nonempty unique bounded artifact paths".into());
    }
    for path in paths {
        if read_artifact(root, path)?.trim().is_empty() {
            return Err("evidence artifact cannot be empty".into());
        }
    }
    Ok(())
}

fn read_artifact(root: &Path, value: &str) -> CheckResult<String> {
    if !safe_path(value) || value.contains(':') || value.chars().any(char::is_control) {
        return Err("evidence/registry paths must be safe repository-relative paths".into());
    }
    let root = root.canonicalize().map_err(|_| "missing evidence root")?;
    let mut path = root.clone();
    for component in value.split('/') {
        path.push(component);
        if fs::symlink_metadata(&path)
            .map_err(|_| "missing evidence/registry artifact")?
            .file_type()
            .is_symlink()
        {
            return Err("evidence/registry artifact cannot use symlinks".into());
        }
    }
    let resolved = path
        .canonicalize()
        .map_err(|_| "missing evidence/registry artifact")?;
    let metadata = resolved
        .metadata()
        .map_err(|_| "unreadable evidence/registry artifact")?;
    if !resolved.starts_with(&root) || !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 {
        return Err("evidence/registry artifact is outside scope or exceeds its bound".into());
    }
    fs::read_to_string(resolved)
        .map_err(|_| "evidence/registry artifact must be readable UTF-8".into())
}

fn exact(values: &[String], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && values.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == BTreeSet::from_iter(expected.iter().copied())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn bounded_label(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && value.bytes().all(|byte| byte.is_ascii_graphic())
}

fn timestamp(value: &str) -> bool {
    value.parse::<toml::value::Datetime>().is_ok_and(|value| {
        value.date.is_some() && value.time.is_some() && value.offset == Some(toml::value::Offset::Z)
    })
}
