use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::budget::Budget;
use super::{
    Collection, CounterSource, LeafRecord, Presence, Record, RecordShape, ScalarField, ScalarKind,
    Selection, TextIdentity, TypedProjection, valid_identity, validate_fields,
};
use crate::CoreError;

// Missing keys may be optional; an existing scalar where the reviewed path
// requires a container is malformed, not an omitted optional field.
fn lookup<'a>(source: &'a Value, pointer: &str) -> Result<Option<&'a Value>, CoreError> {
    let mut current = source;
    for part in pointer.split('/').skip(1) {
        let key = part.replace("~1", "/").replace("~0", "~");
        let next = match current {
            Value::Object(fields) => fields.get(&key),
            Value::Array(items) => {
                if key.is_empty()
                    || (key.len() > 1 && key.starts_with('0'))
                    || !key.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(CoreError::InvalidOutput);
                }
                key.parse::<usize>().ok().and_then(|index| items.get(index))
            }
            _ => return Err(CoreError::InvalidOutput),
        };
        let Some(next) = next else {
            return Ok(None);
        };
        current = next;
    }
    Ok(Some(current))
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Scalar<'a> {
    Boolean(bool),
    Text(&'a str),
    Integer(i64),
    Counter(u64),
}

impl<'a> Scalar<'a> {
    fn read(kind: &ScalarKind, value: &'a Value) -> Result<Self, CoreError> {
        let invalid = CoreError::InvalidOutput;
        match kind {
            ScalarKind::Boolean {} => value.as_bool().map(Self::Boolean).ok_or(invalid),
            ScalarKind::Text { max_bytes } => {
                let text = value.as_str().ok_or(invalid)?;
                if text.len() > *max_bytes || text.contains('\0') {
                    return Err(invalid);
                }
                Ok(Self::Text(text))
            }
            ScalarKind::FalseOrSafeInteger { .. } if value == &Value::Bool(false) => {
                Ok(Self::Boolean(false))
            }
            ScalarKind::SafeInteger { min, max } | ScalarKind::FalseOrSafeInteger { min, max } => {
                let integer = value.as_i64().ok_or(invalid)?;
                if integer < *min || integer > *max {
                    return Err(invalid);
                }
                Ok(Self::Integer(integer))
            }
            ScalarKind::DecimalCounter { source } => {
                let integer = match source {
                    CounterSource::JsonInteger => value.as_u64().ok_or(invalid)?,
                    CounterSource::DecimalText => {
                        let text = value.as_str().ok_or(invalid)?;
                        if text.is_empty()
                            || text.len() > 20
                            || (text.len() > 1 && text.starts_with('0'))
                            || !text.bytes().all(|byte| byte.is_ascii_digit())
                        {
                            return Err(invalid);
                        }
                        text.parse::<u64>().map_err(|_| invalid)?
                    }
                };
                Ok(Self::Counter(integer))
            }
        }
    }

    fn to_value(&self, budget: &mut Budget) -> Result<Value, CoreError> {
        // Charge exact serialized bytes BEFORE cloning text or allocating the
        // returned scalar. Counter formatting is intrinsically bounded to 20.
        match self {
            Self::Boolean(value) => {
                budget.bytes(if *value { 4 } else { 5 })?;
                Ok(Value::Bool(*value))
            }
            Self::Text(value) => {
                budget.text(value)?;
                Ok(Value::String((*value).to_owned()))
            }
            Self::Integer(value) => {
                budget.bytes(value.to_string().len())?;
                Ok(Value::from(*value))
            }
            Self::Counter(value) => {
                let text = value.to_string();
                budget.text(&text)?;
                Ok(Value::String(text))
            }
        }
    }
}

fn record_fields(
    fields: &[ScalarField],
    source: &Value,
    key: Option<(&TextIdentity, &str)>,
    emit: bool,
    budget: &mut Budget,
) -> Result<Map<String, Value>, CoreError> {
    if !source.is_object() {
        return Err(CoreError::InvalidOutput);
    }
    let mut result = Map::new();
    if emit {
        budget.bytes(2)?;
    }
    if let Some((identity, value)) = key {
        if !valid_identity(value, identity.max_bytes) {
            return Err(CoreError::InvalidOutput);
        }
        if emit {
            budget.key(&identity.name, false)?;
            let value = Scalar::Text(value).to_value(budget)?;
            result.insert(identity.name.clone(), value);
        }
    }
    for field in fields {
        let Some(value) = lookup(source, &field.source)? else {
            if field.presence == Presence::Required {
                return Err(CoreError::InvalidOutput);
            }
            continue;
        };
        let scalar = Scalar::read(&field.value, value)?;
        if emit {
            budget.key(&field.name, !result.is_empty())?;
            let value = scalar.to_value(budget)?;
            result.insert(field.name.clone(), value);
        }
    }
    Ok(result)
}

impl RecordShape for LeafRecord {
    fn fields(&self) -> &[ScalarField] {
        &self.fields
    }

    fn validate_record(
        &self,
        depth: usize,
        _nodes: &mut usize,
        key: Option<&TextIdentity>,
    ) -> Result<(), CoreError> {
        validate_fields(
            &self.fields,
            std::iter::empty(),
            self.fields.len(),
            key,
            depth,
        )
    }

    fn project_record(
        &self,
        source: &Value,
        key: Option<(&TextIdentity, &str)>,
        emit: bool,
        budget: &mut Budget,
    ) -> Result<Option<Value>, CoreError> {
        let fields = record_fields(&self.fields, source, key, emit, budget)?;
        Ok(emit.then_some(Value::Object(fields)))
    }
}

impl<R: RecordShape> RecordShape for Record<Collection<R>> {
    fn fields(&self) -> &[ScalarField] {
        &self.fields
    }

    fn validate_record(
        &self,
        depth: usize,
        nodes: &mut usize,
        key: Option<&TextIdentity>,
    ) -> Result<(), CoreError> {
        let count = self
            .fields
            .len()
            .checked_add(self.collections.len())
            .ok_or(CoreError::InvalidDefinition)?;
        validate_fields(
            &self.fields,
            self.collections
                .iter()
                .map(|field| (field.name.as_str(), field.collection.source())),
            count,
            key,
            depth,
        )?;
        for field in &self.collections {
            field.collection.validate_collection(depth + 1, nodes)?;
        }
        Ok(())
    }

    fn project_record(
        &self,
        source: &Value,
        key: Option<(&TextIdentity, &str)>,
        emit: bool,
        budget: &mut Budget,
    ) -> Result<Option<Value>, CoreError> {
        let mut result = record_fields(&self.fields, source, key, emit, budget)?;
        for field in &self.collections {
            let Some(value) = lookup(source, field.collection.source())? else {
                if field.presence == Presence::Required {
                    return Err(CoreError::InvalidOutput);
                }
                continue;
            };
            if emit {
                budget.key(&field.name, !result.is_empty())?;
            }
            if let Some(value) = field.collection.project_at(value, None, emit, budget)? {
                result.insert(field.name.clone(), value);
            }
        }
        Ok(emit.then_some(Value::Object(result)))
    }
}

impl<R> Collection<R> {
    fn project_at(
        &self,
        source: &Value,
        selector: Option<&str>,
        emit: bool,
        budget: &mut Budget,
    ) -> Result<Option<Value>, CoreError>
    where
        R: RecordShape,
    {
        let mut rows = Vec::new();
        let mut selected = None;
        let mut observed = false;
        if emit && selector.is_none() {
            budget.bytes(2)?;
        }
        match self {
            Self::RowArray {
                max_items, record, ..
            } => {
                if selector.is_some() {
                    return Err(CoreError::InvalidDefinition);
                }
                let values = source.as_array().ok_or(CoreError::InvalidOutput)?;
                budget.scan(values.len(), *max_items)?;
                for value in values {
                    if emit {
                        budget.emit()?;
                        budget.bytes(usize::from(!rows.is_empty()))?;
                    }
                    if let Some(result) = record.project_record(value, None, emit, budget)? {
                        rows.push(result);
                    }
                }
            }
            Self::ObjectArray {
                max_items,
                identity,
                record,
                ..
            } => {
                let values = source.as_array().ok_or(CoreError::InvalidOutput)?;
                budget.scan(values.len(), *max_items)?;
                let identity_field = record
                    .fields()
                    .iter()
                    .find(|field| field.name == *identity)
                    .ok_or(CoreError::InvalidDefinition)?;
                let max = self.identity_limit().ok_or(CoreError::InvalidDefinition)?;
                let mut identities = BTreeSet::new();
                for value in values {
                    let identity = lookup(value, &identity_field.source)?
                        .and_then(Value::as_str)
                        .ok_or(CoreError::InvalidOutput)?;
                    if !valid_identity(identity, max) || !identities.insert(identity) {
                        return Err(CoreError::InvalidOutput);
                    }
                    let matches = selector.is_none_or(|selector| selector == identity);
                    let emit_row = emit && matches;
                    if emit_row {
                        budget.emit()?;
                        if selector.is_none() {
                            budget.bytes(usize::from(!rows.is_empty()))?;
                        }
                    }
                    let result = record.project_record(value, None, emit_row, budget)?;
                    observed |= matches;
                    if let Some(result) = result {
                        if selector.is_some() {
                            selected = Some(result);
                        } else {
                            rows.push(result);
                        }
                    }
                }
            }
            Self::ObjectEntries {
                max_items,
                key,
                record,
                ..
            } => {
                let values = source.as_object().ok_or(CoreError::InvalidOutput)?;
                budget.scan(values.len(), *max_items)?;
                for (identity, value) in values {
                    if !valid_identity(identity, key.max_bytes) {
                        return Err(CoreError::InvalidOutput);
                    }
                    let matches = selector.is_none_or(|selector| selector == identity);
                    let emit_row = emit && matches;
                    if emit_row {
                        budget.emit()?;
                        if selector.is_none() {
                            budget.bytes(usize::from(!rows.is_empty()))?;
                        }
                    }
                    let result =
                        record.project_record(value, Some((key, identity)), emit_row, budget)?;
                    observed |= matches;
                    if let Some(result) = result {
                        if selector.is_some() {
                            selected = Some(result);
                        } else {
                            rows.push(result);
                        }
                    }
                }
            }
            Self::ScalarArray {
                max_items,
                name,
                value: kind,
                unique,
                ..
            } => {
                if selector.is_some() {
                    return Err(CoreError::InvalidDefinition);
                }
                let values = source.as_array().ok_or(CoreError::InvalidOutput)?;
                budget.scan(values.len(), *max_items)?;
                let mut identities = BTreeSet::new();
                for value in values {
                    let scalar = Scalar::read(kind, value)?;
                    if *unique && matches!(scalar, Scalar::Text("")) {
                        return Err(CoreError::InvalidOutput);
                    }
                    if emit {
                        budget.emit()?;
                        budget.bytes(2 + usize::from(!rows.is_empty()))?;
                        budget.key(name, false)?;
                        let value = scalar.to_value(budget)?;
                        let mut fields = Map::new();
                        fields.insert(name.clone(), value);
                        rows.push(Value::Object(fields));
                    }
                    if *unique && !identities.insert(scalar) {
                        return Err(CoreError::InvalidOutput);
                    }
                }
            }
        }
        if selector.is_some() && !observed {
            return Err(CoreError::SelectionNotObserved);
        }
        if !emit {
            return Ok(None);
        }
        Ok(if selector.is_some() {
            selected
        } else {
            Some(Value::Array(rows))
        })
    }
}

pub(super) fn project(
    definition: &TypedProjection,
    selector: Option<&str>,
    source: &Value,
) -> Result<Value, CoreError> {
    for pointer in definition.reject_if_present() {
        if lookup(source, pointer)?.is_some() {
            return Err(CoreError::InvalidOutput);
        }
    }
    let mut budget = Budget::default();
    let result = match definition {
        TypedProjection::Record { record, .. } => {
            record.project_record(source, None, true, &mut budget)?
        }
        TypedProjection::Collection {
            collection,
            selection,
            ..
        } => {
            if matches!(selection, Selection::ExactOne { .. }) != selector.is_some() {
                return Err(CoreError::InvalidDefinition);
            }
            let value = lookup(source, collection.source())?.ok_or(CoreError::InvalidOutput)?;
            if selector.is_none() {
                budget.bytes(2)?;
                budget.key("items", false)?;
            }
            let result = collection.project_at(value, selector, true, &mut budget)?;
            if selector.is_some() {
                result
            } else {
                let mut fields = Map::new();
                fields.insert("items".into(), result.ok_or(CoreError::InvalidOutput)?);
                Some(Value::Object(fields))
            }
        }
    };
    result.ok_or(CoreError::InvalidOutput)
}
