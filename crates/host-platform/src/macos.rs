//! Native ACL and volume ownership validation is not implemented.
//! Never treat Linux permission-bit checks as macOS security enforcement.
pub(crate) use super::unsupported::{
    PrivateLog, SystemLog, native_file_protection_supported, read_config, read_secret,
    system_log_supported, verify_openwrt_local,
};
