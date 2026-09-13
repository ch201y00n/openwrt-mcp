# Validation record

Date: 2026-09-13. This records host/fixture validation and separately scoped emulated acceptance, not a production or full physical-router acceptance report.

## Environment

- Intel Core Ultra 7 265K, x86_64.
- Ubuntu under WSL2; Linux 6.18.33.2-microsoft-standard-WSL2.
- Rust 1.97.1; Cargo 1.97.0 from the existing Nix environment.
- rmcp 3.3.0; full dependency resolution recorded in Cargo.lock.
- Release profile: opt-level s, thin LTO, one codegen unit, stripped symbols.

## Verification

The **v9 LuCI/storage increment** follows architecture-only `acf6532`, whose native Windows GNU and Linux-on-WSL gates passed before production migration. Full implementation gates pass **377 distinct native Windows GNU tests** and **398 Linux-on-WSL tests**, zero ignored; **61 harness regressions** are counted once. Two typed storage reads bring the catalog to 20. New contracts cover exact LuCI object descriptions, source-error/malformed/duplicate/oversized responses, integer precision, category isolation, fixed actions, MCP serialization and payload-free audit. Existing native Windows file/ZIP protection and portable SSH/age/protocol cases also pass. This is host/fixture acceptance, not LuCI storage execution on hardware or an emulator; see [storage scope](storage-observations.md). No new RSS/CPU/latency or complete management claim is made.

The **v8 Windows protected-read increment** follows architecture-only `9c0117f`, which passed native Windows GNU and Linux-on-WSL full gates before behavior (361/392 distinct tests respectively, including one Windows declaration scaffold). Implementation full gates pass **371 distinct native Windows GNU tests** and **392 Linux-on-WSL tests**, zero ignored. The scaffold is replaced by seven native file cases, three pure security/path cases and one key-source file/ZIP composition case. The harness has **59 regressions**, counted once even though the gate executes them twice. Native adversarial cases cover actual DACL/inheritance/sharing/link semantics; WSL's zero-test Windows targets are not Windows evidence. Source owner/SDK/lint/API boundaries, existing portable suites, strict Clippy and release builds all passed. See [Windows evidence](windows-validation.md) and [profile](windows-protected-files.md). No Windows private-log, Vault, macOS, BPI-R4 or new latency/RSS/CPU acceptance follows from this increment.

The **v7 package observation increment** follows architecture-only checkpoint `0521c7c`, which passed both native Windows GNU and Linux-on-WSL gates before production changes. Current full gates pass **387 distinct Linux-on-WSL tests** and **355 native Windows GNU tests**, zero ignored; **54 harness regressions** are counted once. This adds a closed same-backend APK query, private expiring snapshots, 16-record pages and bounded adapter-owned OS entropy. The 281-record host fixture traverses all pages; negative cases cover malformed late data, memory/byte bounds, permissions, entropy/audit failures, replay/foreign/expired cursors, cancellation, concurrency and changed epochs. Generic Process remains blocked. The separately scoped [v7 emulator run](emulator-validation-v7.md) enumerated 205 records in 13 pages and checked 39 safe audit events. No package mutation, full-device inventory, physical-device or all-host acceptance is claimed. Older binary size/CPU/RSS measurements below do not describe the v7 executable.

The **subsequent passive wireless increment** adds `wireless_stations`, `wireless_station_status` and `wireless_countries` within the existing v6 owners and bounds. Full gates pass **357 distinct Linux-on-WSL tests** and **325 native Windows GNU tests**, zero ignored, with unchanged 51-test architecture harness, strict Clippy and release builds. Seven added feature regressions and three MCP regressions cover identity disclosure/denial, local-only selection, strict records, decimal counters, empty/oversized observations, literal actions and safe audit. The catalog contains seventeen reads, eight typed. Source contracts preceded implementation; no new dependency, probe family, backend or runtime bypass was introduced. These new operations have synthetic host acceptance only, not radio, emulator or physical-device acceptance. See [wireless contracts](wireless-observation-contracts.md). Previous performance/emulator evidence below remains tied to its earlier executable.

The subsequent **native Windows GNU full gate** passed with **315 distinct tests**, zero ignored, including all five actual-binary stdio cases. The native release is 4,194,304 bytes. A hermetic test launcher now preserves `SystemRoot`, after a local reproduction traced two Windows SSH-test failures to Winsock provider initialization with an empty environment. No production code or test requirement was weakened. Linux-on-WSL was revalidated after this test-only fix. Exact environment, hashes, timing and limitations are in [native Windows validation](windows-validation.md). This establishes the common GNU host/fixture path, not MSVC, macOS, native protected files/Vault or physical-router acceptance.

The **v6 bounded-read implementation** passed the complete Linux-on-WSL repository gate: architecture/evolution, all **51 harness regressions**, formatting, strict all-target Clippy, workspace tests and release compilation. The inventory is **347 distinct tests**, including the compile-fail doctest, with zero ignored; repeated harness execution is counted once. Five typed contracts extend the catalog to fourteen reads. Behavioral coverage includes private selector/action binding, validation of unselected rows, exact escaped-byte accounting before cloning, nested item budgets, strict shared JSON decoding, real MCP text/structured response bounds and local/SSH error handling. Architecture-only checkpoint `eccd3e7` preceded functional changes.

Independent review found a cooperative-timeout boundary: a non-yielding decoder/projector could finish late without the asynchronous timer returning an error. Explicit monotonic checks now reject late results and revoke issued observation leases before success completion; three non-yielding regressions pass. This is not CPU preemption or a hard real-time guarantee, consistent with the [Tokio timeout contract](https://docs.rs/tokio/1.53.1/tokio/time/fn.timeout.html). Independent review of finite projections, byte budgets and both backend decoder integrations found no further actionable issue in this increment; this is not an external security audit.

The **v6 actual MCP/SSH emulator run** validated all five typed contracts and seven legacy reads, with two explicitly unavailable legacy calls, twelve invalid-input rejections, two unobserved selections, cache/refresh recovery and **88 safe audit events**. The exact debug executable hash, timestamps, image and operation scopes are in [v6 emulated acceptance](emulator-validation-v6.md). A valid empty wireless device list is not physical-radio acceptance. This Linux-host emulator run is separate from the later Windows GNU fixture gate; macOS, MSVC and BPI-R4 hardware acceptance remain pending.

The **v6 architecture-only checkpoint** passed the complete Linux-on-WSL repository gate with **272 distinct tests**, zero ignored: the v5 implementation's 260 plus seven new harness regressions and five explicitly named declaration scaffolds. Fifty-one tests now exercise the architecture harness. Requirements, ADR 0006, the versioned projection/decoder/result contracts and thirteen required native suites are recorded before functional implementation. No collection projector, changed feature action or strict action decoder is implemented by this checkpoint, and its scaffold checks do not add operation acceptance evidence. Independent design/source review clarified nonempty identities, bounded legacy traversal before cloning and response field presence.

Current continued-development work has separately performed read-only reference identity, selected installed-package metadata and ubus method-name introspection on BPI-R4. See [reference-target.md](reference-target.md) for exact observation times and scope. No configuration was changed. This is not full operation acceptance, and no historical fixture count below is reclassified as device testing.

The **v5 capability implementation** passed the full repository gate: architecture/evolution, all 44 harness regressions, formatting, strict Clippy, workspace tests and release compilation. The inventory is **260 distinct tests**, including the compile-fail doctest, with zero ignored. Earlier checkpoint scaffolds have been replaced/extended with behavioral contracts. Tests cover required input signatures, signed 32-bit ubus values, bounded duplicate-rejecting describe parsing, local/SSH probes, offline listings, audited metadata, same-backend epochs, expiry, refresh, concurrency and shared deadlines. Eleven MCP/actual-binary integration tests exercise transport and protocol behavior using synthetic endpoints.

Independent review identified a refresh revocation issue: a previously returned cache lease could remain positive after its entry was removed. Shared invalidation tokens now revoke every lease before refreshing; successful, failed and cancelled refresh regressions pass. Further independent review found no additional blocker in this increment; this is not an external security audit.

A separately executed **actual MCP -> native Rust SSH -> OpenWrt ARM64 QEMU** run validated seven successful reads, three explicit unavailable/error cases, four malformed/unknown-input denials and safe audit/lifecycle behavior. See [emulated acceptance](emulator-validation.md) and [environment provenance](emulator-environment.md). This is emulated userspace evidence, not native Windows/macOS or BPI-R4 hardware acceptance. No live-router changes, real keys or Vault access were involved.

Architecture v5's **architecture-only checkpoint** passed the full repository gate in Linux-on-WSL before capability implementation: 12 crate boundaries, 44 harness regressions, formatting, strict Clippy, workspace tests and release build. The inventory is **208 distinct tests**, zero ignored, including the previous 192 plus 12 harness tests and four explicitly named checkpoint scaffold checks. Those four establish the recorded boundary/catalog/evidence contract, not implemented probing, caching or protocol gating. The new codec production library is documentation-only at this checkpoint. No native Windows/macOS or full device-acceptance claim follows from this gate.

Architecture v4 portable-host increment: the complete `tools/Test-Repository.ps1 -UseWsl` gate passes, including architecture/evolution, 32 harness regressions, formatting, strict Clippy, all workspace tests and release compilation. The test inventory contains **192 distinct tests** including one compile-fail doctest; zero tests were ignored. The harness suite is intentionally run first and again with the workspace, not counted twice. Architecture-only checkpoint `d88fb78` passed the full gate before functional implementation; its two placeholder test targets were replaced by actual suites, not counted as feature evidence.

| Area | Historical v4 tests | Evidence |
| --- | ---: | --- |
| Server/configuration/composition | 19 | Four real-binary portable stdio cases, five typed target/config CLI cases, strict byte/schema checks, examples and unchanged protection composition |
| Local adapter and audit | 16 | Seven private Linux runner/argv cases, seven audit/capability cases, portable unconfigured refusal and compile-fail local-constructor protection |
| SSH | 13 | Twelve loopback cases with synthetic in-memory keys; pin/auth failures, quoting, reuse, bounds, no retry, cancellation/socket closure, bounded source worker; one receive-loop WindowAdjusted classifier regression |
| Host platform | 13 | Linux ownership/modes/links/trusted handles/rotation/marker fixtures and explicit unsupported-capability/limit behavior |
| Key sources | 23 | Protected-file/profile and registry/environment/ZIP tests, offline purpose-limited bindings and actual-length rechecking |
| Core/features/runtime/MCP/age | 76 | Existing policy, catalog, dispatcher, protection, protocol and all 18 age provider regressions |
| Architecture harness | 32 | Layer/source/evolution guards plus portable OS-boundary, required-test activation and native CI matrix regressions |

The real-binary SSH test covers MCP -> authorization -> native SSH -> approved output projection -> audit with one reused loopback connection. No remote command actually runs in the fake endpoint. A 128-KiB stdout plus 128-KiB stderr case exercises multiple receive windows; it is distinct from the focused informational WindowAdjusted classifier test and is not a raw inbound window-adjustment injection test. Independent review identified and fixed ignored unknown fields on unit target variants, legitimate flow-control events being rejected, and a smaller configured identity limit not applying to direct sources.

The reviewed dependency combination is russh 0.63.3/ring with age 0.11.5; all 18 age tests pass without provider code changes. See [ADR 0004](adr/0004-cross-platform-hosts.md) for the incompatible ML-KEM transitive resolution and maintained-release choice. Cargo reports a third-party future-Rust incompatibility notice for proc-macro-error2 2.0.1; it is not a current gate failure and is not suppressed. Dependency/security review remains required before release.

**The v4 test execution environment was Linux on WSL, not native Windows.** Three native CI jobs were configured and enforced by the harness but were not run/pushed from that increment. Native Windows/macOS acceptance, native protected files/Vault on those hosts and actual OpenWrt deployment remained outstanding. No real keys, Vault files or router state were read by the v4 fixture suite. See [platform support](platform-support.md).

## Historical architecture-v3 verification

Architecture v3 age/key-custody increment: the complete repository gate passed with 143 distinct tests. The architecture-only checkpoint `c9131a8` passed the full gate with 88 tests before implementing new providers. The mandatory gate covered architecture/evolution, negative regressions, formatting, strict Clippy, workspace behavioral tests and release compilation. The changes added 62 tests to the prior 81-test baseline:

| Added area | Tests | Scope |
| --- | ---: | --- |
| Architecture v3 | 7 | Explicit runtime stream access, no OS handles, no MCP protection imports/re-exports, independent custody/cipher dependencies |
| Application protection | 12 | Provider substitution, public/private purpose separation, fresh bounded reads, no fallback/retry, redacted/non-serializable/non-clonable material, strict limit maps |
| age provider | 18 | Real age round trips and upstream interop, native keys, wrong key, tampering/truncation/trailing data, exact bounds, finalization/flush failures, short I/O, cooperative deadlines, header/key-line limits |
| Key sources/containers | 20 | Four Unix protected-file tests; sixteen registry/environment/ZIP/profile tests including metadata/parser-view/actual-inflation bounds and unsupported formats |
| Protection configuration/composition | 5 | Offline aliases and optional identity wiring, strict configuration, memory-backed source + ZIP + age end-to-end without private-key reads during encryption |

The example-policy regression also covers the new public-recipient environment example without reading the environment. Fixture-only review found and fixed positional/empty-array limit settings being accepted, incomplete short-write handling in the age header serializer, dishonest ZIP expansion metadata and a second ZIP parser view hidden inside a comment. Independent follow-up review found no additional actionable blocker in this scope; this is not an external security audit.

No native Windows protected-file/Vault implementation, real user key, Vault archive, OpenWrt target, encrypted backup publication or restoration workflow was tested. Source/crypto operations remain internal primitives rather than MCP tools. See [key-management.md](key-management.md) for supported formats and staging/deadline limitations.

Prior post-v2 read increment: `tools/Test-Repository.ps1` passed the actual architecture/evolution gate, architecture negative tests, formatting, Clippy with warnings denied, workspace tests, and release build. 81 distinct tests passed (the gate intentionally runs the harness tests first and again with the full workspace). Architecture-only checkpoint 8faf423 passed 65 tests and a release build before these feature additions:

| Area | Tests | What is exercised |
| --- | ---: | --- |
| Config/framing | 3 | deny defaults, unknown config keys, bounded inbound messages |
| Example policies | 2 | valid read-only examples and complete opt-in observability catalog without execution/extension grants |
| Architecture | 17 | 10 adversarial contract/AST/metadata cases and 7 assembled-gate tests using real temporary Cargo/Git workspaces; aliases, cfg/target/dev/build, raw identifiers, macros/attributes, module ownership, unsafe/FFI and evolution evidence |
| MCP SDK integration | 4 | initialize/list/call, allowed reads, direct hidden-tool denial, secret-free responses/logs; category-isolated wireless/watchdog/logd, setter rejection and missing-state preservation |
| Executable stdio/CLI | 2 | actual binary pipes, fixed synthetic process action, exact arguments, audit JSON, offline check failure |
| Core security | 17 | permission matrix, cross-category requirements, extension gate, strict validation, scalar defaults, duplicate builtins, no shell interpolation, projection overlap checks |
| Feature catalog | 15 | ten read definitions denied by default; exact calls, input constraints, all-category isolation, execution independence, private output exclusion and malformed/missing scalar fields |
| Audit filesystem | 6 | size rotation/retention, permissions, symlink/hardlink rejection, append, line injection, disabled logging |
| Local backend | 7 | fixed ubus compilation plus local fixture child processes, literal argv, stdin EOF, JSON-only output, stderr/output caps, timeout and child reap |
| Dispatcher | 8 | never execute denied/invalid/unaudited calls, bounded saturation, success/failure audit correlation, completion audit uncertainty, blocked log writer deadline and bounded workers |

An independent read-only code review found three issues during implementation: blocking audit writes on the async executor, optional argv value omission changing command meaning, and overlapping result projections multiplying memory use. All three were fixed and covered by regression tests; follow-up review found no unresolved blocker in that scope. This is not an external security audit.

The architecture migration was verified before adding the next read operations. Independent source review found and closed simple harness bypasses involving Cargo aliases, production targets in test directories, nested target directories, raw identifiers, macro/attribute expressions and broad namespace re-exports. These findings are exercised by negative fixtures; static checks are not a proof of arbitrary macro/dependency semantics.

## Current architecture-v6 host measurements

After the v6 release gate on the Linux x86_64 environment above: executable **4,676,720 bytes (4.46 MiB)**, idle RSS **5,720 KiB (5.59 MiB)** and two threads. After one second of initialization warmup, a **60.00158-second** interval recorded zero process CPU ticks at 100 ticks/second: **0.0% of one core at that counter resolution**, not proof of literally zero CPU work. Default-deny/unconfigured target, enabled audit, stderr connected to null; no router call, key access or SSH session. Active-work peak memory remains unmeasured.

The diagnostic reads the child process's user/system CPU counters and the host's clock-tick rate; see [Linux process stat fields](https://man7.org/linux/man-pages/man5/proc_pid_stat.5.html). Its Linux-specific implementation is a development example, not a platform restriction in the MCP server. The observation is not ARM/OpenWrt or native Windows/macOS performance acceptance.

A 10,000-iteration release microbenchmark used a **1,014-operation** synthetic catalog, empty input, a prewarmed private capability cache, an in-memory backend and disabled audit destination. Policy lookup/authorization p50 **44 ns**, p95 **47 ns**; dispatcher p50 **53,510 ns**, p95 **83,973 ns**. It excludes real probing, SSH, device execution and log I/O. These are single-run host observations; other verification work ran on the host, so they are not controlled comparative benchmarks.

## Historical architecture-v5 host measurements

After the v5 release gate on the Linux x86_64 environment above: executable **4,443,248 bytes (4.24 MiB)**, idle RSS **5,556 KiB (5.43 MiB)**, two threads, sampled one second after MCP initialization. Default-deny/unconfigured target, enabled audit, stderr connected to null; no router call, key access or SSH session. CPU and active-work peak memory were not measured.

In a separate 10,000-iteration release microbenchmark with a 1,010-operation synthetic catalog and empty input: policy lookup/authorization p50 **44 ns**, p95 **47 ns**; dispatcher p50 **55,131 ns**, p95 **78,370 ns**. The dispatcher used an in-memory backend, a prewarmed private capability cache and disabled audit destination. It includes normal validation/projection/audit machinery but excludes real probing, SSH, device execution and log I/O. These single-run host observations are not controlled comparisons or ARM/OpenWrt/native Windows/macOS performance acceptance.

## Historical architecture-v4 idle sample

After the final v4 release gate on the same Linux x86_64 host: executable **4,316,272 bytes (4.12 MiB)**, idle RSS **5,304 KiB (5.18 MiB)**, two threads. Sampled one second after MCP initialization with default-deny/unconfigured target, enabled audit and stderr connected to null. No router call, SSH connection or key source was opened; CPU was not measured. This is an idle-host observation, not active SSH/crypto peak memory, a native Windows/macOS measurement or ARM/musl/OpenWrt acceptance. Native SSH now contributes to the executable; historical sizes below are not measurements of this build.

## Historical architecture-v3 idle sample

After the final provider/configuration integration and release gate on the same Linux x86_64 host: binary 2,619,328 bytes (2.50 MiB), idle RSS 4,332 KiB (4.23 MiB), two threads, sampled one second after MCP initialization. Default deny/audit configuration, stderr connected to null, no router calls or key sources opened. This measures the current stdio executable at idle, not active encryption, native Windows/Vault access, a future integrated backup workflow, ARM/musl or sustained CPU use. Active crypto throughput/peak memory remain unmeasured.

## Historical architecture-v2 host measurements

Measured again after architecture v2 and ten built-in reads. Conditions are the same host/fixture setup above; the microbenchmark and idle sample ran concurrently, so these are observations, not controlled performance comparisons. For context, the original five-read foundation executable was 2,340,800 bytes and its single idle RSS sample was 4,040 KiB.

| Measurement | Observed | Scope |
| --- | ---: | --- |
| Release executable | 2,353,088 bytes (2.24 MiB) | Linux x86_64 build; not an OpenWrt/musl image |
| Idle RSS | 4,064 KiB (3.97 MiB) | Single sample one second after MCP initialization, zero router calls |
| Idle threads | 2 | Same sample; stderr connected to null |
| Policy lookup + authorization p50 / p95 | 42 / 46 ns | 10,000 iterations, 1,010-entry synthetic catalog |
| Dispatcher p50 / p95 | 58,223 / 87,543 ns | 10,000 iterations, empty input object, in-memory fixture, audit disabled |

The dispatcher number includes validation, projection and audit dispatch machinery; it excludes router CLI/ubus, real log I/O and network cost. These are single-run microbenchmarks, not sustained-load guarantees. CPU use, ARM/musl binary size, on-device RSS, real ubus latency and the 60-second idle target remain unmeasured.

Reproduce on Linux:

```sh
cargo build --locked --release
cargo run --locked --release -p openwrt-mcp --example benchmark
cargo run --locked --release -p openwrt-mcp --example footprint -- target/release/openwrt-mcp
```

Append `60` to the footprint invocation for the current 60-second idle CPU/RSS observation (default interval: one second, after one-second warmup). It requires Linux `/proc` and `/usr/bin/getconf`. If using CARGO_TARGET_DIR, supply that directory's release binary to footprint. The diagnostic examples use synthetic data, never SSH, ubus, or live router configuration.

## Outstanding acceptance work

- Run the configured native Windows/Linux/macOS CI gates and platform-specific acceptance; WSL alone is not platform parity.
- Implement Windows/macOS protected file/log profiles and actual Personal Vault integration through the architecture-first workflow.
- Cross-compile and package using a declared OpenWrt SDK/target.
- Expand the initial OpenWrt emulator acceptance to more management adapters and an opkg-family image, then separately authorized hardware tests.
- Verify built-in field schemas against declared OpenWrt releases and report unavailable interfaces accurately.
- Expand capability families beyond closed ubus signatures; implement mutation/backup/rollback workflows from the requirements.
- Test Unix syslog against OpenWrt logd, rotation under disk pressure, cancellation and transport flood behavior.
- Select a redistribution license and complete release review before publishing code.

Historical fixture setups above did not query or change a real router. Subsequent narrowly scoped read-only reference observations are documented separately at the top of this record. No code was pushed to GitHub and no package release was published.
