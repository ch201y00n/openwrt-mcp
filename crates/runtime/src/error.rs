use openwrt_mcp_core::CoreError;

/// Never attach raw OS errors, arguments, paths or device output to these errors.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("operation rejected")]
    Core(#[from] CoreError),
    #[error("unknown operation")]
    UnknownOperation,
    #[error("invalid runtime configuration")]
    InvalidConfig,
    #[error("requested audit destination is unsupported")]
    UnsupportedAuditDestination,
    #[error("operation capacity is exhausted")]
    Busy,
    #[error("device operation failed")]
    BackendFailed,
    #[error("device operation timed out; completion may be uncertain")]
    Timeout,
    #[error("device output exceeded the configured bound")]
    OutputLimit,
    #[error("device output is not valid JSON")]
    InvalidOutput,
    #[error("audit recording failed; operation was not started")]
    AuditUnavailable,
    #[error("completion audit failed; operation may have completed")]
    CompletionAuditFailed,
}

impl RuntimeError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.code(),
            Self::UnknownOperation => "unknown_operation",
            Self::InvalidConfig => "invalid_config",
            Self::UnsupportedAuditDestination => "unsupported_audit_destination",
            Self::Busy => "busy",
            Self::BackendFailed => "backend_failed",
            Self::Timeout => "timeout",
            Self::OutputLimit => "output_limit",
            Self::InvalidOutput => "invalid_output",
            Self::AuditUnavailable => "audit_unavailable",
            Self::CompletionAuditFailed => "audit_completion_failed",
        }
    }
}
