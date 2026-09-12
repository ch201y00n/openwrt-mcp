use super::ProtectionError;
use serde::{
    Deserialize, Deserializer,
    de::{Error, MapAccess, Visitor},
};
use std::fmt::Formatter;

#[derive(Debug, Clone, Copy)]
pub struct KeyLimits {
    pub max_key_bytes: usize,
    pub max_container_bytes: usize,
    pub max_entries: usize,
    pub max_directory_bytes: usize,
    pub max_expansion_ratio: usize,
}

impl<'de> Deserialize<'de> for KeyLimits {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyLimitsVisitor;

        impl<'de> Visitor<'de> for KeyLimitsVisitor {
            type Value = KeyLimits;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a key-limit mapping")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<KeyLimits, M::Error> {
                let mut limits = KeyLimits::default();
                let mut seen = 0_u8;
                while let Some(field) = map.next_key::<String>()? {
                    match field.as_str() {
                        "max_key_bytes" => {
                            set_once(&mut map, &mut seen, 1, &mut limits.max_key_bytes)?
                        }
                        "max_container_bytes" => {
                            set_once(&mut map, &mut seen, 2, &mut limits.max_container_bytes)?
                        }
                        "max_entries" => set_once(&mut map, &mut seen, 4, &mut limits.max_entries)?,
                        "max_directory_bytes" => {
                            set_once(&mut map, &mut seen, 8, &mut limits.max_directory_bytes)?
                        }
                        "max_expansion_ratio" => {
                            set_once(&mut map, &mut seen, 16, &mut limits.max_expansion_ratio)?
                        }
                        _ => return Err(M::Error::custom("unknown_key_limit_field")),
                    }
                }
                Ok(limits)
            }
        }

        // Derived struct Deserialize accepts positional sequences. Operator
        // configuration deliberately accepts only named fields, never [] defaults.
        deserializer.deserialize_map(KeyLimitsVisitor)
    }
}

impl Default for KeyLimits {
    fn default() -> Self {
        Self {
            max_key_bytes: 64 * 1024,
            max_container_bytes: 4 * 1024 * 1024,
            max_entries: 128,
            max_directory_bytes: 256 * 1024,
            max_expansion_ratio: 100,
        }
    }
}

impl KeyLimits {
    pub fn validate(&self) -> Result<(), ProtectionError> {
        let fields = [
            (self.max_key_bytes, 1024 * 1024),
            (self.max_container_bytes, 16 * 1024 * 1024),
            (self.max_entries, 4096),
            (self.max_directory_bytes, 1024 * 1024),
            (self.max_expansion_ratio, 1000),
        ];
        if fields.iter().any(|(value, cap)| *value == 0 || value > cap)
            || self.max_directory_bytes > self.max_container_bytes
        {
            return Err(ProtectionError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CryptoLimits {
    pub max_input_bytes: u64,
    pub max_output_bytes: u64,
    pub timeout_ms: u64,
    pub max_recipients: usize,
}

impl<'de> Deserialize<'de> for CryptoLimits {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CryptoLimitsVisitor;

        impl<'de> Visitor<'de> for CryptoLimitsVisitor {
            type Value = CryptoLimits;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a crypto-limit mapping")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<CryptoLimits, M::Error> {
                let mut limits = CryptoLimits::default();
                let mut seen = 0_u8;
                while let Some(field) = map.next_key::<String>()? {
                    match field.as_str() {
                        "max_input_bytes" => {
                            set_once(&mut map, &mut seen, 1, &mut limits.max_input_bytes)?
                        }
                        "max_output_bytes" => {
                            set_once(&mut map, &mut seen, 2, &mut limits.max_output_bytes)?
                        }
                        "timeout_ms" => set_once(&mut map, &mut seen, 4, &mut limits.timeout_ms)?,
                        "max_recipients" => {
                            set_once(&mut map, &mut seen, 8, &mut limits.max_recipients)?
                        }
                        _ => return Err(M::Error::custom("unknown_crypto_limit_field")),
                    }
                }
                Ok(limits)
            }
        }

        deserializer.deserialize_map(CryptoLimitsVisitor)
    }
}

fn set_once<'de, M: MapAccess<'de>, T: Deserialize<'de>>(
    map: &mut M,
    seen: &mut u8,
    field: u8,
    value: &mut T,
) -> Result<(), M::Error> {
    if *seen & field != 0 {
        return Err(M::Error::custom("duplicate_limit_field"));
    }
    *value = map.next_value()?;
    *seen |= field;
    Ok(())
}

impl Default for CryptoLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_output_bytes: 65 * 1024 * 1024,
            timeout_ms: 30_000,
            max_recipients: 16,
        }
    }
}

impl CryptoLimits {
    pub fn validate(&self) -> Result<(), ProtectionError> {
        if !(1..=1024 * 1024 * 1024).contains(&self.max_input_bytes)
            || !(1..=1024 * 1024 * 1024).contains(&self.max_output_bytes)
            || !(1..=300_000).contains(&self.timeout_ms)
            || !(1..=64).contains(&self.max_recipients)
        {
            return Err(ProtectionError::InvalidConfig);
        }
        Ok(())
    }
}
