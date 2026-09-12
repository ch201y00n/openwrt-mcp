use std::fmt::{Display, Formatter};

/// Fixed codes only: no paths, aliases, key excerpts or upstream error chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionError {
    InvalidConfig,
    InvalidMaterial,
    SourceUnavailable,
    InteractionRequired,
    UnsupportedProtection,
    UnsupportedContainer,
    InvalidContainer,
    ResourceLimit,
    EncryptionFailed,
    DecryptionFailed,
    StreamFailed,
    DeadlineExceeded,
}

impl ProtectionError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidConfig => "protection_config_invalid",
            Self::InvalidMaterial => "key_material_invalid",
            Self::SourceUnavailable => "key_source_unavailable",
            Self::InteractionRequired => "key_source_interaction_required",
            Self::UnsupportedProtection => "key_source_protection_unsupported",
            Self::UnsupportedContainer => "key_container_unsupported",
            Self::InvalidContainer => "key_container_invalid",
            Self::ResourceLimit => "protection_resource_limit",
            Self::EncryptionFailed => "encryption_failed",
            Self::DecryptionFailed => "decryption_failed",
            Self::StreamFailed => "protection_stream_failed",
            Self::DeadlineExceeded => "protection_deadline_exceeded",
        }
    }
}

impl Display for ProtectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ProtectionError {}
