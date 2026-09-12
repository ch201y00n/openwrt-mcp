# Validation record

Date: 2026-09-13. This is host/fixture validation of the initial foundation, not a production or real-router acceptance report.

## Environment

- Intel Core Ultra 7 265K, x86_64.
- Ubuntu under WSL2; Linux 6.18.33.2-microsoft-standard-WSL2.
- Rust 1.97.1; Cargo 1.97.0 from the existing Nix environment.
- rmcp 3.3.0; full dependency resolution recorded in Cargo.lock.
- Release profile: opt-level s, thin LTO, one codegen unit, stripped symbols.

## Verification

`tools/Test-Repository.ps1` passes formatting, Clippy with warnings denied, workspace tests, and release build. 43 tests pass:

| Area | Tests | What is exercised |
| --- | ---: | --- |
| Config/framing | 3 | deny defaults, unknown config keys, bounded inbound messages |
| Architecture/examples | 2 | crate dependency boundaries, no device I/O in protocol, valid read-only examples |
| MCP SDK integration | 1 | initialize/list/call, allowed reads, direct hidden-tool denial, secret-free responses/logs |
| Executable stdio/CLI | 2 | actual binary pipes, fixed synthetic process action, exact arguments, audit JSON, offline check failure |
| Core security | 15 | full permission combinations, deny priority, cross-category requirements, extension privilege gate, strict input/definition validation, no shell interpolation, output projection and overlap checks |
| Audit filesystem | 6 | size rotation/retention, permissions, symlink/hardlink rejection, append, line injection, disabled logging |
| Local backend | 6 | actual local fixture child processes, literal argv, stdin EOF, JSON-only output, stderr/output caps, timeout and child reap |
| Dispatcher | 8 | never execute denied/invalid/unaudited calls, bounded saturation, success/failure audit correlation, completion audit uncertainty, blocked log writer deadline and bounded workers |

An independent read-only code review found three issues during implementation: blocking audit writes on the async executor, optional argv value omission changing command meaning, and overlapping result projections multiplying memory use. All three were fixed and covered by regression tests; follow-up review found no unresolved blocker in that scope. This is not an external security audit.

## Host measurements

| Measurement | Observed | Scope |
| --- | ---: | --- |
| Release executable | 2,340,800 bytes (2.23 MiB) | Linux x86_64 build; not an OpenWrt/musl image |
| Idle RSS | 4,040 KiB (3.95 MiB) | Single sample one second after MCP initialization, zero router calls |
| Idle threads | 2 | Same sample; stderr connected to null |
| Policy lookup + authorization p50 / p95 | 51 / 56 ns | 10,000 iterations, 1,005-entry synthetic catalog |
| Dispatcher p50 / p95 | 57,516 / 95,729 ns | 10,000 iterations, empty input object, in-memory fixture, audit disabled |

The dispatcher number includes validation, projection and audit dispatch machinery; it excludes router CLI/ubus, real log I/O and network cost. These are single-run microbenchmarks, not sustained-load guarantees. CPU use, ARM/musl binary size, on-device RSS, real ubus latency and the 60-second idle target remain unmeasured.

Reproduce on Linux:

```sh
cargo build --locked --release
cargo run --locked --release -p openwrt-mcp --example benchmark
cargo run --locked --release -p openwrt-mcp --example footprint -- target/release/openwrt-mcp
```

If using CARGO_TARGET_DIR, supply that directory's release binary to footprint. The diagnostic examples use synthetic data, never SSH, ubus, or live router configuration.

## Outstanding acceptance work

- Cross-compile and package using a declared OpenWrt SDK/target.
- Run structured adapter tests in an OpenWrt emulator, then separately authorized hardware tests.
- Verify built-in field schemas against declared OpenWrt releases and report unavailable interfaces accurately.
- Implement device capabilities and mutation/backup/rollback workflows from the requirements.
- Test Unix syslog against OpenWrt logd, rotation under disk pressure, cancellation and transport flood behavior.
- Select a redistribution license and complete release review before publishing code.

No real router configuration or state was queried or changed in this setup. No code was pushed to GitHub and no package release was published.
