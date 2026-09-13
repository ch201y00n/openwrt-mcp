use openwrt_mcp_host_platform::{
    HostError, PrivateLog, SystemLog, native_file_protection_supported, private_log_supported,
    read_config, read_secret, system_log_supported,
};

#[test]
fn native_capability_never_claims_unimplemented_protection() {
    assert_eq!(
        native_file_protection_supported(),
        cfg!(any(target_os = "linux", target_os = "windows"))
    );
    assert_eq!(system_log_supported(), cfg!(target_os = "linux"));
    if !native_file_protection_supported() {
        let path = std::env::temp_dir().join("synthetic-never-opened-platform-file");
        assert_eq!(read_config(&path, 1024).err(), Some(HostError::Unsupported));
        assert_eq!(read_secret(&path, 1024).err(), Some(HostError::Unsupported));
    }
    assert_eq!(private_log_supported(), cfg!(target_os = "linux"));
    if !private_log_supported() {
        let path = std::env::temp_dir().join("synthetic-never-opened-platform-file");
        assert_eq!(
            PrivateLog::open(&path, 1024, 2).err(),
            Some(HostError::Unsupported)
        );
        assert_eq!(SystemLog::open().err(), Some(HostError::Unsupported));
    }
}

#[test]
fn public_limits_are_checked_before_any_host_access() {
    let path = std::env::temp_dir().join("synthetic-never-opened-platform-file");
    assert_eq!(read_config(&path, 0).err(), Some(HostError::Limit));
    assert_eq!(
        read_config(&path, 1024 * 1024 + 1).err(),
        Some(HostError::Limit)
    );
    assert_eq!(
        read_secret(&path, 16 * 1024 * 1024 + 1).err(),
        Some(HostError::Limit)
    );
    assert_eq!(PrivateLog::open(&path, 0, 2).err(), Some(HostError::Limit));
    assert_eq!(
        PrivateLog::open(&path, 1024, 101).err(),
        Some(HostError::Limit)
    );
    for error in [
        HostError::InvalidPath,
        HostError::Unsupported,
        HostError::Unavailable,
        HostError::Insecure,
        HostError::Limit,
    ] {
        assert!(error.to_string().starts_with("host_"));
        assert!(!error.to_string().contains("synthetic"));
    }
}
