//! Ciphertext infrastructure, no client admission or mutation execution.
mod archive;
mod store;
mod worker;
mod workflow;
pub use store::{NativeRecordIo, RecordStore, provisioning_header};
pub use worker::BackupWorker;
pub use workflow::{BackupFailure, restore_inspect, seal_capture};
