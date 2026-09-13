#![cfg(target_os = "windows")]
//! Architecture declaration only; replaced by native behavior after checkpoint.

#[test]
fn architecture_checkpoint_does_not_claim_native_protected_reads() {
    assert!(!openwrt_mcp_host_platform::native_file_protection_supported());
}
