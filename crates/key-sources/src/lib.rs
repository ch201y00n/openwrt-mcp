//! Exact, operator-bound key sources. No crypto, extraction, fallback, or automatic unlocking.

mod archive;
mod config;
mod file;
mod registry;

pub use archive::ZipContainer;
pub use config::{ContainerFormat, SourceConfig};
pub use file::{FileProtection, NativeFileAccess, ProtectedFileAccess};
pub use registry::SourceRegistry;
