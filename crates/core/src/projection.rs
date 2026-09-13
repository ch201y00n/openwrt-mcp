//! Finite, reviewed response shapes. These definitions contain no device I/O.
//!
//! Collection types bottom out at a leaf record: deserialization cannot build
//! arbitrarily recursive schemas. Invocation-bound selectors are private.

mod budget;
mod evaluate;
mod legacy;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::operation::identifier;
use crate::{CoreError, Operation, OutputMode, Parameter, ParameterKind, PreparedAction};

pub use budget::{MAX_NORMALIZED_BYTES, check_normalized_result};

pub const SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;
pub const MAX_COLLECTION_ITEMS: usize = 256;
pub const MAX_COLLECTION_NODES: usize = 8;
pub const MAX_RECORD_FIELDS: usize = 64;
pub const MAX_TEXT_BYTES: usize = 1024;
const MAX_SCHEMA_DEPTH: usize = 8;
const MAX_POINTER_DEPTH: usize = 8;
const MAX_POINTER_BYTES: usize = 512;
const MAX_ROOT_GUARDS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterSource {
    JsonInteger,
    DecimalText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScalarKind {
    Boolean {},
    Text { max_bytes: usize },
    SafeInteger { min: i64, max: i64 },
    FalseOrSafeInteger { min: i64, max: i64 },
    DecimalCounter { source: CounterSource },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarField {
    pub name: String,
    pub source: String,
    pub presence: Presence,
    pub value: ScalarKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextIdentity {
    pub name: String,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeafRecord {
    pub fields: Vec<ScalarField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record<C> {
    pub fields: Vec<ScalarField>,
    pub collections: Vec<CollectionField<C>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionField<C> {
    pub name: String,
    pub presence: Presence,
    pub collection: C,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Collection<R> {
    RowArray {
        source: String,
        max_items: usize,
        record: R,
    },
    ObjectArray {
        source: String,
        max_items: usize,
        identity: String,
        record: R,
    },
    ObjectEntries {
        source: String,
        max_items: usize,
        key: TextIdentity,
        record: R,
    },
    ScalarArray {
        source: String,
        max_items: usize,
        name: String,
        value: ScalarKind,
        unique: bool,
    },
}

pub type InnerRecord = Record<Collection<LeafRecord>>;
pub type RootRecord = Record<Collection<InnerRecord>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Selection {
    All {},
    ExactOne { parameter: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TypedProjection {
    Record {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reject_if_present: Vec<String>,
        record: RootRecord,
    },
    Collection {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        reject_if_present: Vec<String>,
        collection: Collection<InnerRecord>,
        selection: Selection,
    },
}

/// Immutable action and projection binding. Neither field nor constructor is
/// public, and the bound selector cannot be serialized or exposed through Debug.
pub struct PreparedInvocation<'a> {
    action: PreparedAction,
    projection: BoundProjection<'a>,
}

enum BoundProjection<'a> {
    Legacy(&'a Operation),
    Typed {
        definition: &'a TypedProjection,
        selector: Option<String>,
    },
}

impl<'a> PreparedInvocation<'a> {
    pub(crate) fn new(operation: &'a Operation, input: &Value) -> Result<Self, CoreError> {
        let action = operation.prepare(input)?;
        let projection = match &operation.output_mode {
            OutputMode::Scalars | OutputMode::Structured => BoundProjection::Legacy(operation),
            OutputMode::Typed(definition) => BoundProjection::Typed {
                definition,
                selector: definition
                    .selector()
                    .map(|(name, _)| {
                        input
                            .get(name)
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .ok_or(CoreError::InvalidArguments)
                    })
                    .transpose()?,
            },
        };
        Ok(Self { action, projection })
    }

    pub fn action(&self) -> &PreparedAction {
        &self.action
    }

    pub fn project(&self, output: &Value) -> Result<Value, CoreError> {
        if matches!(self.action, PreparedAction::ApkInstalledPage { .. }) {
            return Err(CoreError::InvalidDefinition);
        }
        match &self.projection {
            BoundProjection::Legacy(operation) => legacy::project(operation, output),
            BoundProjection::Typed {
                definition,
                selector,
            } => evaluate::project(definition, selector.as_deref(), output),
        }
    }
}

impl ScalarKind {
    fn validate(&self) -> Result<(), CoreError> {
        let valid = match self {
            Self::Boolean {} | Self::DecimalCounter { .. } => true,
            Self::Text { max_bytes } => (1..=MAX_TEXT_BYTES).contains(max_bytes),
            Self::SafeInteger { min, max } | Self::FalseOrSafeInteger { min, max } => {
                min <= max && *min >= -SAFE_INTEGER_MAX && *max <= SAFE_INTEGER_MAX
            }
        };
        valid.then_some(()).ok_or(CoreError::InvalidDefinition)
    }
}

trait RecordShape {
    fn fields(&self) -> &[ScalarField];
    fn validate_record(
        &self,
        depth: usize,
        nodes: &mut usize,
        key: Option<&TextIdentity>,
    ) -> Result<(), CoreError>;
    fn project_record(
        &self,
        source: &Value,
        key: Option<(&TextIdentity, &str)>,
        emit: bool,
        budget: &mut budget::Budget,
    ) -> Result<Option<Value>, CoreError>;
}

fn validate_fields<'a>(
    fields: &'a [ScalarField],
    collections: impl Iterator<Item = (&'a str, &'a str)>,
    count: usize,
    key: Option<&TextIdentity>,
    depth: usize,
) -> Result<(), CoreError> {
    if depth > MAX_SCHEMA_DEPTH
        || count
            .checked_add(usize::from(key.is_some()))
            .is_none_or(|count| count > MAX_RECORD_FIELDS)
    {
        return Err(CoreError::InvalidDefinition);
    }
    let mut names = BTreeSet::new();
    let mut paths = Vec::new();
    if let Some(key) = key {
        if !identifier(&key.name) || !(1..=MAX_TEXT_BYTES).contains(&key.max_bytes) {
            return Err(CoreError::InvalidDefinition);
        }
        names.insert(key.name.as_str());
    }
    for field in fields {
        if !identifier(&field.name) || !names.insert(field.name.as_str()) {
            return Err(CoreError::InvalidDefinition);
        }
        field.value.validate()?;
        paths.push(pointer_segments(&field.source, false)?);
    }
    for (name, source) in collections {
        if !identifier(name) || !names.insert(name) {
            return Err(CoreError::InvalidDefinition);
        }
        paths.push(pointer_segments(source, true)?);
    }
    paths.sort();
    if paths.windows(2).any(|pair| pair[1].starts_with(&pair[0])) {
        return Err(CoreError::InvalidDefinition);
    }
    Ok(())
}

fn pointer_segments(pointer: &str, root_allowed: bool) -> Result<Vec<String>, CoreError> {
    if (pointer.is_empty() && root_allowed) || pointer.starts_with('/') {
        if pointer.len() > MAX_POINTER_BYTES || pointer.contains('\0') {
            return Err(CoreError::InvalidDefinition);
        }
        let mut result = Vec::new();
        for part in pointer.split('/').skip(1) {
            if result.len() == MAX_POINTER_DEPTH {
                return Err(CoreError::InvalidDefinition);
            }
            let mut bytes = part.bytes();
            while let Some(byte) = bytes.next() {
                if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
                    return Err(CoreError::InvalidDefinition);
                }
            }
            result.push(part.replace("~1", "/").replace("~0", "~"));
        }
        return Ok(result);
    }
    Err(CoreError::InvalidDefinition)
}

impl<R> Collection<R> {
    fn source(&self) -> &str {
        match self {
            Self::ObjectArray { source, .. }
            | Self::RowArray { source, .. }
            | Self::ObjectEntries { source, .. }
            | Self::ScalarArray { source, .. } => source,
        }
    }

    fn identity_limit(&self) -> Option<usize>
    where
        R: RecordShape,
    {
        match self {
            Self::ObjectArray {
                identity, record, ..
            } => record.fields().iter().find_map(|field| {
                if field.name == *identity
                    && field.presence == Presence::Required
                    && let ScalarKind::Text { max_bytes } = field.value
                {
                    return Some(max_bytes);
                }
                None
            }),
            Self::ObjectEntries { key, .. } => Some(key.max_bytes),
            Self::ScalarArray { .. } | Self::RowArray { .. } => None,
        }
    }

    fn validate_collection(&self, depth: usize, nodes: &mut usize) -> Result<(), CoreError>
    where
        R: RecordShape,
    {
        *nodes += 1;
        if *nodes > MAX_COLLECTION_NODES || depth > MAX_SCHEMA_DEPTH {
            return Err(CoreError::InvalidDefinition);
        }
        pointer_segments(self.source(), true)?;
        let max_items = match self {
            Self::RowArray {
                max_items, record, ..
            } => {
                record.validate_record(depth + 1, nodes, None)?;
                *max_items
            }
            Self::ObjectArray {
                max_items, record, ..
            } => {
                if self.identity_limit().is_none() {
                    return Err(CoreError::InvalidDefinition);
                }
                record.validate_record(depth + 1, nodes, None)?;
                *max_items
            }
            Self::ObjectEntries {
                max_items,
                key,
                record,
                ..
            } => {
                record.validate_record(depth + 1, nodes, Some(key))?;
                *max_items
            }
            Self::ScalarArray {
                max_items,
                name,
                value,
                ..
            } => {
                if !identifier(name) {
                    return Err(CoreError::InvalidDefinition);
                }
                value.validate()?;
                *max_items
            }
        };
        if !(1..=MAX_COLLECTION_ITEMS).contains(&max_items) {
            return Err(CoreError::InvalidDefinition);
        }
        Ok(())
    }
}

impl TypedProjection {
    pub(crate) fn validate(
        &self,
        parameters: &BTreeMap<String, Parameter>,
    ) -> Result<(), CoreError> {
        let guards = self.reject_if_present();
        if guards.len() > MAX_ROOT_GUARDS {
            return Err(CoreError::InvalidDefinition);
        }
        let mut paths = guards
            .iter()
            .map(|path| pointer_segments(path, false))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        if paths.windows(2).any(|pair| pair[1].starts_with(&pair[0])) {
            return Err(CoreError::InvalidDefinition);
        }
        let mut nodes = 0;
        match self {
            Self::Record { record, .. } => record.validate_record(1, &mut nodes, None)?,
            Self::Collection {
                collection,
                selection,
                ..
            } => {
                collection.validate_collection(1, &mut nodes)?;
                if let Selection::ExactOne { parameter } = selection {
                    let max = collection
                        .identity_limit()
                        .ok_or(CoreError::InvalidDefinition)?;
                    let parameter = parameters
                        .get(parameter)
                        .ok_or(CoreError::InvalidDefinition)?;
                    if !parameter.required
                        || parameter.kind != ParameterKind::String
                        || parameter
                            .allowed_values
                            .iter()
                            .any(|value| !valid_identity(value, max))
                    {
                        return Err(CoreError::InvalidDefinition);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn selector(&self) -> Option<(&str, usize)> {
        match self {
            Self::Collection {
                collection,
                selection: Selection::ExactOne { parameter },
                ..
            } => collection
                .identity_limit()
                .map(|max| (parameter.as_str(), max)),
            _ => None,
        }
    }

    fn reject_if_present(&self) -> &[String] {
        match self {
            Self::Record {
                reject_if_present, ..
            }
            | Self::Collection {
                reject_if_present, ..
            } => reject_if_present,
        }
    }
}

fn valid_identity(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.contains('\0')
}
