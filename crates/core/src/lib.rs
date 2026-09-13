//! Pure policy, immutable operation catalog, and bounded data transformations.
//!
//! This crate deliberately has no device, filesystem, logging, or protocol I/O.

pub mod capability;
mod catalog;
mod error;
mod operation;
pub mod packages;
mod policy;
pub mod projection;

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
pub use projection::{
    Collection, CollectionField, CounterSource, InnerRecord, LeafRecord, MAX_COLLECTION_ITEMS,
    MAX_COLLECTION_NODES, MAX_NORMALIZED_BYTES, MAX_RECORD_FIELDS, MAX_TEXT_BYTES,
    PreparedInvocation, Presence, Record, RootRecord, SAFE_INTEGER_MAX, ScalarField, ScalarKind,
    Selection, TextIdentity, TypedProjection, check_normalized_result,
};
