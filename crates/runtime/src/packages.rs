//! One ephemeral observation per Dispatcher. No native I/O or protocol handling.
use crate::{Backend, Limits, RuntimeError};
use async_trait::async_trait;
use openwrt_mcp_core::packages::{PackageCursor, PackageObservation, PackageProfile};
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, MutexGuard},
    time::Instant,
};

const TTL: Duration = Duration::from_secs(120);

#[async_trait]
pub trait SnapshotTokens: Send + Sync {
    async fn nonce(&self) -> Result<[u8; 16], RuntimeError>;
}

struct Snapshot {
    records: PackageObservation,
    nonce: [u8; 16],
    operation: String,
    epoch: u64,
    started: Instant,
}

#[derive(Default)]
pub(crate) struct Packages {
    slot: Mutex<Option<Snapshot>>,
    pub(crate) tokens: Option<Arc<dyn SnapshotTokens>>,
}

/// Cancellation or unsuccessful completion drops the private state. The lock is
/// retained through completion audit, preventing a page from racing a refresh.
pub(crate) struct PageLease<'a> {
    slot: MutexGuard<'a, Option<Snapshot>>,
    keep: bool,
}
impl Drop for PageLease<'_> {
    fn drop(&mut self) {
        if !self.keep {
            *self.slot = None;
        }
    }
}
impl PageLease<'_> {
    pub(crate) fn preserve(mut self, backend: &dyn Backend) -> Result<(), RuntimeError> {
        let snapshot = self.slot.as_ref().ok_or(RuntimeError::InvalidCursor)?;
        if snapshot.started.elapsed() >= TTL || backend.capability_epoch() != Some(snapshot.epoch) {
            return Err(RuntimeError::InvalidCursor);
        }
        self.keep = true;
        Ok(())
    }
}

impl Packages {
    pub(crate) fn lease(&self) -> Result<PageLease<'_>, RuntimeError> {
        Ok(PageLease {
            slot: self.slot.try_lock().map_err(|_| RuntimeError::Busy)?,
            keep: false,
        })
    }

    pub(crate) async fn page(
        &self,
        lease: &mut PageLease<'_>,
        operation: &str,
        request: (PackageProfile, Option<&str>),
        backend: &dyn Backend,
        limits: &Limits,
        deadline: Instant,
    ) -> Result<Value, RuntimeError> {
        let (profile, cursor) = request;
        let offset;
        if let Some(cursor) = cursor {
            let cursor = PackageCursor::parse(cursor)?;
            let snapshot = lease.slot.as_ref().ok_or(RuntimeError::InvalidCursor)?;
            if cursor.nonce() != &snapshot.nonce
                || profile != snapshot.records.profile()
                || operation != snapshot.operation
                || snapshot.started.elapsed() >= TTL
                || backend.capability_epoch() != Some(snapshot.epoch)
            {
                return Err(RuntimeError::InvalidCursor);
            }
            offset = cursor.offset();
        } else {
            *lease.slot = None;
            let started = Instant::now();
            let nonce = self
                .tokens
                .as_ref()
                .ok_or(RuntimeError::EntropyUnavailable)?
                .nonce()
                .await?;
            if Instant::now() >= deadline {
                return Err(RuntimeError::Timeout);
            }
            if nonce == [0; 16] {
                return Err(RuntimeError::EntropyUnavailable);
            }
            let previous = backend.capability_epoch();
            let records = match profile {
                PackageProfile::Apk3_0_5 => backend.capture_apk_installed(limits).await?,
                PackageProfile::Opkg38eccbb1RootStatus => {
                    backend.capture_opkg_status(limits).await?
                }
            };
            if records.profile() != profile {
                return Err(RuntimeError::InvalidOutput);
            }
            let epoch = backend
                .capability_epoch()
                .ok_or(RuntimeError::CapabilityUnknown)?;
            if Instant::now() >= deadline {
                return Err(RuntimeError::Timeout);
            }
            if previous.is_some_and(|old| old != epoch) || started.elapsed() >= TTL {
                return Err(RuntimeError::InvalidCursor);
            }
            *lease.slot = Some(Snapshot {
                records,
                nonce,
                operation: operation.into(),
                epoch,
                started,
            });
            offset = 0;
        }
        let snapshot = lease.slot.as_ref().ok_or(RuntimeError::InvalidCursor)?;
        snapshot
            .records
            .page(&snapshot.nonce, offset)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openwrt_mcp_core::PreparedAction;
    struct NoCapture;
    #[async_trait]
    impl Backend for NoCapture {
        fn capability_epoch(&self) -> Option<u64> {
            Some(1)
        }
        async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
            unreachable!()
        }
        async fn capture_apk_installed(
            &self,
            _: &Limits,
        ) -> Result<PackageObservation, RuntimeError> {
            panic!("continuation must not recapture")
        }
    }
    #[tokio::test]
    async fn expired_operation_mismatch_and_dropped_lease_clear_private_state() {
        let packages = Packages::default();
        for expired in [true, false] {
            *packages.slot.lock().await = Some(Snapshot {
                records: PackageObservation::new(vec![]).unwrap(),
                nonce: [1; 16],
                operation: "original".into(),
                epoch: 1,
                started: if expired {
                    Instant::now() - TTL
                } else {
                    Instant::now()
                },
            });
            let mut lease = packages.lease().unwrap();
            let cursor = format!("{}.16", "01".repeat(16));
            let result = packages
                .page(
                    &mut lease,
                    if expired { "original" } else { "alias" },
                    (PackageProfile::Apk3_0_5, Some(&cursor)),
                    &NoCapture,
                    &Limits::default(),
                    Instant::now() + Duration::from_secs(1),
                )
                .await;
            assert_eq!(result.unwrap_err().code(), "invalid_cursor");
            drop(lease);
            assert!(packages.slot.lock().await.is_none());
        }
    }
}
