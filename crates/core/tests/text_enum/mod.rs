use super::{definition, project, rejects_definition, scalar_array, scalar_record};
use openwrt_mcp_core::CoreError;
use serde_json::json;

#[test]
fn text_enums_validate_finite_exact_utf8_definitions_before_preparation() {
    for (max, values) in [
        (0, json!(["x"])),
        (1025, json!(["x"])),
        (8, json!([])),
        (8, json!(["x", "x"])),
        (8, json!([""])),
        (8, json!(["x\u{0}"])),
        (1, json!(["é"])),
        (
            8,
            json!((0..17).map(|i| format!("n{i}")).collect::<Vec<_>>()),
        ),
    ] {
        rejects_definition(definition(
            scalar_record(
                json!({"kind":"text_enum","max_bytes":max,"values":values}),
                true,
            ),
            false,
        ));
    }
    for value in [
        json!({"kind":"text_enum","max_bytes":8}),
        json!({"kind":"text_enum","values":["x"]}),
        json!({"kind":"text_enum","max_bytes":8,"values":["x"],"coerce":true}),
    ] {
        rejects_definition(definition(scalar_record(value, true), false));
    }
    let values: Vec<_> = (0..16).map(|i| format!("n{i}")).collect();
    assert!(
        project(
            scalar_record(
                json!({"kind":"text_enum","max_bytes":8,"values":values}),
                true
            ),
            json!({"value":"n15"})
        )
        .is_ok()
    );
    for text in ["é".repeat(512), "\u{1}".repeat(1024)] {
        assert_eq!(
            project(
                scalar_record(
                    json!({"kind":"text_enum","max_bytes":1024,"values":[text.clone()]}),
                    true
                ),
                json!({"value":text.clone()})
            )
            .unwrap(),
            json!({"value":text})
        );
    }
}

#[test]
fn text_enums_never_trim_coerce_or_normalize_and_keep_existing_byte_budgets() {
    let kind = json!({"kind":"text_enum","max_bytes":16,"values":["WiFi","0","é"]});
    for value in [json!("WiFi"), json!("0"), json!("é")] {
        assert_eq!(
            project(
                scalar_record(kind.clone(), true),
                json!({"value":value.clone()})
            )
            .unwrap(),
            json!({"value":value})
        );
    }
    for value in [
        json!("wifi"),
        json!(" WiFi"),
        json!("WiFi "),
        json!("e\u{301}"),
        json!(""),
        json!(0),
        json!(false),
        json!(null),
        json!(["WiFi"]),
        json!({"value":"WiFi"}),
    ] {
        assert_eq!(
            project(scalar_record(kind.clone(), true), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
    assert_eq!(
        project(scalar_record(kind.clone(), false), json!({})).unwrap(),
        json!({})
    );
    assert_eq!(
        project(
            scalar_array(kind, true, 2),
            json!({"values":["WiFi","WiFi"]})
        ),
        Err(CoreError::InvalidOutput)
    );
    let text = "\u{1}".repeat(1024);
    assert_eq!(
        project(
            scalar_array(
                json!({"kind":"text_enum","max_bytes":1024,"values":[text.clone()]}),
                false,
                128
            ),
            json!({"values":vec![text;128]})
        ),
        Err(CoreError::OutputLimit)
    );
}
