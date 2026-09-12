use openwrt_mcp::config::{Config, LogLevel, Logging};
use openwrt_mcp_adapters::{AuditWriter, LocalBackend};
use openwrt_mcp_runtime::Dispatcher;
use openwrt_mcp_transport::{BoundedInput, McpServer};
use std::{path::PathBuf, sync::Arc, time::Duration};

const HELP: &str = "OpenWrt MCP\nUsage: openwrt-mcp <check|catalog|serve> --config <path>\n\ncheck    Validate policy, catalog and limits without contacting a router\ncatalog  Print names and schemas of authorized operations without contacting a router\nserve    Run the stdio MCP server against this local OpenWrt device\n\nNo network listener is opened. Configuration is local operator authority.\n";

fn arguments() -> Result<Option<(String, PathBuf)>, &'static str> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        print!("{HELP}");
        return Ok(None);
    }
    if args == ["--version"] {
        println!("openwrt-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    if args.len() != 3 || args[1] != "--config" {
        return Err("invalid_cli_arguments");
    }
    let command = args[0].to_str().ok_or("invalid_cli_arguments")?;
    if !matches!(command, "check" | "catalog" | "serve") {
        return Err("invalid_cli_command");
    }
    Ok(Some((command.to_owned(), PathBuf::from(&args[2]))))
}

fn main() -> std::process::ExitCode {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return std::process::ExitCode::FAILURE;
    };
    let exit = match runtime.block_on(run()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(code) => {
            let _ = runtime.block_on(Logging::default().event(LogLevel::Error, code));
            std::process::ExitCode::FAILURE
        }
    };
    // A blocked OS audit write cannot be force-cancelled. Do not wait forever on exit.
    runtime.shutdown_timeout(Duration::from_millis(100));
    exit
}

async fn run() -> Result<(), &'static str> {
    let Some((command, path)) = arguments()? else {
        return Ok(());
    };
    let config = Config::load(&path)?;
    let catalog = config.catalog()?;
    if command == "check" {
        println!(
            "configuration valid; {} authorized operations",
            catalog
                .operations()
                .iter()
                .filter(|op| config.policy.authorize(op).is_ok())
                .count()
        );
        return Ok(());
    }
    if command == "catalog" {
        let operations: Vec<_> = catalog.operations().iter().filter(|op| config.policy.authorize(op).is_ok()).map(|op| serde_json::json!({"name": op.name, "description": op.description, "requirements": op.requirements, "input_schema": op.input_schema()})).collect();
        println!("{}", serde_json::json!({"operations": operations}));
        return Ok(());
    }
    let audit = AuditWriter::new(config.audit).map_err(|_| "audit_initialization_failed")?;
    let dispatcher = Dispatcher::new(
        catalog,
        config.policy,
        Arc::new(LocalBackend),
        Arc::new(audit),
        config.limits,
    )
    .map_err(|_| "dispatcher_initialization_failed")?;
    config
        .logging
        .event(LogLevel::Info, "server_starting")
        .await?;
    McpServer::new(dispatcher)
        .run(BoundedInput::new(tokio::io::stdin()), tokio::io::stdout())
        .await?;
    config
        .logging
        .event(LogLevel::Info, "server_stopped")
        .await?;
    Ok(())
}
