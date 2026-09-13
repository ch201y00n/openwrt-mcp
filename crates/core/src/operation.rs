use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::capability::checked_integer;
use crate::{
    CAPABILITY_TOOL_NAME, CapabilityRequirement, CoreError, PreparedInvocation, Requirement,
    TypedProjection, UBUS_INTEGER_MAX, UBUS_INTEGER_MIN,
};

const MAX_PARAMETERS: usize = 32;
const MAX_ARGUMENTS: usize = 64;
const MAX_OUTPUT_FIELDS: usize = 64;
const MAX_VALUE_BYTES: usize = 1024;
const MAX_DEFINITION_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub kind: ParameterKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub allowed_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    ApkInstalledPage {},
    Ubus {
        object: String,
        method: String,
        #[serde(default)]
        arguments: BTreeMap<String, Value>,
    },
    Process {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

/// Scalar leaves are the safe default; structured projection is explicit operator metadata.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Scalars,
    Structured,
    Typed(Box<TypedProjection>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub name: String,
    pub description: String,
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Parameter>,
    pub action: Action,
    pub capability: CapabilityRequirement,
    #[serde(default)]
    pub output_fields: Vec<String>,
    #[serde(default)]
    pub output_mode: OutputMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedAction {
    ApkInstalledPage {
        cursor: Option<String>,
    },
    Ubus {
        object: String,
        method: String,
        arguments: Value,
    },
    Process {
        program: String,
        args: Vec<String>,
    },
}

pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn bounded_string(value: &str, max: usize) -> bool {
    value.len() <= max && !value.contains('\0')
}

pub(crate) fn ubus_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

pub(crate) fn parameter_reference(value: &str) -> Option<&str> {
    value.strip_prefix('{')?.strip_suffix('}')
}

fn valid_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') || !bounded_string(pointer, 512) {
        return false;
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return false;
        }
    }
    true
}

fn disjoint_output_fields(pointers: &[String]) -> bool {
    if pointers.len() > MAX_OUTPUT_FIELDS {
        return false;
    }
    let mut paths = Vec::with_capacity(pointers.len());
    for pointer in pointers {
        if !valid_pointer(pointer) {
            return false;
        }
        let segments: Vec<String> = pointer
            .split('/')
            .skip(1)
            .map(|part| part.replace("~1", "/").replace("~0", "~"))
            .collect();
        paths.push(segments);
    }
    paths.sort();
    // A prefix sorts immediately before its first descendant. Compare decoded
    // segments so /a and /ab are distinct, as are /a~1b and /a/b.
    !paths.windows(2).any(|pair| pair[1].starts_with(&pair[0]))
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) if value.is_i64() || value.is_u64() => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(crate) fn sensitive_key(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    normalized.ends_with("key")
        || normalized.contains("psk")
        || normalized == "pin"
        || normalized == "pwd"
        || normalized == "sae"
        || normalized.contains("password")
        || normalized.contains("passwd")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("privatekey")
        || normalized.contains("presharedkey")
        || normalized.contains("credential")
        || normalized.contains("qrcode")
        || normalized.contains("qrpayload")
        || normalized.contains("identity")
        || normalized.contains("authorization")
        || normalized.contains("cookie")
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(key, _)| !sensitive_key(key))
                .map(|(key, value)| (key.clone(), redact(value)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.iter().map(redact).collect()),
        other => other.clone(),
    }
}

impl Operation {
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        let invalid = CoreError::InvalidDefinition;
        if !identifier(&self.name)
            || self.name == CAPABILITY_TOOL_NAME
            || self.description.trim().is_empty()
            || !bounded_string(&self.description, 512)
            || self.requirements.is_empty()
            || self.requirements.len() > 36
            || self.parameters.len() > MAX_PARAMETERS
            || self.requirements.iter().collect::<BTreeSet<_>>().len() != self.requirements.len()
            || !disjoint_output_fields(&self.output_fields)
        {
            return Err(invalid);
        }
        for (name, parameter) in &self.parameters {
            if !identifier(name)
                || parameter.allowed_values.len() > 64
                || parameter
                    .allowed_values
                    .iter()
                    .any(|value| !bounded_string(value, MAX_VALUE_BYTES))
                || parameter
                    .allowed_values
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != parameter.allowed_values.len()
                || parameter
                    .allowed_values
                    .iter()
                    .any(|value| match parameter.kind {
                        ParameterKind::String => false,
                        ParameterKind::Integer => {
                            if matches!(self.action, Action::Ubus { .. }) {
                                return !value
                                    .parse::<i32>()
                                    .is_ok_and(|number| number.to_string() == *value);
                            }
                            !value
                                .parse::<i64>()
                                .map(|number| number.to_string() == *value)
                                .unwrap_or(false)
                                && !value
                                    .parse::<u64>()
                                    .map(|number| number.to_string() == *value)
                                    .unwrap_or(false)
                        }
                        ParameterKind::Boolean => value != "true" && value != "false",
                    })
            {
                return Err(invalid);
            }
        }
        let mut used = BTreeSet::new();
        if let OutputMode::Typed(projection) = &self.output_mode {
            if !self.output_fields.is_empty() {
                return Err(invalid);
            }
            projection.validate(&self.parameters)?;
            if let Some((name, _)) = projection.selector() {
                used.insert(name.to_owned());
            }
        }
        let mut validate_reference = |value: &str| -> Result<(), CoreError> {
            if !bounded_string(value, MAX_VALUE_BYTES) {
                return Err(invalid);
            }
            if let Some(name) = parameter_reference(value) {
                if !identifier(name) || !self.parameters.contains_key(name) {
                    return Err(invalid);
                }
                used.insert(name.to_owned());
            }
            Ok(())
        };
        match &self.action {
            Action::ApkInstalledPage {} => {
                let cursor = self.parameters.get("cursor").ok_or(invalid)?;
                if self.parameters.len() != 1
                    || cursor.required
                    || cursor.kind != ParameterKind::String
                    || !cursor.allowed_values.is_empty()
                    || !self.output_fields.is_empty()
                    || self.output_mode != OutputMode::Scalars
                    || !self.requirements.contains(&Requirement {
                        category: crate::Category::Packages,
                        permission: crate::Permission::Read,
                    })
                {
                    return Err(invalid);
                }
                used.insert("cursor".into());
            }
            Action::Ubus {
                object,
                method,
                arguments,
            } => {
                if !ubus_identifier(object)
                    || !ubus_identifier(method)
                    || arguments.len() > MAX_ARGUMENTS
                {
                    return Err(invalid);
                }
                for (key, value) in arguments {
                    if !identifier(key) {
                        return Err(invalid);
                    }
                    match value {
                        Value::String(value) => validate_reference(value)?,
                        Value::Bool(_) => (),
                        Value::Number(_) if checked_integer(value) => (),
                        _ => return Err(invalid),
                    }
                }
            }
            Action::Process { program, args } => {
                if !program.starts_with('/')
                    || program.ends_with('/')
                    || !bounded_string(program, 256)
                    || !program
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"/_.-".contains(&byte))
                    || program.split('/').any(|part| part == "." || part == "..")
                    || args.len() > MAX_ARGUMENTS
                {
                    return Err(invalid);
                }
                for arg in args {
                    validate_reference(arg)?;
                    if parameter_reference(arg)
                        .and_then(|name| self.parameters.get(name))
                        .is_some_and(|parameter| !parameter.required)
                    {
                        // Omitting a positional argument can turn the next
                        // option into its value and change command meaning.
                        return Err(invalid);
                    }
                }
            }
        }
        if self.parameters.keys().any(|name| !used.contains(name)) {
            return Err(invalid);
        }
        crate::uci::validate_read(self)?;
        self.capability
            .validate_for(&self.action, &self.parameters)?;
        // Bound every constituent before allocating the serialized definition.
        if serde_json::to_vec(self).map_err(|_| invalid)?.len() > MAX_DEFINITION_BYTES {
            return Err(invalid);
        }
        Ok(())
    }

    pub fn prepare(&self, input: &Value) -> Result<PreparedAction, CoreError> {
        self.validate()?;
        let values = input.as_object().ok_or(CoreError::InvalidArguments)?;
        for name in values.keys() {
            if !self.parameters.contains_key(name) {
                return Err(CoreError::UnknownArgument);
            }
        }
        for (name, parameter) in &self.parameters {
            let Some(value) = values.get(name) else {
                if parameter.required {
                    return Err(CoreError::MissingArgument);
                }
                continue;
            };
            let valid_type = match parameter.kind {
                ParameterKind::String => value
                    .as_str()
                    .is_some_and(|text| bounded_string(text, MAX_VALUE_BYTES)),
                ParameterKind::Integer => {
                    if matches!(self.action, Action::Ubus { .. }) {
                        checked_integer(value)
                    } else {
                        value.as_i64().is_some() || value.as_u64().is_some()
                    }
                }
                ParameterKind::Boolean => value.is_boolean(),
            };
            if !valid_type
                || (!parameter.allowed_values.is_empty()
                    && !parameter
                        .allowed_values
                        .contains(&scalar_text(value).ok_or(CoreError::InvalidArguments)?))
            {
                return Err(CoreError::InvalidArguments);
            }
        }
        if let OutputMode::Typed(projection) = &self.output_mode
            && let Some((name, max)) = projection.selector()
        {
            let text = values
                .get(name)
                .and_then(Value::as_str)
                .ok_or(CoreError::InvalidArguments)?;
            if text.is_empty() || !bounded_string(text, max) {
                return Err(CoreError::InvalidArguments);
            }
        }
        match &self.action {
            Action::ApkInstalledPage {} => {
                let cursor = values.get("cursor").and_then(Value::as_str);
                if let Some(cursor) = cursor {
                    crate::packages::PackageCursor::parse(cursor)?;
                }
                Ok(PreparedAction::ApkInstalledPage {
                    cursor: cursor.map(str::to_owned),
                })
            }
            Action::Ubus {
                object,
                method,
                arguments,
            } => {
                let mut prepared = Map::new();
                for (key, value) in arguments {
                    if let Some(name) = value.as_str().and_then(parameter_reference) {
                        if let Some(value) = values.get(name) {
                            prepared.insert(key.clone(), value.clone());
                        }
                    } else {
                        prepared.insert(key.clone(), value.clone());
                    }
                }
                Ok(PreparedAction::Ubus {
                    object: object.clone(),
                    method: method.clone(),
                    arguments: Value::Object(prepared),
                })
            }
            Action::Process { program, args } => {
                let mut prepared = Vec::with_capacity(args.len());
                for arg in args {
                    if let Some(name) = parameter_reference(arg) {
                        let value = values.get(name).ok_or(CoreError::MissingArgument)?;
                        prepared.push(scalar_text(value).ok_or(CoreError::InvalidArguments)?);
                    } else {
                        prepared.push(arg.clone());
                    }
                }
                Ok(PreparedAction::Process {
                    program: program.clone(),
                    args: prepared,
                })
            }
        }
    }

    pub fn input_schema(&self) -> Value {
        let properties: Map<String, Value> = self
            .parameters
            .iter()
            .map(|(name, parameter)| {
                let mut schema = Map::new();
                schema.insert(
                    "type".into(),
                    json!(match parameter.kind {
                        ParameterKind::String => "string",
                        ParameterKind::Integer => "integer",
                        ParameterKind::Boolean => "boolean",
                    }),
                );
                if parameter.kind == ParameterKind::String {
                    let selector_limit = match &self.output_mode {
                        OutputMode::Typed(projection) => projection
                            .selector()
                            .filter(|(parameter, _)| *parameter == name)
                            .map(|(_, max)| max),
                        _ => None,
                    };
                    let max = if matches!(self.action, Action::ApkInstalledPage {}) {
                        37
                    } else {
                        selector_limit.unwrap_or(MAX_VALUE_BYTES)
                    };
                    schema.insert("maxLength".into(), json!(max));
                    if selector_limit.is_some() {
                        schema.insert("minLength".into(), json!(1));
                    }
                    if matches!(self.action, Action::ApkInstalledPage {}) {
                        schema.insert("minLength".into(), json!(34));
                        schema.insert("pattern".into(), json!("^[0-9a-f]{32}\\.[1-9][0-9]{0,3}$"));
                    }
                    schema.insert(
                        "description".into(),
                        json!(format!("At most {max} UTF-8 bytes; NUL is not permitted.")),
                    );
                }
                if parameter.kind == ParameterKind::Integer
                    && matches!(self.action, Action::Ubus { .. })
                {
                    schema.insert("minimum".into(), json!(UBUS_INTEGER_MIN));
                    schema.insert("maximum".into(), json!(UBUS_INTEGER_MAX));
                }
                if !parameter.allowed_values.is_empty() {
                    let allowed: Vec<Value> = parameter
                        .allowed_values
                        .iter()
                        .map(|value| match parameter.kind {
                            ParameterKind::String => json!(value),
                            ParameterKind::Integer => {
                                serde_json::from_str(value).unwrap_or(Value::Null)
                            }
                            ParameterKind::Boolean => json!(value == "true"),
                        })
                        .collect();
                    schema.insert("enum".into(), json!(allowed));
                }
                (name.clone(), Value::Object(schema))
            })
            .collect();
        let required: Vec<&String> = self
            .parameters
            .iter()
            .filter_map(|(name, parameter)| parameter.required.then_some(name))
            .collect();
        json!({"type":"object", "properties": properties, "required": required, "additionalProperties": false})
    }

    /// Return only explicitly selected paths. Empty projections disclose nothing.
    ///
    /// Key heuristics are an extra guard, not a guarantee of secret detection in
    /// arbitrary extension output. Built-ins therefore avoid broad subtrees.
    pub fn project(&self, output: &Value) -> Value {
        let mut projected = Map::new();
        // Typed schemas have no unbound legacy execution path.
        if matches!(self.output_mode, OutputMode::Typed(_)) {
            return Value::Object(projected);
        }
        // Catalog construction rejects overlaps. Keep the public projection
        // method defensive even when called on a separately constructed value.
        if !disjoint_output_fields(&self.output_fields) {
            return Value::Object(projected);
        }
        for pointer in &self.output_fields {
            if !valid_pointer(pointer)
                || pointer
                    .split('/')
                    .skip(1)
                    .any(|part| sensitive_key(&part.replace("~1", "/").replace("~0", "~")))
            {
                continue;
            }
            if let Some(value) = output.pointer(pointer) {
                // Scalar schemas select leaves only. A malformed
                // backend cannot widen a scalar field into an entire subtree.
                if self.output_mode == OutputMode::Scalars
                    && (value.is_array() || value.is_object())
                {
                    continue;
                }
                projected.insert(pointer.clone(), redact(value));
            }
        }
        Value::Object(projected)
    }

    pub fn prepare_invocation(&self, input: &Value) -> Result<PreparedInvocation<'_>, CoreError> {
        PreparedInvocation::new(self, input)
    }
}
