# Architecture-first development workflow

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
