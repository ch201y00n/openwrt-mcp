use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use openwrt_mcp_core::{Catalog, CoreError, Operation, Policy, Verdict};
use serde_json::Value;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::{
    AuditEvent, AuditKind, AuditOutcome, AuditPhase, AuditSink, Backend, CapabilityStatus, Limits,
    RuntimeError,
    capability::{CapabilityCache, Observation},
};

// Tokio timeouts are cooperative: a non-yielding backend decoder or pure
// projector can return Ready after the timer elapsed. Reject such late results
// and revoke every clone of evidence issued to the timed-out invocation. This
// is deadline-aware result admission, not preemption of synchronous work.
fn enforce_deadline<T>(
    deadline: tokio::time::Instant,
    observation: Option<&Observation>,
    result: Result<T, RuntimeError>,
) -> Result<T, RuntimeError> {
    if tokio::time::Instant::now() >= deadline {
        if let Some(observation) = observation {
            observation.revoke();
        }
        Err(RuntimeError::Timeout)
    } else {
        result
    }
}

pub struct Dispatcher {
    catalog: Catalog,
    policy: Policy,
    backend: Arc<dyn Backend>,
    audit: Arc<dyn AuditSink>,
    audit_gate: Arc<Semaphore>,
    audit_waiters: Semaphore,
    audit_failed: Arc<AtomicBool>,
    limits: Limits,
    permits: Semaphore,
    sequence: AtomicU64,
    capabilities: CapabilityCache,
    packages: crate::packages::Packages,
}

impl Dispatcher {
    pub fn new(
        catalog: Catalog,
        policy: Policy,
        backend: Arc<dyn Backend>,
        audit: Arc<dyn AuditSink>,
        limits: Limits,
    ) -> Result<Self, RuntimeError> {
        limits.validate()?;
        policy.validate(&catalog)?;
        Ok(Self {
            catalog,
            policy,
            backend,
            audit,
            audit_gate: Arc::new(Semaphore::new(1)),
            audit_waiters: Semaphore::new(limits.max_concurrent + 1),
            audit_failed: Arc::new(AtomicBool::new(false)),
            permits: Semaphore::new(limits.max_concurrent),
            limits,
            sequence: AtomicU64::new(1),
            capabilities: CapabilityCache::default(),
            packages: crate::packages::Packages::default(),
        })
    }

    pub fn available_operations(&self) -> Vec<Operation> {
        self.catalog
            .operations()
            .iter()
            .filter(|operation| self.policy.authorize(operation).is_ok())
            .cloned()
            .collect()
    }

    /// Composition supplies entropy; the default constructor keeps capture closed.
    pub fn with_snapshot_tokens(
        mut self,
        tokens: Arc<dyn crate::packages::SnapshotTokens>,
    ) -> Self {
        self.packages.tokens = Some(tokens);
        self
    }

    pub async fn invoke(&self, name: &str, arguments: Value) -> Result<Value, RuntimeError> {
        let request_sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let kind = AuditKind::Invocation;
        let operation = self.authorize(name, request_sequence, kind).await?;
        let invocation = match operation.prepare_invocation(&arguments) {
            Ok(invocation) => invocation,
            Err(error) => {
                self.reject(
                    request_sequence,
                    &operation.name,
                    AuditOutcome::InvalidArguments,
                    kind,
                )
                .await?;
                return Err(error.into());
            }
        };
        let _permit = self.admit(operation, request_sequence, kind).await?;
        let started = Instant::now();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.limits.timeout_ms);
        if let openwrt_mcp_core::PreparedAction::ApkInstalledPage { cursor } = invocation.action() {
            let mut lease = match self.packages.lease() {
                Ok(lease) => lease,
                Err(error) => {
                    return self
                        .finish(request_sequence, operation, kind, started, Err(error))
                        .await;
                }
            };
            let result = tokio::time::timeout_at(
                deadline,
                self.packages.page(
                    &mut lease,
                    &operation.name,
                    cursor.as_deref(),
                    self.backend.as_ref(),
                    &self.limits,
                    deadline,
                ),
            )
            .await
            .unwrap_or(Err(RuntimeError::Timeout));
            let result = enforce_deadline(deadline, None, result);
            let result = self
                .finish(request_sequence, operation, kind, started, result)
                .await;
            if result.is_ok() {
                lease.preserve(self.backend.as_ref())?;
            }
            return result;
        }
        let mut issued = None;
        let result = tokio::time::timeout_at(deadline, async {
            let observation = self
                .capabilities
                .observe(
                    self.backend.as_ref(),
                    &operation.capability,
                    &self.limits,
                    false,
                )
                .await?;
            issued = Some(observation.clone());
            enforce_deadline(deadline, Some(&observation), Ok(()))?;
            match observation.verdict(
                self.backend.as_ref(),
                &operation.capability,
                Some(invocation.action()),
            ) {
                Verdict::Compatible => (),
                Verdict::Unknown(_) => return Err(RuntimeError::CapabilityUnknown),
                Verdict::Incompatible(_) => return Err(RuntimeError::CapabilityUnsupported),
            }
            enforce_deadline(deadline, Some(&observation), Ok(()))?;
            let output = enforce_deadline(
                deadline,
                Some(&observation),
                self.backend
                    .execute(invocation.action(), &self.limits)
                    .await,
            )?;
            enforce_deadline(
                deadline,
                Some(&observation),
                invocation.project(&output).map_err(Into::into),
            )
        })
        .await
        .unwrap_or(Err(RuntimeError::Timeout));
        let result = enforce_deadline(deadline, issued.as_ref(), result);
        self.finish(request_sequence, operation, kind, started, result)
            .await
    }

    /// Metadata queries have the same policy, admission and start-before-I/O audit.
    /// JSON validation lives here so malformed requests cannot bypass audit in MCP.
    pub async fn capability_status(
        &self,
        arguments: Value,
    ) -> Result<CapabilityStatus, RuntimeError> {
        let request_sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let kind = AuditKind::Capability;
        let Some(fields) = arguments.as_object() else {
            self.reject(
                request_sequence,
                "unknown",
                AuditOutcome::InvalidArguments,
                kind,
            )
            .await?;
            return Err(CoreError::InvalidArguments.into());
        };
        let Some(name) = fields.get("operation").and_then(Value::as_str) else {
            self.reject(
                request_sequence,
                "unknown",
                AuditOutcome::InvalidArguments,
                kind,
            )
            .await?;
            return Err(CoreError::InvalidArguments.into());
        };
        let operation = self.authorize(name, request_sequence, kind).await?;
        if fields
            .keys()
            .any(|key| key != "operation" && key != "refresh")
            || fields
                .get("refresh")
                .is_some_and(|value| !value.is_boolean())
        {
            self.reject(
                request_sequence,
                &operation.name,
                AuditOutcome::InvalidArguments,
                kind,
            )
            .await?;
            return Err(CoreError::InvalidArguments.into());
        }
        let refresh = fields
            .get("refresh")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let _permit = self.admit(operation, request_sequence, kind).await?;
        let started = Instant::now();
        if matches!(
            operation.capability,
            openwrt_mcp_core::CapabilityRequirement::ApkInstalledQuery {}
        ) {
            return self
                .finish(
                    request_sequence,
                    operation,
                    kind,
                    started,
                    Ok(CapabilityStatus {
                        compatibility: "unknown",
                        reason: "capture_required",
                        scope: "closed_query_response",
                        response_contract: operation
                            .capability
                            .response_contract()
                            .map(str::to_owned),
                        remaining_ttl_ms: None,
                    }),
                )
                .await;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.limits.timeout_ms);
        let mut issued = None;
        let result = tokio::time::timeout_at(deadline, async {
            let observation = self
                .capabilities
                .observe(
                    self.backend.as_ref(),
                    &operation.capability,
                    &self.limits,
                    refresh,
                )
                .await?;
            issued = Some(observation.clone());
            enforce_deadline(deadline, Some(&observation), Ok(()))?;
            enforce_deadline(
                deadline,
                Some(&observation),
                Ok(observation.status(self.backend.as_ref(), &operation.capability)),
            )
        })
        .await
        .unwrap_or(Err(RuntimeError::Timeout));
        let result = enforce_deadline(deadline, issued.as_ref(), result);
        self.finish(request_sequence, operation, kind, started, result)
            .await
    }

    async fn authorize(
        &self,
        name: &str,
        sequence: u64,
        kind: AuditKind,
    ) -> Result<&Operation, RuntimeError> {
        let Some(operation) = self.catalog.get(name) else {
            self.reject(sequence, "unknown", AuditOutcome::UnknownOperation, kind)
                .await?;
            return Err(RuntimeError::UnknownOperation);
        };
        if let Err(error) = self.policy.authorize(operation) {
            self.reject(sequence, &operation.name, AuditOutcome::Denied, kind)
                .await?;
            return Err(error.into());
        }
        Ok(operation)
    }

    async fn admit(
        &self,
        operation: &Operation,
        sequence: u64,
        kind: AuditKind,
    ) -> Result<SemaphorePermit<'_>, RuntimeError> {
        let permit = match self.permits.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                self.reject(sequence, &operation.name, AuditOutcome::Busy, kind)
                    .await?;
                return Err(RuntimeError::Busy);
            }
        };
        self.record_audit(
            AuditEvent::new(
                sequence,
                AuditPhase::Start,
                &operation.name,
                AuditOutcome::Attempt,
                None,
            )
            .with_kind(kind),
        )
        .await?;
        Ok(permit)
    }

    async fn finish<T: Send>(
        &self,
        sequence: u64,
        operation: &Operation,
        kind: AuditKind,
        started: Instant,
        result: Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let outcome = if result.is_ok() {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failed
        };
        self.record_audit(
            AuditEvent::new(
                sequence,
                AuditPhase::Finish,
                &operation.name,
                outcome,
                Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
            )
            .with_kind(kind),
        )
        .await
        .map_err(|_| RuntimeError::CompletionAuditFailed)?;
        result
    }

    async fn reject(
        &self,
        sequence: u64,
        operation: &str,
        outcome: AuditOutcome,
        kind: AuditKind,
    ) -> Result<(), RuntimeError> {
        self.record_audit(
            AuditEvent::new(sequence, AuditPhase::Rejection, operation, outcome, None)
                .with_kind(kind),
        )
        .await
    }

    async fn record_audit(&self, event: AuditEvent) -> Result<(), RuntimeError> {
        if self.audit_failed.load(Ordering::Acquire) {
            return Err(RuntimeError::AuditUnavailable);
        }
        // Bound waiting audit futures too, including rejected client requests.
        // Saturation fails this request closed without scheduling more workers.
        let _waiting = self
            .audit_waiters
            .try_acquire()
            .map_err(|_| RuntimeError::AuditUnavailable)?;
        let deadline = Duration::from_millis(self.limits.timeout_ms.min(1000));
        let record = async {
            let permit = self
                .audit_gate
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| RuntimeError::AuditUnavailable)?;
            if self.audit_failed.load(Ordering::Acquire) {
                return Err(RuntimeError::AuditUnavailable);
            }
            let audit = self.audit.clone();
            let failed = self.audit_failed.clone();
            tokio::task::spawn_blocking(move || {
                // The worker owns the only gate until the OS write returns.
                // Dropping a timed-out future cannot release that gate early.
                let _permit = permit;
                let result = audit.record(&event);
                if result.is_err() {
                    failed.store(true, Ordering::Release);
                }
                result
            })
            .await
            .map_err(|_| RuntimeError::AuditUnavailable)?
            .map_err(|_| RuntimeError::AuditUnavailable)
        };
        match tokio::time::timeout(deadline, record).await {
            Ok(Ok(())) => Ok(()),
            _ => {
                // At most one blocked OS worker survives per dispatcher. Never
                // recreate dispatchers to retry a failed sink in a live server;
                // operator restart/reconfiguration is needed after this latch.
                self.audit_failed.store(true, Ordering::Release);
                Err(RuntimeError::AuditUnavailable)
            }
        }
    }
}
