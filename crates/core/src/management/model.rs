use super::{EffectError, MAX_CHANGE_ROOTS, MAX_PROTECTED_RESOURCES, MAX_RESOURCE_ID_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceKind {
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
}

/// Opaque exact identity; never interpreted as a path or normalized as an alias.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceId {
    kind: ResourceKind,
    label: String,
}
impl ResourceId {
    pub fn new(kind: ResourceKind, label: &str) -> Result<Self, EffectError> {
        if label.is_empty()
            || label.len() > MAX_RESOURCE_ID_BYTES
            || !label.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b':' | b'@')
            })
        {
            return Err(EffectError::InvalidIdentity);
        }
        Ok(Self {
            kind,
            label: label.into(),
        })
    }
}

/// Declared outgoing-effect knowledge, not an attestation of the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKnowledge {
    Complete,
    Unknown,
}

#[derive(Clone)]
pub struct Node {
    pub(super) resource: ResourceId,
    pub(super) knowledge: EffectKnowledge,
}
impl Node {
    pub fn new(resource: ResourceId, knowledge: EffectKnowledge) -> Self {
        Self {
            resource,
            knowledge,
        }
    }
}

/// A change to `from` can influence `to`; endpoints are borrowed during capture.
pub struct Influence<'a> {
    pub(super) from: &'a ResourceId,
    pub(super) to: &'a ResourceId,
}
impl<'a> Influence<'a> {
    pub fn new(from: &'a ResourceId, to: &'a ResourceId) -> Self {
        Self { from, to }
    }
}

fn selection(ids: &[ResourceId], maximum: usize) -> Result<Box<[ResourceId]>, EffectError> {
    if ids.len() > maximum {
        return Err(EffectError::LimitExceeded);
    }
    if ids.is_empty() {
        return Err(EffectError::InvalidSelection);
    }
    let mut ids = ids.to_vec();
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(EffectError::InvalidSelection);
    }
    Ok(ids.into_boxed_slice())
}

pub struct ProtectedResources {
    pub(super) ids: Box<[ResourceId]>,
}
impl ProtectedResources {
    pub fn new(ids: &[ResourceId]) -> Result<Self, EffectError> {
        Ok(Self {
            ids: selection(ids, MAX_PROTECTED_RESOURCES)?,
        })
    }
}

pub struct ChangeScope {
    pub(super) roots: Option<Box<[ResourceId]>>,
}
impl ChangeScope {
    pub fn local(roots: &[ResourceId]) -> Result<Self, EffectError> {
        Ok(Self {
            roots: Some(selection(roots, MAX_CHANGE_ROOTS)?),
        })
    }
    pub fn global() -> Self {
        Self { roots: None }
    }
}
