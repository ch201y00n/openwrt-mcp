//! MCP framing and protocol. Every device operation is routed through Dispatcher.

mod framing;
mod protocol;

pub use framing::{BoundedInput, MAX_FRAME_BYTES};
pub use protocol::McpServer;
