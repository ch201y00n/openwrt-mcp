//! Pure synthetic v6 acceptance. No device, keys or host exclusions.
mod text_enum;
mod text_option;
use openwrt_mcp_core::{
    Catalog, CoreError, MAX_NORMALIZED_BYTES, Operation, OutputMode, PreparedAction,
    SAFE_INTEGER_MAX, TypedProjection, check_normalized_result,
};
use serde_json::{Value, json};

fn field(name: &str, source: &str, value: Value, required: bool) -> Value {
    json!({"name":name,"source":source,"value":value,
        "presence":if required {"required"} else {"optional"}})
}
fn text() -> Value {
    json!({"kind":"text","max_bytes":64})
}
fn boolean() -> Value {
    json!({"kind":"boolean"})
}
fn integer(min: i64, max: i64) -> Value {
    json!({"kind":"safe_integer","min":min,"max":max})
}
fn record(fields: Vec<Value>) -> Value {
    json!({"fields":fields,"collections":[]})
}
fn all() -> Value {
    json!({"kind":"all"})
}
fn exact() -> Value {
    json!({"kind":"exact_one","parameter":"selected"})
}
fn array_projection(selection: Value, max: usize) -> Value {
    json!({"kind":"collection","selection":selection,"collection":{
        "kind":"object_array","source":"/rows","max_items":max,"identity":"name",
        "record":record(vec![field("name","/name",text(),true),field("up","/up",boolean(),true)])
    }})
}
fn definition(projection: Value, selector: bool) -> Value {
    json!({"name":"synthetic_read","description":"Synthetic reviewed read",
        "requirements":[{"category":"network","permission":"read"}],
        "parameters":if selector {json!({"selected":{"kind":"string","required":true}})} else {json!({})},
        "action":{"kind":"ubus","object":"network.interface","method":"dump","arguments":{}},
        "capability":{"kind":"ubus_method","object":"network.interface","method":"dump",
            "arguments":{},"response_contract":"synthetic_read.v1"},
        "output_fields":[],"output_mode":{"typed":projection}})
}
fn operation(projection: Value, selector: bool) -> Operation {
    let operation: Operation = serde_json::from_value(definition(projection, selector)).unwrap();
    Catalog::with_builtins(vec![operation.clone()], vec![]).unwrap();
    operation
}
fn project(projection: Value, source: Value) -> Result<Value, CoreError> {
    operation(projection, false)
        .prepare_invocation(&json!({}))
        .unwrap()
        .project(&source)
}
fn rows(count: usize) -> Value {
    json!({"rows":(0..count).map(|index| json!({"name":format!("n{index}"),"up":true})).collect::<Vec<_>>()})
}
fn scalar_array(kind: Value, unique: bool, max: usize) -> Value {
    json!({"kind":"collection","selection":all(),"collection":{
        "kind":"scalar_array","source":"/values","name":"value","value":kind,
        "unique":unique,"max_items":max}})
}
fn scalar_record(kind: Value, required: bool) -> Value {
    json!({"kind":"record","record":record(vec![field("value","/value",kind,required)])})
}
fn nested_entries() -> Value {
    json!({"kind":"collection","selection":all(),"collection":{
    "kind":"object_entries","source":"/services","max_items":128,
    "key":{"name":"name","max_bytes":64},"record":{
        "fields":[],"collections":[{"name":"instances","presence":"optional","collection":{
            "kind":"object_entries","source":"/instances","max_items":128,
            "key":{"name":"name","max_bytes":64},"record":{
                "fields":[field("running","/running",boolean(),true)]}
        }}]}}})
}
fn rejects_definition(value: Value) {
    if let Ok(operation) = serde_json::from_value::<Operation>(value) {
        assert!(Catalog::with_builtins(vec![operation], vec![]).is_err());
    }
}

#[test]
fn object_array_returns_only_fixed_fields_inside_items_envelope() {
    assert_eq!(
        project(
            array_projection(all(), 256),
            json!({"rows":[
        {"name":"lan","up":true,"password":"DO_NOT_RETURN","configuration":{"psk":"hidden"}}
    ],"secret":"hidden"})
        )
        .unwrap(),
        json!({"items":[{"name":"lan","up":true}]})
    );
}

fn observation_rows(max: usize) -> Value {
    json!({"kind":"collection","selection":all(),"collection":{
        "kind":"row_array","source":"/rows","max_items":max,
        "record":record(vec![field("expires","/expires",json!({"kind":"false_or_safe_integer","min":0,"max":10}),true)])
    }})
}

#[test]
fn finite_false_integer_union_preserves_sentinel_and_checks_every_numeric_boundary() {
    let kind =
        json!({"kind":"false_or_safe_integer","min":-SAFE_INTEGER_MAX,"max":SAFE_INTEGER_MAX});
    for value in [
        json!(false),
        json!(0),
        json!(-SAFE_INTEGER_MAX),
        json!(SAFE_INTEGER_MAX),
    ] {
        assert_eq!(
            project(scalar_record(kind.clone(), true), json!({"value":value})).unwrap(),
            json!({"value":value})
        );
    }
    for value in [
        json!(true),
        json!(null),
        json!("0"),
        json!("false"),
        json!(1.0),
        json!(0.0),
        json!(SAFE_INTEGER_MAX + 1),
        json!(-SAFE_INTEGER_MAX - 1),
        json!([]),
        json!({"secret":"private"}),
    ] {
        assert_eq!(
            project(scalar_record(kind.clone(), true), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
    for (min, max) in [
        (1, 0),
        (-SAFE_INTEGER_MAX - 1, 10),
        (0, SAFE_INTEGER_MAX + 1),
    ] {
        rejects_definition(definition(
            scalar_record(
                json!({"kind":"false_or_safe_integer","min":min,"max":max}),
                true,
            ),
            false,
        ));
    }
}

#[test]
fn observation_rows_preserve_order_and_duplicates_without_invented_identity_or_selection() {
    let rows =
        json!([{"expires":10,"secret":"private"},{"expires":false},{"expires":10},{"expires":0}]);
    assert_eq!(
        project(observation_rows(4), json!({"rows":rows})).unwrap(),
        json!({"items":[{"expires":10},{"expires":false},{"expires":10},{"expires":0}]})
    );
    assert_eq!(
        project(observation_rows(4), json!({"rows":[]})).unwrap(),
        json!({"items":[]})
    );
    let mut selected = observation_rows(4);
    selected["selection"] = exact();
    rejects_definition(definition(selected, true));
    for bad in [
        json!({}),
        json!({"rows":null}),
        json!({"rows":{}}),
        json!({"rows":[{"expires":false},null]}),
        json!({"rows":[{"expires":true}]}),
    ] {
        assert_eq!(
            project(observation_rows(4), bad),
            Err(CoreError::InvalidOutput)
        );
    }
    for count in [256, 257] {
        let result = project(
            observation_rows(256),
            json!({"rows":vec![json!({"expires":false});count]}),
        );
        if count == 256 {
            assert_eq!(result.unwrap()["items"].as_array().unwrap().len(), count);
        } else {
            assert_eq!(result, Err(CoreError::OutputLimit));
        }
    }
}

#[test]
fn root_absence_guards_reject_any_present_value_before_projection_without_exporting_it() {
    for mut projection in [scalar_record(text(), true), observation_rows(4)] {
        projection["reject_if_present"] =
            json!(["/error", "/status/failure", "/escaped~1name/~0problem"]);
        let valid =
            json!({"value":"okay","rows":[{"expires":false}],"status":{},"escaped/name":{}});
        assert!(project(projection.clone(), valid.clone()).is_ok());
        for error in [
            json!(null),
            json!(false),
            json!(0),
            json!(""),
            json!({}),
            json!([]),
            json!("private-upstream-error"),
        ] {
            let mut source = valid.clone();
            source["error"] = error;
            assert_eq!(
                project(projection.clone(), source),
                Err(CoreError::InvalidOutput)
            );
        }
        for source in [
            json!({"value":"okay","rows":[],"status":false}),
            json!({"value":"okay","rows":[],"status":{"failure":null}}),
            json!({"value":"okay","rows":[],"escaped/name":{"~problem":false}}),
        ] {
            assert_eq!(
                project(projection.clone(), source),
                Err(CoreError::InvalidOutput)
            );
        }
        let mut source = valid;
        source["error"] = json!("private");
        source["rows"] = json!(vec![json!({"expires":0}); 257]);
        assert_eq!(project(projection, source), Err(CoreError::InvalidOutput));
    }
}

#[test]
fn root_guards_have_strict_decoded_nonoverlap_path_and_count_bounds() {
    for guards in [
        json!([""]),
        json!(["error"]),
        json!(["/bad~2"]),
        json!(["/bad\u{0}"]),
        json!(["/e", "/e"]),
        json!(["/e", "/e/child"]),
        json!(["/a~1b", "/a~1b/c"]),
        json!(["/0", "/1", "/2", "/3", "/4"]),
        json!([format!("/{}", "x".repeat(512))]),
        json!(["/a/b/c/d/e/f/g/h/i"]),
    ] {
        let mut shape = observation_rows(4);
        shape["reject_if_present"] = guards;
        rejects_definition(definition(shape, false));
    }
    let mut shape = observation_rows(4);
    shape["reject_if_present"] = json!(["/a", "/b", "/c", format!("/{}", "x".repeat(511))]);
    assert!(project(shape, json!({"rows":[]})).is_ok());
    let mut shape = observation_rows(4);
    shape["reject_if_present"] = json!(["/a/b/c/d/e/f/g/h", "/a~1b", "/a~01b"]);
    assert!(project(shape, json!({"rows":[]})).is_ok());
}

#[test]
fn nested_observation_rows_charge_unselected_rows_and_cannot_create_a_third_level() {
    let mut shape = nested_entries();
    shape["collection"]["record"]["collections"][0]["collection"] = json!({"kind":"row_array","source":"/instances","max_items":256,"record":{"fields":[field("running","/running",boolean(),true)]}});
    let two = vec![json!({"running":true}), json!({"running":true})];
    assert_eq!(
        project(shape.clone(), json!({"services":{"a":{"instances":two}}})).unwrap(),
        json!({"items":[{"name":"a","instances":[{"running":true},{"running":true}]}]})
    );
    for count in [255, 256] {
        let result = project(
            shape.clone(),
            json!({"services":{"a":{"instances":vec![json!({"running":true});count]}}}),
        );
        assert_eq!(result.is_ok(), count == 255);
    }
    shape["selection"] = exact();
    let op = operation(shape.clone(), true);
    assert_eq!(
        op.prepare_invocation(&json!({"selected":"a"}))
            .unwrap()
            .project(
                &json!({"services":{"a":{"instances":[]},"z":{"instances":[{"running":false},{}]}}})
            ),
        Err(CoreError::InvalidOutput)
    );
    shape["collection"]["record"]["collections"][0]["collection"]["record"]["collections"] =
        json!([]);
    rejects_definition(definition(shape, true));
}

#[test]
fn exact_selection_is_bound_and_excluded_from_transmitted_arguments() {
    let operation = operation(array_projection(exact(), 256), true);
    let invocation = operation
        .prepare_invocation(&json!({"selected":"wan"}))
        .unwrap();
    assert!(
        matches!(invocation.action(), PreparedAction::Ubus { arguments, .. } if arguments == &json!({}))
    );
    assert_eq!(
        invocation
            .project(&json!({"rows":[{"name":"lan","up":true},{"name":"wan","up":false}]}))
            .unwrap(),
        json!({"name":"wan","up":false})
    );
    assert_eq!(
        operation.project(&rows(1)),
        json!({}),
        "no unbound legacy typed path"
    );
}

#[test]
fn selector_required_type_nonempty_unknown_and_byte_bounds_are_preparation_errors() {
    let operation = operation(array_projection(exact(), 256), true);
    for input in [
        json!({}),
        json!({"selected":""}),
        json!({"selected":null}),
        json!({"selected":1}),
        json!({"selected":"x\0y"}),
        json!({"selected":"é".repeat(33)}),
        json!({"selected":"lan","other":true}),
    ] {
        assert!(operation.prepare_invocation(&input).is_err());
    }
    operation
        .prepare_invocation(&json!({"selected":"é".repeat(32)}))
        .unwrap();
    assert_eq!(
        operation.input_schema()["properties"]["selected"]["minLength"],
        1
    );
    assert_eq!(
        operation.input_schema()["properties"]["selected"]["maxLength"],
        64
    );
}

#[test]
fn empty_no_match_and_malformed_are_distinct() {
    assert_eq!(
        project(array_projection(all(), 256), json!({"rows":[]})).unwrap(),
        json!({"items":[]})
    );
    let selected = operation(array_projection(exact(), 256), true);
    assert_eq!(
        selected
            .prepare_invocation(&json!({"selected":"missing"}))
            .unwrap()
            .project(&json!({"rows":[]})),
        Err(CoreError::SelectionNotObserved)
    );
    for value in [
        Value::Null,
        json!({}),
        json!({"rows":null}),
        json!({"rows":{}}),
        json!({"rows":[false]}),
    ] {
        assert_eq!(
            project(array_projection(all(), 256), value),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn duplicate_identities_anywhere_are_not_first_match() {
    let operation = operation(array_projection(exact(), 256), true);
    let prepared = operation
        .prepare_invocation(&json!({"selected":"wanted"}))
        .unwrap();
    for source in [
        json!({"rows":[{"name":"wanted","up":true},{"name":"wanted","up":false}]}),
        json!({"rows":[{"name":"wanted","up":true},{"name":"other","up":true},{"name":"other","up":true}]}),
    ] {
        assert_eq!(prepared.project(&source), Err(CoreError::InvalidOutput));
    }
}

#[test]
fn unselected_rows_have_all_reviewed_fields_validated() {
    let operation = operation(array_projection(exact(), 256), true);
    let invocation = operation
        .prepare_invocation(&json!({"selected":"wanted"}))
        .unwrap();
    for other in [
        json!({"name":"other"}),
        json!({"name":"other","up":"true"}),
        json!({"name":"","up":true}),
        json!({"name":"x".repeat(65),"up":true}),
    ] {
        assert_eq!(
            invocation.project(&json!({"rows":[{"name":"wanted","up":true},other]})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn optional_fields_omit_missing_but_reject_null_and_wrong_types() {
    let schema = scalar_record(boolean(), false);
    assert_eq!(project(schema.clone(), json!({})).unwrap(), json!({}));
    for value in [Value::Null, json!(1), json!("true"), json!([]), json!({})] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
    assert_eq!(
        project(scalar_record(boolean(), true), json!({})),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn optional_pointer_rejects_existing_noncontainer_parent() {
    let mut schema = scalar_record(boolean(), false);
    schema["record"]["fields"][0]["source"] = json!("/parent/value");
    assert_eq!(project(schema.clone(), json!({})).unwrap(), json!({}));
    assert_eq!(
        project(schema.clone(), json!({"parent":{}})).unwrap(),
        json!({})
    );
    assert_eq!(
        project(schema, json!({"parent":null})),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn entries_emit_explicit_identity_and_reviewed_nested_fields_only() {
    assert_eq!(project(nested_entries(),json!({"services":{
        "daemon":{"instances":{"main":{"running":true,"command":["hidden"],"env":{"PASSWORD":"hidden"}}}},"empty":{}
    }})).unwrap(),json!({"items":[{"name":"daemon","instances":[{"name":"main","running":true}]},{"name":"empty"}]}));
}

#[test]
fn nested_null_empty_identity_and_wrong_record_are_malformed() {
    for source in [
        json!({"services":{"daemon":{"instances":null}}}),
        json!({"services":{"":{}}}),
        json!({"services":{"daemon":{"instances":{"":{"running":true}}}}}),
        json!({"services":{"daemon":{"instances":{"main":false}}}}),
    ] {
        assert_eq!(
            project(nested_entries(), source),
            Err(CoreError::InvalidOutput)
        );
    }
    let mut projection = nested_entries();
    projection["selection"] = exact();
    assert_eq!(operation(projection,true).prepare_invocation(&json!({"selected":"first"})).unwrap()
        .project(&json!({"services":{"first":{},"later":{"instances":{"main":{"running":"invalid"}}}}})),Err(CoreError::InvalidOutput));
}

#[test]
fn aggregate_nested_scan_is_charged_even_on_unselected_rows() {
    let instances = |count| {
        (0..count)
            .map(|i| (format!("i{i}"), json!({"running":true})))
            .collect::<serde_json::Map<_, _>>()
    };
    let source = |second| json!({"services":{"a":{"instances":instances(127)},"b":{"instances":instances(second)}}});
    assert!(project(nested_entries(), source(127)).is_ok());
    assert_eq!(
        project(nested_entries(), source(128)),
        Err(CoreError::OutputLimit)
    );
    let mut selected = nested_entries();
    selected["selection"] = exact();
    assert_eq!(
        operation(selected, true)
            .prepare_invocation(&json!({"selected":"a"}))
            .unwrap()
            .project(&source(128)),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn source_count_is_checked_before_selection() {
    let operation = operation(array_projection(exact(), 256), true);
    let invocation = operation
        .prepare_invocation(&json!({"selected":"n0"}))
        .unwrap();
    assert!(invocation.project(&rows(256)).is_ok());
    assert_eq!(invocation.project(&rows(257)), Err(CoreError::OutputLimit));
    assert_eq!(
        project(array_projection(all(), 2), rows(3)),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn scalar_array_rows_and_unique_text_identity_rules() {
    assert_eq!(
        project(
            scalar_array(text(), true, 256),
            json!({"values":["wlan0","wlan1"]})
        )
        .unwrap(),
        json!({"items":[{"value":"wlan0"},{"value":"wlan1"}]})
    );
    for values in [json!(["wlan0", "wlan0"]), json!([""]), json!(["x", null])] {
        assert_eq!(
            project(scalar_array(text(), true, 256), json!({"values":values})),
            Err(CoreError::InvalidOutput)
        );
    }
    assert!(project(scalar_array(text(), false, 256), json!({"values":["",""]})).is_ok());
    assert_eq!(
        project(
            scalar_array(boolean(), true, 256),
            json!({"values":[true,true]})
        ),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn safe_integer_exact_bounds_without_coercion_or_rounding() {
    let schema = scalar_record(integer(-SAFE_INTEGER_MAX, SAFE_INTEGER_MAX), true);
    for value in [json!(-SAFE_INTEGER_MAX), json!(SAFE_INTEGER_MAX), json!(0)] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})).unwrap(),
            json!({"value":value})
        );
    }
    for value in [
        json!(SAFE_INTEGER_MAX + 1),
        json!(-SAFE_INTEGER_MAX - 1),
        json!(1.0),
        json!(1e3),
        json!("123"),
        json!(u64::MAX),
    ] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
    assert_eq!(
        project(scalar_record(integer(1, 10), true), json!({"value":0})),
        Err(CoreError::InvalidOutput)
    );
}

#[test]
fn unsigned_json_counter_always_emits_decimal_string() {
    let schema = scalar_record(
        json!({"kind":"decimal_counter","source":"json_integer"}),
        true,
    );
    for value in [0, 1, SAFE_INTEGER_MAX as u64 + 1, u64::MAX] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})).unwrap(),
            json!({"value":value.to_string()})
        );
    }
    for value in [json!(-1), json!(1.0), json!("12"), json!(null)] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn decimal_text_counter_rejects_noncanonical_and_overflow() {
    let schema = scalar_record(
        json!({"kind":"decimal_counter","source":"decimal_text"}),
        true,
    );
    assert_eq!(
        project(schema.clone(), json!({"value":u64::MAX.to_string()})).unwrap(),
        json!({"value":u64::MAX.to_string()})
    );
    for value in [
        "",
        "01",
        "+1",
        "-0",
        "1.0",
        "1e3",
        " 1",
        "1\n",
        "１２",
        "18446744073709551616",
        "123456789012345678901",
    ] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn text_uses_utf8_byte_bounds_and_nul_rejection() {
    let schema = scalar_record(json!({"kind":"text","max_bytes":4}), true);
    for value in ["", "éé", "😀"] {
        assert!(project(schema.clone(), json!({"value":value})).is_ok());
    }
    for value in ["ééa", "a\0", "😀a"] {
        assert_eq!(
            project(schema.clone(), json!({"value":value})),
            Err(CoreError::InvalidOutput)
        );
    }
}

#[test]
fn escaped_pointer_segments_remain_distinct() {
    let schema = json!({"kind":"record","record":record(vec![field("slash","/a~1b",text(),true),
        field("nested","/a/b",text(),true),field("tilde","/x~0y",text(),true)])});
    assert_eq!(
        project(schema, json!({"a/b":"one","a":{"b":"two"},"x~y":"three"})).unwrap(),
        json!({"slash":"one","nested":"two","tilde":"three"})
    );
}

#[test]
fn legacy_metadata_cannot_mix_and_structured_is_extension_only() {
    let mut value = definition(array_projection(all(), 256), false);
    value["output_fields"] = json!(["/raw"]);
    rejects_definition(value);
    let mut operation = operation(array_projection(all(), 256), false);
    operation.output_mode = OutputMode::Structured;
    operation.output_fields = vec!["/raw".into()];
    assert!(Catalog::with_builtins(vec![operation.clone()], vec![]).is_err());
    assert!(Catalog::new(vec![operation]).is_ok());
}

#[test]
fn schema_alias_path_escape_and_map_key_collisions_fail_closed() {
    for fields in [
        vec![
            field("same", "/a", text(), true),
            field("same", "/b", text(), true),
        ],
        vec![
            field("a", "/a", text(), true),
            field("b", "/a/b", text(), true),
        ],
        vec![field("a", "/a~2b", text(), true)],
        vec![field("a", "", text(), true)],
        vec![field("a", "/a/b/c/d/e/f/g/h/i", text(), true)],
        vec![field(
            "a",
            "/a",
            json!({"kind":"text","max_bytes":1025}),
            true,
        )],
    ] {
        rejects_definition(definition(
            json!({"kind":"record","record":record(fields)}),
            false,
        ));
    }
    let mut value = definition(nested_entries(), false);
    value["output_mode"]["typed"]["collection"]["record"]["fields"] =
        json!([field("name", "/other", text(), true)]);
    rejects_definition(value);
}

#[test]
fn array_identity_must_reference_existing_required_text_field() {
    for replacement in [json!("missing"), json!("up")] {
        let mut schema = array_projection(all(), 256);
        schema["collection"]["identity"] = replacement;
        rejects_definition(definition(schema, false));
    }
    let mut schema = array_projection(all(), 256);
    schema["collection"]["record"]["fields"][0]["presence"] = json!("optional");
    rejects_definition(definition(schema, false));
    let mut schema = scalar_array(text(), true, 256);
    schema["selection"] = exact();
    rejects_definition(definition(schema, true));
}

#[test]
fn selector_metadata_and_allowed_values_are_validated() {
    for parameter in [
        json!({"kind":"string","required":false}),
        json!({"kind":"integer","required":true}),
        json!({"kind":"string","required":true,"allowed_values":[""]}),
        json!({"kind":"string","required":true,"allowed_values":["é".repeat(33)]}),
    ] {
        let mut value = definition(array_projection(exact(), 256), true);
        value["parameters"]["selected"] = parameter;
        rejects_definition(value);
    }
    rejects_definition(definition(array_projection(all(), 256), true));
    let mut value = definition(array_projection(exact(), 256), true);
    value["output_mode"]["typed"]["selection"]["parameter"] = json!("other");
    rejects_definition(value);
}

#[test]
fn strict_metadata_rejects_unknown_fields_and_third_collection_level() {
    let mut value = definition(array_projection(all(), 256), false);
    value["output_mode"]["typed"]["selection"]["unexpected"] = json!(true);
    rejects_definition(value);
    let mut value = definition(array_projection(all(), 256), false);
    value["output_mode"]["typed"]["collection"]["record"]["fields"][1]["value"]["unexpected"] =
        json!(true);
    rejects_definition(value);
    let mut value = definition(nested_entries(), false);
    value["output_mode"]["typed"]["collection"]["record"]["collections"][0]["collection"]["selection"] =
        all();
    rejects_definition(value);
    let mut value = definition(nested_entries(), false);
    value["output_mode"]["typed"]["collection"]["record"]["collections"][0]["collection"]["record"]
        ["collections"] = json!([]);
    rejects_definition(value);
    assert!(serde_json::from_value::<TypedProjection>(json!({"kind":"arbitrary_node"})).is_err());
}

#[test]
fn schema_nodes_fields_and_scalar_hard_bounds_are_enforced() {
    for max in [0, 257] {
        rejects_definition(definition(array_projection(all(), max), false));
    }
    let mut value = definition(scalar_record(text(), true), false);
    value["output_mode"]["typed"]["record"]["fields"] = json!(
        (0..65)
            .map(|i| field(&format!("f{i}"), &format!("/f{i}"), boolean(), true))
            .collect::<Vec<_>>()
    );
    rejects_definition(value);
    let make = |count| {
        json!({"kind":"record","record":{"fields":[],"collections":(0..count).map(|i|json!({
        "name":format!("c{i}"),"presence":"required","collection":{
            "kind":"scalar_array","source":format!("/c{i}"),"max_items":1,"name":"value","value":boolean(),"unique":false
        }})).collect::<Vec<_>>()}})
    };
    operation(make(8), false);
    rejects_definition(definition(make(9), false));
    for kind in [
        integer(1, 0),
        integer(-SAFE_INTEGER_MAX - 1, 0),
        integer(0, SAFE_INTEGER_MAX + 1),
        json!({"kind":"text","max_bytes":0}),
    ] {
        rejects_definition(definition(scalar_record(kind, true), false));
    }
}

#[test]
fn normalized_size_matches_serde_json_exact_boundary_for_escaping_and_utf8() {
    let source = json!({"a":"x".repeat(MAX_NORMALIZED_BYTES-8)});
    assert_eq!(
        serde_json::to_vec(&source).unwrap().len(),
        MAX_NORMALIZED_BYTES
    );
    assert_eq!(check_normalized_result(&source), Ok(()));
    assert_eq!(
        check_normalized_result(&json!({"a":"x".repeat(MAX_NORMALIZED_BYTES-7)})),
        Err(CoreError::OutputLimit)
    );
    for text in ["é", "😀", "\"", "\\", "\n", "\u{0001}"] {
        let mut value = json!({"a":text.repeat(1000)});
        let used = serde_json::to_vec(&value).unwrap().len();
        value["z"] = json!("x".repeat(MAX_NORMALIZED_BYTES - used - 7));
        assert_eq!(
            serde_json::to_vec(&value).unwrap().len(),
            MAX_NORMALIZED_BYTES
        );
        assert_eq!(check_normalized_result(&value), Ok(()));
        value["z"] = json!(format!("{}x", value["z"].as_str().unwrap()));
        assert_eq!(check_normalized_result(&value), Err(CoreError::OutputLimit));
    }
}

#[test]
fn typed_serialized_budget_counts_escape_amplification() {
    let schema = scalar_array(json!({"kind":"text","max_bytes":1024}), false, 256);
    assert!(project(schema.clone(), json!({"values":vec!["x".repeat(1024);60]})).is_ok());
    assert_eq!(
        project(schema.clone(), json!({"values":vec!["x".repeat(1024);64]})),
        Err(CoreError::OutputLimit)
    );
    assert_eq!(
        project(schema, json!({"values":vec!["\u{0001}".repeat(1024);11]})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn prepared_legacy_redaction_is_bounded_before_clone() {
    let mut operation = operation(array_projection(all(), 256), false);
    operation.output_mode = OutputMode::Structured;
    operation.output_fields = vec!["/data".into()];
    let invocation = operation.prepare_invocation(&json!({})).unwrap();
    assert_eq!(invocation.project(&json!({"data":{"public":[true,5,"ok"],"private-key":"hidden","env":{"PASSWORD":"hidden"}}})).unwrap(),
        json!({"/data":{"public":[true,5,"ok"],"env":{}}}));
    assert_eq!(
        invocation.project(&json!({"data":{"large":"x".repeat(1_000_000)}})),
        Err(CoreError::OutputLimit)
    );
    assert_eq!(
        invocation.project(&json!({"data":{"large":"\u{0001}".repeat(11_000)}})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn legacy_scalar_guards_and_number_semantics_remain_intact() {
    let mut operation = operation(array_projection(all(), 256), false);
    operation.output_mode = OutputMode::Scalars;
    operation.output_fields = vec!["/data".into()];
    let invocation = operation.prepare_invocation(&json!({})).unwrap();
    assert_eq!(
        invocation
            .project(&json!({"data":{"secret":"hidden"}}))
            .unwrap(),
        json!({})
    );
    assert_eq!(
        invocation.project(&json!({"data":u64::MAX})).unwrap(),
        json!({"/data":u64::MAX})
    );
    assert_eq!(
        invocation.project(&json!({"data":"x".repeat(MAX_NORMALIZED_BYTES)})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn defensive_depth_limit_stops_legacy_recursive_work() {
    let mut source = json!(true);
    for _ in 0..40 {
        source = json!([source]);
    }
    assert_eq!(
        check_normalized_result(&source),
        Err(CoreError::OutputLimit)
    );
    let mut operation = operation(array_projection(all(), 256), false);
    operation.output_mode = OutputMode::Structured;
    operation.output_fields = vec!["/data".into()];
    assert_eq!(
        operation
            .prepare_invocation(&json!({}))
            .unwrap()
            .project(&json!({"data":source})),
        Err(CoreError::OutputLimit)
    );
}

#[test]
fn errors_are_fixed_nonsecret_codes() {
    for (error, code) in [
        (CoreError::InvalidOutput, "invalid_output"),
        (CoreError::OutputLimit, "output_limit"),
        (CoreError::SelectionNotObserved, "selection_not_observed"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
    }
}
