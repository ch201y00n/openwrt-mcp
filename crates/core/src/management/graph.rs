use super::{EffectError, Influence, MAX_EFFECT_EDGES, MAX_EFFECT_NODES, Node, ResourceId};

/// Validated immutable graph. Edge storage uses indices, not repeated labels.
pub struct EffectGraph {
    pub(super) nodes: Box<[Node]>,
    pub(super) edges: Box<[(usize, usize)]>,
}
impl EffectGraph {
    pub fn new(nodes: &[Node], edges: &[Influence<'_>]) -> Result<Self, EffectError> {
        if nodes.len() > MAX_EFFECT_NODES || edges.len() > MAX_EFFECT_EDGES {
            return Err(EffectError::LimitExceeded);
        }
        let mut nodes = nodes.to_vec();
        nodes.sort_by(|a, b| a.resource.cmp(&b.resource));
        if nodes
            .windows(2)
            .any(|pair| pair[0].resource == pair[1].resource)
        {
            return Err(EffectError::InvalidGraph);
        }
        let mut indexed = Vec::with_capacity(edges.len());
        for edge in edges {
            let from = nodes
                .binary_search_by(|node| node.resource.cmp(edge.from))
                .map_err(|_| EffectError::InvalidGraph)?;
            let to = nodes
                .binary_search_by(|node| node.resource.cmp(edge.to))
                .map_err(|_| EffectError::InvalidGraph)?;
            indexed.push((from, to));
        }
        indexed.sort_unstable();
        if indexed.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(EffectError::InvalidGraph);
        }
        Ok(Self {
            nodes: nodes.into_boxed_slice(),
            edges: indexed.into_boxed_slice(),
        })
    }
    pub(super) fn index(&self, id: &ResourceId) -> Option<usize> {
        self.nodes
            .binary_search_by(|node| node.resource.cmp(id))
            .ok()
    }
}
