//! Architecture checkpoint only: parser/encoder behavior follows the accepted gate.
use std::{fs, path::Path};

#[test]
fn checkpoint_limits_the_new_codec_to_the_seven_literal_listing_commands() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = fs::read_to_string(root.join("architecture/capability-probes.toml")).unwrap();
    let objects = [
        "system",
        "network.device",
        "network.interface",
        "network.interface.lan",
        "network.interface.wan",
        "iwinfo",
        "service",
    ];
    assert_eq!(registry.matches("[[probes]]").count(), objects.len());
    assert_eq!(
        registry.matches("program = \"/bin/ubus\"").count(),
        objects.len()
    );
    for object in objects {
        assert!(registry.contains(&format!("arguments = [\"-v\", \"list\", \"{object}\"]")));
    }
    assert!(!registry.contains("\"call\""));
    assert!(!registry.contains("\"-S\""));
}
