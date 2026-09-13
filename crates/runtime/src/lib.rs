//! Policy-enforcing use cases and injectable ports; no device or filesystem I/O.

mod audit;
mod backend;
mod capability;
mod dispatcher;
mod error;
pub mod packages;
pub mod protection;

pub use audit::{AuditEvent, AuditKind, AuditOutcome, AuditPhase, AuditSink, safe_operation_name};
pub use backend::{Backend, Limits};
pub use capability::CapabilityStatus;
pub use dispatcher::Dispatcher;
pub use error::RuntimeError;
