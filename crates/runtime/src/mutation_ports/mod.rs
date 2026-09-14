//! Private infrastructure ports. These types confer no client mutation authority.
pub mod secrets;
use async_trait::async_trait;
use secrets::{SecretReference, SecretValue};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationError {
    Invalid,
    Limit,
    Cancelled,
    Deadline,
    Busy,
    Unavailable,
    Integrity,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPurpose {
    WirelessPassword,
    WireGuardPrivateKey,
    ServiceToken,
}

#[derive(Clone)]
pub struct WorkBudget {
    inner: Arc<BudgetState>,
}
struct BudgetState {
    deadline: Instant,
    cancelled: AtomicBool,
}
impl WorkBudget {
    pub fn new(timeout: Duration) -> Result<Self, MutationError> {
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(MutationError::Invalid);
        }
        Ok(Self {
            inner: Arc::new(BudgetState {
                deadline: Instant::now() + timeout,
                cancelled: AtomicBool::new(false),
            }),
        })
    }
    pub fn check(&self) -> Result<(), MutationError> {
        if self.inner.cancelled.load(Ordering::Acquire) {
            Err(MutationError::Cancelled)
        } else if Instant::now() >= self.inner.deadline {
            Err(MutationError::Deadline)
        } else {
            Ok(())
        }
    }
    pub fn deadline(&self) -> Instant {
        self.inner.deadline
    }
    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);
    }
}

#[async_trait]
pub trait SecretSource: Send + Sync {
    async fn resolve(
        &self,
        reference: &SecretReference,
        purpose: SecretPurpose,
        budget: WorkBudget,
    ) -> Result<SecretValue, MutationError>;
}
