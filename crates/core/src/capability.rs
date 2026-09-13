//! Pure capability prerequisites and bounded observations, never authorization.
//!
//! An observed input signature does not validate response contents or safe effects.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::operation::{identifier, parameter_reference, ubus_identifier};
use crate::{Action, CoreError, Parameter, ParameterKind, PreparedAction};

pub const CAPABILITY_TOOL_NAME: &str = "operation_capability";
pub const MAX_OBSERVED_METHODS: usize = 128;
pub const MAX_OBSERVED_ARGUMENTS: usize = 64;
pub const MAX_OBSERVED_METHOD_NAME_BYTES: usize = 128;
pub const MAX_OBSERVED_ARGUMENT_NAME_BYTES: usize = 64;
pub const UBUS_INTEGER_MIN: i32 = i32::MIN;
pub const UBUS_INTEGER_MAX: i32 = i32::MAX;

/// Exact object names admitted by the reviewed introspection profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReviewedObject {
    System,
    NetworkDevice,
    NetworkInterface,
    NetworkInterfaceLan,
    NetworkInterfaceWan,
    Iwinfo,
    Service,
    Luci,
    LuciRpc,
}

impl ReviewedObject {
    pub const ALL: [Self; 9] = [
        Self::System,
        Self::NetworkDevice,
        Self::NetworkInterface,
        Self::NetworkInterfaceLan,
        Self::NetworkInterfaceWan,
        Self::Iwinfo,
        Self::Service,
        Self::Luci,
        Self::LuciRpc,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::NetworkDevice => "network.device",
            Self::NetworkInterface => "network.interface",
            Self::NetworkInterfaceLan => "network.interface.lan",
            Self::NetworkInterfaceWan => "network.interface.wan",
            Self::Iwinfo => "iwinfo",
            Self::Service => "service",
            Self::Luci => "luci",
            Self::LuciRpc => "luci-rpc",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|object| object.as_str() == name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeRequest {
    DescribeUbusObject(ReviewedObject),
}

/// Operator-owned metadata is mandatory and cannot be supplied by a tool caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapabilityRequirement {
    ApkInstalledQuery {},
    UbusMethod {
        object: String,
        method: String,
        arguments: BTreeMap<String, ParameterKind>,
        response_contract: String,
    },
    // A struct variant deliberately rejects unknown fields during deserialization.
    Unverified {},
}

/// Labels represented by verbose ubus listing. Unknown labels are not coerced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UbusArgumentType {
    String,
    Integer,
    Boolean,
    Array,
    Table,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignature {
    pub arguments: BTreeMap<String, UbusArgumentType>,
}

/// Infrastructure-only metadata; intentionally not serializable to MCP or logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectObservation {
    pub object: ReviewedObject,
    pub methods: BTreeMap<String, MethodSignature>,
}

impl ObjectObservation {
    /// Defense in depth for observations supplied by an adapter or fixture.
    /// The byte parser must additionally bound bytes and detect duplicate names.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.methods.len() > MAX_OBSERVED_METHODS {
            return Err(CoreError::InvalidDefinition);
        }
        for (method, signature) in &self.methods {
            if method.len() > MAX_OBSERVED_METHOD_NAME_BYTES
                || !ubus_identifier(method)
                || signature.arguments.len() > MAX_OBSERVED_ARGUMENTS
                || signature.arguments.keys().any(|argument| {
                    argument.len() > MAX_OBSERVED_ARGUMENT_NAME_BYTES || !ubus_identifier(argument)
                })
            {
                return Err(CoreError::InvalidDefinition);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityObservation {
    Unknown(UnknownReason),
    Ubus(ObjectObservation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    NotObservedOrHidden,
    IncompleteSignature,
    UnrecognizedType,
    UnreviewedProbe,
    ProbeUnavailable,
    InvalidObservation,
    StaleObservation,
}

impl UnknownReason {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotObservedOrHidden => "not_observed_or_hidden",
            Self::IncompleteSignature => "incomplete_signature",
            Self::UnrecognizedType => "unrecognized_type",
            Self::UnreviewedProbe => "unreviewed_probe",
            Self::ProbeUnavailable => "probe_unavailable",
            Self::InvalidObservation => "invalid_observation",
            Self::StaleObservation => "stale_observation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IncompatibilityReason {
    ArgumentTypeMismatch,
    InvalidPreparedAction,
}

impl IncompatibilityReason {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::ArgumentTypeMismatch => "argument_type_mismatch",
            Self::InvalidPreparedAction => "invalid_prepared_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Compatible,
    Incompatible(IncompatibilityReason),
    Unknown(UnknownReason),
}

fn response_identifier(value: &str) -> bool {
    let Some((name, version)) = value.rsplit_once(".v") else {
        return false;
    };
    value.len() <= 128
        && ubus_identifier(name)
        && !version.is_empty()
        && !version.starts_with('0')
        && version.bytes().all(|byte| byte.is_ascii_digit())
        && version.parse::<u32>().is_ok()
}

pub(crate) fn checked_integer(value: &Value) -> bool {
    value
        .as_i64()
        .is_some_and(|number| i32::try_from(number).is_ok())
}

fn value_matches(value: &Value, kind: ParameterKind) -> bool {
    match kind {
        ParameterKind::String => value
            .as_str()
            .is_some_and(|value| value.len() <= 1024 && !value.contains('\0')),
        ParameterKind::Integer => checked_integer(value),
        ParameterKind::Boolean => value.is_boolean(),
    }
}

impl CapabilityRequirement {
    pub fn probe_object(&self) -> Option<ReviewedObject> {
        match self {
            Self::UbusMethod { object, .. } => ReviewedObject::from_name(object),
            Self::Unverified {} => None,
            Self::ApkInstalledQuery {} => None,
        }
    }

    pub fn response_contract(&self) -> Option<&str> {
        match self {
            Self::UbusMethod {
                response_contract, ..
            } => Some(response_contract),
            Self::Unverified {} => None,
            Self::ApkInstalledQuery {} => Some(crate::packages::PACKAGE_RESPONSE_CONTRACT),
        }
    }

    fn valid_shape(&self) -> bool {
        match self {
            Self::Unverified {} => true,
            Self::ApkInstalledQuery {} => true,
            Self::UbusMethod {
                object,
                method,
                arguments,
                response_contract,
            } => {
                ubus_identifier(object)
                    && ubus_identifier(method)
                    && arguments.len() <= MAX_OBSERVED_ARGUMENTS
                    && arguments.keys().all(|name| identifier(name))
                    && response_identifier(response_contract)
            }
        }
    }

    pub(crate) fn validate_for(
        &self,
        action: &Action,
        parameters: &BTreeMap<String, Parameter>,
    ) -> Result<(), CoreError> {
        let invalid = CoreError::InvalidDefinition;
        if !self.valid_shape() {
            return Err(invalid);
        }
        match (self, action) {
            (Self::ApkInstalledQuery {}, Action::ApkInstalledPage {}) => Ok(()),
            (Self::Unverified {}, Action::Process { .. }) => Ok(()),
            (
                Self::UbusMethod {
                    object,
                    method,
                    arguments,
                    ..
                },
                Action::Ubus {
                    object: action_object,
                    method: action_method,
                    arguments: action_arguments,
                },
            ) if object == action_object
                && method == action_method
                && arguments.len() == action_arguments.len() =>
            {
                for (name, value) in action_arguments {
                    let kind = match value {
                        Value::String(value) => match parameter_reference(value) {
                            Some(parameter) => parameters.get(parameter).ok_or(invalid)?.kind,
                            None => ParameterKind::String,
                        },
                        Value::Bool(_) => ParameterKind::Boolean,
                        Value::Number(_) if checked_integer(value) => ParameterKind::Integer,
                        _ => return Err(invalid),
                    };
                    if arguments.get(name) != Some(&kind) {
                        return Err(invalid);
                    }
                }
                Ok(())
            }
            _ => Err(invalid),
        }
    }

    /// Evaluate actual transmitted fields, or all potential fields for metadata status.
    /// This does not authorize a call, validate a response, or establish freshness.
    pub fn evaluate(
        &self,
        observation: &CapabilityObservation,
        prepared: Option<&PreparedAction>,
    ) -> Verdict {
        let Self::UbusMethod {
            object,
            method,
            arguments,
            ..
        } = self
        else {
            return Verdict::Unknown(UnknownReason::UnreviewedProbe);
        };
        if !self.valid_shape() {
            return Verdict::Unknown(UnknownReason::InvalidObservation);
        }
        let Some(reviewed) = self.probe_object() else {
            return Verdict::Unknown(UnknownReason::UnreviewedProbe);
        };
        let actual = match prepared {
            None => None,
            Some(PreparedAction::Ubus {
                object: actual_object,
                method: actual_method,
                arguments: actual_arguments,
            }) if actual_object == object && actual_method == method => {
                let Some(actual) = actual_arguments.as_object() else {
                    return Verdict::Incompatible(IncompatibilityReason::InvalidPreparedAction);
                };
                if actual.len() > arguments.len()
                    || actual.iter().any(|(name, value)| {
                        !arguments
                            .get(name)
                            .is_some_and(|kind| value_matches(value, *kind))
                    })
                {
                    return Verdict::Incompatible(IncompatibilityReason::InvalidPreparedAction);
                }
                Some(actual)
            }
            Some(_) => {
                return Verdict::Incompatible(IncompatibilityReason::InvalidPreparedAction);
            }
        };
        let observation = match observation {
            CapabilityObservation::Unknown(reason) => return Verdict::Unknown(*reason),
            CapabilityObservation::Ubus(observation) => observation,
        };
        if observation.object != reviewed || observation.validate().is_err() {
            return Verdict::Unknown(UnknownReason::InvalidObservation);
        }
        let Some(signature) = observation.methods.get(method) else {
            return Verdict::Unknown(UnknownReason::NotObservedOrHidden);
        };
        let mut unknown_type = false;
        let mut incomplete = false;
        for (name, kind) in arguments {
            if actual.is_some_and(|values| !values.contains_key(name)) {
                continue;
            }
            let Some(observed_type) = signature.arguments.get(name) else {
                incomplete = true;
                continue;
            };
            let expected = match kind {
                ParameterKind::String => UbusArgumentType::String,
                ParameterKind::Integer => UbusArgumentType::Integer,
                ParameterKind::Boolean => UbusArgumentType::Boolean,
            };
            if *observed_type == UbusArgumentType::Unknown {
                unknown_type = true;
            } else if *observed_type != expected {
                return Verdict::Incompatible(IncompatibilityReason::ArgumentTypeMismatch);
            }
        }
        if incomplete {
            Verdict::Unknown(UnknownReason::IncompleteSignature)
        } else if unknown_type {
            Verdict::Unknown(UnknownReason::UnrecognizedType)
        } else {
            Verdict::Compatible
        }
    }
}
