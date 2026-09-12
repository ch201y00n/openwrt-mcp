//! Architecture checkpoint only, not a test of implemented discovery behavior.
use std::{fs, path::Path};

#[test]
fn checkpoint_records_backend_cache_audit_and_deadline_ownership() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = fs::read_to_string(root.join("architecture/capability-probes.toml")).unwrap();
    for invariant in [
        "backend_ownership = \"same_immutable_backend\"",
        "cache_ownership = \"private_dispatcher\"",
        "epoch_binding = \"opaque_backend_epoch\"",
        "ttl_seconds = 30",
        "unknown_behavior = \"fail_closed\"",
        "audit_order = \"start_before_cache_probe_execute\"",
        "device_deadline = \"shared_probe_execute\"",
        "metadata_audit_kind = \"capability\"",
    ] {
        assert!(
            registry.contains(invariant),
            "missing architecture invariant"
        );
    }
}
