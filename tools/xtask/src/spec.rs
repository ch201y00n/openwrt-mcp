use crate::{
    CheckResult,
    capability::CapabilityContract,
    projection::{ActionResponseContract, McpResultContract, ProjectionContract},
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    #[serde(default)]
    pub uci_read_contract: Option<crate::uci_reads::UciReadContract>,
    #[serde(default)]
    pub windows_read_contract: Option<crate::windows_reads::WindowsReadContract>,
    pub version: u64,
    pub decision: String,
    pub requirements: String,
    pub architecture: String,
    #[serde(default)]
    pub required_hosts: Vec<String>,
    #[serde(default)]
    pub portable_crates: Vec<String>,
    #[serde(default)]
    pub required_portable_tests: Vec<String>,
    #[serde(default)]
    pub native_ci: String,
    #[serde(default)]
    pub capability_contract: Option<CapabilityContract>,
    #[serde(default)]
    pub projection_contract: Option<ProjectionContract>,
    #[serde(default)]
    pub action_response_contract: Option<ActionResponseContract>,
    #[serde(default)]
    pub mcp_result_contract: Option<McpResultContract>,
    #[serde(default)]
    pub package_contract: Option<crate::packages::PackageContract>,
    pub crates: Vec<CrateRule>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CrateRule {
    pub name: String,
    pub path: String,
    pub layer: String,
    pub dependencies: Vec<String>,
    pub dev_dependencies: Vec<String>,
    pub build_dependencies: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub allowed_macros: Vec<String>,
}

impl Contract {
    pub fn parse(text: &str) -> CheckResult<Self> {
        toml::from_str(text).map_err(|_| "invalid architecture contract or unknown field".into())
    }

    pub fn validate(&self, root: &Path) -> CheckResult {
        if self.version == 0 || self.crates.is_empty() {
            return Err("empty/versionless contract".into());
        }
        let mut names = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for rule in &self.crates {
            if !names.insert(&rule.name)
                || !paths.insert(&rule.path)
                || !safe_path(&rule.path)
                || !matches!(
                    rule.layer.as_str(),
                    "domain"
                        | "features"
                        | "application"
                        | "infrastructure"
                        | "transport"
                        | "composition"
                        | "development"
                )
            {
                return Err("duplicate or invalid crate ownership".into());
            }
            for edges in [
                &rule.dependencies,
                &rule.dev_dependencies,
                &rule.build_dependencies,
            ] {
                if edges.iter().collect::<BTreeSet<_>>().len() != edges.len() {
                    return Err("duplicate allowed dependency".into());
                }
            }
        }
        for first in &paths {
            if paths
                .iter()
                .any(|second| first != second && second.starts_with(&format!("{first}/")))
            {
                return Err("overlapping crate ownership".into());
            }
        }
        for document in [&self.decision, &self.requirements, &self.architecture] {
            if !safe_path(document) || !root.join(document).is_file() {
                return Err("architecture contract references a missing/unsafe document".into());
            }
        }
        if self.version >= 4 {
            self.validate_portability(root)?;
        }
        if self.version >= 5 {
            self.validate_capabilities(root)?;
        }
        if self.version >= 6 {
            self.validate_projection()?;
        }
        if self.version >= 7 {
            self.validate_packages()?;
        }
        if self.version >= 8 {
            self.validate_windows_reads(root)?;
        }
        self.validate_uci_reads()?;
        Ok(())
    }

    pub fn owner(&self, file: &str) -> CheckResult<(&CrateRule, bool)> {
        if !safe_path(file) {
            return Err("unsafe source path".into());
        }
        for rule in &self.crates {
            if let Some(local) = file.strip_prefix(&format!("{}/", rule.path)) {
                if local.starts_with("src/") {
                    return Ok((rule, rule.layer == "development"));
                }
                if local.starts_with("tests/") || local.starts_with("examples/") {
                    return Ok((rule, true));
                }
                return Err(format!(
                    "Rust source outside standard owned directories: {file}"
                ));
            }
        }
        Err(format!("unowned Rust source: {file}"))
    }
}

pub(crate) fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}
