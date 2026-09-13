//! Passive iwinfo views; never scan, disconnect or set regulatory state.
use crate::definition::{argument, read};
use openwrt_mcp_core::{
    Category, Collection, CounterSource, InnerRecord, Operation, OutputMode, Parameter,
    ParameterKind, Presence, ScalarField, ScalarKind, Selection, TypedProjection,
};
use serde_json::json;

pub(super) fn operations() -> Vec<Operation> {
    vec![stations(false), stations(true), countries()]
}

fn device_read(name: &str, description: &str, method: &str) -> Operation {
    let mut operation = read(
        name,
        description,
        Category::Wireless,
        "iwinfo",
        method,
        &format!("{name}.v1"),
        &[],
    );
    operation
        .parameters
        .insert("device".into(), string_parameter());
    argument(
        &mut operation,
        "device",
        ParameterKind::String,
        json!("{device}"),
    );
    operation
}

fn string_parameter() -> Parameter {
    Parameter {
        kind: ParameterKind::String,
        required: true,
        allowed_values: vec![],
    }
}

fn field(name: &str, source: &str, presence: Presence, value: ScalarKind) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: source.into(),
        presence,
        value,
    }
}

fn text(name: &str, max_bytes: usize) -> ScalarField {
    field(
        name,
        &format!("/{name}"),
        Presence::Required,
        ScalarKind::Text { max_bytes },
    )
}

fn collection(
    identity: &str,
    max_items: usize,
    fields: Vec<ScalarField>,
    selection: Selection,
) -> OutputMode {
    OutputMode::Typed(Box::new(TypedProjection::Collection {
        collection: Collection::ObjectArray {
            source: "/results".into(),
            max_items,
            identity: identity.into(),
            record: InnerRecord {
                fields,
                collections: vec![],
            },
        },
        selection,
    }))
}

fn stations(exact: bool) -> Operation {
    let (name, description) = if exact {
        (
            "wireless_station_status",
            "Read one exact case-sensitive station MAC from a bounded passive association observation; no match is not proof of disconnection.",
        )
    } else {
        (
            "wireless_stations",
            "Read bounded station MAC identities and link metrics without scanning; empty RPC results do not prove no clients or a healthy driver.",
        )
    };
    let mut operation = device_read(name, description, "assoclist");
    let selection = if exact {
        operation
            .parameters
            .insert("mac".into(), string_parameter());
        // This selector deliberately never becomes rpcd's optional MAC filter.
        Selection::ExactOne {
            parameter: "mac".into(),
        }
    } else {
        Selection::All {}
    };
    let signed_signal = ScalarKind::SafeInteger {
        min: -128,
        max: 127,
    };
    let mut fields = vec![
        text("mac", 17),
        field(
            "signal_dbm",
            "/signal",
            Presence::Required,
            signed_signal.clone(),
        ),
        field(
            "noise_dbm",
            "/noise",
            Presence::Required,
            signed_signal.clone(),
        ),
        field(
            "signal_average_dbm",
            "/signal_avg",
            Presence::Optional,
            signed_signal,
        ),
    ];
    for (name, source) in [
        ("inactive_ms", "/inactive"),
        ("connected_seconds", "/connected_time"),
        ("estimated_throughput_kbps", "/thr"),
        ("rx_rate_kbps", "/rx/rate"),
        ("rx_bandwidth_mhz", "/rx/mhz"),
        ("tx_rate_kbps", "/tx/rate"),
        ("tx_bandwidth_mhz", "/tx/mhz"),
    ] {
        fields.push(field(
            name,
            source,
            Presence::Optional,
            ScalarKind::SafeInteger {
                min: 0,
                max: i32::MAX.into(),
            },
        ));
    }
    for name in ["authorized", "authenticated", "wme", "mfp"] {
        fields.push(field(
            name,
            &format!("/{name}"),
            Presence::Optional,
            ScalarKind::Boolean {},
        ));
    }
    for (name, source) in [
        ("rx_bytes", "/rx/bytes"),
        ("tx_bytes", "/tx/bytes"),
        ("rx_dropped", "/rx/drop_misc"),
    ] {
        fields.push(field(
            name,
            source,
            Presence::Optional,
            ScalarKind::DecimalCounter {
                source: CounterSource::JsonInteger,
            },
        ));
    }
    operation.output_mode = collection("mac", 128, fields, selection);
    operation
}

fn countries() -> Operation {
    let mut operation = device_read(
        "wireless_countries",
        "Read bounded driver-reported country metadata, not regulatory permission; empty RPC results can also reflect driver failure. Does not set a country.",
        "countrylist",
    );
    operation.output_mode = collection(
        "iso3166",
        256,
        vec![
            text("iso3166", 2),
            text("code", 4),
            text("country", 256),
            field(
                "active",
                "/active",
                Presence::Optional,
                ScalarKind::Boolean {},
            ),
        ],
        Selection::All {},
    );
    operation
}
