use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::RuntimeError;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Invocation,
    Capability,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditPhase {
    Start,
    Finish,
    Rejection,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Attempt,
    Success,
    Denied,
    InvalidArguments,
    UnknownOperation,
    Busy,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEvent {
    pub kind: AuditKind,
    pub timestamp_ms: u64,
    pub request_sequence: u64,
    pub phase: AuditPhase,
    pub operation: String,
    pub outcome: AuditOutcome,
    pub duration_ms: Option<u64>,
}

impl AuditEvent {
    pub fn new(
        request_sequence: u64,
        phase: AuditPhase,
        operation: &str,
        outcome: AuditOutcome,
        duration_ms: Option<u64>,
    ) -> Self {
        Self {
            kind: AuditKind::Invocation,
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u64::MAX as u128) as u64,
            request_sequence,
            phase,
            operation: safe_operation_name(operation).to_owned(),
            outcome,
            duration_ms,
        }
    }

    pub fn with_kind(mut self, kind: AuditKind) -> Self {
        self.kind = kind;
        self
    }
}

pub fn safe_operation_name(name: &str) -> &str {
    if !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    {
        name
    } else {
        "unknown"
    }
}

pub trait AuditSink: Send + Sync {
    fn record(&self, event: &AuditEvent) -> Result<(), RuntimeError>;
}
