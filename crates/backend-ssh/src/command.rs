use openwrt_mcp_core::PreparedAction;
use openwrt_mcp_runtime::RuntimeError;
use std::io::{Error, Result as IoResult, Write};

/// Includes JSON encoding and POSIX quoting expansion; never allocate a raw
/// unbounded serialization before checking this limit.
const MAX_COMMAND_BYTES: usize = 256 * 1024;

struct CommandBuffer(Vec<u8>);

impl Write for CommandBuffer {
    fn write(&mut self, bytes: &[u8]) -> IoResult<usize> {
        if bytes.len() > MAX_COMMAND_BYTES - self.0.len() {
            return Err(Error::other("command_too_large"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl CommandBuffer {
    fn append(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        self.write_all(bytes)
            .map_err(|_| RuntimeError::BackendFailed)
    }

    fn argument(&mut self, value: &str) -> Result<(), RuntimeError> {
        if value.contains('\0') {
            return Err(RuntimeError::BackendFailed);
        }
        self.append(b" '")?;
        for part in value.split_inclusive('\'') {
            if let Some(part) = part.strip_suffix('\'') {
                self.append(part.as_bytes())?;
                self.append(b"'\\''")?;
            } else {
                self.append(part.as_bytes())?;
            }
        }
        self.append(b"'")
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

fn arguments_valid(arguments: &serde_json::Value) -> bool {
    let Some(arguments) = arguments.as_object() else {
        return false;
    };
    arguments.len() <= 64
        && arguments.iter().all(|(key, value)| {
            identifier(key)
                && key.len() <= 64
                && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && match value {
                    serde_json::Value::String(value) => {
                        value.len() <= 1024 && !value.contains('\0')
                    }
                    serde_json::Value::Number(value) => value.is_i64() || value.is_u64(),
                    serde_json::Value::Bool(_) | serde_json::Value::Null => true,
                    _ => false,
                }
        })
}

fn program_path(value: &str) -> bool {
    value.starts_with('/')
        && !value.ends_with('/')
        && value.len() <= 256
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_.-".contains(&b))
        && !value.split('/').any(|part| part == "." || part == "..")
}

pub(crate) fn encode(action: &PreparedAction) -> Result<Vec<u8>, RuntimeError> {
    let mut output = CommandBuffer(b"exec".to_vec());
    match action {
        PreparedAction::Ubus {
            object,
            method,
            arguments,
        } => {
            if !identifier(object) || !identifier(method) || !arguments_valid(arguments) {
                return Err(RuntimeError::BackendFailed);
            }
            let mut json = CommandBuffer(Vec::new());
            serde_json::to_writer(&mut json, arguments).map_err(|_| RuntimeError::BackendFailed)?;
            let json = std::str::from_utf8(&json.0).map_err(|_| RuntimeError::BackendFailed)?;
            for argument in ["/bin/ubus", "-S", "call", object, method, json] {
                output.argument(argument)?;
            }
        }
        PreparedAction::Process { program, args } => {
            if !program_path(program) || args.len() > 64 {
                return Err(RuntimeError::BackendFailed);
            }
            output.argument(program)?;
            for argument in args {
                if argument.len() > 1024 {
                    return Err(RuntimeError::BackendFailed);
                }
                output.argument(argument)?;
            }
        }
    }
    Ok(output.0)
}
