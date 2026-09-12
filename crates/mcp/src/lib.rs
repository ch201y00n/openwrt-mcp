//! MCP framing and protocol. Every device operation is routed through Dispatcher.

mod framing;
mod protocol;
mod result;

pub use framing::{BoundedInput, MAX_FRAME_BYTES};
pub use protocol::McpServer;
pub use result::MAX_CALL_RESULT_BYTES;
