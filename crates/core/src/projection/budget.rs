use serde_json::Value;

use crate::CoreError;

pub const MAX_NORMALIZED_BYTES: usize = 65_536;
const MAX_VALUE_DEPTH: usize = 32;
const MAX_VALUE_NODES: usize = 65_536;

#[derive(Default)]
pub(super) struct Budget {
    bytes: usize,
    scanned: usize,
    emitted: usize,
    nodes: usize,
}

impl Budget {
    pub(super) fn bytes(&mut self, count: usize) -> Result<(), CoreError> {
        self.bytes = self
            .bytes
            .checked_add(count)
            .ok_or(CoreError::OutputLimit)?;
        if self.bytes > MAX_NORMALIZED_BYTES {
            return Err(CoreError::OutputLimit);
        }
        Ok(())
    }

    pub(super) fn scan(&mut self, count: usize, max: usize) -> Result<(), CoreError> {
        if count > max {
            return Err(CoreError::OutputLimit);
        }
        self.scanned = self
            .scanned
            .checked_add(count)
            .ok_or(CoreError::OutputLimit)?;
        if self.scanned > super::MAX_COLLECTION_ITEMS {
            return Err(CoreError::OutputLimit);
        }
        Ok(())
    }

    pub(super) fn emit(&mut self) -> Result<(), CoreError> {
        self.emitted += 1;
        if self.emitted > super::MAX_COLLECTION_ITEMS {
            return Err(CoreError::OutputLimit);
        }
        Ok(())
    }

    pub(super) fn node(&mut self, depth: usize) -> Result<(), CoreError> {
        self.nodes += 1;
        if depth > MAX_VALUE_DEPTH || self.nodes > MAX_VALUE_NODES {
            return Err(CoreError::OutputLimit);
        }
        Ok(())
    }

    pub(super) fn text(&mut self, value: &str) -> Result<(), CoreError> {
        // Every UTF-8 byte costs at least one serialized byte; check length
        // before inspecting a possibly enormous untrusted string.
        if value.len() > MAX_NORMALIZED_BYTES {
            return Err(CoreError::OutputLimit);
        }
        let mut count = 2usize;
        for byte in value.bytes() {
            count = count
                .checked_add(match byte {
                    b'"' | b'\\' | 8 | 9 | 10 | 12 | 13 => 2,
                    0..=31 => 6,
                    _ => 1,
                })
                .ok_or(CoreError::OutputLimit)?;
            if count > MAX_NORMALIZED_BYTES {
                return Err(CoreError::OutputLimit);
            }
        }
        self.bytes(count)
    }

    pub(super) fn key(&mut self, value: &str, previous: bool) -> Result<(), CoreError> {
        self.text(value)?;
        self.bytes(1 + usize::from(previous))
    }

    pub(super) fn scalar(&mut self, value: &Value) -> Result<(), CoreError> {
        match value {
            Value::Null => self.bytes(4),
            Value::Bool(value) => self.bytes(if *value { 4 } else { 5 }),
            Value::Number(value) => self.bytes(value.to_string().len()),
            Value::String(value) => self.text(value),
            _ => Err(CoreError::InvalidOutput),
        }
    }
}

/// Check the exact serde_json encoding size without serializing or cloning the
/// result. Defensive depth/node limits also cover trusted fake backend Values.
pub fn check_normalized_result(value: &Value) -> Result<(), CoreError> {
    fn visit(value: &Value, depth: usize, budget: &mut Budget) -> Result<(), CoreError> {
        budget.node(depth)?;
        match value {
            Value::Array(items) => {
                budget.bytes(2)?;
                for (index, item) in items.iter().enumerate() {
                    budget.bytes(usize::from(index != 0))?;
                    visit(item, depth + 1, budget)?;
                }
                Ok(())
            }
            Value::Object(fields) => {
                budget.bytes(2)?;
                for (index, (key, value)) in fields.iter().enumerate() {
                    budget.key(key, index != 0)?;
                    visit(value, depth + 1, budget)?;
                }
                Ok(())
            }
            _ => budget.scalar(value),
        }
    }
    visit(value, 0, &mut Budget::default())
}
