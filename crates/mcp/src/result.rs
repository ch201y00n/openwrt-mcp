//! Count the actual JSON encoding without allocating another serialized copy.
use std::io::{Error, ErrorKind, Result as IoResult, Write};

use openwrt_mcp_core::MAX_NORMALIZED_BYTES;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Value, json};

pub const MAX_CALL_RESULT_BYTES: usize = 262_144;

struct Counter {
    remaining: usize,
}

impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        self.remaining = self
            .remaining
            .checked_sub(bytes.len())
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "mcp_result_limit"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

fn fits(limit: usize, encode: impl FnOnce(&mut Counter) -> serde_json::Result<()>) -> bool {
    encode(&mut Counter { remaining: limit }).is_ok()
}

pub(crate) fn failure(code: &'static str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(json!({"error": code}).to_string())])
}

pub(crate) fn success(value: Value) -> CallToolResult {
    // Runtime already enforces this before its completion audit. Keep the
    // protocol guard defensive, before allocating text or structured copies.
    if !fits(MAX_NORMALIZED_BYTES, |counter| {
        serde_json::to_writer(counter, &value)
    }) {
        return failure("output_limit");
    }
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    if !fits(MAX_CALL_RESULT_BYTES, |counter| {
        serde_json::to_writer(counter, &result)
    }) {
        return failure("output_limit");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_actual_escaping_and_exact_bound_without_retaining_serialized_bytes() {
        for value in [
            json!({"field": "한글\n\t\"\\\u{1}"}),
            json!({"items": [{"name": "synthetic", "up": true}]}),
        ] {
            let encoded = serde_json::to_vec(&value).unwrap();
            assert!(fits(encoded.len(), |counter| serde_json::to_writer(
                counter, &value
            )));
            assert!(!fits(encoded.len() - 1, |counter| serde_json::to_writer(
                counter, &value
            )));
        }
    }

    #[test]
    fn defense_rejects_oversize_value_before_making_both_protocol_copies() {
        let result = success(json!({"field": "x".repeat(MAX_NORMALIZED_BYTES)}));
        assert_eq!(result.is_error, Some(true));
        assert!(result.structured_content.is_none());
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(encoded.contains("output_limit"));
        assert!(encoded.len() < 256);
    }
}
