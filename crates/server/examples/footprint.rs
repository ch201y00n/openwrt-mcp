//! Measure an initialized, idle Linux stdio server without contacting a router.
#![cfg_attr(not(unix), allow(unused_imports))]
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let binary = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a release binary path")?,
    );
    let dir = std::env::temp_dir().join(format!(
        "openwrt-mcp-footprint-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    let config = dir.join("config.toml");
    fs::write(&config, "# default deny, default audit enabled\n")?;
    fs::set_permissions(&config, fs::Permissions::from_mode(0o600))?;
    let mut child = Command::new(&binary)
        .arg("serve")
        .arg("--config")
        .arg(&config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let pid = child.id().ok_or("no child pid")?;
    let mut input = child.stdin.take().ok_or("no stdin")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("no stdout")?);
    input.write_all(format!("{}\n", json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"footprint-fixture","version":"1"}}})).as_bytes()).await?;
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut line)).await??;
    let hello: Value = serde_json::from_str(&line)?;
    if hello.get("result").is_none() {
        return Err("initialization failed".into());
    }
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
    let rss = status
        .lines()
        .find(|line| line.starts_with("VmRSS:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or("no RSS")?
        .parse::<u64>()?;
    let threads = status
        .lines()
        .find(|line| line.starts_with("Threads:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or("no thread count")?
        .parse::<u64>()?;
    let binary_bytes = fs::metadata(&binary)?.len();
    drop(input);
    drop(output);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait()).await??;
    fs::remove_file(config)?;
    fs::remove_dir(dir)?;
    if !status.success() {
        return Err("server exit failed".into());
    }
    println!(
        "{}",
        json!({"kind":"linux_host_idle_sample","binary_bytes":binary_bytes,"rss_kib":rss,"threads":threads,"sampling":"one second after initialize","audit":"enabled; stderr connected to null","router_calls":0,"cpu_measured":false})
    );
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("This diagnostic reads Linux /proc.");
}
