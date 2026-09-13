//! Analysis of supplied influence graphs, never permission or live-state proof.
//! No production consumer is admitted until a separate integration checkpoint.
mod analysis;
mod graph;
mod model;

pub use analysis::analyze_effects;
pub use graph::EffectGraph;
pub use model::{
    ChangeScope, EffectKnowledge, Influence, Node, ProtectedResources, ResourceId, ResourceKind,
};

pub const MAX_EFFECT_NODES: usize = 4096;
pub const MAX_EFFECT_EDGES: usize = 8192;
pub const MAX_CHANGE_ROOTS: usize = 256;
pub const MAX_PROTECTED_RESOURCES: usize = 128;
pub const MAX_RESOURCE_ID_BYTES: usize = 128;

/// A count-only observation about the supplied model, not a mutation permit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectAssessment {
    NoKnownProtectedImpact { affected_resources: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectError {
    InvalidIdentity,
    InvalidGraph,
    InvalidSelection,
    LimitExceeded,
    MissingRoot,
    MissingProtectedResource,
    GlobalEffects,
    ProtectedImpact,
    IncompleteEffects,
}

impl EffectError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidIdentity => "invalid_resource_identity",
            Self::InvalidGraph => "invalid_effect_graph",
            Self::InvalidSelection => "invalid_effect_selection",
            Self::LimitExceeded => "effect_limit_exceeded",
            Self::MissingRoot => "effect_root_missing",
            Self::MissingProtectedResource => "protected_resource_missing",
            Self::GlobalEffects => "global_effects_denied",
            Self::ProtectedImpact => "protected_resource_affected",
            Self::IncompleteEffects => "incomplete_effects",
        }
    }
}
impl std::fmt::Display for EffectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for EffectError {}
