# OpenWrt MCP

A lightweight Rust MCP server for OpenWrt management, with category-based permissions and configurable audit logging.

[한국어 안내](README.ko.md)

**Status: v0.1 foundation, under development.** Independent community project; not affiliated with or endorsed by OpenWrt. Full OpenWrt feature coverage is the product goal, not a claim about this initial implementation. See [requirements](docs/requirements.md), [architecture](docs/architecture.md), and [coverage](docs/coverage.md).

## What works in this foundation

- Standard MCP over stdio using the official Rust SDK.
- Operator-owned category access: deny, read, read_write, plus independent execute permission.
- Authorization enforced on every call, with the same filtering for tool discovery.
- Fourteen conservative ubus read operations across system, network, wireless, services and diagnostics; fixed-action operator extensions.
- Five typed response contracts for bounded interface, wireless-device and service collections, including exact local interface/service selection. [Read contracts](docs/collection-read-contracts.md).
- Same-target input-signature discovery, fail-closed compatibility checks, 30-second bounded cache and an authorized `operation_capability` metadata tool. [Limits and version differences](docs/capabilities.md).
- Audit attempts and outcomes without raw arguments, configuration or device payloads.
- JSON/text audit output to stderr, or optional protected Linux rotating files/syslog.
- Explicit OpenWrt targets: unconfigured by default, verified on-device execution, or native persistent SSH independent of the workstation OS.
- Bounded process output, deadlines, input frames and backend concurrency; strict action JSON decoding and bounded normalized/MCP tool results.
- Separate core, features, runtime, adapters, MCP and composition crates, plus a development-only architecture harness.
- Internal age primitives with independent key-source/container adapters and public/private key separation. See [key custody and platform limits](docs/key-management.md).

This version has no built-in configuration mutation, firmware upgrade, encrypted backup/rollback workflow, web UI or remote HTTP listener. Physical-device deployment and full acceptance are still pending. Custom actions are privileged operator definitions, not a substitute for tested feature adapters; unverified Process extensions and Ubus prerequisites without reviewed probes are blocked.

Full gates pass 347 distinct tests on Linux-on-WSL and 315 on [native Windows GNU](docs/windows-validation.md). A separate actual MCP/SSH run against isolated official OpenWrt 25.12.5 ARM64 QEMU validated twelve reads, including five typed contracts, and two explicit unavailable/error cases. See [scoped v6 emulated acceptance](docs/emulator-validation-v6.md) and [verification and measurements](docs/validation.md). BPI-R4 hardware, MSVC and macOS acceptance remain pending.

## Build and check

Use Rust 1.95 or newer, a C compiler and a linker (the SSH crypto backend includes native code). Build on a workstation, not a router:

```sh
cargo build --locked --release
cargo run --locked -p openwrt-mcp -- check --config config/read-only.toml
cargo run --locked -p openwrt-mcp -- catalog --config config/read-only.toml
```

`check` and `catalog` are offline and never read SSH or age key sources. The file examples above require the implemented Linux protected-file profile and trusted parent directories. They are not native Windows/macOS instructions. For the common host path, explicitly provision the TOML configuration in an operator-managed environment variable and use `--config-env OPENWRT_MCP_CONFIG`. Do not put private keys in that TOML or command history. No `.env` discovery or fallback occurs. See the [platform support and evidence matrix](docs/platform-support.md).

## Deployment model

Run the binary on a Windows/Linux/macOS workstation with an explicit SSH target, as illustrated by [ssh-environment.toml](config/ssh-environment.toml). Provision the configuration and separate authentication source securely, then let the MCP client own the stdio process:

```sh
openwrt-mcp serve --config-env OPENWRT_MCP_CONFIG
```

The built-in SSH backend uses a pinned Ed25519 host key, a separate Ed25519 authentication source, and one reused connection. It never invokes a host SSH executable or falls back to local execution. A configuration without a target cannot execute any device program. Alternatively deploy a matching binary on OpenWrt and explicitly select `[target] kind = "openwrt_local"`; the constructor verifies the host before local execution. No router deployment has been performed. Windows GNU common-path fixtures pass; MSVC/macOS acceptance, Windows/macOS protected files and OpenWrt hardware acceptance remain pending. The required three-host CI has not been run remotely.

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

Start from [read-only.toml](config/read-only.toml) for system/network only, [observability.toml](config/observability.toml) to opt into all currently implemented read categories, or [deny-all.toml](config/deny-all.toml). No example grants execute permission. See [security](docs/security.md) for authority boundaries and [configuration](docs/configuration.md) for audit settings and extensions.

Generic Services.Read exposes service/instance names and running/PID/exit metadata across service categories, never command lines, environment or settings. To retain only the fixed logd/sysntpd views, deny `service_status` and `service_status_list` through the operation denylist.

`network_interface_status` now always uses `network.interface.dump {}` and exact local selection. Its development-stage response contract is v2: ordinary field names, including `interface`, replace the former slash-prefixed keys. It does not retry or bypass an incomplete `status` signature.

## Development

Follow the [architecture-first workflow](docs/development.md). [Architecture contract v6](architecture/spec.toml) specifies directories, dependencies, portable layers, capability/evidence contracts, finite projections, response limits and mandatory native host tests. The capability and bounded-collection designs were separately checkpointed before functional work; their required suites now contain behavioral tests. Incompatible requirements must update the requirements, ADR, architecture and harness before feature implementation. The full gate checks evolution against HEAD locally and the change base in CI. WSL validation requires explicit `-UseWsl` and counts as Linux only.

```powershell
./tools/Test-Repository.ps1
```

Or on Linux/macOS with the toolchain installed:

```sh
sh tools/test.sh
```

Verification uses fake backends and synthetic data. It never accesses a live router. Results and unmeasured device targets are tracked in [validation](docs/validation.md). The repository does not yet declare a redistribution license; choose one before the first public code release.
