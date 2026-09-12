use std::{collections::BTreeMap, fmt};

use openwrt_mcp_core::{
    CapabilityObservation, MethodSignature, ObjectObservation, ReviewedObject, UbusArgumentType,
    UnknownReason,
};
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};

use crate::{CodecError, command::identifier};

pub const MAX_PROBE_BYTES: usize = 65_536;

struct Arguments(BTreeMap<String, UbusArgumentType>);

impl<'de> Deserialize<'de> for Arguments {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ArgumentsVisitor;
        impl<'de> Visitor<'de> for ArgumentsVisitor {
            type Value = Arguments;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded unique ubus argument map")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut arguments = BTreeMap::new();
                while let Some(name) = map.next_key::<String>()? {
                    if arguments.len() == 64
                        || !identifier(&name, 64)
                        || arguments.contains_key(&name)
                    {
                        return Err(de::Error::custom("invalid ubus argument map"));
                    }
                    let label = map.next_value::<String>()?;
                    if label.is_empty() || label.len() > 64 || label.chars().any(char::is_control) {
                        return Err(de::Error::custom("invalid ubus type label"));
                    }
                    let kind = match label.as_str() {
                        "String" => UbusArgumentType::String,
                        "Integer" => UbusArgumentType::Integer,
                        "Boolean" => UbusArgumentType::Boolean,
                        "Array" => UbusArgumentType::Array,
                        "Table" => UbusArgumentType::Table,
                        _ => UbusArgumentType::Unknown,
                    };
                    arguments.insert(name, kind);
                }
                Ok(Arguments(arguments))
            }
        }
        deserializer.deserialize_map(ArgumentsVisitor)
    }
}

struct Method(String, MethodSignature);

impl<'de> Deserialize<'de> for Method {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MethodVisitor;
        impl<'de> Visitor<'de> for MethodVisitor {
            type Value = Method;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("one named ubus method")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let name = map
                    .next_key::<String>()?
                    .ok_or_else(|| de::Error::custom("missing ubus method"))?;
                if !identifier(&name, 128) {
                    return Err(de::Error::custom("invalid ubus method name"));
                }
                let Arguments(arguments) = map.next_value()?;
                if map.next_key::<String>()?.is_some() {
                    return Err(de::Error::custom("duplicate or multiple ubus methods"));
                }
                Ok(Method(name, MethodSignature { arguments }))
            }
        }
        deserializer.deserialize_map(MethodVisitor)
    }
}

/// Parse only complete successful transport output. No source text is retained in
/// errors; missing ACL-filtered objects remain unknown, never proven unsupported.
pub fn parse_ubus_describe(
    object: ReviewedObject,
    bytes: &[u8],
    maximum: usize,
) -> Result<CapabilityObservation, CodecError> {
    if maximum == 0 || bytes.len() > maximum.min(MAX_PROBE_BYTES) {
        return Err(CodecError::OutputLimit);
    }
    if bytes.is_empty() {
        return Ok(CapabilityObservation::Unknown(
            UnknownReason::NotObservedOrHidden,
        ));
    }
    if !bytes.ends_with(b"\n") {
        return Err(CodecError::InvalidObservation);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| CodecError::InvalidObservation)?;
    let mut lines = text.lines();
    let header = lines.next().ok_or(CodecError::InvalidObservation)?;
    let prefix = format!("'{}' @", object.as_str());
    let id = header
        .strip_prefix(&prefix)
        .ok_or(CodecError::InvalidObservation)?;
    if id.len() != 8 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CodecError::InvalidObservation);
    }
    let mut methods = BTreeMap::new();
    for line in lines {
        if methods.len() == 128 {
            return Err(CodecError::InvalidObservation);
        }
        let fragment = line
            .strip_prefix('\t')
            .ok_or(CodecError::InvalidObservation)?;
        // One already byte-bounded line becomes one object for serde's map
        // visitors. They reject duplicate keys before a map could overwrite them.
        let Method(name, signature) = serde_json::from_str(&format!("{{{fragment}}}"))
            .map_err(|_| CodecError::InvalidObservation)?;
        if methods.insert(name, signature).is_some() {
            return Err(CodecError::InvalidObservation);
        }
    }
    let observation = ObjectObservation { object, methods };
    observation
        .validate()
        .map_err(|_| CodecError::InvalidObservation)?;
    Ok(CapabilityObservation::Ubus(observation))
}
