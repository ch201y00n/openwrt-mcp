//! Portable codecs over provided data only: no target access or policy decisions.
//! Command templates remain router POSIX data on Windows, Linux and macOS hosts.

mod command;
mod describe;
pub mod packages;
mod response;

pub use command::{CommandSpec, compile_action, compile_probe, encode_remote};
pub use describe::{MAX_PROBE_BYTES, parse_ubus_describe};
pub use response::{MAX_ACTION_BYTES, MAX_ACTION_DEPTH, MAX_ACTION_NODES, parse_action_response};

/// Fixed failures never contain arguments, device output, or serializer details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    InvalidAction,
    OutputLimit,
    InvalidObservation,
    InvalidResponse,
}
