use super::*;
use openwrt_mcp_core::management::{
    MAX_CHANGE_ROOTS, MAX_EFFECT_EDGES, MAX_EFFECT_NODES, MAX_PROTECTED_RESOURCES,
    MAX_RESOURCE_ID_BYTES,
};

#[test]
fn implementation_limits_match_reviewed_contract_and_selection_boundaries() {
    assert_eq!(
        (
            MAX_EFFECT_NODES,
            MAX_EFFECT_EDGES,
            MAX_CHANGE_ROOTS,
            MAX_PROTECTED_RESOURCES,
            MAX_RESOURCE_ID_BYTES
        ),
        (4096, 8192, 256, 128, 128)
    );
    let ids = ids(257);
    assert!(ChangeScope::local(&ids[..256]).is_ok());
    assert_eq!(
        ChangeScope::local(&ids).err(),
        Some(EffectError::LimitExceeded)
    );
    assert!(ProtectedResources::new(&ids[..128]).is_ok());
    assert_eq!(
        ProtectedResources::new(&ids[..129]).err(),
        Some(EffectError::LimitExceeded)
    );
    assert_eq!(
        ChangeScope::local(&[ids[0].clone(), ids[0].clone()]).err(),
        Some(EffectError::InvalidSelection)
    );
    assert_eq!(
        ProtectedResources::new(&[ids[0].clone(), ids[0].clone()]).err(),
        Some(EffectError::InvalidSelection)
    );
}

#[test]
fn each_graph_and_combined_nodes_have_independent_exact_ceiling() {
    let ids = ids(4097);
    let nodes: Vec<_> = ids
        .iter()
        .cloned()
        .map(|id| Node::new(id, EffectKnowledge::Complete))
        .collect();
    assert_eq!(
        EffectGraph::new(&nodes, &[]).err(),
        Some(EffectError::LimitExceeded)
    );
    let maximum = EffectGraph::new(&nodes[..4096], &[]).unwrap();
    assert_eq!(check(&maximum, &maximum, &ids, &[1], &[0]), clear(1));
    let before = graph(&ids[..2048], &[], &[]);
    let mut after_ids = ids[2048..4096].to_vec();
    after_ids.push(ids[0].clone());
    let after = graph(&after_ids, &[], &[]);
    assert_eq!(check(&before, &after, &ids, &[1], &[0]), clear(1));
    let overflow = graph(&[ids[0].clone(), ids[4096].clone()], &[], &[]);
    assert_eq!(
        check(&maximum, &overflow, &ids, &[1], &[0]),
        Err(EffectError::LimitExceeded)
    );
}

#[test]
fn exact_edge_limit_coalesces_cross_snapshot_duplicates_but_rejects_union_overflow() {
    let ids = ids(4096);
    let mut edges: Vec<_> = (0..4096)
        .flat_map(|i| [(i, i), (i, (i + 1) % 4096)])
        .collect();
    let maximum = graph(&ids, &edges, &[]);
    assert_eq!(
        check(&maximum, &maximum, &ids, &[1], &[0]),
        Err(EffectError::ProtectedImpact)
    );
    edges.push((0, 2));
    let nodes: Vec<_> = ids
        .iter()
        .cloned()
        .map(|id| Node::new(id, EffectKnowledge::Complete))
        .collect();
    let influences: Vec<_> = edges
        .iter()
        .map(|(a, b)| Influence::new(&ids[*a], &ids[*b]))
        .collect();
    assert_eq!(
        EffectGraph::new(&nodes, &influences).err(),
        Some(EffectError::LimitExceeded)
    );
    let before = graph(&ids, &edges[..4096], &[]);
    let after = graph(&ids, &edges[4096..], &[]);
    assert_eq!(
        check(&before, &after, &ids, &[1], &[0]),
        Err(EffectError::LimitExceeded)
    );
}

#[test]
fn maximum_depth_and_cycles_use_bounded_iterative_visited_state() {
    let ids = ids(4096);
    let mut edges: Vec<_> = (0..4094).map(|i| (i, i + 1)).collect();
    let chain = graph(&ids, &edges, &[]);
    assert_eq!(check(&chain, &chain, &ids, &[0], &[4095]), clear(4095));
    edges.push((4094, 0));
    let cycle = graph(&ids, &edges, &[]);
    assert_eq!(check(&cycle, &cycle, &ids, &[0], &[4095]), clear(4095));
    edges.push((4094, 4095));
    let protected = graph(&ids, &edges, &[]);
    assert_eq!(
        check(&cycle, &protected, &ids, &[0], &[4095]),
        Err(EffectError::ProtectedImpact)
    );
}
