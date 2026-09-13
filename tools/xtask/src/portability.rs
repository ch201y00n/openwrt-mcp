//! Host portability is checked separately from architectural dependency direction.
//! These static checks cannot establish native execution or dependency semantics.
use std::{collections::BTreeSet, fs, path::Path};

use proc_macro2::{TokenStream, TokenTree};
use serde_json::Value;
use syn::{
    Attribute, Item,
    visit::{self, Visit},
};

use crate::{CheckResult, Contract, spec::safe_path};

const HOSTS: [&str; 3] = ["windows", "linux", "macos"];
const RUNNERS: [&str; 3] = ["windows-latest", "ubuntu-latest", "macos-latest"];
const GATE: &str = "./tools/Test-Repository.ps1 -BaseRef $env:ARCHITECTURE_BASE";

impl Contract {
    pub(crate) fn validate_portability(&self, root: &Path) -> CheckResult {
        if !exact_strings(&self.required_hosts, &HOSTS) {
            return Err("portability requires exactly Windows, Linux and macOS hosts".into());
        }
        let portable: BTreeSet<_> = self.portable_crates.iter().collect();
        if portable.len() != self.portable_crates.len()
            || portable.is_empty()
            || portable
                .iter()
                .any(|name| !self.crates.iter().any(|rule| &rule.name == *name))
        {
            return Err("portable crate classification is missing, duplicate or unowned".into());
        }
        for rule in &self.crates {
            if matches!(
                rule.layer.as_str(),
                "domain" | "features" | "application" | "transport"
            ) && !portable.contains(&rule.name)
            {
                return Err("core architectural layers must remain portable".into());
            }
        }
        if self.required_portable_tests.is_empty()
            || self
                .required_portable_tests
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                != self.required_portable_tests.len()
        {
            return Err("required portable suites must be nonempty and unique".into());
        }
        for file in &self.required_portable_tests {
            self.portable_test_command(file)?;
            self.validate_portable_target(root, file)?;
            let source = fs::read_to_string(root.join(file))
                .map_err(|_| format!("missing required portable suite: {file}"))?;
            check_portable_suite(&source).map_err(|error| format!("{file}: {error}"))?;
        }
        if !safe_path(&self.native_ci)
            || !self.native_ci.starts_with(".github/workflows/")
            || !self.native_ci.ends_with(".yml")
        {
            return Err("native CI must name a reviewed workflow path".into());
        }
        let workflow = fs::read_to_string(root.join(&self.native_ci))
            .map_err(|_| "missing native CI workflow")?;
        check_native_ci(self, &workflow)
    }

    fn portable_test_command(&self, file: &str) -> CheckResult<String> {
        let (owner, _) = self.owner(file)?;
        let name = file
            .strip_prefix(&format!("{}/tests/", owner.path))
            .and_then(|name| name.strip_suffix(".rs"))
            .filter(|name| {
                !name.is_empty()
                    && !name.contains('/')
                    && name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            })
            .ok_or("required portable suites must be standard integration-test targets")?;
        Ok(format!(
            "cargo test --locked -p {} --test {name}",
            owner.name
        ))
    }

    pub(crate) fn validate_portable_target(&self, root: &Path, file: &str) -> CheckResult {
        let (owner, _) = self.owner(file)?;
        let local = file
            .strip_prefix(&format!("{}/", owner.path))
            .ok_or("unowned portable target")?;
        let name = Path::new(file)
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("invalid portable target name")?;
        let manifest: toml::Value = toml::from_str(
            &fs::read_to_string(root.join(&owner.path).join("Cargo.toml"))
                .map_err(|_| "portable target manifest is missing")?,
        )
        .map_err(|_| "invalid portable target manifest")?;
        let mut explicit = false;
        if let Some(targets) = manifest.get("test").and_then(toml::Value::as_array) {
            for target in targets {
                if target.get("name").and_then(toml::Value::as_str) == Some(name)
                    || target.get("path").and_then(toml::Value::as_str) == Some(local)
                {
                    if explicit
                        || target.get("name").and_then(toml::Value::as_str) != Some(name)
                        || target
                            .get("path")
                            .and_then(toml::Value::as_str)
                            .is_some_and(|path| path != local)
                        || target.get("harness").and_then(toml::Value::as_bool) == Some(false)
                        || target.get("test").and_then(toml::Value::as_bool) == Some(false)
                        || target
                            .get("required-features")
                            .and_then(toml::Value::as_array)
                            .is_some_and(|features| !features.is_empty())
                    {
                        return Err("required portable target cannot disable its harness or require features".into());
                    }
                    explicit = true;
                }
            }
        }
        if !explicit
            && manifest
                .get("package")
                .and_then(|package| package.get("autotests"))
                .and_then(toml::Value::as_bool)
                == Some(false)
        {
            return Err("required portable target cannot be hidden by autotests=false".into());
        }
        Ok(())
    }
}

fn exact_strings(values: &[String], expected: &[&str]) -> bool {
    values.len() == expected.len()
        && values.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == expected.iter().copied().collect::<BTreeSet<_>>()
}

fn identifier(value: &syn::Ident) -> String {
    value.to_string().trim_start_matches("r#").to_owned()
}

fn os_predicate(tokens: TokenStream) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(value) => matches!(
            identifier(&value).as_str(),
            "unix" | "windows" | "target_os" | "target_family" | "target_env" | "target_vendor"
        ),
        TokenTree::Group(value) => os_predicate(value.stream()),
        _ => false,
    })
}

fn is_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        let name = attribute
            .path()
            .segments
            .iter()
            .map(|part| identifier(&part.ident))
            .collect::<Vec<_>>()
            .join("::");
        name == "test" || name == "tokio::test"
    })
}

fn test_module(item: &Item) -> bool {
    match item {
        Item::Mod(item) => item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("test"))
        }),
        Item::Fn(item) => is_test(&item.attrs),
        _ => false,
    }
}

struct PortableCheck {
    suite: bool,
    tests: usize,
    errors: BTreeSet<String>,
}

impl PortableCheck {
    fn configuration(&mut self, name: &str, tokens: TokenStream) {
        if self.suite && matches!(name, "cfg" | "cfg_attr" | "ignore") {
            self.errors.insert(
                "required portable suites cannot use cfg/cfg_attr/ignore attributes".into(),
            );
        } else if matches!(name, "cfg" | "cfg_attr") && os_predicate(tokens) {
            self.errors
                .insert("portable production cannot branch on host OS cfg".into());
        }
    }

    // Inspect approved macro arguments as well as ordinary Rust AST nodes.
    fn tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.tokens(group.stream());
            }
            if let TokenTree::Ident(name) = token {
                let name = identifier(name);
                let next = if matches!(tokens.get(index + 1), Some(TokenTree::Punct(p)) if p.as_char() == '!')
                {
                    index + 2
                } else {
                    index + 1
                };
                if let Some(TokenTree::Group(group)) = tokens.get(next)
                    && matches!(name.as_str(), "cfg" | "cfg_attr" | "ignore")
                {
                    self.configuration(&name, group.stream());
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for PortableCheck {
    fn visit_item(&mut self, item: &'ast Item) {
        if !self.suite && test_module(item) {
            return;
        }
        visit::visit_item(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if self.suite && is_test(&item.attrs) {
            if item.block.stmts.is_empty() {
                self.errors
                    .insert("required portable test bodies cannot be empty".into());
            }
            self.tests += 1;
        }
        visit::visit_item_fn(self, item);
    }

    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        let name = attribute
            .path()
            .segments
            .last()
            .map(|part| identifier(&part.ident))
            .unwrap_or_default();
        let tokens = match &attribute.meta {
            syn::Meta::List(list) => list.tokens.clone(),
            _ => TokenStream::new(),
        };
        self.configuration(&name, tokens.clone());
        self.tokens(tokens);
        visit::visit_attribute(self, attribute);
    }

    fn visit_macro(&mut self, value: &'ast syn::Macro) {
        let name = value
            .path
            .segments
            .last()
            .map(|part| identifier(&part.ident))
            .unwrap_or_default();
        // cfg! is an expression: portable tests may inspect CPU configuration,
        // but must not turn their behavior into an OS-dependent no-op.
        if name == "cfg" && os_predicate(value.tokens.clone()) {
            self.errors
                .insert("portable code cannot branch on host OS cfg!".into());
        }
        self.tokens(value.tokens.clone());
    }
}

fn check_portable(source: &str, suite: bool) -> CheckResult {
    let ast = syn::parse_file(source).map_err(|_| "invalid portable Rust source")?;
    let mut checker = PortableCheck {
        suite,
        tests: 0,
        errors: BTreeSet::new(),
    };
    checker.visit_file(&ast);
    if suite && checker.tests == 0 {
        checker
            .errors
            .insert("required portable suite has no executable tests".into());
    }
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors.into_iter().collect::<Vec<_>>().join("; "))
    }
}

pub fn check_portable_source(source: &str) -> CheckResult {
    check_portable(source, false)
}

pub fn check_portable_suite(source: &str) -> CheckResult {
    check_portable(source, true)
}

fn keys_are(value: &Value, allowed: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.keys().all(|key| allowed.contains(&key.as_str())))
}

/// The workflow uses JSON syntax (a YAML subset) to avoid another parser dependency.
/// Required job/step shape is deliberately conservative: no conditional or soft-fail gate.
pub fn check_native_ci(contract: &Contract, source: &str) -> CheckResult {
    let ci: Value =
        serde_json::from_str(source).map_err(|_| "native CI must use checked JSON syntax")?;
    if !keys_are(&ci, &["name", "on", "permissions", "jobs"])
        || ci["on"].as_object().is_none_or(|events| {
            events.len() != 2
                || !events.contains_key("push")
                || !events.contains_key("pull_request")
                || events.values().any(|event| {
                    !(event.is_null() || event.as_object().is_some_and(|v| v.is_empty()))
                })
        })
    {
        return Err("native CI must run unconditionally on pushes and pull requests".into());
    }
    let job = &ci["jobs"]["rust"];
    let matrix = &job["strategy"]["matrix"];
    let runners = matrix["os"]
        .as_array()
        .and_then(|values| {
            values
                .iter()
                .map(|value| value.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or("native CI must declare an OS matrix")?;
    if !exact_strings(&runners, &RUNNERS)
        || job["runs-on"] != "${{ matrix.os }}"
        || !keys_are(job, &["runs-on", "strategy", "steps"])
        || !keys_are(&job["strategy"], &["fail-fast", "matrix"])
        || job["strategy"]["fail-fast"] != false
        || !keys_are(matrix, &["os"])
    {
        return Err(
            "native CI requires all three native runners without exclusions or allowed failure"
                .into(),
        );
    }
    let steps = job["steps"]
        .as_array()
        .ok_or("native CI steps are missing")?;
    let mut expected = BTreeSet::from([GATE.to_owned()]);
    if contract.version >= 8 {
        expected.insert("cargo test --locked -p openwrt-mcp-host-platform --test windows".into());
    }
    for file in &contract.required_portable_tests {
        expected.insert(contract.portable_test_command(file)?);
    }
    let mut found = BTreeSet::new();
    let mut gate_seen = false;
    for step in steps {
        if !keys_are(step, &["name", "uses", "with", "shell", "run", "env"]) {
            return Err("native CI steps cannot be conditional or continue on error".into());
        }
        if let Some(run) = step["run"].as_str() {
            if run.contains("UseWsl") || run.contains("wsl") {
                return Err("native CI cannot substitute WSL for a native host".into());
            }
            if expected.contains(run) {
                if step["shell"] != "pwsh"
                    || !step["uses"].is_null()
                    || !step["with"].is_null()
                    || (run != GATE && !step["env"].is_null())
                {
                    return Err(
                        "native CI required commands must run directly in PowerShell".into(),
                    );
                }
                if run == GATE {
                    if step["env"]
                        != serde_json::json!({"ARCHITECTURE_BASE":"${{ github.event.pull_request.base.sha || github.event.before }}"})
                    {
                        return Err("native CI must use the reviewed architecture baseline".into());
                    }
                    gate_seen = true;
                } else if !gate_seen {
                    return Err("native CI must run the full gate before portable suites".into());
                }
                if !found.insert(run.to_owned()) {
                    return Err("native CI repeats a required command".into());
                }
            }
        }
    }
    if found != expected {
        return Err(
            "native CI is missing the full gate or an explicit required portable suite".into(),
        );
    }
    Ok(())
}
