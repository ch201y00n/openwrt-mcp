use openwrt_mcp_core::{
    Action, CapabilityRequirement, Category, Operation, OutputMode, Parameter, ParameterKind,
    Permission, Requirement,
};

pub(crate) fn operations() -> Vec<Operation> {
    vec![Operation {
        name: "packages_apk_installed".into(),
        description: "Capture APK 3.0.5 installed-query observations and read immutable 16-record pages. Omit cursor for a new capture; supply next_cursor to continue within 120 seconds. Non-atomic APK-visible scope, not whole-device completeness or a mutation baseline.".into(),
        requirements: vec![Requirement { category: Category::Packages, permission: Permission::Read }],
        parameters: [("cursor".into(), Parameter { kind: ParameterKind::String, required: false, allowed_values: vec![] })].into(),
        action: Action::ApkInstalledPage {}, capability: CapabilityRequirement::ApkInstalledQuery {},
        output_fields: vec![], output_mode: OutputMode::Scalars,
    }]
}
