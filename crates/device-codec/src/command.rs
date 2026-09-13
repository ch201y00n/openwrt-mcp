use openwrt_mcp_core::{PreparedAction, ProbeRequest};
use std::io::{Error, Result as IoResult, Write};

use crate::CodecError;

const MAX_COMMAND_BYTES: usize = 256 * 1024;

/// Construction is private: both local argv and SSH quoting consume validated data.
pub struct CommandSpec {
    pub(crate) program: String,
    pub(crate) arguments: Vec<String>,
}

impl CommandSpec {
    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
}

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
    fn append(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        self.write_all(bytes).map_err(|_| CodecError::InvalidAction)
    }

    fn argument(&mut self, value: &str) -> Result<(), CodecError> {
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

pub(crate) fn identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

pub(crate) fn argument_name(value: &str) -> bool {
    identifier(value, 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn arguments_valid(arguments: &serde_json::Value) -> bool {
    let Some(arguments) = arguments.as_object() else {
        return false;
    };
    arguments.len() <= 64
        && arguments.iter().all(|(key, value)| {
            argument_name(key)
                && match value {
                    serde_json::Value::String(value) => {
                        value.len() <= 1024 && !value.contains('\0')
                    }
                    serde_json::Value::Number(value) => value
                        .as_i64()
                        .is_some_and(|value| i32::try_from(value).is_ok()),
                    serde_json::Value::Bool(_) => true,
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
            .all(|byte| byte.is_ascii_alphanumeric() || b"/_.-".contains(&byte))
        && !value.split('/').any(|part| part == "." || part == "..")
}

/// Validate even public PreparedActions constructed without the domain catalog.
pub fn compile_action(action: &PreparedAction) -> Result<CommandSpec, CodecError> {
    let command = match action {
        PreparedAction::ApkInstalledPage { .. } => return Err(CodecError::InvalidAction),
        PreparedAction::Ubus {
            object,
            method,
            arguments,
        } => {
            if !identifier(object, 128) || !identifier(method, 128) || !arguments_valid(arguments) {
                return Err(CodecError::InvalidAction);
            }
            let mut json = CommandBuffer(Vec::new());
            serde_json::to_writer(&mut json, arguments).map_err(|_| CodecError::InvalidAction)?;
            let json = String::from_utf8(json.0).map_err(|_| CodecError::InvalidAction)?;
            CommandSpec {
                program: "/bin/ubus".into(),
                arguments: vec![
                    "-S".into(),
                    "call".into(),
                    object.clone(),
                    method.clone(),
                    json,
                ],
            }
        }
        PreparedAction::Process { program, args } => {
            if !program_path(program)
                || args.len() > 64
                || args
                    .iter()
                    .any(|value| value.len() > 1024 || value.contains('\0'))
            {
                return Err(CodecError::InvalidAction);
            }
            CommandSpec {
                program: program.clone(),
                arguments: args.clone(),
            }
        }
    };
    // Enforce the same bound before local spawning or remote encoding. Counting
    // is checked and allocation-free, including JSON/POSIX single-quote expansion.
    let size = std::iter::once(command.program())
        .chain(command.arguments.iter().map(String::as_str))
        .try_fold(4_usize, |total, value| {
            let quotes = value.bytes().filter(|byte| *byte == b'\'').count();
            total
                .checked_add(value.len())?
                .checked_add(3)?
                .checked_add(quotes.checked_mul(3)?)
        });
    if size.is_none_or(|size| size > MAX_COMMAND_BYTES) {
        return Err(CodecError::InvalidAction);
    }
    Ok(command)
}

/// The closed request cannot select another executable, switch or wildcard object.
pub fn compile_probe(request: ProbeRequest) -> CommandSpec {
    let ProbeRequest::DescribeUbusObject(object) = request;
    CommandSpec {
        program: "/bin/ubus".into(),
        arguments: vec!["-v".into(), "list".into(), object.as_str().into()],
    }
}

/// SSH exec invokes the target shell; every element is one literal POSIX argument.
pub fn encode_remote(command: &CommandSpec) -> Result<Vec<u8>, CodecError> {
    let mut output = CommandBuffer(b"exec".to_vec());
    output.argument(command.program())?;
    for argument in command.arguments() {
        output.argument(argument)?;
    }
    Ok(output.0)
}
