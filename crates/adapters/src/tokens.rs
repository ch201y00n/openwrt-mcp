//! Bounded OS entropy work. A cancelled caller cannot free a blocked worker slot.
use async_trait::async_trait;
use openwrt_mcp_runtime::{RuntimeError, packages::SnapshotTokens};
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct RandomSnapshotTokens {
    workers: Arc<Semaphore>,
}
impl Default for RandomSnapshotTokens {
    fn default() -> Self {
        Self {
            workers: Arc::new(Semaphore::new(1)),
        }
    }
}
#[async_trait]
impl SnapshotTokens for RandomSnapshotTokens {
    async fn nonce(&self) -> Result<[u8; 16], RuntimeError> {
        let permit = self
            .workers
            .clone()
            .try_acquire_owned()
            .map_err(|_| RuntimeError::EntropyUnavailable)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut bytes = [0; 16];
            getrandom::fill(&mut bytes).map_err(|_| RuntimeError::EntropyUnavailable)?;
            Ok(bytes)
        })
        .await
        .map_err(|_| RuntimeError::EntropyUnavailable)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn native_entropy_produces_nonzero_distinct_private_nonces() {
        let source = RandomSnapshotTokens::default();
        let first = source.nonce().await.unwrap();
        let second = source.nonce().await.unwrap();
        assert_ne!(first, [0; 16]);
        assert_ne!(second, [0; 16]);
        assert_ne!(first, second);
    }
    #[tokio::test]
    async fn occupied_worker_slot_fails_without_starting_another_worker() {
        let source = RandomSnapshotTokens::default();
        let _permit = source.workers.clone().try_acquire_owned().unwrap();
        assert_eq!(
            source.nonce().await.unwrap_err().code(),
            "entropy_unavailable"
        );
    }
}
