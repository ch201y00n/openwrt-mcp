use super::{array_projection, definition, exact, operation, project, record, rejects_definition};
use openwrt_mcp_core::{CoreError, MAX_NORMALIZED_BYTES, check_normalized_result};
use serde_json::{Value, json};

fn option(max_items: usize, max_bytes: usize) -> Value {
    json!({"kind":"text_option","source":"/option","max_items":max_items,"max_bytes":max_bytes})
}
fn projection() -> Value {
    let mut body = record(vec![]);
    body["collections"] =
        json!([{"name":"option","presence":"optional","collection":option(128,1024)}]);
    json!({"kind":"record","record":body})
}
fn observe(value: Value) -> Result<Value, CoreError> {
    project(projection(), json!({"option":value}))
}
fn normalized(kind: &str, values: Value) -> Value {
    json!({"option":{"kind":kind,"values":values}})
}

#[test]
fn text_options_preserve_absence_empty_forms_original_text_order_and_duplicates() {
    assert_eq!(project(projection(), json!({})).unwrap(), json!({}));
    for text in [
        "",
        "one two\tthree\n",
        "  original  ",
        "é e\u{301} 한글",
        "a; $() ' / \\",
    ] {
        assert_eq!(
            observe(json!(text)).unwrap(),
            normalized("string", json!([text]))
        );
        assert_eq!(
            observe(json!([text])).unwrap(),
            normalized("list", json!([text]))
        );
    }
    for values in [json!([]), json!(["", "", "two", "one", "two"])] {
        assert_eq!(observe(values.clone()).unwrap(), normalized("list", values));
    }
    let mut required = projection();
    required["record"]["collections"][0]["presence"] = json!("required");
    assert_eq!(project(required, json!({})), Err(CoreError::InvalidOutput));
}

#[test]
fn text_option_definitions_reject_roots_selection_coercion_bad_bounds_and_overlap() {
    for selection in [json!({"kind":"all"}), exact()] {
        rejects_definition(definition(
            json!({"kind":"collection","selection":selection,"collection":option(128,1024)}),
            false,
        ));
    }
    for (field, value) in [
        ("max_items", json!(0)),
        ("max_items", json!(129)),
        ("max_bytes", json!(0)),
        ("max_bytes", json!(1025)),
        ("max_items", json!(1.5)),
        ("max_bytes", json!("8")),
        ("unique", json!(true)),
        ("record", json!({})),
        ("source", json!("/bad~escape")),
    ] {
        let mut bad = projection();
        bad["record"]["collections"][0]["collection"][field] = value;
        rejects_definition(definition(bad, false));
    }
    for field in ["source", "max_items", "max_bytes"] {
        let mut bad = projection();
        bad["record"]["collections"][0]["collection"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        rejects_definition(definition(bad, false));
    }
    let mut bad = projection();
    bad["record"]["fields"] = json!([{"name":"other","source":"/option/value","presence":"optional","value":{"kind":"text","max_bytes":8}}]);
    rejects_definition(definition(bad, false));
}

#[test]
fn text_options_reject_non_text_late_malformed_values_and_individual_utf8_or_item_overflow() {
    for bad in [
        json!(null),
        json!(false),
        json!(1),
        json!({}),
        json!(["ok", null]),
        json!(["ok", 1]),
        json!(["ok", []]),
        json!(["ok", {}]),
        json!("bad\u{0}"),
        json!(["ok", "bad\u{0}"]),
        json!("x".repeat(1025)),
        json!("é".repeat(513)),
    ] {
        assert_eq!(observe(bad), Err(CoreError::InvalidOutput));
    }
    assert!(observe(json!("é".repeat(512))).is_ok());
    assert_eq!(
        observe(json!(vec![""; 128])).unwrap(),
        normalized("list", json!(vec![""; 128]))
    );
    assert_eq!(observe(json!(vec![""; 129])), Err(CoreError::OutputLimit));
    let mut one = projection();
    one["record"]["collections"][0]["collection"]["max_items"] = json!(1);
    assert!(project(one.clone(), json!({"option":"one two three"})).is_ok());
    assert_eq!(
        project(one, json!({"option":["one","two"]})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn text_options_share_scanning_and_emission_with_rows_and_validate_unselected_sources() {
    let mut shape = array_projection(exact(), 128);
    shape["collection"]["record"]["collections"] =
        json!([{"name":"option","presence":"optional","collection":option(128,1024)}]);
    let op = operation(shape, true);
    let bound = op.prepare_invocation(&json!({"selected":"a"})).unwrap();
    let mut source = json!({"rows":[{"name":"a","up":true,"option":vec!["a";127]},{"name":"b","up":false,"option":vec!["b";127]}]});
    assert_eq!(
        bound.project(&source).unwrap(),
        json!({"name":"a","up":true,"option":{"kind":"list","values":vec!["a";127]}})
    );
    source["rows"][1]["option"] = json!(vec!["b"; 128]);
    assert_eq!(bound.project(&source), Err(CoreError::OutputLimit));
    source["rows"][1]["option"] = json!(["fine", false]);
    assert_eq!(bound.project(&source), Err(CoreError::InvalidOutput));
    // Scalar representation is one scanned value even for an unselected row.
    source["rows"][0]["option"] = json!(vec!["a"; 128]);
    source["rows"][1]["option"] = json!("b");
    assert!(bound.project(&source).is_ok());
}

#[test]
fn text_option_wrappers_and_escaped_values_obey_exact_shared_serialized_byte_limit() {
    let mut values = vec!["x".repeat(1024); 63];
    values.push(String::new());
    let remaining = MAX_NORMALIZED_BYTES
        - serde_json::to_vec(&normalized("list", json!(values)))
            .unwrap()
            .len();
    assert!((1..=1024).contains(&remaining));
    values[63] = "x".repeat(remaining);
    let expected = normalized("list", json!(values));
    assert_eq!(
        serde_json::to_vec(&expected).unwrap().len(),
        MAX_NORMALIZED_BYTES
    );
    let result = observe(json!(values)).unwrap();
    assert_eq!(result, expected);
    check_normalized_result(&result).unwrap();
    values[63].push('x');
    assert_eq!(observe(json!(values)), Err(CoreError::OutputLimit));
    assert_eq!(
        observe(json!(vec!["\u{1}".repeat(1024); 128])),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn text_options_are_terminal_nodes_inside_the_existing_finite_schema() {
    for count in [8, 9] {
        let mut shape = projection();
        shape["record"]["collections"] =
            json!((0..count).map(|i|{
            let mut collection=option(1,1);collection["source"]=json!(format!("/p{i}"));
            json!({"name":format!("p{i}"),"presence":"optional","collection":collection})
        }).collect::<Vec<_>>());
        if count == 8 {
            assert_eq!(project(shape, json!({})).unwrap(), json!({}));
        } else {
            rejects_definition(definition(shape, false));
        }
    }
    let mut bad = projection();
    bad["record"]["collections"][0]["collection"]["collection"] = option(1, 1);
    rejects_definition(definition(bad, false));
}
