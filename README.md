# OpenWrt MCP

A lightweight Rust MCP server for OpenWrt management, with category-based permissions and configurable audit logging.

[한국어 안내](README.ko.md)

**Status: v0.1 foundation, under development.** Independent community project; not affiliated with or endorsed by OpenWrt. Full OpenWrt feature coverage is the product goal, not a claim about this initial implementation. See [requirements](docs/requirements.md), [architecture](docs/architecture.md), and [coverage](docs/coverage.md).

## What works in this foundation

- Standard MCP over stdio using the official Rust SDK.
- Operator-owned category access: deny, read, read_write, plus independent execute permission.
- Authorization enforced on every call, with the same filtering for tool discovery.
- Forty-nine conservative ubus read operations across system, network, wireless, firewall, DHCP/DNS, services, storage and diagnostics; fixed-action operator extensions excluding raw UCI.
- Two additional package observations: [APK-installed query](docs/package-observations.md) and [opkg root status file](docs/opkg-observations.md). Complete bounded captures, 16-record pages, per-page authorization/audit and one shared expiring private snapshot. Explicit source-specific non-atomic scope, not whole-device completeness or package mutation; no automatic manager fallback.
- Forty typed response contracts, including twenty-four closed UCI observations: [initial six](docs/uci-observations.md) and [eighteen base-service additions](docs/base-uci-observations.md). Other contracts cover [initial reads](docs/collection-read-contracts.md), [interface IP](docs/interface-ip-observations.md), [passive wireless](docs/wireless-observation-contracts.md), [DHCP leases](docs/dhcp-observations.md) and [scoped storage](docs/storage-observations.md). UCI reads are non-atomic shared-delta views, not committed-only or effective state.
- Same-target input-signature discovery, fail-closed compatibility checks, 30-second bounded cache and an authorized `operation_capability` metadata tool. [Limits and version differences](docs/capabilities.md).
- Network/dnsmasq configuration v2 preserves selected string/list options with fixed kind/values output, including empty forms and duplicate values. No splitting/coercion or increased global limits; [exact fields](docs/uci-observations.md).
- Audit attempts and outcomes without raw arguments, configuration or device payloads.
- JSON/text audit output to stderr, or optional protected Linux rotating files/syslog.
- Explicit OpenWrt targets: unconfigured by default, verified on-device execution, or native persistent SSH independent of the workstation OS.
- Bounded process output, deadlines, input frames and backend concurrency; strict action JSON decoding and bounded normalized/MCP tool results.
- Separate core, features, runtime, adapters, MCP and composition crates, plus a development-only architecture harness.
- Internal age primitives with independent key-source/container adapters and public/private key separation. See [key custody and platform limits](docs/key-management.md).
- Native Windows protected config/key files and exact ZIP member selection under a [closed local NTFS profile](docs/windows-protected-files.md). Personal Vault and Windows file logging remain separate, unsupported facilities.

This version has no built-in configuration mutation, firmware upgrade, encrypted backup/rollback workflow, web UI or remote HTTP listener. Physical-device deployment and full acceptance are still pending. Custom actions are privileged operator definitions, not a substitute for tested feature adapters; unverified Process extensions and Ubus prerequisites without reviewed probes are blocked.

Full gates cover Linux-on-WSL and [native Windows GNU](docs/windows-validation.md); current counts and scope are in [verification and measurements](docs/validation.md). A separate [v7 package emulator run](docs/emulator-validation-v7.md) enumerated 205 APK-visible records in 13 pages and verified replay, refresh invalidation and safe audit. The earlier [v6 OpenWrt 25.12.5 ARM64 run](docs/emulator-validation-v6.md) validated twelve reads and two explicit unavailable/error cases; it does not validate later station/country additions. BPI-R4 hardware, MSVC and macOS acceptance remain pending.

The [v10 emulator run](docs/emulator-validation-v10.md) adds scoped interface-IP and LuCI acceptance: IPv4 address rows, three mounts and valid empty route/neighbor/DNS/block/lease lists. Those empty cases are not populated-device acceptance.

The [v12 emulator run](docs/emulator-validation-v12.md) validates network/dnsmasq v2 text/list fields with fixed synthetic pending RAM sections and 66 safe audit events. Those sections were never committed or applied; this is not physical-router or mutation acceptance.

The [v13 emulator run](docs/emulator-validation-v13.md) exercises eighteen additional closed base-service UCI reads with fixed pending RAM fixtures and 198 safe audit events. Only the recorded fields/forms have emulated evidence; no configuration was committed or applied.

The [v14 opkg emulator run](docs/emulator-validation-v14.md) on official 24.10.4 enumerated 196 root-status records in 13 pages and verified 50 safe audit events, replay and cross-manager invalidation. This does not establish package health, mutation, automatic manager selection or physical-router acceptance.

## Build and check

Use Rust 1.95 or newer, a C compiler and a linker (the SSH crypto backend includes native code). Build on a workstation, not a router:

```sh
cargo build --locked --release
cargo run --locked -p openwrt-mcp -- check --config config/read-only.toml
cargo run --locked -p openwrt-mcp -- catalog --config config/read-only.toml
```

`check` and `catalog` are offline and never read SSH or age key sources. The relative file examples above require the Linux protected-file profile and trusted parent directories. Windows instead requires an absolute path in its [protected-file profile](docs/windows-protected-files.md); macOS protected files remain unsupported. For the common host path, explicitly provision TOML in an operator-managed environment variable and use `--config-env OPENWRT_MCP_CONFIG`. Do not put private keys in that TOML or command history. No `.env` discovery or fallback occurs. See the [platform support matrix](docs/platform-support.md).

## Deployment model

Run the binary on a Windows/Linux/macOS workstation with an explicit SSH target, as illustrated by [ssh-environment.toml](config/ssh-environment.toml). Provision the configuration and separate authentication source securely, then let the MCP client own the stdio process:

```sh
openwrt-mcp serve --config-env OPENWRT_MCP_CONFIG
```

The built-in SSH backend uses a pinned Ed25519 host key, a separate Ed25519 authentication source, and one reused connection. It never invokes a host SSH executable or falls back to local execution. A configuration without a target cannot execute any device program. Alternatively deploy a matching binary on OpenWrt and explicitly select `[target] kind = "openwrt_local"`; the constructor verifies the host before local execution. No router deployment has been performed. Windows GNU common-path/protected-read fixtures pass; MSVC/macOS acceptance, macOS protection, Vault and OpenWrt hardware acceptance remain pending. The required three-host CI has not been run remotely.

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

Wireless.Read includes station MAC identities and passive link metrics. Deny `wireless_stations` and `wireless_station_status` to withhold client identities. No wireless read scans, disconnects clients or changes a country; an empty driver-reported list is not proof of absence or health.

DhcpDns.Read includes LuCI-reported client IP/MAC/DUID/hostname data. Deny `dhcp_v4_leases` and `dhcp_v6_leases` to withhold these observations. Duplicate lease rows and false expiry sentinels are preserved; this is not complete lease/DNS inventory or proof of client connectivity. Storage response contracts are now v2: any present root error rejects even a mixed success/error payload; successful field shapes are unchanged.

Network.Read includes scoped address, route and netifd-managed neighbor observations through `network_interface_addresses`, `network_interface_routes` and `network_interface_neighbors`. DhcpDns.Read also includes `dhcp_interface_dns`. All select one exact interface locally, preserve omitted lists/duplicate rows and exclude raw protocol/configuration data. These are not kernel FIB/neighbor-cache completeness or DNS query tools; deny individual operations for narrower disclosure.

`network_interface_status` now always uses `network.interface.dump {}` and exact local selection. Its development-stage response contract is v2: ordinary field names, including `interface`, replace the former slash-prefixed keys. It does not retry or bypass an incomplete `status` signature.

## Development

Follow the [architecture-first workflow](docs/development.md). [Architecture contract v16](architecture/spec.toml) specifies directories, dependencies, portable layers, capability/evidence contracts, bounded scalar/list projections, closed UCI reads, paged APK/opkg observations, protected Windows reads, isolated pure effect analysis and mandatory native host tests. Each incompatible evolution was separately checkpointed before functional work; required suites contain behavioral tests. New incompatible requirements must update the requirements, ADR, architecture and harness before feature implementation. The full gate checks evolution against HEAD locally and the change base in CI. WSL validation requires explicit `-UseWsl` and counts as Linux only.

```powershell
./tools/Test-Repository.ps1
```

Or on Linux/macOS with the toolchain installed:

```sh
sh tools/test.sh
```

Verification uses fake backends and synthetic data. It never accesses a live router. Results and unmeasured device targets are tracked in [validation](docs/validation.md). The repository does not yet declare a redistribution license; choose one before the first public code release.
