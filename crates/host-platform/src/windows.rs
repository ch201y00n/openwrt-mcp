//! Native DACL/owner/inheritance/reparse/volume validation is not implemented.
//! Never treat readonly or POSIX emulation as Windows security enforcement.
pub(crate) use super::unsupported::{
    PrivateLog, SystemLog, native_file_protection_supported, read_config, read_secret,
    system_log_supported, verify_openwrt_local,
};
