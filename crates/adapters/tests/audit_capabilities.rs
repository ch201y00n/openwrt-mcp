use openwrt_mcp_adapters::{AuditConfig, AuditDestination, AuditWriter};
use openwrt_mcp_host_platform::{native_file_protection_supported, system_log_supported};
use openwrt_mcp_runtime::RuntimeError;

#[test]
fn portable_stderr_is_available_and_native_destinations_are_explicit() {
    AuditWriter::new(AuditConfig::default()).unwrap();
    if !native_file_protection_supported() {
        let config = AuditConfig {
            destination: AuditDestination::File,
            path: Some(std::env::temp_dir().join("synthetic-audit-never-opened")),
            ..AuditConfig::default()
        };
        assert!(matches!(
            config.validate(),
            Err(RuntimeError::UnsupportedAuditDestination)
        ));
        assert!(AuditWriter::new(config).is_err());
    }
    if !system_log_supported() {
        let config = AuditConfig {
            destination: AuditDestination::Syslog,
            ..AuditConfig::default()
        };
        assert!(matches!(
            config.validate(),
            Err(RuntimeError::UnsupportedAuditDestination)
        ));
    }
}
