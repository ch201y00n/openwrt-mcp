# OpenWrt MCP

A lightweight Rust MCP server for OpenWrt management, with category-based permissions and configurable audit logging.

[한국어 안내](README.ko.md)

**Status: v0.1 foundation, under development.** Independent community project; not affiliated with or endorsed by OpenWrt. Full OpenWrt feature coverage is the product goal, not a claim about this initial implementation. See [requirements](docs/requirements.md), [architecture](docs/architecture.md), and [coverage](docs/coverage.md).

## What works in this foundation

- Standard MCP over stdio using the official Rust SDK.
- Operator-owned category access: deny, read, read_write, plus independent execute permission.
- Authorization enforced on every call, with the same filtering for tool discovery.
- Five conservative, structured ubus read operations; fixed-action operator extensions.
- Audit attempts and outcomes without raw arguments, configuration or device payloads.
- JSON/text audit output to stderr, rotating files, or Unix syslog.
- Bounded process output, deadlines, input frames and backend concurrency.
- Separate core, runtime and server crates with automated dependency checks.

This version has no built-in configuration mutation, firmware upgrade, encrypted backup/rollback workflow, web UI or remote HTTP listener. Device-side deployment and real OpenWrt validation are still pending. Custom actions are privileged operator definitions, not a substitute for tested feature adapters.

## Build and check

Use Rust 1.95 or newer and a C linker. Build on a workstation, not a router:

```sh
cargo build --locked --release
cargo run --locked -p openwrt-mcp -- check --config config/read-only.toml
cargo run --locked -p openwrt-mcp -- catalog --config config/read-only.toml
```

`check` and `catalog` are offline. No router connection is attempted. Check validates configuration structure; audit destination access is checked when serving. Config files must be regular files, not symlinks; on Unix they must not be writable by group/others. On WSL-mounted Windows drives, Unix mode emulation can make the checked-in file appear writable by everyone; use a copy in an operator-owned Linux directory with mode 0600 for runtime commands, or enable WSL metadata. Do not weaken permission checks.

## Deployment model

Run the binary on an OpenWrt device with `/bin/ubus`. The MCP client owns the stdio process, directly or through SSH:

```sh
openwrt-mcp serve --config /etc/openwrt-mcp/config.toml
```

A client can start `ssh -T <router-alias> /usr/bin/openwrt-mcp serve --config /etc/openwrt-mcp/config.toml` once the operator has deployed a matching binary and configured trusted SSH host keys and authentication. The alias belongs in the client's SSH configuration. This is a persistent stdio connection; there is no SSH connection per tool call. No router deployment has been performed by this repository setup.

Only JSON-RPC goes to stdout. Audit and operational messages use their configured destination (stderr by default). One process/config represents one principal; client metadata never selects privileges. On-device builds require the correct OpenWrt SDK/musl target and linker. The host verification binary is not an OpenWrt release artifact.

## Policy example

```toml
[policy.categories.system]
access = "read"
execute = false

[policy.categories.network]
access = "read_write"
execute = false
```

This grants system reads and network read/write capability, but no execution. It does not create write tools that are not implemented. Missing categories are denied. `access = "deny"` also blocks execution even if execute is true. Read/write does not imply execute; actions can require multiple categories and permissions.

Start from [read-only.toml](config/read-only.toml) or [deny-all.toml](config/deny-all.toml). See [security](docs/security.md) for authority boundaries and [configuration](docs/configuration.md) for audit settings and extensions.

## Development

```powershell
./tools/Test-Repository.ps1
```

Or on Linux/macOS with the toolchain installed:

```sh
sh tools/test.sh
```

Verification uses fake backends and synthetic data. It never accesses a live router. Results and unmeasured device targets are tracked in [validation](docs/validation.md). The repository does not yet declare a redistribution license; choose one before the first public code release.
