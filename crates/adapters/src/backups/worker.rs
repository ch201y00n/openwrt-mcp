use openwrt_mcp_runtime::{backups::BackupError, mutation_ports::WorkBudget};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
};
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Drop cooperatively cancels and joins. OS-blocked I/O may delay shutdown, but
/// cannot release the slot or detach an accumulating background task.
pub struct BackupWorker<T> {
    handle: Option<JoinHandle<Result<T, BackupError>>>,
    budget: WorkBudget,
}
impl<T: Send + 'static> BackupWorker<T> {
    pub fn spawn<F>(budget: WorkBudget, work: F) -> Result<Self, BackupError>
    where
        F: FnOnce(WorkBudget) -> Result<T, BackupError> + Send + 'static,
    {
        budget.check()?;
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| BackupError::Busy)?;
        let inner = budget.clone();
        let handle = match thread::Builder::new()
            .name("openwrt-backup".into())
            .spawn(move || {
                inner.check()?;
                let result = work(inner.clone());
                inner.check()?;
                result
            }) {
            Ok(handle) => handle,
            Err(_) => {
                ACTIVE.store(false, Ordering::Release);
                return Err(BackupError::Unavailable);
            }
        };
        Ok(Self {
            handle: Some(handle),
            budget,
        })
    }
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn cancel(&self) {
        self.budget.cancel();
    }
    pub fn join(mut self) -> Result<T, BackupError> {
        self.join_inner()
    }
}
impl<T> BackupWorker<T> {
    fn join_inner(&mut self) -> Result<T, BackupError> {
        let Some(handle) = self.handle.take() else {
            return Err(BackupError::Invalid);
        };
        let result = handle.join().map_err(|_| BackupError::Unavailable);
        ACTIVE.store(false, Ordering::Release);
        result?
    }
}
impl<T> Drop for BackupWorker<T> {
    fn drop(&mut self) {
        if self.handle.is_some() {
            self.budget.cancel();
            let _ = self.join_inner();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, atomic::AtomicUsize},
        time::Duration,
    };
    #[test]
    fn worker_capacity_cancellation_drop_panic_and_join_do_not_detach() {
        let budget = || WorkBudget::new(Duration::from_secs(2)).unwrap();
        let dropped = Arc::new(AtomicUsize::new(0));
        struct Marker(Arc<AtomicUsize>);
        impl Drop for Marker {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let marker = Marker(dropped.clone());
        let worker = BackupWorker::spawn(budget(), move |budget| {
            let _marker = marker;
            while budget.check().is_ok() {
                thread::yield_now();
            }
            Err::<(), _>(BackupError::Cancelled)
        })
        .unwrap();
        assert!(matches!(
            BackupWorker::spawn(budget(), |_| Ok(())),
            Err(BackupError::Busy)
        ));
        drop(worker);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(
            BackupWorker::spawn(budget(), |_| Ok(42))
                .unwrap()
                .join()
                .unwrap(),
            42
        );
        let panicking =
            BackupWorker::<()>::spawn(budget(), |_| panic!("synthetic worker fault")).unwrap();
        assert_eq!(panicking.join(), Err(BackupError::Unavailable));
        assert!(
            BackupWorker::spawn(budget(), |_| Ok(()))
                .unwrap()
                .join()
                .is_ok()
        );
        let expired = WorkBudget::new(Duration::from_millis(1)).unwrap();
        thread::sleep(Duration::from_millis(3));
        assert!(matches!(
            BackupWorker::spawn(expired, |_| Ok(())),
            Err(BackupError::Deadline)
        ));
    }
}
