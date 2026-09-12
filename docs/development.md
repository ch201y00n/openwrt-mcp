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
| crates/mcp/src | MCP protocol/framing; cannot import concrete adapters |
| crates/server/src | Configuration and dependency wiring; no device command execution |
| tools/xtask/src | Architecture validation, negative fixtures and development gate |
| architecture | Machine-readable boundary contract |
| docs/adr | Architecture decisions and versioned evolution rationale |

## Feature workflow

Add the acceptance requirement and classify the feature's category/effects. Reuse the owning module and existing ports. If an interface cannot express the feature, stop feature implementation and evolve the architecture under ADR 0002 first. Add deny/allow/cross-category/secret-output tests, then implement inside the approved boundary. Update coverage with one of fixture-validated, device-validated, unavailable or planned; do not equate a generic command with tested support.

## Gates

Run `cargo run --locked -p xtask -- architecture`, then `tools/Test-Repository.ps1` (or `sh tools/test.sh`). The full gate includes harness negative tests first, Rust formatting, strict Clippy, behavior tests and release compilation. Both default to HEAD as the local evolution baseline. CI uses the same gate against the pull request base or previous push revision. For another reviewed baseline, run `cargo run --locked -p xtask -- architecture --base <commit>`, `tools/Test-Repository.ps1 -BaseRef <commit>`, or `sh tools/test.sh <commit>`.

Changes to security boundaries must update architecture/spec.toml, docs/architecture.md, docs/requirements.md, an ADR, and a harness regression before feature implementation. Rust compilation and static checks complement one another; review remains required for semantics, macros and newly approved dependencies.

Restricted production layers use narrow imports. Importing/re-exporting an entire namespace that contains a forbidden API (for example std::io in transport) is rejected; import only permitted types such as std::io::Error. The harness normalizes Rust/Cargo aliases and raw identifiers, inspects inactive cfg branches and approved macro/attribute inputs, and rejects remapped modules, unreviewed macros, unsafe blocks and FFI. Real temporary Git/Cargo fixture workspaces test that the assembled gate rejects violations; fixture workspaces never access the router.

The evolution gate checks the final change evidence, not the temporal order of edits. Keep an architecture-only validated checkpoint before incompatible functional work, and review the sequence. No tool can prove all semantics of arbitrary approved third-party code or prevent an authorized maintainer from rewriting the gate itself.
