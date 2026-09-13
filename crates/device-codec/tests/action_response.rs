//! Pure strict action JSON fixtures, required on every supported host.
mod archive;
use openwrt_mcp_device_codec::{
    CodecError, MAX_ACTION_BYTES, MAX_ACTION_DEPTH, MAX_ACTION_NODES, parse_action_response,
};
use serde_json::{Value, json};

fn parse(bytes: &[u8]) -> Result<Value, CodecError> {
    parse_action_response(bytes, MAX_ACTION_BYTES)
}

#[test]
fn valid_json_types_and_exact_integer_values_are_preserved() {
    let value = parse(
        br#"{"null":null,"boolean":true,"integer":-17,"large":18446744073709551615,"decimal":1.5,"exponent":2e3,"text":"\uD83D\uDE00","array":[{},false]}"#,
    )
    .unwrap();
    assert_eq!(value["null"], Value::Null);
    assert_eq!(value["boolean"], true);
    assert_eq!(value["integer"].as_i64(), Some(-17));
    assert_eq!(value["large"].as_u64(), Some(u64::MAX));
    assert_eq!(value["decimal"].as_f64(), Some(1.5));
    assert!(value["decimal"].as_i64().is_none());
    assert!(value["exponent"].as_u64().is_none());
    assert_eq!(value["text"], "😀");
    assert_eq!(value["array"], json!([{}, false]));
    for bytes in [b"null".as_slice(), b"false", b"0", b"\"value\"", b"[]"] {
        assert_eq!(
            parse(bytes).unwrap(),
            serde_json::from_slice::<Value>(bytes).unwrap()
        );
    }
}

#[test]
fn only_empty_json_whitespace_preserves_legacy_null() {
    for bytes in [b"".as_slice(), b" \t\r\n", b"\nnull \r\t"] {
        assert_eq!(parse(bytes), Ok(Value::Null));
    }
    for bytes in [b"\x0b".as_slice(), b"\x0c", b"\0", b"\xc2\xa0"] {
        assert_eq!(parse(bytes), Err(CodecError::InvalidResponse));
    }
}

#[test]
fn configured_byte_limit_is_checked_before_empty_or_json_parsing() {
    assert_eq!(MAX_ACTION_BYTES, 16_777_216);
    assert_eq!(MAX_ACTION_DEPTH, 32);
    assert_eq!(MAX_ACTION_NODES, 65_536);
    assert_eq!(parse_action_response(b"{}", 2), Ok(json!({})));
    assert_eq!(
        parse_action_response(b"{}", 1),
        Err(CodecError::OutputLimit)
    );
    assert_eq!(
        parse_action_response(b"  ", 1),
        Err(CodecError::OutputLimit)
    );
    assert_eq!(parse_action_response(b"", 0), Err(CodecError::OutputLimit));
    assert_eq!(parse_action_response("\"é\"".as_bytes(), 4), Ok(json!("é")));
    assert_eq!(
        parse_action_response("\"é\"".as_bytes(), 3),
        Err(CodecError::OutputLimit)
    );
}

#[test]
fn hard_byte_limit_cannot_be_raised_and_includes_trailing_whitespace() {
    let mut bytes = vec![b' '; MAX_ACTION_BYTES];
    bytes[..2].copy_from_slice(b"{}");
    assert_eq!(parse_action_response(&bytes, usize::MAX), Ok(json!({})));
    bytes.push(b' ');
    assert_eq!(
        parse_action_response(&bytes, usize::MAX),
        Err(CodecError::OutputLimit)
    );
}

#[test]
fn duplicate_keys_are_rejected_at_every_depth_even_in_unselected_data() {
    for bytes in [
        br#"{"a":1,"a":2}"#.as_slice(),
        br#"{"a":1,"\u0061":2}"#,
        br#"{"a/b":1,"a\/b":2}"#,
        br#"{"":1,"":2}"#,
        br#"{"public":true,"foreign":{"credentials":1,"credentials":2}}"#,
        br#"[{"nested":[{"name":1,"name":2}]}]"#,
        "{\"é\":1,\"\\u00e9\":2}".as_bytes(),
        br#"{"\uD83D\uDE00":1,"\ud83d\ude00":2}"#,
    ] {
        assert_eq!(parse(bytes), Err(CodecError::InvalidResponse));
    }
    assert_eq!(parse(br#"[{"a":1},{"a":2}]"#), Ok(json!([{"a":1},{"a":2}])));
    assert_eq!(parse(br#"{"a":1,"A":2}"#), Ok(json!({"a":1,"A":2})));
}

#[test]
fn duplicate_is_rejected_before_its_value_is_parsed_or_inserted() {
    let bytes = format!("{{\"same\":1,\"same\":{}", "[".repeat(MAX_ACTION_DEPTH + 1));
    assert_eq!(parse(bytes.as_bytes()), Err(CodecError::InvalidResponse));
}

#[test]
fn depth_counts_all_containers_and_rejects_before_descending_too_far() {
    for depth in [MAX_ACTION_DEPTH, MAX_ACTION_DEPTH + 1] {
        for (open, close) in [("[", "]"), ("{\"child\":", "}")] {
            let bytes = format!("{}null{}", open.repeat(depth), close.repeat(depth));
            let result = parse(bytes.as_bytes());
            if depth == MAX_ACTION_DEPTH {
                assert!(result.is_ok());
            } else {
                assert_eq!(result, Err(CodecError::OutputLimit));
            }
        }
    }
    let mixed = format!("{}0{}", "{\"a\":[".repeat(16), "]}".repeat(16));
    assert!(parse(mixed.as_bytes()).is_ok());
    let malformed_deep = "[".repeat(MAX_ACTION_DEPTH + 1);
    assert_eq!(
        parse(malformed_deep.as_bytes()),
        Err(CodecError::OutputLimit)
    );
}

fn flat_array(elements: usize) -> Vec<u8> {
    format!("[{}]", vec!["0"; elements].join(",")).into_bytes()
}

#[test]
fn node_budget_counts_the_root_and_is_not_reset_per_sibling() {
    let exact = flat_array(MAX_ACTION_NODES - 1);
    assert_eq!(
        parse(&exact).unwrap().as_array().unwrap().len(),
        MAX_ACTION_NODES - 1
    );
    assert_eq!(
        parse(&flat_array(MAX_ACTION_NODES)),
        Err(CodecError::OutputLimit)
    );
    let truncated_extra = format!(
        "{},[",
        std::str::from_utf8(&exact[..exact.len() - 1]).unwrap()
    );
    assert_eq!(
        parse(truncated_extra.as_bytes()),
        Err(CodecError::OutputLimit)
    );

    // Root + two child arrays + their elements share a single budget.
    let left = String::from_utf8(flat_array(MAX_ACTION_NODES / 2 - 1)).unwrap();
    let right = String::from_utf8(flat_array(MAX_ACTION_NODES / 2 - 2)).unwrap();
    assert!(parse(format!("[{left},{right}]").as_bytes()).is_ok());
    assert_eq!(
        parse(format!("[{left},{left}]").as_bytes()),
        Err(CodecError::OutputLimit)
    );
}

#[test]
fn object_values_share_node_budget_but_keys_are_not_value_nodes() {
    let fields = (0..MAX_ACTION_NODES - 1)
        .map(|index| format!("\"k{index}\":null"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        parse(format!("{{{fields}}}").as_bytes())
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        MAX_ACTION_NODES - 1
    );
    assert_eq!(
        parse(format!("{{{fields},\"extra\":null}}").as_bytes()),
        Err(CodecError::OutputLimit)
    );
}

#[test]
fn whole_document_utf8_truncation_and_nonfinite_numbers_are_strict() {
    for bytes in [
        b"{}{}".as_slice(),
        b"{} trailing-private-text",
        b"true false",
        b"[1,]",
        b"{\"a\":1,}",
        b"{\"a\":",
        b"[1",
        b"\"truncated",
        b"\"\xff\"",
        br#""\uD800""#,
        br#""\uDC00""#,
        b"NaN",
        b"Infinity",
        b"1e400",
        b"-1e400",
        b"01",
        b"+1",
        b".5",
        b"1.",
        b"true\0",
        b"\xef\xbb\xbf{}",
    ] {
        assert_eq!(parse(bytes), Err(CodecError::InvalidResponse));
    }
    assert_eq!(parse(b"{} \t\n\r"), Ok(json!({})));
}

#[test]
fn errors_never_retain_untrusted_keys_or_response_text() {
    let marker = "synthetic-private-marker";
    for bytes in [
        format!("{{\"{marker}\":1,\"{marker}\":2}}"),
        format!("{{\"{marker}\":"),
    ] {
        assert_eq!(
            format!("{:?}", parse(bytes.as_bytes()).unwrap_err()),
            "InvalidResponse"
        );
    }
}
