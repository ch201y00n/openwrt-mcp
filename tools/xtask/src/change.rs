use crate::{CheckResult, Contract};
use std::collections::BTreeSet;

/// Enforce evidence of architecture evolution against an explicit Git baseline.
/// Temporal authoring order is a review obligation, not inferable from a final diff.
pub fn validate_evolution(
    previous: Option<&str>,
    current: &str,
    changed: &BTreeSet<String>,
) -> CheckResult {
    let current = Contract::parse(current)?;
    let prior = previous.map(Contract::parse).transpose()?;
    if prior.as_ref() == Some(&current) {
        return Ok(());
    }
    if prior
        .as_ref()
        .is_some_and(|prior| current.version <= prior.version)
    {
        return Err("architecture changes require an increased contract version".into());
    }
    if let Some(prior) = prior
        && current.decision == prior.decision
    {
        return Err("architecture changes require a new ADR path".into());
    }
    for evidence in [
        "architecture/spec.toml",
        &current.decision,
        &current.requirements,
        &current.architecture,
    ] {
        if !changed.contains(evidence) {
            return Err(format!(
                "architecture evolution lacks changed evidence: {evidence}"
            ));
        }
    }
    if !current.decision.starts_with("docs/adr/") || !current.decision.ends_with(".md") {
        return Err("architecture decision must be an ADR document".into());
    }
    if !changed
        .iter()
        .any(|path| path.starts_with("tools/xtask/tests/") && path.ends_with(".rs"))
    {
        return Err("architecture changes require a harness regression test change".into());
    }
    if !changed
        .iter()
        .any(|path| path.starts_with("tools/xtask/src/") && path.ends_with(".rs"))
    {
        return Err("architecture changes require a harness implementation change".into());
    }
    Ok(())
}
