//! Pure policy, immutable operation catalog, and bounded data transformations.
//!
//! This crate deliberately has no device, filesystem, logging, or protocol I/O.

mod catalog;
mod error;
mod operation;
mod policy;

pub use catalog::Catalog;
pub use error::CoreError;
pub use operation::{Action, Invocation, Operation, Parameter, ParameterKind};
pub use policy::{Access, Category, Grant, Permission, Policy, Requirement};
