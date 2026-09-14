use crate::SourceRegistry;
use async_trait::async_trait;
use openwrt_mcp_runtime::{
    mutation_ports::{
        MutationError, SecretPurpose, SecretSource, WorkBudget,
        secrets::{SecretReference, SecretValue},
    },
    protection::KeySource,
};
use std::{collections::BTreeMap, sync::Arc};
use zeroize::Zeroizing;

pub struct ServiceSecrets {
    sources: BTreeMap<String, (SecretPurpose, Arc<dyn KeySource>)>,
}
impl ServiceSecrets {
    /// Resolves configuration only, without touching any source until requested.
    pub fn new(
        registry: &SourceRegistry,
        bindings: Vec<(SecretReference, SecretPurpose, String)>,
    ) -> Result<Self, MutationError> {
        if bindings.len() > 128 {
            return Err(MutationError::Limit);
        }
        let mut sources = BTreeMap::new();
        for (reference, purpose, source) in bindings {
            let source = registry
                .resolve(&source)
                .map_err(|_| MutationError::Invalid)?;
            if sources
                .insert(reference.alias().to_owned(), (purpose, source))
                .is_some()
            {
                return Err(MutationError::Invalid);
            }
        }
        Ok(Self { sources })
    }
}
#[async_trait]
impl SecretSource for ServiceSecrets {
    /// Call on the owned bounded worker: file/env providers can block.
    async fn resolve(
        &self,
        reference: &SecretReference,
        purpose: SecretPurpose,
        budget: WorkBudget,
    ) -> Result<SecretValue, MutationError> {
        budget.check()?;
        let (expected, source) = self
            .sources
            .get(reference.alias())
            .ok_or(MutationError::Invalid)?;
        if *expected != purpose {
            return Err(MutationError::Invalid);
        }
        let material = source.read(65536).map_err(|_| MutationError::Unavailable)?;
        budget.check()?;
        if material.expose_bytes().len() > 65536 {
            return Err(MutationError::Limit);
        }
        SecretValue::new(Zeroizing::new(material.expose_bytes().to_vec()), purpose)
    }
}
