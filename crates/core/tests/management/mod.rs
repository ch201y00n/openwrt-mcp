//! Synthetic supplied models only; no device topology or mutation acceptance.
mod boundaries;
mod reference;

use openwrt_mcp_core::management::{
    ChangeScope, EffectAssessment, EffectError, EffectGraph, EffectKnowledge, Influence, Node,
    ProtectedResources, ResourceId, ResourceKind, analyze_effects,
};

fn resource(kind: ResourceKind, label: &str) -> ResourceId {
    ResourceId::new(kind, label).unwrap()
}
fn ids(count: usize) -> Vec<ResourceId> {
    (0..count)
        .map(|i| resource(ResourceKind::NetworkDevice, &format!("fixture-{i:05}")))
        .collect()
}
fn graph_order(
    ids: &[ResourceId],
    edges: &[(usize, usize)],
    unknown: &[usize],
    reverse: bool,
) -> EffectGraph {
    let mut nodes: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            Node::new(
                id.clone(),
                if unknown.contains(&i) {
                    EffectKnowledge::Unknown
                } else {
                    EffectKnowledge::Complete
                },
            )
        })
        .collect();
    let mut edges: Vec<_> = edges
        .iter()
        .map(|(a, b)| Influence::new(&ids[*a], &ids[*b]))
        .collect();
    if reverse {
        nodes.reverse();
        edges.reverse();
    }
    EffectGraph::new(&nodes, &edges).unwrap()
}
fn graph(ids: &[ResourceId], edges: &[(usize, usize)], unknown: &[usize]) -> EffectGraph {
    graph_order(ids, edges, unknown, false)
}
fn check(
    before: &EffectGraph,
    after: &EffectGraph,
    ids: &[ResourceId],
    roots: &[usize],
    protected: &[usize],
) -> Result<EffectAssessment, EffectError> {
    let selected: Vec<_> = roots.iter().map(|i| ids[*i].clone()).collect();
    let guarded: Vec<_> = protected.iter().map(|i| ids[*i].clone()).collect();
    analyze_effects(
        before,
        after,
        &ProtectedResources::new(&guarded).unwrap(),
        &ChangeScope::local(&selected).unwrap(),
    )
}
fn clear(count: usize) -> Result<EffectAssessment, EffectError> {
    Ok(EffectAssessment::NoKnownProtectedImpact {
        affected_resources: count,
    })
}

#[test]
fn all_finite_resource_kinds_are_distinct_exact_labels_not_categories_or_aliases() {
    use ResourceKind::*;
    let kinds = [
        PhysicalPort,
        NetworkDevice,
        Bridge,
        Vlan,
        Interface,
        Routing,
        Firewall,
        DhcpDns,
        Multicast,
        Service,
        Storage,
        Vpn,
        System,
        Packages,
        Firmware,
    ];
    // Exhaustiveness makes extending the enum require a reviewed test change.
    fn ordinal(kind: ResourceKind) -> usize {
        match kind {
            PhysicalPort => 0,
            NetworkDevice => 1,
            Bridge => 2,
            Vlan => 3,
            Interface => 4,
            Routing => 5,
            Firewall => 6,
            DhcpDns => 7,
            Multicast => 8,
            Service => 9,
            Storage => 10,
            Vpn => 11,
            System => 12,
            Packages => 13,
            Firmware => 14,
        }
    }
    for (i, kind) in kinds.iter().enumerate() {
        assert_eq!(ordinal(*kind), i);
    }
    let ids: Vec<_> = kinds
        .into_iter()
        .map(|kind| resource(kind, "lan3"))
        .collect();
    let g = graph(&ids, &[], &[]);
    for i in 1..ids.len() {
        assert_eq!(check(&g, &g, &ids, &[i], &[0]), clear(1));
    }
    let lower = resource(PhysicalPort, "lan3");
    assert!(lower != resource(PhysicalPort, "LAN3"));
    assert!(resource(Bridge, "a/b") != resource(Bridge, "a//b"));
    assert!(ResourceId::new(Bridge, "AZaz09_-./:@").is_ok());
    assert!(ResourceId::new(Bridge, &"a".repeat(128)).is_ok());
    for label in [
        "", " lan3", "lan3 ", "lan 3", "\n", "x\0y", "x\\y", "🧪", "é", "x\t", "x\u{7f}",
    ] {
        assert_eq!(
            ResourceId::new(Bridge, label).err(),
            Some(EffectError::InvalidIdentity)
        );
    }
    assert_eq!(
        ResourceId::new(Bridge, &"a".repeat(129)).err(),
        Some(EffectError::InvalidIdentity)
    );
}

#[test]
fn graph_construction_rejects_duplicates_and_dangling_endpoints_without_raw_errors() {
    let ids = ids(3);
    let nodes = [
        Node::new(ids[0].clone(), EffectKnowledge::Complete),
        Node::new(ids[1].clone(), EffectKnowledge::Complete),
    ];
    assert!(EffectGraph::new(&[], &[]).is_ok());
    assert!(EffectGraph::new(&nodes, &[]).is_ok());
    assert_eq!(
        EffectGraph::new(&[nodes[0].clone(), nodes[0].clone()], &[]).err(),
        Some(EffectError::InvalidGraph)
    );
    for (from, to) in [(0, 2), (2, 0), (2, 2)] {
        assert_eq!(
            EffectGraph::new(&nodes, &[Influence::new(&ids[from], &ids[to])]).err(),
            Some(EffectError::InvalidGraph)
        );
    }
    let duplicates = [
        Influence::new(&ids[0], &ids[1]),
        Influence::new(&ids[0], &ids[1]),
    ];
    assert_eq!(
        EffectGraph::new(&nodes, &duplicates).err(),
        Some(EffectError::InvalidGraph)
    );
    assert!(EffectGraph::new(&nodes, &[Influence::new(&ids[0], &ids[0])]).is_ok());
}

#[test]
fn direct_and_indirect_protection_obey_influence_direction() {
    let ids = [
        resource(ResourceKind::Service, "network"),
        resource(ResourceKind::Bridge, "br-passthrough"),
        resource(ResourceKind::Vlan, "iptv:vid"),
        resource(ResourceKind::PhysicalPort, "lan3"),
    ];
    let forward = graph(&ids, &[(0, 1), (1, 2), (2, 3)], &[]);
    assert_eq!(
        check(&forward, &forward, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
    assert_eq!(
        check(&forward, &forward, &ids, &[3], &[3]),
        Err(EffectError::ProtectedImpact)
    );
    let reverse = graph(&ids, &[(3, 2), (2, 1), (1, 0)], &[]);
    assert_eq!(check(&reverse, &reverse, &ids, &[0], &[3]), clear(1));
}

#[test]
fn before_after_union_preserves_removed_added_and_cross_snapshot_paths() {
    let ids = ids(4);
    let empty = graph(&ids, &[], &[]);
    let direct = graph(&ids, &[(0, 3)], &[]);
    assert_eq!(
        check(&direct, &empty, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
    assert_eq!(
        check(&empty, &direct, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
    let before = graph(&ids, &[(0, 1)], &[]);
    let after = graph(&ids, &[(1, 3)], &[]);
    assert_eq!(check(&before, &before, &ids, &[0], &[3]), clear(2));
    assert_eq!(check(&after, &after, &ids, &[0], &[3]), clear(1));
    assert_eq!(
        check(&before, &after, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
    assert_eq!(
        check(&after, &before, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
}

#[test]
fn ordinary_additions_removals_are_retained_but_protected_identity_must_exist_both_sides() {
    let ids = ids(3);
    let before = graph(&[ids[0].clone(), ids[2].clone()], &[], &[]);
    let after = graph(&[ids[1].clone(), ids[2].clone()], &[], &[]);
    assert_eq!(check(&before, &after, &ids, &[0], &[2]), clear(1));
    assert_eq!(check(&before, &after, &ids, &[1], &[2]), clear(1));
    assert_eq!(
        check(&before, &after, &ids, &[0], &[0]),
        Err(EffectError::MissingProtectedResource)
    );
    assert_eq!(
        check(&before, &after, &ids, &[0], &[1]),
        Err(EffectError::MissingProtectedResource)
    );
    let absent = graph(&[], &[], &[]);
    assert_eq!(
        check(&absent, &absent, &ids, &[0], &[2]),
        Err(EffectError::MissingProtectedResource)
    );
}

#[test]
fn reached_unknown_in_either_snapshot_denies_but_unrelated_unknown_does_not() {
    let ids = ids(4);
    let complete = graph(&ids, &[(0, 1)], &[]);
    let reached = graph(&ids, &[(0, 1)], &[1]);
    for (a, b) in [(&complete, &reached), (&reached, &complete)] {
        assert_eq!(
            check(a, b, &ids, &[0], &[3]),
            Err(EffectError::IncompleteEffects)
        );
    }
    let unrelated = graph(&ids, &[(0, 1)], &[2]);
    assert_eq!(check(&unrelated, &unrelated, &ids, &[0], &[3]), clear(2));
    let both = graph(&ids, &[(0, 1), (0, 3)], &[0, 1]);
    assert_eq!(
        check(&both, &complete, &ids, &[0], &[3]),
        Err(EffectError::ProtectedImpact)
    );
}

#[test]
fn explicit_global_effects_and_missing_roots_cannot_turn_into_empty_success() {
    let ids = ids(3);
    let g = graph(&ids[..2], &[], &[]);
    let protected = ProtectedResources::new(&ids[..1]).unwrap();
    assert_eq!(
        analyze_effects(&g, &g, &protected, &ChangeScope::global()),
        Err(EffectError::GlobalEffects)
    );
    assert_eq!(
        check(&g, &g, &ids, &[2], &[0]),
        Err(EffectError::MissingRoot)
    );
    assert_eq!(
        ChangeScope::local(&[]).err(),
        Some(EffectError::InvalidSelection)
    );
    assert_eq!(
        ProtectedResources::new(&[]).err(),
        Some(EffectError::InvalidSelection)
    );
}

#[test]
fn bulk_roots_cycles_self_loops_and_shared_descendants_are_counted_once() {
    let ids = ids(6);
    let g = graph(&ids, &[(0, 2), (1, 2), (2, 3), (3, 4), (4, 2), (4, 4)], &[]);
    for roots in [&[0, 1][..], &[1, 0][..], &[0, 1, 2][..]] {
        assert_eq!(check(&g, &g, &ids, roots, &[5]), clear(5));
    }
    assert_eq!(
        check(&g, &g, &ids, &[0, 5], &[5]),
        Err(EffectError::ProtectedImpact)
    );
    for _ in 0..3 {
        assert_eq!(check(&g, &g, &ids, &[0], &[5]), clear(4));
    }
}

#[test]
fn failures_have_exact_fixed_codes_without_supplied_identity_or_graph_data() {
    use EffectError::*;
    for (error, code) in [
        (InvalidIdentity, "invalid_resource_identity"),
        (InvalidGraph, "invalid_effect_graph"),
        (InvalidSelection, "invalid_effect_selection"),
        (LimitExceeded, "effect_limit_exceeded"),
        (MissingRoot, "effect_root_missing"),
        (MissingProtectedResource, "protected_resource_missing"),
        (GlobalEffects, "global_effects_denied"),
        (ProtectedImpact, "protected_resource_affected"),
        (IncompleteEffects, "incomplete_effects"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
        assert!(!format!("{error:?}").contains("synthetic_private_label"));
    }
}

#[test]
fn translated_edges_and_knowledge_remain_exact_when_snapshots_have_different_typed_nodes() {
    let kinds = [
        ResourceKind::Service,
        ResourceKind::NetworkDevice,
        ResourceKind::Firewall,
        ResourceKind::Bridge,
        ResourceKind::Vlan,
        ResourceKind::Routing,
        ResourceKind::PhysicalPort,
    ];
    let ids: Vec<_> = kinds
        .into_iter()
        .enumerate()
        .map(|(i, kind)| resource(kind, &format!("typed-{i}")))
        .collect();
    let before_ids: Vec<_> = [1, 3, 5, 6].into_iter().map(|i| ids[i].clone()).collect();
    let after_ids: Vec<_> = [0, 2, 3, 4, 6]
        .into_iter()
        .map(|i| ids[i].clone())
        .collect();
    let before = graph(&before_ids, &[(0, 1), (1, 2)], &[]);
    let after = graph_order(&after_ids, &[(0, 1), (2, 3), (3, 4)], &[], true);
    assert_eq!(
        check(&before, &after, &ids, &[1], &[6]),
        Err(EffectError::ProtectedImpact)
    );
    assert_eq!(check(&before, &after, &ids, &[0], &[6]), clear(2));
    let unknown = graph(&after_ids, &[(0, 1), (2, 3)], &[2]);
    assert_eq!(
        check(&before, &unknown, &ids, &[1], &[6]),
        Err(EffectError::IncompleteEffects)
    );
    let duplicate_before = graph(&before_ids, &[(0, 1), (1, 3)], &[]);
    let duplicate_after = graph(&after_ids, &[(2, 4)], &[]);
    assert_eq!(
        check(&duplicate_before, &duplicate_after, &ids, &[1], &[6]),
        Err(EffectError::ProtectedImpact)
    );
}
