//! Non-atomic LuCI lease-file observations; not active-client or DNS discovery.
use crate::definition::{argument, read};
use openwrt_mcp_core::{
    Category, Collection, CollectionField, InnerRecord, Operation, OutputMode, ParameterKind,
    Presence, ScalarField, ScalarKind, Selection, TypedProjection,
};
use serde_json::json;

pub(crate) fn operations() -> Vec<Operation> {
    vec![leases(false), leases(true)]
}

fn text(name: &str, max_bytes: usize, presence: Presence) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: format!("/{name}"),
        presence,
        value: ScalarKind::Text { max_bytes },
    }
}

fn leases(ipv6: bool) -> Operation {
    let (name, source, address, max_address, family) = if ipv6 {
        ("dhcp_v6_leases", "/dhcp6_leases", "ip6addr", 45, 6)
    } else {
        ("dhcp_v4_leases", "/dhcp_leases", "ipaddr", 15, 4)
    };
    let mut operation = read(
        name,
        "Read bounded, non-atomic LuCI-visible lease-file rows, preserving duplicates/order. Discloses reported addresses, MAC/DUID/IAID, hostname and interface. expires_seconds is an integer or false (upstream no-expiry sentinel), not connected-client proof. Missing files/lines may be omitted upstream; no DNS lookup or mutation.",
        Category::DhcpDns,
        "luci-rpc",
        "getDHCPLeases",
        &format!("{name}.v1"),
        &[],
    );
    argument(
        &mut operation,
        "family",
        ParameterKind::Integer,
        json!(family),
    );
    let mut fields = vec![
        text(address, max_address, Presence::Required),
        ScalarField {
            name: "expires_seconds".into(),
            source: "/expires".into(),
            presence: Presence::Required,
            value: ScalarKind::FalseOrSafeInteger {
                min: 0,
                max: i64::from(u32::MAX),
            },
        },
    ];
    fields.extend(
        [
            ("interface", 256),
            ("hostname", 512),
            ("macaddr", 17),
            ("duid", 512),
            ("iaid", 64),
        ]
        .into_iter()
        .map(|(name, max)| text(name, max, Presence::Optional)),
    );
    let collections = if ipv6 {
        vec![CollectionField {
            name: "ip6addrs".into(),
            presence: Presence::Required,
            collection: Collection::ScalarArray {
                source: "/ip6addrs".into(),
                max_items: 10,
                name: "address_prefix".into(),
                value: ScalarKind::Text { max_bytes: 49 },
                unique: false,
            },
        }]
    } else {
        vec![]
    };
    operation.output_mode = OutputMode::Typed(Box::new(TypedProjection::Collection {
        reject_if_present: vec!["/error".into()],
        collection: Collection::RowArray {
            source: source.into(),
            max_items: 128,
            record: InnerRecord {
                fields,
                collections,
            },
        },
        selection: Selection::All {},
    }));
    operation
}
