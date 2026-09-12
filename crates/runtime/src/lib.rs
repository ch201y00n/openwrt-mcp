//! Device I/O is reachable through the policy-enforcing dispatcher only.
//! Backends and audit sinks are injectable so tests never require a router.

mod audit;
mod backend;
mod dispatcher;
mod error;

pub use audit::{
    AuditConfig, AuditDestination, AuditEvent, AuditFormat, AuditOutcome, AuditPhase, AuditSink,
    AuditWriter,
};
pub use backend::{Backend, Limits, LocalBackend};
pub use dispatcher::Dispatcher;
pub use error::RuntimeError;
