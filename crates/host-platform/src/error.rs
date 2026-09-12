use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostError {
    InvalidPath,
    Unsupported,
    Unavailable,
    Insecure,
    Limit,
}

impl HostError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidPath => "host_invalid_path",
            Self::Unsupported => "host_unsupported",
            Self::Unavailable => "host_unavailable",
            Self::Insecure => "host_insecure",
            Self::Limit => "host_limit",
        }
    }
}
impl fmt::Display for HostError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(self.code())
    }
}
impl std::error::Error for HostError {}
