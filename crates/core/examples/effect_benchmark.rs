//! Synthetic domain microbenchmark; no device, key, topology discovery or MCP.
use openwrt_mcp_core::management::{
    ChangeScope, EffectAssessment, EffectGraph, EffectKnowledge, Influence, Node,
    ProtectedResources, ResourceId, ResourceKind, analyze_effects,
};
use std::{hint::black_box, time::Instant};

fn sample(mut work: impl FnMut()) -> (u128, u128) {
    for _ in 0..16 {
        work();
    }
    let mut samples = Vec::with_capacity(256);
    for _ in 0..256 {
        let start = Instant::now();
        work();
        samples.push(start.elapsed().as_nanos());
    }
    samples.sort_unstable();
    (samples[127], samples[243])
}

fn main() {
    let ids: Vec<_> = (0..4096)
        .map(|i| ResourceId::new(ResourceKind::NetworkDevice, &format!("fixture-{i:05}")).unwrap())
        .collect();
    let nodes: Vec<_> = ids
        .iter()
        .cloned()
        .map(|id| Node::new(id, EffectKnowledge::Complete))
        .collect();
    let mut pairs: Vec<_> = (0..4095)
        .flat_map(|i| [(i, i), (i, (i + 1) % 4095)])
        .collect();
    pairs.extend([(4095, 4095), (0, 2)]);
    let edges: Vec<_> = pairs
        .iter()
        .map(|(from, to)| Influence::new(&ids[*from], &ids[*to]))
        .collect();
    assert_eq!(edges.len(), 8192);
    let before = EffectGraph::new(&nodes, &edges).unwrap();
    let after = EffectGraph::new(&nodes, &edges).unwrap();
    let guarded = ProtectedResources::new(&ids[4095..]).unwrap();
    let scope = ChangeScope::local(&ids[..256]).unwrap();
    assert_eq!(
        analyze_effects(&before, &after, &guarded, &scope).unwrap(),
        EffectAssessment::NoKnownProtectedImpact {
            affected_resources: 4095
        }
    );
    let create = sample(|| {
        black_box(EffectGraph::new(black_box(&nodes), black_box(&edges)).unwrap());
    });
    let analyze = sample(|| {
        black_box(
            analyze_effects(
                black_box(&before),
                black_box(&after),
                black_box(&guarded),
                black_box(&scope),
            )
            .unwrap(),
        );
    });
    println!(
        "synthetic_effect_model nodes=4096 edges_per_graph=8192 identity_bytes=13 roots=256 protected=1 warmup=16 samples=256 create_and_drop_p50_ns={} create_and_drop_p95_ns={} analyze_p50_ns={} analyze_p95_ns={}",
        create.0, create.1, analyze.0, analyze.1
    );
}
