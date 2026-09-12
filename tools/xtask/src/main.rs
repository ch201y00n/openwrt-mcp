use std::path::Path;

fn main() -> std::process::ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let baseline = match arguments.as_slice() {
        [command] if command == "architecture" => None,
        [command, flag, value] if command == "architecture" && flag == "--base" => {
            Some(value.as_str())
        }
        _ => {
            eprintln!("usage: cargo run --locked -p xtask -- architecture [--base <commit>]");
            return std::process::ExitCode::FAILURE;
        }
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match xtask::architecture(&root, baseline) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("architecture check failed: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
