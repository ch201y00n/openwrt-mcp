//! Netifd-managed observations, not complete kernel or connectivity state.
use crate::definition::interface_observation;
use openwrt_mcp_core::{
    Category, Collection, CollectionField, LeafRecord, Operation, Presence, ScalarField, ScalarKind,
};

fn text(name: &str, max_bytes: usize, presence: Presence) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: format!("/{name}"),
        presence,
        value: ScalarKind::Text { max_bytes },
    }
}

fn integer(name: &str, source: &str, max: i64, presence: Presence) -> ScalarField {
    ScalarField {
        name: name.into(),
        source: format!("/{source}"),
        presence,
        value: ScalarKind::SafeInteger { min: 0, max },
    }
}

fn rows(
    name: &str,
    source: &str,
    fields: Vec<ScalarField>,
) -> CollectionField<Collection<LeafRecord>> {
    CollectionField {
        name: name.into(),
        presence: Presence::Optional,
        collection: Collection::RowArray {
            source: source.into(),
            max_items: 128,
            record: LeafRecord { fields },
        },
    }
}

pub(super) fn addresses() -> Operation {
    let mut collections = Vec::new();
    for (family, max_bytes, mask) in [("ipv4", 15, 32), ("ipv6", 45, 128)] {
        let fields = vec![
            text("address", max_bytes, Presence::Required),
            integer("mask", "mask", mask, Presence::Required),
            text("ptpaddress", max_bytes, Presence::Optional),
            integer(
                "preferred_seconds",
                "preferred",
                i64::from(u32::MAX),
                Presence::Optional,
            ),
            integer(
                "valid_seconds",
                "valid",
                i64::from(u32::MAX),
                Presence::Optional,
            ),
            text("class", 256, Presence::Optional),
        ];
        collections.push(rows(
            &format!("{family}_addresses"),
            &format!("/{family}-address"),
            fields.clone(),
        ));
        collections.push(rows(
            &format!("inactive_{family}_addresses"),
            &format!("/inactive/{family}-address"),
            fields,
        ));
    }
    interface_observation(
        "network_interface_addresses",
        "Read one interface's bounded netifd-reported active/inactive address rows. Discloses addresses and class; preserves duplicates and omitted lists. Not kernel completeness, delegated prefixes or connectivity proof; no mutation.",
        Category::Network,
        collections,
    )
}

pub(super) fn routes() -> Operation {
    let mut fields = vec![
        text("target", 45, Presence::Required),
        integer("mask", "mask", 128, Presence::Required),
        text("nexthop", 45, Presence::Required),
        text("source", 49, Presence::Required),
    ];
    fields.extend(
        ["type", "proto", "mtu", "metric", "table"]
            .into_iter()
            .map(|name| integer(name, name, i64::from(u32::MAX), Presence::Optional)),
    );
    fields.push(integer(
        "valid_seconds",
        "valid",
        i64::from(u32::MAX),
        Presence::Optional,
    ));
    interface_observation(
        "network_interface_routes",
        "Read one interface's bounded netifd-managed active/inactive routes, preserving duplicate rows and omitted lists. Discloses targets, next hops and source prefixes; not complete kernel FIB/policy rules or route-selection proof. No mutation.",
        Category::Network,
        vec![
            rows("routes", "/route", fields.clone()),
            rows("inactive_routes", "/inactive/route", fields),
        ],
    )
}

pub(super) fn neighbors() -> Operation {
    let fields = vec![
        text("address", 45, Presence::Required),
        text("mac", 17, Presence::Optional),
        integer("proxy", "proxy", i64::from(u32::MAX), Presence::Optional),
        integer("router", "router", i64::from(u32::MAX), Presence::Optional),
    ];
    interface_observation(
        "network_interface_neighbors",
        "Read one interface's bounded netifd-managed active/inactive neighbor entries. Discloses addresses/MACs; preserves duplicates and omission. Not kernel ARP/NDP cache, discovery or connected-client proof. No solicitation or mutation.",
        Category::Network,
        vec![
            rows("neighbors", "/neighbors", fields.clone()),
            rows("inactive_neighbors", "/inactive/neighbors", fields),
        ],
    )
}
