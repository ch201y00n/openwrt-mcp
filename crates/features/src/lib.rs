//! Reviewed feature specifications. This crate never performs device I/O.
mod categories;
mod definition;

use openwrt_mcp_core::{Catalog, CoreError, Operation};

pub fn builtins() -> Vec<Operation> {
    let mut operations = categories::system::operations();
    operations.extend(categories::network::operations());
    operations.extend(categories::wireless::operations());
    operations.extend(categories::services::operations());
    operations.extend(categories::diagnostics::operations());
    operations.extend(categories::packages::operations());
    operations.extend(categories::storage::operations());
    operations.extend(categories::dhcp_dns::operations());
    operations.extend(categories::firewall::operations());
    operations
}

pub fn catalog(custom: Vec<Operation>) -> Result<Catalog, CoreError> {
    Catalog::with_builtins(builtins(), custom)
}
