# ADR 0002: Architecture-first ports and adapters with an executable contract

Status: accepted for implementation. Architecture contract version: 2.

## Requirement

The owner requires a documented architecture and clear directories before feature work; established Rust layout practices where applicable; a harness that rejects deviations; and architecture/harness evolution before implementing incompatible new requirements.

## Decision

Use a Cargo workspace with domain, feature specifications, application use cases/ports, infrastructure adapters, MCP transport, a thin composition executable, and a development-only xtask. This is a project-specific ports-and-adapters design using Rust's standard packages/modules/visibility conventions, not a claim that Rust mandates hexagonal architecture.

The application owns Backend and AuditSink ports. Infrastructure implements them. Domain code validates actions into PreparedAction values; it must not compile OpenWrt-specific CLI argv. The infrastructure adapter compiles PreparedAction into local process invocations. Feature catalogs own built-in device action definitions and category directories. Core does not depend on features. Only the composition crate wires adapters into the application and transport. No runtime dynamic plugin loading is introduced.

The declarative architecture/spec.toml is authoritative for crate ownership and allowed dependencies. A syn-based xtask validates Cargo metadata including renamed, target, dev and build edges, recursively parses all Rust source (including inactive cfg branches), resolves import aliases and rejects disallowed OS calls, includes, external modules and unreviewed macros in restricted production layers. Build scripts are forbidden unless explicitly designed. Tests/examples may use fixture I/O but remain subject to dependency and source ownership checks.

No static checker proves all semantic behavior or expands arbitrary procedural macros. Limit allowed dependencies/macros, enforce Rust visibility and narrow crate edges, exercise real permission denial with fake ports, and retain security review. The harness is a development control, not a runtime sandbox against someone authorized to edit the repository and the harness itself.

## Evolution protocol

1. Record the new or changed requirement with an acceptance condition.
2. Decide whether existing boundaries can express it. Routine feature work within an established category does not require redesign.
3. If boundaries must change: write an ADR describing responsibility/dependency changes, update architecture.md and spec.toml, and bump the spec version.
4. Extend the harness and at least one negative regression case to enforce the changed contract. Do not weaken checks just to pass.
5. Run the harness before adding functional code against the new boundary. A migration may temporarily fail until files are moved, but no feature change uses that transitional state.
6. Implement, test all invariants, update coverage, and run the full repository gate.

The change gate compares to a Git baseline. Contract changes require a newer version, an ADR, and changes to architecture, requirements, and harness tests. A newly introduced workspace member or dependency not covered by the contract fails even if it compiles. No 'skip architecture', ignore failure, blanket exception or feature-specific direct I/O is an accepted workaround.

## Migration

Extract concrete backend/audit I/O from runtime to adapters. Move framing/protocol into mcp. Extract category definitions from core/catalog to features. Change core preparation to a platform-neutral validated action. Keep public binary name and config permissions stable. Move tests with their owning layers. Preserve existing behavioral regressions, then implement category adapters only after architecture checks pass.

## Sources

- [Rust packages, crates and modules](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html)
- [Cargo workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Cargo package layout](https://doc.rust-lang.org/cargo/guide/project-layout.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
