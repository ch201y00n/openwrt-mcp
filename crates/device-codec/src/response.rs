//! Bounded JSON decoding before duplicate keys can be erased by `Value`.

use std::fmt;

use serde::{
    Deserializer,
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};

use crate::CodecError;

pub const MAX_ACTION_BYTES: usize = 16_777_216;
pub const MAX_ACTION_DEPTH: usize = 32;
pub const MAX_ACTION_NODES: usize = 65_536;

/// Decode complete successful action bytes, without retaining parser diagnostics.
/// The root is one value node; map keys are not value nodes. Container depth
/// counts only arrays/objects, including the root container. Empty JSON
/// whitespace preserves the legacy null result, but no other blank bytes do.
pub fn parse_action_response(bytes: &[u8], configured_limit: usize) -> Result<Value, CodecError> {
    if configured_limit == 0 || bytes.len() > configured_limit.min(MAX_ACTION_BYTES) {
        return Err(CodecError::OutputLimit);
    }
    if bytes
        .iter()
        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return Ok(Value::Null);
    }

    let mut budget = Budget {
        remaining: MAX_ACTION_NODES,
        failure: None,
    };
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = ValueSeed {
        budget: &mut budget,
        depth: 0,
    }
    .deserialize(&mut decoder)
    .map_err(|_| budget.failure.unwrap_or(CodecError::InvalidResponse))?;
    decoder.end().map_err(|_| CodecError::InvalidResponse)?;
    Ok(value)
}

struct Budget {
    remaining: usize,
    failure: Option<CodecError>,
}

impl Budget {
    fn exhausted<E: de::Error>(&mut self) -> E {
        self.failure = Some(CodecError::OutputLimit);
        E::custom("action response limit")
    }
}

struct ValueSeed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, decoder: D) -> Result<Value, D::Error> {
        // Charge before visiting or allocating any part of this value.
        if self.budget.remaining == 0 {
            return Err(self.budget.exhausted());
        }
        self.budget.remaining -= 1;
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for ValueSeed<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded unique-key JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<Value, S::Error> {
        if self.depth == MAX_ACTION_DEPTH {
            return Err(self.budget.exhausted());
        }
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(ValueSeed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        if self.depth == MAX_ACTION_DEPTH {
            return Err(self.budget.exhausted());
        }
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            // Key escapes have already been decoded. Reject before reading the
            // repeated value, and before insertion could replace the first one.
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate action response key"));
            }
            let value = map.next_value_seed(ValueSeed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
