//! Development-only traceability checks inside the mandatory feature suite.
//! The real catalog is the authority; this is not a second runtime registry.
use openwrt_mcp_core::{Catalog, Permission};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};
use toml::Value;

type Check<T> = Result<T, &'static str>;

fn fields(value: &Value, expected: &[&str]) -> Check<()> {
    let table = value.as_table().ok_or("table required")?;
    let actual: BTreeSet<_> = table.keys().map(String::as_str).collect();
    if actual != expected.iter().copied().collect() {
        return Err("unknown or missing field");
    }
    Ok(())
}

fn string<'a>(value: &'a Value, key: &str) -> Check<&'a str> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or("text required")?;
    if text.is_empty() || text.len() > 2048 || text.chars().any(char::is_control) {
        return Err("invalid text");
    }
    Ok(text)
}

fn array<'a>(value: &'a Value, key: &str) -> Check<&'a [Value]> {
    let items = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or("array required")?;
    if items.is_empty() || items.len() > 4096 {
        return Err("invalid array bound");
    }
    Ok(items)
}

fn strings<'a>(value: &'a Value, key: &str) -> Check<Vec<&'a str>> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for item in array(value, key)? {
        let text = item.as_str().ok_or("text item required")?;
        if text.is_empty()
            || text.len() > 2048
            || text.chars().any(char::is_control)
            || !seen.insert(text)
        {
            return Err("invalid or repeated text item");
        }
        output.push(text);
    }
    Ok(output)
}

fn evidence(value: &Value, root: &Path) -> Check<()> {
    let root = root.canonicalize().map_err(|_| "invalid repository")?;
    for text in strings(value, "evidence")? {
        let path = Path::new(text);
        if text.len() > 512
            || text.contains('\\')
            || text.contains(':')
            || !matches!(
                text.split('/').next(),
                Some("docs" | "crates" | "compatibility")
            )
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("unsafe evidence path");
        }
        let resolved = root
            .join(path)
            .canonicalize()
            .map_err(|_| "missing evidence")?;
        if !resolved.starts_with(&root) || !resolved.is_file() {
            return Err("invalid evidence target");
        }
    }
    Ok(())
}

fn feature_ids(document: &str) -> Check<BTreeSet<&str>> {
    let before_mapping = document
        .split("## 4.")
        .next()
        .ok_or("missing feature specification")?;
    let mut result = BTreeSet::new();
    for line in before_mapping.lines().filter(|s| s.starts_with("| ")) {
        let id = line.split('|').nth(1).unwrap().trim();
        if id.split_once('-').is_some_and(|(family, number)| {
            !family.is_empty()
                && family.bytes().all(|c| c.is_ascii_uppercase())
                && number.len() == 2
                && number.bytes().all(|c| c.is_ascii_digit())
        }) && !result.insert(id)
        {
            return Err("duplicate feature ID");
        }
    }
    if result.len() != 156 {
        return Err("feature specification cardinality changed");
    }
    Ok(result)
}

fn documented_operations(document: &str) -> Check<BTreeMap<&str, &str>> {
    let section = document
        .split("## 4.")
        .nth(1)
        .ok_or("missing mapping section")?
        .split("## 5.")
        .next()
        .unwrap();
    let mut result = BTreeMap::new();
    for line in section.lines().filter(|s| s.starts_with("| ")) {
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        if columns.len() != 4 || !columns[2].contains('`') {
            continue;
        }
        for name in columns[2].split('`').skip(1).step_by(2) {
            if result.insert(name, columns[1]).is_some() {
                return Err("duplicate documented operation");
            }
        }
    }
    Ok(result)
}

fn recorded_tokens(reference: &str) -> Check<BTreeMap<&str, BTreeSet<&str>>> {
    let package_section = reference
        .split("| Package/family | Observed version |")
        .nth(1)
        .ok_or("missing package record")?
        .split("## Visible management API metadata")
        .next()
        .unwrap();
    let packages = package_section
        .lines()
        .filter(|s| s.starts_with("| ") && !s.starts_with("| ---"))
        .map(|s| s.split('|').nth(1).unwrap().trim())
        .collect();
    let objects = reference
        .lines()
        .find_map(|line| {
            line.split_once("Common visible objects included ")
                .and_then(|(_, objects)| objects.strip_suffix('.'))
        })
        .ok_or("missing object record")?;
    let objects = objects.split(", ").flat_map(|s| s.split(" and ")).collect();
    let methods = reference
        .split("Relevant observations include ")
        .nth(1)
        .and_then(|s| s.split_once(". Presence").map(|p| p.0))
        .ok_or("missing method record")?;
    Ok(BTreeMap::from([
        ("package", packages),
        ("object", objects),
        ("methods", methods.split("; ").collect()),
    ]))
}

fn validate(
    data: &Value,
    root: &Path,
    catalog: &Catalog,
    spec: &str,
    reference: &str,
) -> Check<()> {
    fields(
        data,
        &[
            "schema_version",
            "snapshot",
            "scope",
            "whole_device_complete",
            "profiles",
            "surfaces",
            "observations",
        ],
    )?;
    if data.get("schema_version").and_then(Value::as_integer) != Some(1)
        || string(data, "snapshot")? != "p1-v20"
        || string(data, "scope")? != "recorded_not_live"
        || data.get("whole_device_complete").and_then(Value::as_bool) != Some(false)
    {
        return Err("unsupported or overstated inventory scope");
    }
    let ids = feature_ids(spec)?;
    let registry: Value = toml::from_str(
        &fs::read_to_string(root.join("compatibility/evidence.toml"))
            .map_err(|_| "missing target registry")?,
    )
    .map_err(|_| "invalid target registry")?;
    let targets = array(&registry, "targets")?;
    let mut profiles = BTreeSet::new();
    for profile in array(data, "profiles")? {
        fields(profile, &["id", "scope", "evidence"])?;
        let id = string(profile, "id")?;
        if !profiles.insert(id) || string(profile, "scope")? != "historical" {
            return Err("invalid profile");
        }
        let target = targets
            .iter()
            .find(|t| t.get("id").and_then(Value::as_str) == Some(id))
            .ok_or("unknown target")?;
        if strings(profile, "evidence")? != strings(target, "sources")? {
            return Err("target provenance mismatch");
        }
        evidence(profile, root)?;
    }
    let expected_profiles: BTreeSet<_> = targets
        .iter()
        .map(|t| string(t, "id"))
        .collect::<Check<_>>()?;
    if profiles != expected_profiles {
        return Err("missing recorded profile");
    }
    let mut operations = BTreeMap::new();
    let mut features = BTreeSet::new();
    for surface in array(data, "surfaces")? {
        fields(
            surface,
            &[
                "feature",
                "risk",
                "operations",
                "selectors",
                "actions",
                "status",
                "evidence",
                "gap",
            ],
        )?;
        let feature = string(surface, "feature")?;
        if !ids.contains(feature) || !features.insert(feature) {
            return Err("unknown or duplicate feature");
        }
        if string(surface, "status")? != "partial" || strings(surface, "actions")? != ["R"] {
            return Err("overstated feature scope");
        }
        string(surface, "gap")?;
        string(surface, "risk")?;
        strings(surface, "selectors")?;
        evidence(surface, root)?;
        for name in strings(surface, "operations")? {
            let operation = catalog.get(name).ok_or("unknown operation")?;
            if operation
                .requirements
                .iter()
                .any(|r| r.permission != Permission::Read)
                || operations.insert(name, feature).is_some()
            {
                return Err("invalid or duplicate operation mapping");
            }
        }
    }
    if operations.len() != catalog.operations().len() {
        return Err("unmapped catalog operation");
    }
    if operations != documented_operations(spec)? {
        return Err("feature mapping differs from specification");
    }
    let mut observed: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut observation_ids = BTreeSet::new();
    let mut unknown_ids = BTreeSet::new();
    for observation in array(data, "observations")? {
        fields(
            observation,
            &[
                "id", "risk", "kind", "profile", "tokens", "features", "state", "evidence", "gap",
            ],
        )?;
        let id = string(observation, "id")?;
        if !observation_ids.insert(id) || !profiles.contains(string(observation, "profile")?) {
            return Err("invalid observation identity");
        }
        for feature in strings(observation, "features")? {
            if !ids.contains(feature) {
                return Err("unknown observation feature");
            }
        }
        string(observation, "gap")?;
        string(observation, "risk")?;
        evidence(observation, root)?;
        let kind = string(observation, "kind")?;
        let state = string(observation, "state")?;
        let tokens = strings(observation, "tokens")?;
        match (kind, state) {
            ("package" | "object" | "methods", "recorded") => {
                if string(observation, "profile")? != "bpi-r4-openwrt-25.12.5-reference"
                    || strings(observation, "evidence")? != ["docs/reference-target.md"]
                {
                    return Err("record provenance mismatch");
                }
                for token in tokens {
                    if !observed.entry(kind).or_default().insert(token) {
                        return Err("duplicate recorded token");
                    }
                }
            }
            ("unknown" | "cli" | "hardware", "unknown") => {
                unknown_ids.insert(id);
            }
            _ => return Err("unsupported observation state"),
        }
    }
    if observed != recorded_tokens(reference)? {
        return Err("recorded surface omitted or invented");
    }
    let inventory_doc = fs::read_to_string(root.join("docs/management-inventory.md"))
        .map_err(|_| "missing inventory contract")?;
    let documented_unknowns: BTreeSet<_> = inventory_doc
        .lines()
        .filter_map(|line| {
            line.strip_prefix("| `")
                .and_then(|s| s.split_once("` | ").map(|pair| pair.0))
        })
        .collect();
    if unknown_ids.is_empty() || unknown_ids != documented_unknowns {
        return Err("missing unknown surface");
    }
    Ok(())
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn data() -> Value {
    toml::from_str(include_str!(
        "../../../../compatibility/management-surfaces.toml"
    ))
    .unwrap()
}
fn check(value: &Value) -> Check<()> {
    validate(
        value,
        &root(),
        &openwrt_mcp_features::catalog(vec![]).unwrap(),
        include_str!("../../../../docs/management-feature-spec.md"),
        include_str!("../../../../docs/reference-target.md"),
    )
}

#[test]
fn inventory_maps_actual_catalog_and_all_retained_reference_surfaces() {
    check(&data()).unwrap();
}

#[test]
fn inventory_rejects_missing_duplicate_unknown_or_misclassified_tools() {
    let original = data();
    let mut bad = original.clone();
    bad["surfaces"].as_array_mut().unwrap().remove(0);
    assert_eq!(check(&bad), Err("unmapped catalog operation"));
    let mut bad = original.clone();
    bad["surfaces"][1]["operations"][0] = "system_board".into();
    assert_eq!(check(&bad), Err("invalid or duplicate operation mapping"));
    let mut bad = original.clone();
    bad["surfaces"][0]["operations"][0] = "unimplemented_tool".into();
    assert_eq!(check(&bad), Err("unknown operation"));
    let mut bad = original.clone();
    bad["surfaces"][0]["feature"] = "TXN-01".into();
    assert_eq!(
        check(&bad),
        Err("feature mapping differs from specification")
    );
    let mut bad = original;
    bad["surfaces"][0]["feature"] = "SYS-99".into();
    assert_eq!(check(&bad), Err("unknown or duplicate feature"));
}

#[test]
fn inventory_rejects_evidence_escalation_schema_drift_and_unsafe_paths() {
    for (field, value) in [
        ("schema_version", 2.into()),
        ("scope", "live".into()),
        ("whole_device_complete", true.into()),
        ("execute", true.into()),
    ] {
        let mut bad = data();
        bad.as_table_mut().unwrap().insert(field.into(), value);
        assert!(check(&bad).is_err());
    }
    for (field, value) in [
        ("status", "complete".into()),
        ("actions", vec![Value::from("W")].into()),
        ("gap", "".into()),
        ("risk", "".into()),
    ] {
        let mut bad = data();
        bad["surfaces"][0][field] = value;
        assert!(check(&bad).is_err());
    }
    for path in [
        "../outside",
        "docs/../../outside",
        "C:/private/key",
        "docs\\validation.md",
        "docs/missing-evidence.md",
        "docs",
    ] {
        let mut bad = data();
        bad["surfaces"][0]["evidence"][0] = path.into();
        assert!(check(&bad).is_err());
    }
    let mut bad = data();
    bad["profiles"][0]["scope"] = "current".into();
    assert!(check(&bad).is_err());
    let mut bad = data();
    bad["observations"][0]
        .as_table_mut()
        .unwrap()
        .remove("risk");
    assert!(check(&bad).is_err());
    let mut bad = data();
    bad["observations"][0]["state"] = "absent".into();
    assert!(check(&bad).is_err());
    let mut bad = data();
    bad["observations"][0]["profile"] = "qemu-arm64-openwrt-25.12.5".into();
    assert!(check(&bad).is_err());
}

#[test]
fn inventory_rejects_lost_records_and_unknowns_without_guessing_absence() {
    let mut bad = data();
    bad["observations"].as_array_mut().unwrap().remove(0);
    assert_eq!(check(&bad), Err("recorded surface omitted or invented"));
    let mut bad = data();
    bad["observations"].as_array_mut().unwrap().pop();
    assert_eq!(check(&bad), Err("missing unknown surface"));
    let mut bad = data();
    bad["profiles"].as_array_mut().unwrap().pop();
    assert_eq!(check(&bad), Err("missing recorded profile"));
    let mut bad = data();
    bad["observations"][0]["tokens"][0] = "invented package".into();
    assert_eq!(check(&bad), Err("recorded surface omitted or invented"));
    let mut bad = data();
    bad["observations"][0]["features"][0] = "CTL-99".into();
    assert_eq!(check(&bad), Err("unknown observation feature"));
}
