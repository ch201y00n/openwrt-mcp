//! Measure an initialized, idle Linux stdio server without contacting a router.
#![cfg_attr(not(target_os = "linux"), allow(unused_imports))]
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    process::Stdio,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

#[cfg(target_os = "linux")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;
    let binary = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a release binary path")?,
    );
    let sample_seconds = std::env::args()
        .nth(2)
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(1);
    if !(1..=60).contains(&sample_seconds) {
        return Err("sample duration must be 1 to 60 seconds".into());
    }
    // Linux exposes process CPU in USER_HZ ticks, not wall-clock milliseconds.
    // This diagnostic is host-only; no helper is invoked by the server.
    let ticks = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("/usr/bin/getconf")
            .arg("CLK_TCK")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    if !ticks.status.success() {
        return Err("cannot determine host clock ticks".into());
    }
    let ticks_per_second = std::str::from_utf8(&ticks.stdout)?.trim().parse::<u64>()?;
    if ticks_per_second == 0 {
        return Err("invalid host clock ticks".into());
    }
    let dir = std::env::temp_dir().canonicalize()?.join(format!(
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
    let before = cpu_ticks(pid)?;
    let sample_started = Instant::now();
    tokio::time::sleep(Duration::from_secs(sample_seconds)).await;
    let after = cpu_ticks(pid)?;
    let elapsed_seconds = sample_started.elapsed().as_secs_f64();
    let cpu_ticks = after.checked_sub(before).ok_or("CPU counter decreased")?;
    let cpu_percent_one_core = cpu_ticks as f64 / ticks_per_second as f64 / elapsed_seconds * 100.0;
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
        json!({"kind":"linux_host_idle_sample","binary_bytes":binary_bytes,"rss_kib":rss,"threads":threads,"warmup_seconds":1,"sample_seconds":elapsed_seconds,"cpu_ticks":cpu_ticks,"clock_ticks_per_second":ticks_per_second,"cpu_percent_one_core":cpu_percent_one_core,"audit":"enabled; stderr connected to null","router_calls":0,"cpu_measured":true})
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn cpu_ticks(pid: u32) -> Result<u64, Box<dyn std::error::Error>> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    // comm (field 2) may contain whitespace and parentheses. Numeric fields
    // begin after the final ')'; utime/stime are fields 14/15 (indices 11/12).
    let (_, fields) = stat.rsplit_once(')').ok_or("invalid process stat")?;
    let mut fields = fields.split_whitespace().skip(11);
    let user = fields.next().ok_or("missing user CPU")?.parse::<u64>()?;
    let system = fields.next().ok_or("missing system CPU")?.parse::<u64>()?;
    user.checked_add(system)
        .ok_or_else(|| "CPU counter overflow".into())
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("unsupported_diagnostic: this measurement requires Linux /proc");
    std::process::exit(1);
}
