use thiserror::Error;

/// Stable errors contain no configuration, device output, or client input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("permission_denied")]
    PermissionDenied,
    #[error("invalid_definition")]
    InvalidDefinition,
    #[error("duplicate_operation")]
    DuplicateOperation,
    #[error("catalog_limit_exceeded")]
    CatalogLimitExceeded,
    #[error("unknown_operation_reference")]
    UnknownOperationReference,
    #[error("invalid_arguments")]
    InvalidArguments,
    #[error("missing_argument")]
    MissingArgument,
    #[error("unknown_argument")]
    UnknownArgument,
    #[error("invalid_output")]
    InvalidOutput,
    #[error("output_limit")]
    OutputLimit,
    #[error("selection_not_observed")]
    SelectionNotObserved,
}

impl CoreError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PermissionDenied => "permission_denied",
            Self::InvalidDefinition => "invalid_definition",
            Self::DuplicateOperation => "duplicate_operation",
            Self::CatalogLimitExceeded => "catalog_limit_exceeded",
            Self::UnknownOperationReference => "unknown_operation_reference",
            Self::InvalidArguments => "invalid_arguments",
            Self::MissingArgument => "missing_argument",
            Self::UnknownArgument => "unknown_argument",
            Self::InvalidOutput => "invalid_output",
            Self::OutputLimit => "output_limit",
            Self::SelectionNotObserved => "selection_not_observed",
        }
    }
}
