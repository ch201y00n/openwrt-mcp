# Architecture

Future scope and delivery are tracked in the [management feature specification](management-feature-spec.md)
and [staged plan](implementation-plan.md). Proposed modules/guardian/storage interfaces
there are approved only where explicitly covered by the current contract. P2's
[guarded mutation design](guarded-mutation-contracts.md) now fixes the first profile,
ports, state guards, recovery authority and exact scoped consumer edges.

Version 21 is the P2 architecture-only checkpoint. Keep the twelve owners and
existing production behavior. Admit only the exact future module consumers in
guarded_mutation_contract, preserving all producer purity and default namespace
bans. Six purpose-specific ports and finite guarded transitions separate private
data, ciphertext storage, authenticated device admission and the same Dispatcher.
An opt-in device guardian has explicit durable recovery/key/resource obligations;
it is not auto-deployed and cannot imply desktop-local router execution. See ADR
0021 for D1/D2/D3 decisions and P3/P4 acceptance still required.

Version 20 extends the exact UCI recipe set from twenty-four to twenty-eight for
LED, Dropbear, uHTTPd and odhcpd selected configuration views. Reuse category-owned
definitions, pure validation, bounded typed projection and the same authorized,
audited, capability-gated dispatcher. No new I/O/port/dependency/probe/form/budget
or mutation authority. Earlier versions retain their exact original recipe sets.
Validate and commit declaration/harness before production expansion; see ADR 0020.

Version 19 admits Windows protected rotating audit files behind the existing host-platform PrivateLog API. A safe windows/log facade and one exact opaque log-writer factory extend the existing native SDK bridge; no handle or SDK type escapes it. Preserve v8 read/security/dependency rules, validate all rotation targets and use no-replacement handle-relative changes with explicit private creation and terminal failure. No Vault/syslog/backup/mutation authority or new portable-layer behavior is introduced. Validate and commit architecture/harness before native writes; see ADR 0019.

Version 18 adds runtime::sealing for validated provided-stream encryption and one-shot ciphertext publication over trusted injected ports. It independently checks bounded read/write counts, true EOF, full archive validation, producer completion, cipher finalization and publication acknowledgement, with explicit uncertainty/cleanup/deadline states. It opens no handles and has no production caller; existing codec/cipher/custody boundaries remain. Only server fixture composition may add a development-only codec edge. Architecture/harness must be validated and committed before behavior. See ADR 0018.

Version 17 adds an explicit bounded single-gzip validator in device-codec/src/gzip, the sole internal consumer of the v16 supplied-tar predicate. Its low-level flate2 1.1.10 Rust-backend edge, header/source/output/ratio limits, count-only completion and production namespace bans are checked before implementation. Owned buffers are zeroizing; the bounded backend history is explicitly not guaranteed scrubbed. No I/O, cipher, custody, capture, extraction, application consumer or management tool is admitted. See ADR 0017.

Version 16 admits a pure incremental archive validator in device-codec/src/archive, with exact nonempty path/size manifests, strict regular-only tar records and count-only completion. The owned header/path buffers may use zeroize; no payload storage, I/O, crypto, extraction or production consumer is admitted. Existing native action-response tests host later behavior cases. All bounds and producer/consumer source bans are enforced before implementation. See ADR 0016; this is neither a backup workflow nor restore authority.

Contract: version 21 in [architecture/spec.toml](../architecture/spec.toml). Read current [ADR 0021](adr/0021-guarded-mutation-boundaries.md) and [ADR 0020](adr/0020-system-service-uci-observations.md), [ADR 0019](adr/0019-windows-private-logs.md) and [ADR 0018](adr/0018-validated-archive-sealing.md) and [ADR 0017](adr/0017-bounded-gzip-archive-validation.md) and [ADR 0016](adr/0016-bounded-backup-archive-validation.md) and [ADR 0015](adr/0015-protected-resource-effects.md), opkg [ADR 0014](adr/0014-opkg-status-observations.md), base UCI [ADR 0013](adr/0013-base-uci-families.md), text options [ADR 0012](adr/0012-bounded-text-options.md), closed UCI [ADR 0011](adr/0011-closed-uci-observations.md), finite observations [ADR 0010](adr/0010-typed-observation-rows.md), LuCI probes [ADR 0009](adr/0009-reviewed-luci-observations.md), Windows reads [ADR 0008](adr/0008-windows-protected-reads.md), package observations [ADR 0007](adr/0007-paged-package-observations.md), typed reads [ADR 0006](adr/0006-bounded-read-projections.md), capabilities [ADR 0005](adr/0005-capability-observations.md), portable hosts [ADR 0004](adr/0004-cross-platform-hosts.md), key custody [ADR 0003](adr/0003-key-custody-and-age.md), the foundational [ADR 0002](adr/0002-architecture-first.md), [requirements](requirements.md), and the [development workflow](development.md) before implementation.

Version 15 adds only the pure core::management model in its own directory: bounded typed identities, immutable supplied effect graphs and conservative before/after influence closure. It returns a count-only non-authorizing assessment, never executes or certifies live topology. All production consumers are forbidden from this namespace; serde/JSON/action/policy APIs stay outside its modules. Later workflow integration requires a separate checkpoint. Existing twelve owners, dependencies, read/package behaviors and all resource ceilings remain; the core security suite is newly mandatory on every native host. No deployed lan3 protection or mutation tool follows from this library prerequisite.

Version 11 admits one additional exact uci description only alongside closed built-in get recipes and unconditional custom UCI denial. Pure core::uci owns config/type/category recipe validation; features owns non-secret fields, while existing runtime/codec/MCP owners handle invocation. TextEnum adds exact finite string validation within the existing scalar projector. No UCI write/session/CLI path or raw option subtree is admitted. Sessionless reads may include shared pending deltas and are not committed/effective state. The architecture-only declaration precedes production migration; see ADR 0011.

## Rust structure and dependency direction

Rust does not prescribe one application architecture. This project combines conventional Cargo workspaces/packages, explicit module visibility and integration-test directories with ports and adapters. Separate crates make dependency direction compiler-visible; the harness further constrains dependencies and production source access.

```text
server (composition) ─┬─> mcp (protocol) ─────> runtime (use cases / ports) ─> core
                     ├─> adapters (I/O) ────> runtime + core
                     ├─> features (category definitions) ────────────────> core
                     ├─> key-sources (custody / containers) ──> runtime::protection
                     ├─> crypto-age (provided streams only) ─> runtime::protection
                     ├─> backend-ssh (remote execution) ────> runtime + core
                     └─> host-platform <── key-sources + adapters (native host I/O)

xtask (development only) -> architecture contract + Cargo metadata + source AST
```

| Crate / directory | Owns | Must not own |
| --- | --- | --- |
| core | Permission rules, validation, prepared actions, safe output projection | Device I/O, processes, async runtime, logging, MCP, built-in device catalog |
| features | Built-in definitions in src/categories, fixture contracts | I/O, orchestration, mutable policy, client-defined capabilities |
| runtime | Dispatcher, concurrency/deadlines, backend and audit ports, safe errors | Concrete device/audit implementations, MCP, configuration file access |
| adapters | Process execution, audit destinations and other port implementations | Authorization decisions, protocol handlers |
| key-sources | Protected file/environment access and bounded archive-entry selection | Encryption algorithm selection/implementation, processes, automatic Vault unlocking |
| host-platform | Purpose-specific config/secret/private-log protection and native system-log facilities | Policy decisions, cryptography, process execution, MCP |
| backend-ssh | Portable persistent SSH connection and bounded remote execution | Local host commands, key paths/environment access, authorization bypass |
| device-codec | Fixed target command/probe encoding and bounded parsing of supplied bytes; core dependency only | Any I/O, runtime use cases, policy decisions, target selection |
| crypto-age | age over provided streams and purpose-specific material | Key paths, filesystem/environment access, containers or Vault behavior |
| mcp | MCP mapping, bounded framing, SDK lifecycle | Concrete backends, filesystem/process access, policy selection |
| server | CLI, trusted configuration loading, lifecycle logs, dependency wiring | Router command execution, alternate device invocation paths |
| tools/xtask | Contract validation and architectural regression checks | Production dependency or runtime feature implementation |

Exact normal, development and build dependency allowlists are versioned in the contract; target-specific declarations are checked too. Test/example fixtures may perform local fixture I/O, but do not grant production access or authorize live router calls.

Version 3 adds two specialized infrastructure crates: server -> key-sources -> runtime, and server -> crypto-age -> runtime. They cannot depend on one another. runtime::protection owns key/source/container/crypto ports, purpose-separated zeroizing material and orchestration. Runtime may name standard Read/Write traits for provided streams, but cannot open filesystem, environment, network, process or standard I/O handles. MCP cannot import protection material or concrete providers. See ADR 0003 for custody, staging, bounds and capability limitations; no raw encryption/identity-management MCP tool is introduced.

## Invocation lifecycle

Version 4 separates the Windows/Linux/macOS MCP host from its OpenWrt target. Portable layers contain no platform cfg/API behavior. Operator configuration selects unconfigured, verified OpenWrt-local, or SSH; desktop hosts must never execute router extension programs locally. Native file/log semantics live behind host-platform and optional unsupported profiles do not silently downgrade. Explicit environment configuration and stderr are the portable baseline. Required native CI and non-skipped portable tests are part of the machine contract; see ADR 0004. Existing v3 native paths migrate after the architecture checkpoint.

1. MCP maps a named tool and JSON object into a dispatcher invocation.
2. The dispatcher resolves immutable, operator-installed metadata.
3. Policy checks ALL category requirements, exact allowlists and explicit denials.
4. Pure validation rejects unknown/missing arguments, invalid types and unapproved values, then creates a PreparedAction.
5. The dispatcher acquires bounded execution capacity and records a start event. Audit failure prevents execution.
6. The dispatcher obtains a fresh capability observation from the same backend and rejects unknown/incompatible prerequisites. Backend-bound private snapshots expire after 30 seconds and on epoch invalidation. Probe and execution share one bounded device-work deadline; the start audit precedes both. The backend executes the prepared action once. Local and SSH adapters use device-codec for fixed target commands and bounded probe parsing; local execution has no shell.
7. The operation projects approved output fields. The dispatcher records the outcome and duration before returning.

The monotonic device-work deadline is checked after observation, immediately before action submission and after synchronous response projection as well as by the asynchronous timeout. A late observation is revoked; a late result cannot receive a success completion audit. This does not preempt synchronous parsing or guarantee hard real-time execution: byte/node/depth/collection limits bound that work, and expiry rejects its result when control returns. Audit admission/completion have their separately bounded logging lifecycle.

Handlers cannot bypass the dispatcher. Backend ports are trusted infrastructure, not client-accessible tools. Discovery and execution use the same authorization; a catalog entry indicates configured support, not proof that the device provides the method. Device actions are never automatically retried.

Version 5 adds core capability contracts and a portable device-codec below infrastructure, not a new invocation path. runtime owns a private per-dispatcher cache and capability_status use case; Backend owns both probe and execute on one immutable target authority. Only seven reviewed exact-object introspection probes are initially admitted. Missing ACL-filtered results are unknown, not proven absence. No version/profile/client argument proves compatibility. The metadata MCP tool delegates authorization and audit to the dispatcher; check/catalog/tools-list remain offline. Unverified Process extensions are blocked until a reviewed prerequisite exists. See ADR 0005 for schema matching, expiry, evidence and migration details.

Version 6 adds typed bounded collection projection within the existing owners. core::projection declares finite Record/ObjectArray/ObjectEntries/ScalarArray forms, fixed typed fields, root exact selection and at most two collection levels; private PreparedInvocation binds validated selectors with the action before I/O. Runtime executes only its action and applies its fallible projection before completion audit. device-codec strictly decodes action JSON, including duplicate-key/depth/node rejection, for both backends. MCP receives normalized results and limits the serialized tool result including text/structured copies. No new crate, dependency, probe family or mutation authority is introduced. The architecture-only checkpoint precedes this implementation; see ADR 0006 and collection-read-contracts.md.

## Domain contracts

- Category: system, network, wireless, firewall, dhcp_dns, services, packages, storage, vpn, firmware, diagnostics, extensions. Unknown values fail validation.
- A category Grant combines Access (Deny, Read, ReadWrite) and an independent execute boolean. Defaults are deny / false. Mutations can require write AND execute; cross-category actions require every affected category.
- Policy holds grants and optional exact operation allow/deny sets. Denial wins; references must resolve against the catalog.
- Parameter has a scalar kind (string/integer/boolean), required flag and optional allowed values. Client strings are bounded to 1024 bytes and cannot contain NUL. Unknown keys are rejected.
- Action is an operator-owned Ubus { object, method, arguments } or Process { program, args } template. Only a whole {parameter} value is substituted. Programs are fixed absolute paths. Process placeholders are required so missing arguments cannot shift meanings; optional ubus parameters omit their entire key.
- Operation::prepare returns PreparedAction::Ubus { object, method, arguments: Value } or PreparedAction::Process { program, args }. Core does not know /bin/ubus or compile command lines.
- Operation capability metadata is required without a default. Catalog validation binds Ubus prerequisites to the action's object, method and argument types plus a versioned response-contract ID. Explicit unverified metadata cannot pass a checked Ubus contract. Client arguments cannot replace these facts.
- Catalog::with_builtins(builtins, custom) validates definitions and rejects duplicate names. Catalog::new(custom) is a generic zero-builtins convenience. The features crate composes the device catalog.
- Operation::project maps approved JSON pointers to values. OutputMode::Scalars defaults to rejecting object/array subtrees. Structured output is opt-in for trusted extensions, with sensitive-key redaction as defense in depth, not guaranteed secret detection.
- v6 typed projections are separate from legacy output_fields/output_mode and mutually exclusive with them. Built-ins cannot use Structured. Private prepared projections own validated root selectors; local-only input parameters do not appear in the actual ubus signature. Typed records use explicit types/presence, unique bounded identities and global scan/emission/serialized-byte budgets. No raw subtree, coercion, silent truncation or fallback is permitted. Legacy helpers retain only their documented scalar/privileged-extension scope; Dispatcher uses the fallible prepared path.
- Empty projection lists return no raw output. Decoded pointers must be distinct and non-overlapping to prevent response amplification. Errors never echo untrusted arguments, configuration excerpts or backend text.

The MCP process uses one operator-selected policy. Client labels and tool arguments cannot select another policy or elevate permission. Separate stdio processes/configurations isolate principals; HTTP and multi-user authentication are not implemented.

## Runtime and infrastructure contracts

Backend::execute(&PreparedAction, &Limits) is an async port returning JSON or a safe RuntimeError. Version 5 extends that same backend with a closed probe and opaque observation epoch, defaulting to unknown; it does not inject an independent discovery target. device-codec parses supplied probe bytes and encodes target commands without runtime or I/O dependencies. AuditSink::record(&AuditEvent) is a synchronous port. Dispatcher owns authorization and orchestration with injected implementations and cannot import an adapter.

Limits have bounded nonzero values: defaults are 10 seconds, 64 KiB backend output and two concurrent actions. The local adapter clears the child environment, supplies fixed PATH/LANG, null stdin, concurrent bounded stdout/stderr reads, and kills/reaps timed-out or overflowing children. Empty output becomes null; other output must be JSON. Raw stderr is never returned or logged.

Audit events contain timestamp, request sequence, phase, trusted operation name, safe outcome and duration, not payloads. Unknown names become a constant. Denials and invalid arguments are audited. The adapter renders JSON/text; host-platform owns private rotating files and optional native system logs. Linux file and /dev/log profiles and Windows v19 private-file logging are implemented; remaining native profiles fail explicitly until available. Files must be regular, restricted and in trusted directories; symlinks/hardlinks are rejected. stdout is reserved for MCP.

Audit writes run outside the async executor, with one writer per dispatcher, bounded waiting, a deadline and a permanent failure latch. A blocked write cannot spawn unbounded tasks. Runtime shutdown also has a deadline; an OS-blocked write cannot be forcibly cancelled. This is not durable/fsync logging or a sandbox. Failed completion logging does not imply undo, and cancellation is not rollback.

## Coverage and extension boundary

The executable is an on-device or companion stdio server with explicit target selection. The native persistent SSH backend manages a remote OpenWrt target from a workstation; verified OpenWrt-local mode runs on the device. There is no application network listener, polling daemon, database or embedded interpreter. The official Rust MCP SDK handles the protocol. See [platform support](platform-support.md) for implementation versus native acceptance.

The [coverage matrix](coverage.md) distinguishes configured operations, fixture validation and device acceptance. Trusted local extensions can declare extra ubus methods or fixed executables, but always require extensions.write AND extensions.execute plus declared effects. Version 5 additionally blocks unverified Process contracts and Ubus prerequisites without a reviewed probe. They cannot replace built-ins, reserve metadata tool names or grant permission. Extensions are an administrator capability, not a sandbox for untrusted authors. Privileged programs can defeat category isolation and must not be exposed to restricted clients.

Full OpenWrt support is a product target, not the current implementation claim. UCI transactions, encrypted backup streaming, verification/rollback, protected resources, package/firmware workflows, native ubus and package-specific coverage need explicit contracts and acceptance tests before being advertised. A generic action template does not constitute a tested workflow.

## Mandatory architecture evolution

Architecture v14 adds an explicit opkg root-status capture profile beside APK, with no manager fallback. Keep core records distinct, the existing shared private snapshot/dispatcher lifecycle, same-backend purpose-specific capture ports and bounded pure codec parsing. The exact reviewed opkg version option exits before initialization; a fixed cat reads only the root status file. No opkg list/status initialization, configuration loading, directory creation or scripts under Read. All owners, dependency edges and shared package/output budgets remain. Add profile binding and bounded stanza validation, then validate/commit the declaration and negative harness before behavior. See ADR 0014.

Architecture v13 expands only the closed UCI config/type recipe set from six to twenty-four. Exact profile/category mappings remain pure core::uci; category modules own selected scalar/TextOption fields. The eighteen added base-service profiles are enumerated in ADR 0013 and the harness, with no dynamic types, client configuration paths, new introspection or privileged extension exception. Existing twelve owners, prepared invocation, no-retry dispatcher, response forms and every resource ceiling remain unchanged. Commit the validated architecture-only checkpoint before enum/catalog migration. Reads are not an effect graph or mutation authorization.

Architecture v12 adds only a terminal nested TextOption collection in core::projection. Preserve string/list representation using fixed kind/values output, exact text bytes, list order and duplicates; reject root/selection, coercion and mixed/nested non-text values. Count all values under existing global scan/emission/serialized-byte budgets, with a 128-value per-option cap. Core::uci admits only this nested form while retaining all six closed recipes and custom denial. Features owns exact option selection and changed response IDs; no new I/O owner, runtime path, dependency, probe or mutation. Validate and commit the architecture-only checkpoint before behavior. See ADR 0012.

Architecture v10 extends only pure finite projection: RowArray preserves ordered observations without invented identity/selection, FalseOrSafeInteger preserves the exact finite sentinel union, and up to four operator-owned root absence guards reject explicit error payloads before projection. Existing resource identity rules, owners, two-level schemas, budgets and dispatcher/MCP paths stay unchanged. DHCP leases are first consumers; storage adopts stricter error guards with v2 response IDs. The validated architecture-only checkpoint precedes projector/feature implementation. See ADR 0010.

Architecture v9 admits exactly two more LuCI object descriptions under the versioned `base_luci_v2` profile. Registry schema 2 requires v9 and all nine exact objects; schema 1 is the closed pre-migration subset. No runtime owner, response form, authority or dependency changes. Storage reads remain finite typed projections in their category, and DHCP unions/composite identities must not be forced into v6. The architecture-only checkpoint precedes production enum/registry migration and feature implementations. See ADR 0009.

Architecture v8 admits one private native Windows read boundary, `host-platform/src/windows/native.rs`, and a safe sibling `policy.rs`. Only this exact native source file may use unsafe expressions for reviewed Windows SDK calls; unsafe functions/traits/impls, manual FFI declarations, public handle/pointer APIs and namespace re-exports remain forbidden. The host package uses deny(unsafe_code), with a file-local allowance; every other production package retains workspace forbid. The harness checks file ownership, SDK target/version/features, lint boundaries and the required native suite. Handle-relative NTFS traversal, conservative owner/DACL validation and bounded reads implement the existing config/secret ports, not a new MCP or raw filesystem API. Private logs, system logs, Personal Vault and macOS protection are outside this profile and remain explicit gaps. See ADR 0008; validate and commit this architecture before behavior.

Architecture v7 adds the purpose-specific package observation workflow in [ADR 0007](adr/0007-paged-package-observations.md). `core::packages` owns bounded records and pages; `runtime::packages` owns a single ephemeral snapshot and injected entropy port; `device-codec::packages` owns closed APK commands and supplied-byte parsing; `adapters::tokens` owns getrandom entropy. The existing local/SSH backends implement the same closed capture port. MCP does not own pagination or I/O. This extends Action/CapabilityRequirement without enabling generic Process, widening v6 projections, or adding any mutation. Every page uses Dispatcher authorization, admission and audit. Its APK-visible non-atomic scope is not whole-device completeness.

When a requirement cannot fit this design, feature implementation pauses: update acceptance requirements, record an ADR, revise this document and the versioned contract, then add harness rejection/regression tests. Migrate the affected layers and pass the architecture gate BEFORE implementing new behavior. Convenience imports, command escape hatches, ignored checks and temporary exceptions are prohibited.

The repository gate validates architecture and negative fixtures first, then formatting, strict linting, behavioral tests and release compilation. CI checks evolution against the pull request base; local verification compares with HEAD unless supplied another baseline. Static analysis cannot prove all semantic properties of approved dependencies or macros; review remains required.

## Rust references

This design uses the official [module/package model](https://doc.rust-lang.org/book/ch07-00-managing-growing-projects-with-packages-crates-and-modules.html), [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html), [package layout](https://doc.rust-lang.org/cargo/guide/project-layout.html), and [API Guidelines](https://rust-lang.github.io/api-guidelines/). Ports-and-adapters and this package split are project decisions, not an official Rust application-architecture mandate.
