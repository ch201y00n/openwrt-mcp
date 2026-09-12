# Validation record

Date: 2026-09-13. This is host/fixture validation, not a production or real-router acceptance report.

## Environment

- Intel Core Ultra 7 265K, x86_64.
- Ubuntu under WSL2; Linux 6.18.33.2-microsoft-standard-WSL2.
- Rust 1.97.1; Cargo 1.97.0 from the existing Nix environment.
- rmcp 3.3.0; full dependency resolution recorded in Cargo.lock.
- Release profile: opt-level s, thin LTO, one codegen unit, stripped symbols.

## Verification

Architecture-v2 checkpoint: `tools/Test-Repository.ps1` passes the actual architecture/evolution gate, architecture negative tests, formatting, Clippy with warnings denied, workspace tests, and release build. 65 distinct tests pass (the gate intentionally runs the harness tests first and again with the full workspace):

| Area | Tests | What is exercised |
| --- | ---: | --- |
| Config/framing | 3 | deny defaults, unknown config keys, bounded inbound messages |
| Example policies | 1 | valid read-only examples |
| Architecture | 17 | 10 adversarial contract/AST/metadata cases and 7 assembled-gate tests using real temporary Cargo/Git workspaces; aliases, cfg/target/dev/build, raw identifiers, macros/attributes, module ownership, unsafe/FFI and evolution evidence |
| MCP SDK integration | 1 | initialize/list/call, allowed reads, direct hidden-tool denial, secret-free responses/logs |
| Executable stdio/CLI | 2 | actual binary pipes, fixed synthetic process action, exact arguments, audit JSON, offline check failure |
| Core security | 17 | permission matrix, cross-category requirements, extension gate, strict validation, scalar defaults, duplicate builtins, no shell interpolation, projection overlap checks |
| Feature catalog | 3 | five reviewed definitions remain read-only and denied by default; scalar projection excludes configuration and malformed subtrees |
| Audit filesystem | 6 | size rotation/retention, permissions, symlink/hardlink rejection, append, line injection, disabled logging |
| Local backend | 7 | fixed ubus compilation plus local fixture child processes, literal argv, stdin EOF, JSON-only output, stderr/output caps, timeout and child reap |
| Dispatcher | 8 | never execute denied/invalid/unaudited calls, bounded saturation, success/failure audit correlation, completion audit uncertainty, blocked log writer deadline and bounded workers |

An independent read-only code review found three issues during implementation: blocking audit writes on the async executor, optional argv value omission changing command meaning, and overlapping result projections multiplying memory use. All three were fixed and covered by regression tests; follow-up review found no unresolved blocker in that scope. This is not an external security audit.

The architecture migration was verified before adding the next read operations. Independent source review found and closed simple harness bypasses involving Cargo aliases, production targets in test directories, nested target directories, raw identifiers, macro/attribute expressions and broad namespace re-exports. These findings are exercised by negative fixtures; static checks are not a proof of arbitrary macro/dependency semantics.

## Historical foundation host measurements

The following measurements were captured before the architecture-v2 migration. They are retained as a baseline, not claimed as measurements of the refactored executable.

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
