use super::{
    ChangeScope, EffectAssessment, EffectError, EffectGraph, EffectKnowledge, MAX_EFFECT_EDGES,
    MAX_EFFECT_NODES, ProtectedResources, ResourceId,
};
use std::cmp::Ordering;

type UnionNode<'a> = (&'a ResourceId, EffectKnowledge);
struct CombinedGraph<'a> {
    nodes: Vec<UnionNode<'a>>,
    edges: Vec<(usize, usize)>,
}

// Missing means that sorted stream is exhausted, not a smaller value.
fn next_order<T: Ord>(left: Option<&T>, right: Option<&T>) -> Option<Ordering> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.cmp(right)),
        (Some(_), None) => Some(Ordering::Less),
        (None, Some(_)) => Some(Ordering::Greater),
        (None, None) => None,
    }
}

fn combined_graph<'a>(
    before: &'a EffectGraph,
    after: &'a EffectGraph,
) -> Result<CombinedGraph<'a>, EffectError> {
    let mut left = before.nodes.iter().peekable();
    let mut right = after.nodes.iter().peekable();
    let mut nodes =
        Vec::with_capacity((before.nodes.len() + after.nodes.len()).min(MAX_EFFECT_NODES));
    let mut before_map = Vec::with_capacity(before.nodes.len());
    let mut after_map = Vec::with_capacity(after.nodes.len());
    while let Some(order) = next_order(
        left.peek().map(|n| &n.resource),
        right.peek().map(|n| &n.resource),
    ) {
        if nodes.len() == MAX_EFFECT_NODES {
            return Err(EffectError::LimitExceeded);
        }
        let position = nodes.len();
        match order {
            Ordering::Less => {
                let node = left.next().ok_or(EffectError::InvalidGraph)?;
                before_map.push(position);
                nodes.push((&node.resource, node.knowledge));
            }
            Ordering::Greater => {
                let node = right.next().ok_or(EffectError::InvalidGraph)?;
                after_map.push(position);
                nodes.push((&node.resource, node.knowledge));
            }
            Ordering::Equal => {
                let a = left.next().ok_or(EffectError::InvalidGraph)?;
                let b = right.next().ok_or(EffectError::InvalidGraph)?;
                before_map.push(position);
                after_map.push(position);
                let knowledge = if a.knowledge == EffectKnowledge::Unknown
                    || b.knowledge == EffectKnowledge::Unknown
                {
                    EffectKnowledge::Unknown
                } else {
                    EffectKnowledge::Complete
                };
                nodes.push((&a.resource, knowledge));
            }
        }
    }
    // Each validated graph's node mapping is strictly increasing. Translating
    // its sorted edge indices therefore preserves lexicographic edge order.
    // Merge the two streams directly: no per-edge string lookup or tree nodes.
    let mut left = before
        .edges
        .iter()
        .map(|(a, b)| (before_map[*a], before_map[*b]))
        .peekable();
    let mut right = after
        .edges
        .iter()
        .map(|(a, b)| (after_map[*a], after_map[*b]))
        .peekable();
    let mut edges =
        Vec::with_capacity((before.edges.len() + after.edges.len()).min(MAX_EFFECT_EDGES));
    while let Some(order) = next_order(left.peek(), right.peek()) {
        if edges.len() == MAX_EFFECT_EDGES {
            return Err(EffectError::LimitExceeded);
        }
        let edge = match order {
            Ordering::Less => left.next(),
            Ordering::Greater => right.next(),
            Ordering::Equal => {
                right.next();
                left.next()
            }
        }
        .ok_or(EffectError::InvalidGraph)?;
        edges.push(edge);
    }
    Ok(CombinedGraph { nodes, edges })
}

fn index(nodes: &[UnionNode<'_>], id: &ResourceId) -> Option<usize> {
    nodes.binary_search_by(|node| node.0.cmp(id)).ok()
}

/// Conservatively evaluates only the supplied model. No approval or live binding.
pub fn analyze_effects(
    before: &EffectGraph,
    after: &EffectGraph,
    protected: &ProtectedResources,
    scope: &ChangeScope,
) -> Result<EffectAssessment, EffectError> {
    let CombinedGraph { nodes, edges } = combined_graph(before, after)?;
    for id in &protected.ids {
        if before.index(id).is_none() || after.index(id).is_none() {
            return Err(EffectError::MissingProtectedResource);
        }
    }
    let roots = scope.roots.as_ref().ok_or(EffectError::GlobalEffects)?;
    let mut stack = Vec::with_capacity(nodes.len());
    let mut visited = vec![false; nodes.len()];
    for root in roots {
        let node = index(&nodes, root).ok_or(EffectError::MissingRoot)?;
        visited[node] = true;
        stack.push(node);
    }
    let mut protected_flags = vec![false; nodes.len()];
    for id in &protected.ids {
        protected_flags[index(&nodes, id).ok_or(EffectError::MissingProtectedResource)?] = true;
    }
    // Compact sorted adjacency: N+1 offsets and E targets, no per-node heap list.
    let mut offsets = vec![0_usize; nodes.len() + 1];
    let mut targets = Vec::with_capacity(edges.len());
    for (from, to) in edges {
        offsets[from + 1] += 1;
        targets.push(to);
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    let mut affected = 0;
    let (mut hit, mut incomplete) = (false, false);
    while let Some(node) = stack.pop() {
        affected += 1;
        hit |= protected_flags[node];
        incomplete |= nodes[node].1 == EffectKnowledge::Unknown;
        for &dependent in &targets[offsets[node]..offsets[node + 1]] {
            if !visited[dependent] {
                visited[dependent] = true;
                stack.push(dependent);
            }
        }
    }
    if hit {
        Err(EffectError::ProtectedImpact)
    } else if incomplete {
        Err(EffectError::IncompleteEffects)
    } else {
        Ok(EffectAssessment::NoKnownProtectedImpact {
            affected_resources: affected,
        })
    }
}
