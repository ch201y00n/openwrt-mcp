# Architecture

Contract: version 2 in [architecture/spec.toml](../architecture/spec.toml). Read [ADR 0002](adr/0002-architecture-first.md), [requirements](requirements.md), and the [development workflow](development.md) before implementation. This replaces the initial three-crate foundation design.

## Rust structure and dependency direction

Rust does not prescribe one application architecture. This project combines conventional Cargo workspaces/packages, explicit module visibility and integration-test directories with ports and adapters. Separate crates make dependency direction compiler-visible; the harness further constrains dependencies and production source access.

```text
server (composition) ─┬─> mcp (protocol) ─────> runtime (use cases / ports) ─> core
                     ├─> adapters (I/O) ────> runtime + core
                     └─> features (category definitions) ────────────────> core

xtask (development only) -> architecture contract + Cargo metadata + source AST
```

| Crate / directory | Owns | Must not own |
| --- | --- | --- |
| core | Permission rules, validation, prepared actions, safe output projection | Device I/O, processes, async runtime, logging, MCP, built-in device catalog |
| features | Built-in definitions in src/categories, fixture contracts | I/O, orchestration, mutable policy, client-defined capabilities |
| runtime | Dispatcher, concurrency/deadlines, backend and audit ports, safe errors | Concrete device/audit implementations, MCP, configuration file access |
| adapters | Process execution, audit destinations and other port implementations | Authorization decisions, protocol handlers |
| mcp | MCP mapping, bounded framing, SDK lifecycle | Concrete backends, filesystem/process access, policy selection |
| server | CLI, trusted configuration loading, lifecycle logs, dependency wiring | Router command execution, alternate device invocation paths |
| tools/xtask | Contract validation and architectural regression checks | Production dependency or runtime feature implementation |

Exact normal, development and build dependency allowlists are versioned in the contract; target-specific declarations are checked too. Test/example fixtures may perform local fixture I/O, but do not grant production access or authorize live router calls.

## Invocation lifecycle

1. MCP maps a named tool and JSON object into a dispatcher invocation.
2. The dispatcher resolves immutable, operator-installed metadata.
3. Policy checks ALL category requirements, exact allowlists and explicit denials.
4. Pure validation rejects unknown/missing arguments, invalid types and unapproved values, then creates a PreparedAction.
5. The dispatcher acquires bounded execution capacity and records a start event. Audit failure prevents execution.
6. The backend executes the prepared action. The local adapter compiles ubus actions into fixed program/argv calls without a shell.
7. The operation projects approved output fields. The dispatcher records the outcome and duration before returning.

Handlers cannot bypass the dispatcher. Backend ports are trusted infrastructure, not client-accessible tools. Discovery and execution use the same authorization; a catalog entry indicates configured support, not proof that the device provides the method. Device actions are never automatically retried.

## Domain contracts

- Category: system, network, wireless, firewall, dhcp_dns, services, packages, storage, vpn, firmware, diagnostics, extensions. Unknown values fail validation.
- A category Grant combines Access (Deny, Read, ReadWrite) and an independent execute boolean. Defaults are deny / false. Mutations can require write AND execute; cross-category actions require every affected category.
- Policy holds grants and optional exact operation allow/deny sets. Denial wins; references must resolve against the catalog.
- Parameter has a scalar kind (string/integer/boolean), required flag and optional allowed values. Client strings are bounded to 1024 bytes and cannot contain NUL. Unknown keys are rejected.
- Action is an operator-owned Ubus { object, method, arguments } or Process { program, args } template. Only a whole {parameter} value is substituted. Programs are fixed absolute paths. Process placeholders are required so missing arguments cannot shift meanings; optional ubus parameters omit their entire key.
- Operation::prepare returns PreparedAction::Ubus { object, method, arguments: Value } or PreparedAction::Process { program, args }. Core does not know /bin/ubus or compile command lines.
- Catalog::with_builtins(builtins, custom) validates definitions and rejects duplicate names. Catalog::new(custom) is a generic zero-builtins convenience. The features crate composes the device catalog.
- Operation::project maps approved JSON pointers to values. OutputMode::Scalars defaults to rejecting object/array subtrees. Structured output is opt-in for trusted extensions, with sensitive-key redaction as defense in depth, not guaranteed secret detection.
- Empty projection lists return no raw output. Decoded pointers must be distinct and non-overlapping to prevent response amplification. Errors never echo untrusted arguments, configuration excerpts or backend text.

The MCP process uses one operator-selected policy. Client labels and tool arguments cannot select another policy or elevate permission. Separate stdio processes/configurations isolate principals; HTTP and multi-user authentication are not implemented.

## Runtime and infrastructure contracts

Backend::execute(&PreparedAction, &Limits) is an async port returning JSON or a safe RuntimeError. AuditSink::record(&AuditEvent) is a synchronous port. Dispatcher owns authorization and orchestration with injected implementations and cannot import an adapter.

Limits have bounded nonzero values: defaults are 10 seconds, 64 KiB backend output and two concurrent actions. The local adapter clears the child environment, supplies fixed PATH/LANG, null stdin, concurrent bounded stdout/stderr reads, and kills/reaps timed-out or overflowing children. Empty output becomes null; other output must be JSON. Raw stderr is never returned or logged.

Audit events contain timestamp, request sequence, phase, trusted operation name, safe outcome and duration, not payloads. Unknown names become a constant. Denials and invalid arguments are audited. The adapter supports JSON/text and stderr/file/Unix syslog, size rotation and retention. Files must be regular, restricted and in trusted directories; symlinks/hardlinks are rejected. Unsupported settings fail validation. stdout is reserved for MCP.

Audit writes run outside the async executor, with one writer per dispatcher, bounded waiting, a deadline and a permanent failure latch. A blocked write cannot spawn unbounded tasks. Runtime shutdown also has a deadline; an OS-blocked write cannot be forcibly cancelled. This is not durable/fsync logging or a sandbox. Failed completion logging does not imply undo, and cancellation is not rollback.

## Coverage and extension boundary

The executable is an on-device or companion stdio server. An operator can carry stdio over SSH to a binary on the device; there is no built-in SSH client, TCP listener, polling daemon, database or embedded interpreter. The official Rust MCP SDK handles the protocol.

The [coverage matrix](coverage.md) distinguishes configured operations, fixture validation and device acceptance. Trusted local extensions can name extra ubus methods or fixed executables, but always require extensions.write AND extensions.execute plus declared effects. They cannot replace built-ins or grant permission. Extensions are an administrator capability, not a sandbox for untrusted authors. Privileged programs can defeat category isolation and must not be exposed to restricted clients.

Full OpenWrt support is a product target, not the current implementation claim. UCI transactions, encrypted backup streaming, verification/rollback, protected resources, package/firmware workflows, native ubus and package-specific coverage need explicit contracts and acceptance tests before being advertised. A generic action template does not constitute a tested workflow.

## Mandatory architecture evolution

When a requirement cannot fit this design, feature implementation pauses: update acceptance requirements, record an ADR, revise this document and the versioned contract, then add harness rejection/regression tests. Migrate the affected layers and pass the architecture gate BEFORE implementing new behavior. Convenience imports, command escape hatches, ignored checks and temporary exceptions are prohibited.

The repository gate validates architecture and negative fixtures first, then formatting, strict linting, behavioral tests and release compilation. CI checks evolution against the pull request base; local verification compares with HEAD unless supplied another baseline. Static analysis cannot prove all semantic properties of approved dependencies or macros; review remains required.

## Rust references

This design uses the official [module/package model](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html), [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), [package layout](https://doc.rust-lang.org/cargo/guide/project-layout.html), and [API Guidelines](https://rust-lang.github.io/api-guidelines/). Ports-and-adapters and this seven-package split are project decisions, not an official Rust application-architecture mandate.
