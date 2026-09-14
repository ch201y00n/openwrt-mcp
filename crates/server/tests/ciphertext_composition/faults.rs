use super::*;
use openwrt_mcp_adapters::backups::NativeRecordIo;
use openwrt_mcp_runtime::backups::{CiphertextRecordIo, MAX_STORE_BYTES, MacPurpose};

#[derive(Clone, Copy)]
enum Fault {
    Full,
    Partial,
    Sync,
    Ack,
    Cancel,
}
struct FaultIo {
    inner: NativeRecordIo,
    fault: Fault,
    budget: WorkBudget,
    writes: Arc<AtomicU64>,
}
impl CiphertextRecordIo for FaultIo {
    fn size(&self) -> Result<u64, BackupError> {
        self.inner.size()
    }
    fn read_at(&mut self, offset: u64, bytes: &mut [u8]) -> Result<(), BackupError> {
        self.inner.read_at(offset, bytes)
    }
    fn append(&mut self, size: u64, bytes: &[u8]) -> Result<(), BackupError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        match self.fault {
            Fault::Full => Err(BackupError::Unknown),
            Fault::Partial => {
                self.inner.append(size, &bytes[..5.min(bytes.len())])?;
                Err(BackupError::Unknown)
            }
            _ => self.inner.append(size, bytes),
        }
    }
    fn synchronize(&mut self) -> Result<(), BackupError> {
        if matches!(self.fault, Fault::Sync) {
            return Err(BackupError::Unknown);
        }
        self.inner.synchronize()?;
        if matches!(self.fault, Fault::Cancel) {
            self.budget.cancel();
            // A successful sync followed by cancellation is distinct from a
            // failed sync/lost acknowledgement; the budget check must catch it.
            return Ok(());
        }
        Err(BackupError::Unknown)
    }
}

#[test]
fn real_file_partial_write_full_sync_lost_ack_and_cancel_preserve_uncertainty() {
    let crypto = Crypto::new();
    let bytes = tar(b"synthetic");
    let bound = binding(9, bytes.len(), ArchiveFormat::Tar, 4);
    for fault in [
        Fault::Full,
        Fault::Partial,
        Fault::Sync,
        Fault::Ack,
        Fault::Cancel,
    ] {
        let file = StoreFile::new(crypto.auth.as_ref());
        let work = budget();
        let writes = Arc::new(AtomicU64::new(0));
        let io = FaultIo {
            inner: NativeRecordIo::open(&file.path).unwrap(),
            fault,
            budget: work.clone(),
            writes: writes.clone(),
        };
        let mut store =
            RecordStore::from_io(Box::new(io), [1; 16], crypto.auth.clone(), &work).unwrap();
        let result = seal_capture(
            capture(&bound, bytes.clone()),
            &crypto.encrypt,
            &mut store,
            work,
        );
        assert_eq!(result.unwrap_err().publication, RecordPublication::Unknown);
        let count = writes.load(Ordering::SeqCst);
        assert_eq!(
            store.reconcile(&bound, &budget()),
            if matches!(fault, Fault::Cancel) {
                // Fresh reconciliation has its own budget: the previous job's
                // cancellation cannot hide a now-authenticated successful sync.
                RecordPublication::Durable
            } else {
                RecordPublication::Unknown
            }
        );
        assert_eq!(writes.load(Ordering::SeqCst), count);
        drop(store);
        let reopened = RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget());
        match fault {
            Fault::Full => assert_eq!(
                reopened.unwrap().reconcile(&bound, &budget()),
                RecordPublication::Unknown
            ),
            Fault::Partial => assert!(reopened.is_err()),
            _ => {
                let mut reopened = reopened.unwrap();
                assert_eq!(
                    reopened.reconcile(&bound, &budget()),
                    RecordPublication::Durable
                );
                assert!(restore_inspect(&mut reopened, &bound, &crypto.decrypt, budget()).is_ok());
            }
        }
    }
}

fn rewrite_authenticated_record(
    original: &[u8],
    binding: &ArtifactBinding,
    ciphertext: &[u8],
    auth: &dyn RecordAuthenticator,
) -> Vec<u8> {
    let mut record = b"OMCPBK01".to_vec();
    record.extend_from_slice(binding.encoded());
    record.extend_from_slice(&(ciphertext.len() as u64).to_be_bytes());
    let tag = auth
        .tag(MacPurpose::ArtifactRecord, &[&record, ciphertext])
        .unwrap();
    record.extend_from_slice(ciphertext);
    record.extend_from_slice(&tag);
    let mut store = original[..64].to_vec();
    store.extend_from_slice(&record);
    store
}

#[test]
fn valid_store_mac_is_not_age_authentication_or_manifest_authority() {
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let bytes = tar(&vec![b's'; 65536]);
    let bound = binding(65536, bytes.len(), ArchiveFormat::Tar, 4);
    let mut store = file.open(&crypto);
    seal_capture(
        capture(&bound, bytes),
        &crypto.encrypt,
        &mut store,
        budget(),
    )
    .unwrap();
    drop(store);
    let original = fs::read(&file.path).unwrap();
    let cipher = &original[168..original.len() - 32];
    let mut changed = cipher.to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    let mut trailing = cipher.to_vec();
    trailing.extend_from_slice(b"suffix");
    for bad in [
        changed,
        trailing,
        cipher[..cipher.len() - 1].to_vec(),
        cipher[..cipher.len() - 32].to_vec(),
    ] {
        fs::write(
            &file.path,
            rewrite_authenticated_record(&original, &bound, &bad, crypto.auth.as_ref()),
        )
        .unwrap();
        let mut store = file.open(&crypto);
        assert!(restore_inspect(&mut store, &bound, &crypto.decrypt, budget()).is_err());
    }
    let wrong = binding(65535, bound.source_bytes() as usize, ArchiveFormat::Tar, 4);
    fs::write(
        &file.path,
        rewrite_authenticated_record(&original, &wrong, cipher, crypto.auth.as_ref()),
    )
    .unwrap();
    let mut store = file.open(&crypto);
    assert!(restore_inspect(&mut store, &wrong, &crypto.decrypt, budget()).is_err());
}

#[test]
fn hard_limits_unknown_header_and_cancel_before_work_do_not_modify_store() {
    assert!(
        ArtifactBinding::system(
            [1; 16],
            [2; 16],
            [3; 16],
            [4; 16],
            65537,
            2048,
            ArchiveFormat::Tar
        )
        .is_err()
    );
    assert!(
        ArtifactBinding::system(
            [1; 16],
            [2; 16],
            [3; 16],
            [4; 16],
            1,
            131073,
            ArchiveFormat::Tar
        )
        .is_err()
    );
    assert!(WorkBudget::new(Duration::ZERO).is_err());
    assert!(WorkBudget::new(Duration::from_secs(31)).is_err());
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let bytes = tar(b"synthetic");
    let bound = binding(9, bytes.len(), ArchiveFormat::Tar, 4);
    let mut store = file.open(&crypto);
    let cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        seal_capture(
            capture(&bound, bytes),
            &crypto.encrypt,
            &mut store,
            cancelled
        )
        .unwrap_err()
        .publication,
        RecordPublication::NotPublished
    );
    drop(store);
    assert_eq!(fs::metadata(&file.path).unwrap().len(), 64);
    fs::write(
        &file.path,
        b"unrelated synthetic file, not an authenticated store",
    )
    .unwrap();
    let before = fs::read(&file.path).unwrap();
    assert!(RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget()).is_err());
    assert!(fs::read(&file.path).unwrap() == before);
    File::options()
        .write(true)
        .open(&file.path)
        .unwrap()
        .set_len(MAX_STORE_BYTES + 1)
        .unwrap();
    assert!(RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget()).is_err());
}

#[test]
fn finite_record_capacity_and_duplicate_ids_never_overwrite_or_evict() {
    let crypto = Crypto::new();
    let file = StoreFile::new(crypto.auth.as_ref());
    let bytes = tar(&[]);
    let mut store = file.open(&crypto);
    for job in 1..=32 {
        let bound = binding(0, bytes.len(), ArchiveFormat::Tar, job);
        seal_capture(
            capture(&bound, bytes.clone()),
            &crypto.encrypt,
            &mut store,
            budget(),
        )
        .unwrap();
    }
    let extra = binding(0, bytes.len(), ArchiveFormat::Tar, 33);
    assert_eq!(
        seal_capture(
            capture(&extra, bytes.clone()),
            &crypto.encrypt,
            &mut store,
            budget()
        )
        .unwrap_err()
        .publication,
        RecordPublication::NotPublished
    );
    let first = binding(0, bytes.len(), ArchiveFormat::Tar, 1);
    assert!(restore_inspect(&mut store, &first, &crypto.decrypt, budget()).is_ok());
    drop(store);
    let mut duplicated = fs::read(&file.path).unwrap();
    let first_length = u64::from_be_bytes(duplicated[160..168].try_into().unwrap()) as usize + 136;
    let first_record = duplicated[64..64 + first_length].to_vec();
    duplicated.truncate(64 + first_length);
    duplicated.extend_from_slice(&first_record);
    fs::write(&file.path, duplicated).unwrap();
    assert!(RecordStore::open(&file.path, [1; 16], crypto.auth.clone(), &budget()).is_err());
}
