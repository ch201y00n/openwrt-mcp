use openwrt_mcp_core::{
    Access, Action, CapabilityRequirement, Catalog, Category, Grant, OutputMode, Permission,
    Policy, PreparedAction,
};
use serde_json::json;

#[test]
fn opkg_profile_is_bound_to_exact_metadata_category_and_canonical_cursor_only() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let op = catalog.get("packages_opkg_status").unwrap();
    assert!(matches!(op.action, Action::OpkgStatusPage {}));
    assert!(matches!(
        op.capability,
        CapabilityRequirement::OpkgRootStatusFile {}
    ));
    assert_eq!(
        op.capability.response_contract(),
        Some("packages_opkg_status.v1")
    );
    assert_eq!(op.capability.probe_object(), None);
    assert_eq!(op.requirements.len(), 1);
    assert_eq!(op.requirements[0].category, Category::Packages);
    assert_eq!(op.requirements[0].permission, Permission::Read);
    for category in [
        Category::System,
        Category::Network,
        Category::Wireless,
        Category::Firewall,
        Category::DhcpDns,
        Category::Services,
        Category::Packages,
        Category::Storage,
        Category::Vpn,
        Category::Firmware,
        Category::Diagnostics,
        Category::Extensions,
    ] {
        let policy = Policy {
            categories: [(
                category,
                Grant {
                    access: Access::Read,
                    execute: false,
                },
            )]
            .into(),
            ..Default::default()
        };
        assert_eq!(policy.authorize(op).is_ok(), category == Category::Packages);
    }
    assert!(Policy::default().authorize(op).is_err());
    let prepared = op.prepare_invocation(&json!({})).unwrap();
    assert!(matches!(
        prepared.action(),
        PreparedAction::OpkgStatusPage { cursor: None }
    ));
    assert!(prepared.project(&json!({"secret":"fixture"})).is_err());
    let cursor = format!("{}.16", "ff".repeat(16));
    assert!(matches!(
        op.prepare(&json!({"cursor":cursor})).unwrap(),
        PreparedAction::OpkgStatusPage { cursor: Some(_) }
    ));
    assert_eq!(op.input_schema()["properties"]["cursor"]["maxLength"], 37);
    for input in [
        json!({"path":"/etc/shadow"}),
        json!({"manager":"apk"}),
        json!({"program":"/bin/sh"}),
        json!({"cursor":null}),
        json!({"cursor":""}),
        json!({"cursor":1}),
        json!({"page_size":4096}),
    ] {
        assert!(op.prepare(&input).is_err());
    }
    for bad in 0..7 {
        let mut changed = op.clone();
        match bad {
            0 => changed.requirements[0].category = Category::System,
            1 => changed.output_fields.push("/secret".into()),
            2 => changed.capability = CapabilityRequirement::ApkInstalledQuery {},
            3 => changed.capability = CapabilityRequirement::Unverified {},
            4 => changed.parameters.get_mut("cursor").unwrap().required = true,
            5 => changed.action = Action::ApkInstalledPage {},
            _ => changed.output_mode = OutputMode::Structured,
        }
        assert!(Catalog::with_builtins(vec![changed], vec![]).is_err());
    }
}
