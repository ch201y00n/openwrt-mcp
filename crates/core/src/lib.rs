//! Pure policy, immutable operation catalog, and bounded data transformations.
//!
//! This crate deliberately has no device, filesystem, logging, or protocol I/O.

pub mod capability;
mod catalog;
mod error;
mod operation;
mod policy;

pub use capability::{
    CAPABILITY_TOOL_NAME, CapabilityObservation, CapabilityRequirement, IncompatibilityReason,
    MAX_OBSERVED_ARGUMENT_NAME_BYTES, MAX_OBSERVED_ARGUMENTS, MAX_OBSERVED_METHOD_NAME_BYTES,
    MAX_OBSERVED_METHODS, MethodSignature, ObjectObservation, ProbeRequest, ReviewedObject,
    UBUS_INTEGER_MAX, UBUS_INTEGER_MIN, UbusArgumentType, UnknownReason, Verdict,
};
pub use catalog::Catalog;
pub use error::CoreError;
pub use operation::{Action, Operation, OutputMode, Parameter, ParameterKind, PreparedAction};
pub use policy::{Access, Category, Grant, Permission, Policy, Requirement};
