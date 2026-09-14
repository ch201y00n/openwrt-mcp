//! P2 declarations and narrowly scoped source admission, not a transaction engine.
use crate::{CheckResult, Contract, CrateRule};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path};
use syn::visit::{self, Visit};

// The closed field set is validated below, including unknown fields and types.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GuardedMutationContract(BTreeMap<String, DeclarationValue>);

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
enum DeclarationValue {
    Text(String),
    Limit(i64),
    List(Vec<String>),
}

const RULES: &[(&str, &str)] = &[
    ("profile", "guarded_hostname_v1"),
    ("design", "docs/guarded-mutation-contracts.md"),
    ("delivery", "architecture_only_no_mutation_tool"),
    (
        "entry",
        "same_dispatcher_authorize_all_requirements_audit_each_request",
    ),
    (
        "intent",
        "one_existing_system_section_hostname_ascii_label_1_to_63",
    ),
    (
        "effects",
        "persistent_hostname_and_kernel_hostname_no_system_reload",
    ),
    (
        "protected_resources",
        "operator_owned_nonempty_before_after_closure_lan3_preserved",
    ),
    (
        "baseline",
        "private_exact_committed_bytes_pending_effects_target_boot_profile_policy",
    ),
    (
        "admission",
        "authenticated_device_single_owner_exclusive_operator_window",
    ),
    (
        "concurrency",
        "external_root_not_isolated_detected_drift_denies_no_blind_restore",
    ),
    (
        "recovery",
        "durable_ciphertext_journal_dedicated_device_identity_no_archival_identity",
    ),
    (
        "restart",
        "unconfirmed_job_recovers_before_new_admission_no_deadline_extension",
    ),
    (
        "confirmation",
        "owner_bound_fresh_verify_durable_compare_and_swap_before_expiry",
    ),
    (
        "uncertainty",
        "retain_recovery_authority_reconcile_status_never_resubmit",
    ),
    (
        "audit",
        "before_effect_fail_closed_after_effect_preserve_recovery",
    ),
    (
        "channels",
        "bounded_control_binary_capture_secret_and_job_status_separate",
    ),
    (
        "storage",
        "private_no_replace_atomic_visibility_durable_receipt_or_unknown",
    ),
    (
        "restore",
        "fully_authenticated_bounded_private_memory_no_plaintext_backup_files",
    ),
    (
        "secrets",
        "purpose_bound_zeroizing_non_debug_non_serde_no_paths_in_mcp",
    ),
    (
        "worker",
        "one_joinable_bounded_worker_abort_before_apply_not_detached",
    ),
    (
        "publication",
        "operator_provisioned_no_auto_install_or_key_generation",
    ),
];
const LIMITS: &[(&str, i64)] = &[
    ("max_jobs_per_device", 1),
    ("max_plans_per_host", 8),
    ("max_workers_per_host", 1),
    ("max_queued_jobs", 0),
    ("plan_ttl_seconds", 60),
    ("admission_timeout_seconds", 30),
    ("apply_timeout_seconds", 10),
    ("confirm_timeout_seconds", 120),
    ("recovery_timeout_seconds", 60),
    ("max_control_bytes", 16384),
    ("max_frame_bytes", 65536),
    ("max_secret_bytes", 65536),
    ("max_hostname_bytes", 63),
    ("max_system_file_bytes", 65536),
    ("max_recovery_plaintext_bytes", 131072),
    ("max_recovery_ciphertext_bytes", 1048576),
    ("max_journal_bytes", 1048576),
    ("max_retained_jobs", 32),
    ("retention_seconds", 604800),
    ("guardian_binary_mib", 10),
    ("guardian_idle_rss_mib", 16),
    ("guardian_peak_rss_mib", 32),
    ("guardian_idle_cpu_percent", 1),
];
const REQUIREMENTS: &[&str] = &[
    "plan:system.read",
    "status:system.read+job_owner",
    "apply:system.read_write+execute+effects+backup",
    "confirm:original_union+job_owner",
    "recover:original_union+job_owner",
    "automatic_recovery:admitted_obligation_not_new_client_authority",
];
const STATES: &[&str] = &[
    "planned",
    "admitted",
    "backup_complete",
    "recovery_armed",
    "applying",
    "awaiting_confirmation",
    "confirmed",
    "recovering",
    "recovered",
    "cancelled",
    "recovery_failed",
    "outcome_unknown",
];
const TRANSITIONS: &[&str] = &[
    "planned|admit|admitted|fresh_owner_policy_audit_baseline_effects_capacity",
    "admitted|seal|backup_complete|complete_capture_manifest_cipher_durable_receipt",
    "backup_complete|arm|recovery_armed|device_recovery_ciphertext_key_and_journal_durable",
    "recovery_armed|apply|applying|recheck_all_then_durable_intent_before_first_write",
    "applying|verify|awaiting_confirmation|fresh_committed_and_kernel_postconditions",
    "awaiting_confirmation|confirm|confirmed|owner_reauthorize_fresh_verify_unexpired_durable_cas",
    "planned|cancel|cancelled|no_device_effect",
    "admitted|cancel|cancelled|prove_no_write_release_owner",
    "backup_complete|cancel|cancelled|prove_no_write_preserve_published_backup",
    "recovery_armed|cancel|cancelled|durable_disarm_only_if_no_apply_intent",
    "applying|failure_or_expiry|recovering|durable_recovery_obligation",
    "awaiting_confirmation|failure_or_expiry|recovering|durable_recovery_obligation",
    "recovering|verify_recovery|recovered|fresh_preimage_postconditions_and_durable_terminal",
    "recovering|failure|recovery_failed|retain_ciphertext_and_admission_block",
    "applying|lost_evidence|outcome_unknown|retain_owner_journal_no_retry",
    "awaiting_confirmation|lost_evidence|outcome_unknown|retain_owner_journal_no_retry",
    "recovering|lost_evidence|outcome_unknown|retain_owner_journal_no_retry",
    "outcome_unknown|reconcile|recovering|authenticated_journal_unconfirmed_apply_intent",
    "outcome_unknown|reconcile_confirmed|confirmed|authenticated_durable_confirm_record",
    "outcome_unknown|reconcile_recovered|recovered|authenticated_terminal_and_fresh_preimage",
    "outcome_unknown|reconcile_failed|recovery_failed|authenticated_failure_preserve_artifacts",
];
const PORTS: &[&str] = &[
    "trait MutationObservation: Send + Sync { async fn observe(&self, binding: &TargetBinding, budget: WorkBudget) -> Result<PrivateBaseline, MutationError>; }",
    "trait DeviceAdmission: Send + Sync { async fn admit(&self, plan: &ValidatedPlan, owner: &OwnerBinding, budget: WorkBudget) -> Result<Admission, MutationError>; }",
    "trait BackupCapture: Send + Sync { async fn capture(&self, admission: &Admission, budget: WorkBudget) -> Result<CaptureLease, MutationError>; }",
    "trait CiphertextStore: Send + Sync { async fn stage(&self, artifact: &ArtifactBinding, budget: WorkBudget) -> Result<StageLease, MutationError>; async fn reconcile(&self, artifact: &ArtifactBinding) -> Result<PublicationState, MutationError>; }",
    "trait SecretSource: Send + Sync { async fn resolve(&self, reference: &SecretReference, purpose: SecretPurpose, budget: WorkBudget) -> Result<SecretValue, MutationError>; }",
    "trait DeviceJobControl: Send + Sync { async fn arm(&self, admission: &Admission, backup: &DurableBackup, budget: WorkBudget) -> Result<ArmedJob, MutationError>; async fn apply_once(&self, job: &ArmedJob, budget: WorkBudget) -> Result<JobReceipt, MutationError>; async fn status(&self, job: &OwnedJob, budget: WorkBudget) -> Result<PrivateJobStatus, MutationError>; async fn confirm(&self, job: &VerifiedJob, budget: WorkBudget) -> Result<JobReceipt, MutationError>; async fn recover(&self, job: &OwnedJob, budget: WorkBudget) -> Result<JobReceipt, MutationError>; }",
];
const EDGES: &[&str] = &[
    "crates/runtime/src/transactions/ -> openwrt_mcp_core::management",
    "crates/runtime/src/transactions/ -> openwrt_mcp_core::transactions",
    "crates/runtime/src/mutation_ports/ -> openwrt_mcp_core::transactions",
    "crates/device-codec/src/transactions/ -> openwrt_mcp_core::transactions",
    "crates/adapters/src/backups/ -> openwrt_mcp_runtime::sealing",
    "crates/crypto-age/src/sealing/ -> openwrt_mcp_runtime::sealing",
    "crates/adapters/src/backups/ -> openwrt_mcp_device_codec::gzip",
    "crates/adapters/src/backups/ -> openwrt_mcp_device_codec::archive",
    "crates/adapters/src/backups/ -> openwrt_mcp_runtime::mutation_ports",
    "crates/adapters/src/guardian/ -> openwrt_mcp_runtime::mutation_ports",
    "crates/backend-ssh/src/transactions/ -> openwrt_mcp_runtime::mutation_ports",
    "crates/key-sources/src/secrets/ -> openwrt_mcp_runtime::mutation_ports",
    "crates/server/src/transactions/ -> openwrt_mcp_runtime::mutation_ports",
    "crates/adapters/src/guardian/ -> openwrt_mcp_device_codec::transactions",
    "crates/backend-ssh/src/transactions/ -> openwrt_mcp_device_codec::transactions",
];
pub(crate) const PRIVATE_NAMESPACES: &[&str] = &[
    "openwrt_mcp_core::transactions",
    "openwrt_mcp_runtime::mutation_ports",
    "openwrt_mcp_runtime::transactions",
    "openwrt_mcp_device_codec::transactions",
];
const TESTS: &[&str] = &[
    "tools/xtask/tests/guarded_mutations.rs",
    "tools/xtask/tests/end_to_end.rs",
];

impl Contract {
    pub(crate) fn validate_guarded_mutations(&self, root: &Path) -> CheckResult {
        if self.version < 21 {
            return if self.guarded_mutation_contract.is_some() {
                Err("guarded mutation declarations require architecture v21".into())
            } else {
                Ok(())
            };
        }
        let actual = &self
            .guarded_mutation_contract
            .as_ref()
            .ok_or("v21 requires guarded mutation declarations")?
            .0;
        let mut expected = BTreeMap::new();
        for (key, value) in RULES {
            expected.insert((*key).to_owned(), DeclarationValue::Text((*value).into()));
        }
        for (key, value) in LIMITS {
            expected.insert((*key).to_owned(), DeclarationValue::Limit(*value));
        }
        for (key, values) in [
            ("requirements", REQUIREMENTS),
            ("states", STATES),
            ("transitions", TRANSITIONS),
            ("ports", PORTS),
            ("edges", EDGES),
            ("required_tests", TESTS),
        ] {
            expected.insert(
                key.into(),
                DeclarationValue::List(values.iter().map(|v| (*v).into()).collect()),
            );
        }
        if actual != &expected {
            return Err("guarded mutation requires exact reviewed boundaries, ports, transitions and budgets".into());
        }
        for path in TESTS
            .iter()
            .copied()
            .chain(["docs/guarded-mutation-contracts.md"])
        {
            if !root.join(path).is_file() {
                return Err("guarded mutation design and regression suites must exist".into());
            }
            if path.ends_with(".rs") {
                let source = std::fs::read_to_string(root.join(path))
                    .map_err(|_| "cannot read guarded mutation regression suite")?;
                crate::check_portable_suite(&source)?;
            }
        }
        for edge in EDGES {
            let (directory, namespace) = edge.split_once(" -> ").ok_or("invalid reviewed edge")?;
            let (rule, development) = self.owner(&format!("{directory}mod.rs"))?;
            let package = directory
                .strip_prefix("crates/")
                .and_then(|s| s.split('/').next())
                .ok_or("invalid mutation owner")?;
            let expected_owner = if package == "server" {
                "openwrt-mcp".into()
            } else {
                format!("openwrt-mcp-{package}")
            };
            let dependency = namespace
                .split("::")
                .next()
                .ok_or("invalid mutation namespace")?
                .replace('_', "-");
            if development
                || rule.name != expected_owner
                || !rule.dependencies.contains(&dependency)
            {
                return Err("mutation consumers must retain production ownership".into());
            }
        }
        for port in PORTS {
            syn::parse_str::<syn::ItemTrait>(port).map_err(|_| "invalid reviewed port syntax")?;
        }
        Ok(())
    }
}

/// Keep crate-wide bans; open only the exact v21 module/namespace pairs.
/// Older contracts never gain these edges. Other I/O/unsafe/portability bans stay.
pub(crate) fn scoped_rule(contract: &Contract, file: &str, rule: &CrateRule) -> CrateRule {
    let mut rule = rule.clone();
    rule.forbidden_paths
        .extend(PRIVATE_NAMESPACES.iter().map(|p| (*p).into()));
    if contract.version >= 21 && contract.guarded_mutation_contract.is_some() {
        for edge in EDGES {
            let (directory, namespace) = edge.split_once(" -> ").expect("reviewed edge");
            if file.starts_with(directory) {
                rule.forbidden_paths.retain(|p| p != namespace);
            }
        }
    }
    rule
}

/// Model/port modules must not turn private bytes into ordinary JSON or acquire
/// a generic action escape. Internal workflow invocation stays with Dispatcher.
pub fn check_guarded_mutation_source(contract: &Contract, file: &str, source: &str) -> CheckResult {
    let (_, development) = contract.owner(file)?;
    if development {
        return Ok(());
    }
    let directories = [
        "crates/core/src/transactions/",
        "crates/runtime/src/transactions/",
        "crates/runtime/src/mutation_ports/",
        "crates/device-codec/src/transactions/",
    ];
    for directory in directories {
        if file == format!("{}.rs", directory.trim_end_matches('/')) {
            return Err("guarded mutation uses reviewed module directories".into());
        }
        if file.starts_with(directory) && contract.version < 21 {
            return Err("guarded mutation source requires architecture v21".into());
        }
    }
    let private_data = file.starts_with("crates/core/src/transactions/")
        || file.starts_with("crates/runtime/src/mutation_ports/");
    let workflow = file.starts_with("crates/runtime/src/transactions/");
    struct Check {
        private_data: bool,
        workflow: bool,
        rejected: bool,
    }
    impl Check {
        fn ident(&mut self, name: &str) {
            let name = name.trim_start_matches("r#");
            self.rejected |= self.private_data
                && matches!(
                    name,
                    "Serialize"
                        | "Deserialize"
                        | "serde"
                        | "serde_json"
                        | "json"
                        | "PreparedAction"
                        | "Action"
                        | "Command"
                        | "Backend"
                );
            self.rejected |=
                self.workflow && matches!(name, "PreparedAction" | "Command" | "spawn_blocking");
        }
        fn tokens(&mut self, stream: proc_macro2::TokenStream) {
            for token in stream {
                match token {
                    proc_macro2::TokenTree::Ident(n) => self.ident(&n.to_string()),
                    proc_macro2::TokenTree::Group(g) => self.tokens(g.stream()),
                    _ => {}
                }
            }
        }
    }
    impl<'ast> Visit<'ast> for Check {
        fn visit_ident(&mut self, name: &'ast syn::Ident) {
            self.ident(&name.to_string());
        }
        fn visit_macro(&mut self, item: &'ast syn::Macro) {
            self.tokens(item.tokens.clone());
            self.visit_path(&item.path);
        }
        fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
            if let syn::Meta::List(list) = &attr.meta {
                self.tokens(list.tokens.clone());
            }
            visit::visit_attribute(self, attr);
        }
    }
    let ast = syn::parse_file(source).map_err(|_| "invalid mutation source")?;
    // Reserve the six port names globally, and accept their declarations only
    // inside the owned port directory with exactly the reviewed signatures.
    struct Ports {
        inside: bool,
        rejected: bool,
    }
    impl<'ast> Visit<'ast> for Ports {
        fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
            let approved: Vec<syn::ItemTrait> = PORTS
                .iter()
                .map(|p| syn::parse_str(p).expect("reviewed port syntax"))
                .collect();
            let expected = approved.iter().find(|p| p.ident == item.ident);
            if self.inside || expected.is_some() {
                let mut normalized = item.clone();
                normalized.vis = syn::Visibility::Inherited;
                normalized.attrs.clear();
                for member in &mut normalized.items {
                    if let syn::TraitItem::Fn(method) = member
                        && method.sig.inputs.trailing_punct()
                    {
                        method.sig.inputs.pop_punct();
                    }
                }
                self.rejected |= !self.inside
                    || expected.is_none_or(|p| {
                        quote::quote!(#p).to_string() != quote::quote!(#normalized).to_string()
                    });
            }
            visit::visit_item_trait(self, item);
        }
    }
    let mut ports = Ports {
        inside: file.starts_with("crates/runtime/src/mutation_ports/"),
        rejected: false,
    };
    ports.visit_file(&ast);
    if ports.rejected {
        return Err("mutation port declarations must retain exact owned signatures".into());
    }
    if file.starts_with("crates/runtime/src/mutation_ports/secrets/") {
        struct SecretCheck {
            rejected: bool,
        }
        impl SecretCheck {
            fn tokens(&mut self, tokens: proc_macro2::TokenStream) {
                for token in tokens {
                    match token {
                        proc_macro2::TokenTree::Ident(name) => self.visit_ident(&name),
                        proc_macro2::TokenTree::Group(group) => self.tokens(group.stream()),
                        _ => {}
                    }
                }
            }
        }
        impl<'ast> Visit<'ast> for SecretCheck {
            fn visit_ident(&mut self, name: &'ast syn::Ident) {
                self.rejected |= matches!(
                    name.to_string().trim_start_matches("r#"),
                    "Debug" | "Display" | "Clone" | "Serialize" | "Deserialize"
                );
            }
            fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
                // Derive names live in tokens, not ordinary AST paths.
                if let syn::Meta::List(list) = &attr.meta {
                    self.tokens(list.tokens.clone());
                }
                visit::visit_attribute(self, attr);
            }
        }
        let mut secret = SecretCheck { rejected: false };
        secret.visit_file(&ast);
        if secret.rejected {
            return Err(
                "secret values cannot acquire printing serialization or implicit copies".into(),
            );
        }
    }
    let mut check = Check {
        private_data,
        workflow,
        rejected: false,
    };
    check.visit_file(&ast);
    if check.rejected {
        Err("guarded mutation private data/action/worker boundary violated".into())
    } else {
        Ok(())
    }
}
