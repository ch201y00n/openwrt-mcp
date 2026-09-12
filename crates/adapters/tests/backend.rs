//! Portable default target behavior, without running host commands.
use openwrt_mcp_adapters::UnconfiguredBackend;
use openwrt_mcp_core::PreparedAction;
use openwrt_mcp_runtime::{Backend, Limits};

#[tokio::test]
async fn missing_target_never_executes_a_router_program_on_the_host() {
    let error = UnconfiguredBackend
        .execute(
            &PreparedAction::Process {
                program: "/bin/reboot".into(),
                args: Vec::new(),
            },
            &Limits::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "target_not_configured");
}
