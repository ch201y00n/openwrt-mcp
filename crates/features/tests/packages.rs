use openwrt_mcp_core::{
    Access, Action, CapabilityRequirement, Catalog, Category, Grant, Permission, Policy,
    PreparedAction,
};
use serde_json::json;
#[test]
fn package_read_is_closed_and_denied_by_default() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let op = catalog.get("packages_apk_installed").unwrap();
    assert!(matches!(op.action, Action::ApkInstalledPage {}));
    assert!(matches!(
        op.capability,
        CapabilityRequirement::ApkInstalledQuery {}
    ));
    assert_eq!(op.requirements.len(), 1);
    assert_eq!(op.requirements[0].category, Category::Packages);
    assert_eq!(op.requirements[0].permission, Permission::Read);
    assert!(Policy::default().authorize(op).is_err());
    let policy = Policy {
        categories: [(
            Category::Packages,
            Grant {
                access: Access::Read,
                execute: false,
            },
        )]
        .into(),
        ..Default::default()
    };
    policy.authorize(op).unwrap();
    let prepared = op.prepare_invocation(&json!({})).unwrap();
    assert!(matches!(
        prepared.action(),
        PreparedAction::ApkInstalledPage { cursor: None }
    ));
    assert!(prepared.project(&json!({"secret":"fixture"})).is_err());
    assert_eq!(
        op.capability.response_contract(),
        Some("packages_apk_installed.v1")
    );
    assert_eq!(op.capability.probe_object(), None);
}
#[test]
fn client_cannot_choose_command_source_or_unsafe_cursor() {
    let catalog = openwrt_mcp_features::catalog(vec![]).unwrap();
    let op = catalog.get("packages_apk_installed").unwrap();
    for input in [
        json!({"manager":"opkg"}),
        json!({"program":"/bin/sh"}),
        json!({"pattern":"*"}),
        json!({"cursor":null}),
        json!({"cursor":""}),
        json!({"cursor":1}),
        json!({"page_size":4096}),
    ] {
        assert!(op.prepare(&input).is_err());
    }
    for bad in 0..4 {
        let mut changed = op.clone();
        match bad {
            0 => changed.requirements[0].category = Category::System,
            1 => changed.output_fields.push("/secret".into()),
            2 => changed.capability = CapabilityRequirement::Unverified {},
            _ => changed.parameters.get_mut("cursor").unwrap().required = true,
        }
        assert!(Catalog::with_builtins(vec![changed], vec![]).is_err());
    }
}
