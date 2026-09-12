//! Policy-enforcing use cases and injectable ports; no device or filesystem I/O.

mod audit;
mod backend;
mod dispatcher;
mod error;

pub use audit::{AuditEvent, AuditOutcome, AuditPhase, AuditSink, safe_operation_name};
pub use backend::{Backend, Limits};
pub use dispatcher::Dispatcher;
pub use error::RuntimeError;
