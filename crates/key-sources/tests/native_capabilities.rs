use openwrt_mcp_host_platform::native_file_protection_supported;
use openwrt_mcp_key_sources::{FileProtection, NativeFileAccess, ProtectedFileAccess};
use openwrt_mcp_runtime::protection::ProtectionError;

#[test]
fn unsupported_native_file_protection_is_not_silently_downgraded() {
    if !native_file_protection_supported() {
        let path = std::env::temp_dir().join("synthetic-key-never-opened");
        assert_eq!(
            NativeFileAccess
                .read(&path, FileProtection::Restricted, 1024)
                .err(),
            Some(ProtectionError::UnsupportedProtection)
        );
    }
}
