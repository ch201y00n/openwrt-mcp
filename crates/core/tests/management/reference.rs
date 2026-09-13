use super::*;

// Independent fixed-point adjacency matrix, not the production index/CSR walk.
fn reference(
    before: &[(usize, usize)],
    after: &[(usize, usize)],
    unknown: u8,
    roots: u8,
) -> Result<EffectAssessment, EffectError> {
    let mut matrix = [[false; 3]; 3];
    for &(from, to) in before.iter().chain(after.iter()) {
        matrix[from][to] = true;
    }
    let mut reached = [false; 3];
    for (i, flag) in reached.iter_mut().enumerate() {
        *flag = roots & (1 << i) != 0;
    }
    loop {
        let previous = reached;
        for (from, row) in matrix.iter().enumerate() {
            for (to, edge) in row.iter().enumerate() {
                if previous[from] && *edge {
                    reached[to] = true;
                }
            }
        }
        if reached == previous {
            break;
        }
    }
    if reached[2] {
        return Err(EffectError::ProtectedImpact);
    }
    if reached
        .iter()
        .enumerate()
        .any(|(i, yes)| *yes && unknown & (1 << i) != 0)
    {
        return Err(EffectError::IncompleteEffects);
    }
    clear(reached.into_iter().filter(|yes| *yes).count())
}
#[test]
fn exhaustive_small_models_match_reference_order_invariance_and_monotonic_denial() {
    let ids = ids(3);
    let pairs = [(0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
    let mut cases = 0;
    for mask in 0_u8..64 {
        let before_edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .filter_map(|(i, e)| (mask & (1 << i) != 0).then_some(*e))
            .collect();
        let after_mask = ((mask << 1) | (mask >> 5)) & 63;
        let after_edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .filter_map(|(i, e)| (after_mask & (1 << i) != 0).then_some(*e))
            .collect();
        for unknown in 0_u8..8 {
            let unknown_nodes: Vec<_> = (0..3).filter(|i| unknown & (1 << i) != 0).collect();
            let before = graph(&ids, &before_edges, &unknown_nodes);
            let after = graph(&ids, &after_edges, &[]);
            let reversed_before = graph_order(&ids, &before_edges, &unknown_nodes, true);
            let reversed_after = graph_order(&ids, &after_edges, &[], true);
            for root_bits in 1_u8..8 {
                let mut roots: Vec<_> = (0..3).filter(|i| root_bits & (1 << i) != 0).collect();
                let expected = reference(&before_edges, &after_edges, unknown, root_bits);
                assert_eq!(check(&before, &after, &ids, &roots, &[2]), expected);
                roots.reverse();
                assert_eq!(
                    check(&reversed_before, &reversed_after, &ids, &roots, &[2]),
                    expected
                );
                if root_bits == 1 && expected.is_err() {
                    let mut more = after_edges.clone();
                    if !more.contains(&(0, 2)) {
                        more.push((0, 2));
                    }
                    let additional = graph(&ids, &more, &[]);
                    assert!(check(&before, &additional, &ids, &roots, &[2]).is_err());
                    let all_unknown = graph(&ids, &after_edges, &[0, 1, 2]);
                    assert!(check(&before, &all_unknown, &ids, &roots, &[2]).is_err());
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 3584);
}
