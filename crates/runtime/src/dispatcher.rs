use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use openwrt_mcp_core::{Catalog, Operation, Policy};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::{AuditEvent, AuditOutcome, AuditPhase, AuditSink, Backend, Limits, RuntimeError};

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

    pub async fn invoke(&self, name: &str, arguments: Value) -> Result<Value, RuntimeError> {
        let request_sequence = self.sequence.fetch_add(1, Ordering::Relaxed);
        let Some(operation) = self.catalog.get(name) else {
            self.reject(request_sequence, "unknown", AuditOutcome::UnknownOperation)
                .await?;
            return Err(RuntimeError::UnknownOperation);
        };
        if let Err(error) = self.policy.authorize(operation) {
            self.reject(request_sequence, &operation.name, AuditOutcome::Denied)
                .await?;
            return Err(error.into());
        }
        let invocation = match operation.prepare(&arguments) {
            Ok(invocation) => invocation,
            Err(error) => {
                self.reject(
                    request_sequence,
                    &operation.name,
                    AuditOutcome::InvalidArguments,
                )
                .await?;
                return Err(error.into());
            }
        };
        let _permit = match self.permits.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                self.reject(request_sequence, &operation.name, AuditOutcome::Busy)
                    .await?;
                return Err(RuntimeError::Busy);
            }
        };
        self.record_audit(AuditEvent::new(
            request_sequence,
            AuditPhase::Start,
            &operation.name,
            AuditOutcome::Attempt,
            None,
        ))
        .await?;

        let started = Instant::now();
        let result = self
            .backend
            .execute(&invocation, &self.limits)
            .await
            .map(|output| operation.project(&output));
        let outcome = if result.is_ok() {
            AuditOutcome::Success
        } else {
            AuditOutcome::Failed
        };
        self.record_audit(AuditEvent::new(
            request_sequence,
            AuditPhase::Finish,
            &operation.name,
            outcome,
            Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
        ))
        .await
        .map_err(|_| RuntimeError::CompletionAuditFailed)?;
        result
    }

    async fn reject(
        &self,
        sequence: u64,
        operation: &str,
        outcome: AuditOutcome,
    ) -> Result<(), RuntimeError> {
        self.record_audit(AuditEvent::new(
            sequence,
            AuditPhase::Rejection,
            operation,
            outcome,
            None,
        ))
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
