//! Private target-bound observations. No externally supplied snapshot or polling.
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use openwrt_mcp_core::{
    CapabilityObservation, CapabilityRequirement, PreparedAction, ProbeRequest, ReviewedObject,
    UnknownReason, Verdict,
};
use serde::Serialize;
use tokio::{sync::Mutex, time::Instant};

use crate::{Backend, Limits, RuntimeError};

const TTL: Duration = Duration::from_secs(30);

/// Safe status only; raw signatures, target identifiers and epochs stay private.
#[derive(Debug, Serialize)]
pub struct CapabilityStatus {
    pub compatibility: &'static str,
    pub reason: &'static str,
    pub scope: &'static str,
    pub response_contract: Option<String>,
    pub remaining_ttl_ms: Option<u64>,
}

#[derive(Clone)]
pub(crate) struct Observation {
    value: Arc<CapabilityObservation>,
    epoch: Option<u64>,
    started: Instant,
    valid: Arc<AtomicBool>,
}

impl Observation {
    fn unknown(reason: UnknownReason) -> Self {
        Self {
            value: Arc::new(CapabilityObservation::Unknown(reason)),
            epoch: None,
            started: Instant::now(),
            valid: Arc::new(AtomicBool::new(true)),
        }
    }

    fn fresh(&self, backend: &dyn Backend) -> bool {
        self.epoch.is_some()
            && self.valid.load(Ordering::Acquire)
            && self.epoch == backend.capability_epoch()
            && self.started.elapsed() < TTL
    }

    pub(crate) fn revoke(&self) {
        self.valid.store(false, Ordering::Release);
    }

    pub(crate) fn verdict(
        &self,
        backend: &dyn Backend,
        requirement: &CapabilityRequirement,
        action: Option<&PreparedAction>,
    ) -> Verdict {
        if self.epoch.is_some() && !self.fresh(backend) {
            return Verdict::Unknown(UnknownReason::StaleObservation);
        }
        requirement.evaluate(&self.value, action)
    }

    pub(crate) fn status(
        &self,
        backend: &dyn Backend,
        requirement: &CapabilityRequirement,
    ) -> CapabilityStatus {
        let verdict = self.verdict(backend, requirement, None);
        let (compatibility, reason) = match verdict {
            Verdict::Compatible => ("compatible", "signature_matched"),
            Verdict::Incompatible(reason) => ("incompatible", reason.code()),
            Verdict::Unknown(reason) => ("unknown", reason.code()),
        };
        CapabilityStatus {
            compatibility,
            reason,
            scope: "input_signature_only",
            response_contract: requirement.response_contract().map(str::to_owned),
            remaining_ttl_ms: self
                .fresh(backend)
                .then(|| TTL.saturating_sub(self.started.elapsed()).as_millis() as u64),
        }
    }
}

/// Owned by one dispatcher together with its immutable backend/configuration.
/// At most ReviewedObject::ALL.len() entries (nine in v9); bounded waiters.
#[derive(Default)]
pub(crate) struct CapabilityCache {
    entries: Mutex<BTreeMap<ReviewedObject, Observation>>,
}

impl CapabilityCache {
    pub(crate) async fn observe(
        &self,
        backend: &dyn Backend,
        requirement: &CapabilityRequirement,
        limits: &Limits,
        refresh: bool,
    ) -> Result<Observation, RuntimeError> {
        let Some(object) = requirement.probe_object() else {
            return Ok(Observation::unknown(UnknownReason::UnreviewedProbe));
        };
        let mut entries = self.entries.lock().await;
        entries.retain(|_, entry| {
            let fresh = entry.fresh(backend);
            if !fresh {
                entry.revoke();
            }
            fresh
        });
        if !refresh && let Some(entry) = entries.get(&object) {
            return Ok(entry.clone());
        }
        // Remove prior success before probing. Failure/cancellation never restores it.
        if let Some(previous) = entries.remove(&object) {
            // Revoke already-issued clones too, not only the map's copy.
            previous.revoke();
        }
        let started = Instant::now();
        let previous_epoch = backend.capability_epoch();
        let value = backend
            .probe(ProbeRequest::DescribeUbusObject(object), limits)
            .await?;
        let Some(epoch) = backend.capability_epoch() else {
            return Ok(Observation::unknown(UnknownReason::ProbeUnavailable));
        };
        if previous_epoch.is_some_and(|previous| previous != epoch) {
            return Ok(Observation::unknown(UnknownReason::StaleObservation));
        }
        if started.elapsed() >= TTL {
            return Ok(Observation::unknown(UnknownReason::StaleObservation));
        }
        let value = match value {
            CapabilityObservation::Ubus(observed)
                if observed.object != object || observed.validate().is_err() =>
            {
                CapabilityObservation::Unknown(UnknownReason::InvalidObservation)
            }
            other => other,
        };
        let entry = Observation {
            value: Arc::new(value),
            epoch: Some(epoch),
            started,
            valid: Arc::new(AtomicBool::new(true)),
        };
        entries.insert(object, entry.clone());
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use openwrt_mcp_core::{MethodSignature, ObjectObservation};
    use serde_json::Value;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    struct Target {
        epoch: AtomicU64,
        probes: AtomicUsize,
        fail: AtomicBool,
    }
    #[async_trait]
    impl Backend for Target {
        fn capability_epoch(&self) -> Option<u64> {
            Some(self.epoch.load(Ordering::SeqCst))
        }
        async fn probe(
            &self,
            _: ProbeRequest,
            _: &Limits,
        ) -> Result<CapabilityObservation, RuntimeError> {
            self.probes.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::SeqCst) {
                return Err(RuntimeError::BackendFailed);
            }
            Ok(CapabilityObservation::Ubus(ObjectObservation {
                object: ReviewedObject::System,
                methods: [(
                    "info".into(),
                    MethodSignature {
                        arguments: Default::default(),
                    },
                )]
                .into(),
            }))
        }
        async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
            unreachable!()
        }
    }
    #[tokio::test]
    async fn expired_lease_is_rejected_and_reprobed_without_waiting_wall_clock() {
        let target = Target {
            epoch: AtomicU64::new(1),
            probes: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
        };
        let cache = CapabilityCache::default();
        let requirement = CapabilityRequirement::UbusMethod {
            object: "system".into(),
            method: "info".into(),
            arguments: Default::default(),
            response_contract: "system_info.v1".into(),
        };
        cache
            .observe(&target, &requirement, &Limits::default(), false)
            .await
            .unwrap();
        let expired = {
            let mut entries = cache.entries.lock().await;
            let entry = entries.get_mut(&ReviewedObject::System).unwrap();
            entry.started = Instant::now() - TTL;
            entry.clone()
        };
        assert_eq!(
            expired.verdict(&target, &requirement, None),
            Verdict::Unknown(UnknownReason::StaleObservation)
        );
        assert_eq!(expired.status(&target, &requirement).remaining_ttl_ms, None);
        let fresh = cache
            .observe(&target, &requirement, &Limits::default(), false)
            .await
            .unwrap();
        assert_eq!(target.probes.load(Ordering::SeqCst), 2);
        assert_eq!(
            fresh.verdict(&target, &requirement, None),
            Verdict::Compatible
        );
        target.epoch.store(2, Ordering::SeqCst);
        assert_eq!(
            fresh.verdict(&target, &requirement, None),
            Verdict::Unknown(UnknownReason::StaleObservation)
        );
    }

    #[tokio::test]
    async fn forced_refresh_revokes_outstanding_clones_even_when_refresh_fails() {
        let target = Target {
            epoch: AtomicU64::new(1),
            probes: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
        };
        let cache = CapabilityCache::default();
        let requirement = CapabilityRequirement::UbusMethod {
            object: "system".into(),
            method: "info".into(),
            arguments: Default::default(),
            response_contract: "system_info.v1".into(),
        };
        let old = cache
            .observe(&target, &requirement, &Limits::default(), false)
            .await
            .unwrap();
        let current = cache
            .observe(&target, &requirement, &Limits::default(), true)
            .await
            .unwrap();
        assert_eq!(
            old.verdict(&target, &requirement, None),
            Verdict::Unknown(UnknownReason::StaleObservation)
        );
        assert_eq!(
            current.verdict(&target, &requirement, None),
            Verdict::Compatible
        );
        target.fail.store(true, Ordering::SeqCst);
        assert!(
            cache
                .observe(&target, &requirement, &Limits::default(), true)
                .await
                .is_err()
        );
        assert_eq!(
            current.verdict(&target, &requirement, None),
            Verdict::Unknown(UnknownReason::StaleObservation)
        );
        assert_eq!(current.status(&target, &requirement).remaining_ttl_ms, None);
        assert!(cache.entries.lock().await.is_empty());
    }
}
