# Architecture-first development workflow

For v18, commit the exact provided-stream sealing contract and negative harness before adding runtime::sealing or the server fixture dependency. No production consumers, real source/store access or MCP behavior are admitted. Test independent counts/EOF, producer/validator/cipher failures, deadlines, cleanup, publication uncertainty and complete synthetic age/gzip composition. Neither a mock durability capability nor a supplied-stream result is real backup acceptance. See ADR 0018.

For v17, validate and commit the separate gzip codec declaration before dependency/behavior changes. Preserve v16 regular-tar rules and all external archive bans, allow only the gzip wrapper as an internal consumer, and confine pinned low-level flate2 use to its owned module. Native malformed/completeness/resource tests must precede a gzip acceptance claim. A correct CRC is not authentication and decompressor reset/free is not guaranteed scrubbing. No workflow integration is implied; see ADR 0017.

Architecture v16 adds only the declared archive codec boundary before implementing its parser. Validate and commit requirements, ADR and negative harness first; preserve exact source ownership, zeroize-only extra dependency allowance, production consumer bans and mandatory native action-response suite. Add behavioral fixtures in a separate archive module afterward. Neither declaration tests nor a valid tar predicate count as implemented backup, producer completion or restore authorization. See ADR 0016.

Read requirements.md, architecture.md, architecture/spec.toml and the active ADR before modifying behavior. Build artifacts and synthetic tests never authorize live router changes.

## Standard directories

Each Cargo package has src/lib.rs or src/main.rs, integration tests under tests/, and runnable diagnostics under examples/. Modules use foo.rs and foo/child.rs with private implementation details and deliberate lib.rs re-exports. Workspaces share Cargo.lock, edition, toolchain bounds and lints. Production code cannot depend on development tooling or test helpers.

The workspace default member is the server: ordinary release builds produce the application and its dependencies. The full verification gate explicitly uses --workspace to check every layer including xtask. Deploy only the server executable; the source scanner and Rust parser are development dependencies, not router runtime components.

| Directory | Ownership |
| --- | --- |
| crates/core/src | Pure permission, validation and action domain |
| crates/features/src/categories | Device action definitions grouped by functional category |
| crates/runtime/src | Invocation use cases and ports; no device or filesystem I/O |
| crates/adapters/src | Local process, audit destinations and future backup/ubus persistence adapters |
| crates/key-sources/src | Protected file/environment providers and independent container decoders |
| crates/crypto-age/src | age encryption/decryption; no key-location or OS access |
| crates/host-platform/src | Purpose-specific host OS protection, explicit Linux/Windows/macOS implementations |
| crates/backend-ssh/src | Native portable remote OpenWrt connection; never host command execution |
| crates/device-codec/src | Pure fixed target encoders and bounded probe parsers shared by local/SSH infrastructure |
| crates/mcp/src | MCP protocol/framing; cannot import concrete adapters |
| crates/server/src | Configuration and dependency wiring; no device command execution |
| tools/xtask/src | Architecture validation, negative fixtures and development gate |
| architecture | Machine-readable boundary contract |
| docs/adr | Architecture decisions and versioned evolution rationale |

## Feature workflow

Add the acceptance requirement and classify the feature's category/effects. Reuse the owning module and existing ports. If an interface cannot express the feature, stop feature implementation and evolve the architecture under ADR 0002 first. Add deny/allow/cross-category/secret-output tests, then implement inside the approved boundary. Update coverage with one of fixture-validated, device-validated, unavailable or planned; do not equate a generic command with tested support.

## Gates

Architecture v15 admits only a pure, non-authorizing effect-graph model under core/src/management. Commit the validated declaration and negative harness before production behavior. Existing core security tests become a required native suite; graph behavior is added afterward in their own module and is never claimed from declaration tests. Keep all production consumers and serde/JSON/action/policy access blocked, retain exact finite budgets and preserve earlier checkpoint restrictions. This is not deployed protected-resource enforcement or mutation authority. See ADR 0015.

Architecture v14 adds a closed opkg root-status contract before extending production actions/ports/parsing. Commit the validated architecture-only checkpoint first. Require the existing package suites and native loopback SSH suite, exact fixed commands/version, bounded stanza/field parsing and cross-manager profile/slot isolation. Preserve the earlier APK contract, shared limits and twelve owners; no generic file read or opkg initialization under read permission. Older checkpoint fixtures must remove the v14-only contract explicitly. See ADR 0014.

Architecture v13 expands the exact UCI recipe set only after a validated declaration/harness checkpoint. Preserve the v11/v12 six-profile contract when testing older versions; newly admitted sections require v13, not a wildcard. Keep category definitions grouped by domain, independent expected-field fixtures and actual MCP category-isolation tests for every added tool. Do not change owners, probes, projection budgets or mutation safety requirements. See ADR 0013.

Architecture v11 adds a closed UCI-read recipe contract and exact finite TextEnum without new I/O owners or dependencies. Validate and commit the declaration/harness before production UCI probing. Custom UCI definitions must be rejected before the new object becomes available; parameterless built-ins must validate config/type/category, guarded typed section maps and exact expected type. Native core capability and MCP read-contract suites are mandatory. Configuration lists, mutations and false committed/effective-state claims are not shortcuts around this boundary; see ADR 0011.

Architecture v12 adds nested terminal TextOption to represent UCI string/list options distinctly. Commit the validated declaration/harness before changing production collection forms or UCI admission. Keep fixed kind/values structure, 128-value option cap and existing global budgets; no root selector, raw union, coerced scalar or arbitrary UCI collection. Extend existing required projection and MCP suites with behavioral cases and preserve historical response-version evidence. See ADR 0012.

Architecture v10 adds bounded RowArray, FalseOrSafeInteger and root absence guards inside the existing projection owner, with no new production dependency or I/O authority. Validate/commit the declaration and negative harness checkpoint before implementing; afterward replace no existing unique-resource rule with guessed identity or a raw subtree. Extend actual core/feature/runtime/MCP behavior suites and bump storage response IDs for stricter mixed-error rejection. See ADR 0010.

Architecture v9 extends only the closed probe surface to `luci`/`luci-rpc`. Architecture-only `acf6532` passed native Windows GNU and Linux-on-WSL gates with registry schema 1 and unchanged production behavior; the exact schema 2 registry, core enum and encoding fixtures migrated afterward. The assembled gate rejects unreviewed profiles and method-call probes. New typed consumers need actual catalog/negative/runtime/MCP fixtures, not an architecture scaffold counted as device acceptance.

Architecture v8 adds a narrowly scoped Windows SDK boundary and a mandatory Windows-only protected-read suite (not a portable zero-test success claim). Architecture-only `9c0117f` passed native Windows and Linux gates before behavior. The source gate rejects unsafe expressions and SDK access outside its exact owned file, all manual FFI/unsafe items, widened lint allowances and dependency-target changes. Safe policy code must not acquire native APIs. Synthetic Windows fixtures belong in a private, non-synced directory with a trusted ancestor chain; do not modify existing directory ACLs to make tests pass. The actual interface remains exact even with rustfmt's trailing parameter comma.

Architecture v7 adds five mandatory portable package suites for records/pages, closed commands/parsing, runtime lifecycle, feature metadata and MCP behavior. Architecture-only checkpoint `0521c7c` passed both Windows GNU and Linux-on-WSL full gates before implementation. Its declaration scaffolds are replaced by behavior, not counted as package acceptance. Keep entropy in adapters behind the injected runtime port; package capture is a closed same-backend workflow, never a generic Process permission exception. See ADR 0007.

Architecture v6 adds five mandatory native suites for pure collection projection, actual feature contracts, dispatcher integration, strict action JSON decoding and serialized MCP tool results. Its architecture-only checkpoint contains explicitly named declaration scaffolds, not feature acceptance. Implement finite typed shapes/private prepared selectors, shared parser adoption and response bounds only after the validated checkpoint. New reviewed reads cannot use legacy Structured or an unverified extension to avoid these contracts. The actual feature catalog and fixture response-contract IDs determine coverage; the harness must not maintain a second hardcoded tool inventory. See ADR 0006.

Architecture v5 adds required capability contract suites, a closed exact-object probe registry and scoped compatibility evidence. Checkpoint scaffolds verify the architectural contract only; do not count them as implemented discovery or device acceptance. After the architecture-only checkpoint, migrate operation metadata, same-backend probing, private expiring caches and MCP status mapping within those owners. Never add an unchecked extension, caller-supplied availability assertion or direct protocol discovery path to preserve an old test. Adapt synthetic fixtures to the new contract instead.

Windows/Linux/macOS are required native host gates, not just build targets. Required portable suites cannot disappear behind OS cfg or ignore. WSL is an explicitly selected Linux development environment and must never be reported as native Windows acceptance. Host-specific capabilities and target OpenWrt acceptance remain separately named. ADR 0004 establishes the portability contract before implementing the new adapters.

Run `cargo run --locked -p xtask -- architecture`, then `tools/Test-Repository.ps1` (or `sh tools/test.sh`). The full gate includes harness negative tests first, Rust formatting, strict Clippy, behavior tests and release compilation. Both default to HEAD as the local evolution baseline. CI uses the same gate against the pull request base or previous push revision. For another reviewed baseline, run `cargo run --locked -p xtask -- architecture --base <commit>`, `tools/Test-Repository.ps1 -BaseRef <commit>`, or `sh tools/test.sh <commit>`.

Changes to security boundaries must update architecture/spec.toml, docs/architecture.md, docs/requirements.md, an ADR, and a harness regression before feature implementation. Rust compilation and static checks complement one another; review remains required for semantics, macros and newly approved dependencies.

Restricted production layers use narrow imports. Importing/re-exporting an entire namespace that contains a forbidden API (for example std::io in transport) is rejected; import only permitted types such as std::io::Error. The harness normalizes Rust/Cargo aliases and raw identifiers, inspects inactive cfg branches and approved macro/attribute inputs, and rejects remapped modules, unreviewed macros, unsafe blocks and FFI. Real temporary Git/Cargo fixture workspaces test that the assembled gate rejects violations; fixture workspaces never access the router.

The evolution gate checks the final change evidence, not the temporal order of edits. Keep an architecture-only validated checkpoint before incompatible functional work, and review the sequence. No tool can prove all semantics of arbitrary approved third-party code or prevent an authorized maintainer from rewriting the gate itself.
