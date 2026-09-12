use serde_json::{Map, Value};

use super::budget::{Budget, MAX_NORMALIZED_BYTES};
use crate::operation::sensitive_key;
use crate::{CoreError, Operation, OutputMode};

fn redact(value: &Value, depth: usize, budget: &mut Budget) -> Result<Value, CoreError> {
    budget.node(depth)?;
    match value {
        Value::Object(fields) => {
            budget.bytes(2)?;
            let mut result = Map::new();
            for (key, value) in fields {
                // Even filtered keys consume bounded traversal work. Do not
                // allocate the heuristic's normalized copy of an enormous key.
                budget.node(depth + 1)?;
                if key.len() > MAX_NORMALIZED_BYTES {
                    return Err(CoreError::OutputLimit);
                }
                if sensitive_key(key) {
                    continue;
                }
                budget.key(key, !result.is_empty())?;
                let value = redact(value, depth + 1, budget)?;
                result.insert(key.clone(), value);
            }
            Ok(Value::Object(result))
        }
        Value::Array(values) => {
            budget.bytes(2)?;
            let mut result = Vec::new();
            for value in values {
                budget.bytes(usize::from(!result.is_empty()))?;
                result.push(redact(value, depth + 1, budget)?);
            }
            Ok(Value::Array(result))
        }
        _ => {
            budget.scalar(value)?;
            Ok(value.clone())
        }
    }
}

pub(super) fn project(operation: &Operation, source: &Value) -> Result<Value, CoreError> {
    let mut budget = Budget::default();
    let mut result = Map::new();
    budget.bytes(2)?;
    for pointer in &operation.output_fields {
        if pointer
            .split('/')
            .skip(1)
            .any(|part| sensitive_key(&part.replace("~1", "/").replace("~0", "~")))
        {
            continue;
        }
        let Some(value) = source.pointer(pointer) else {
            continue;
        };
        if matches!(operation.output_mode, OutputMode::Scalars)
            && (value.is_object() || value.is_array())
        {
            continue;
        }
        budget.key(pointer, !result.is_empty())?;
        let value = redact(value, 1, &mut budget)?;
        result.insert(pointer.clone(), value);
    }
    Ok(Value::Object(result))
}
