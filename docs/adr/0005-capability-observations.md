# ADR 0005: Observed capabilities before device execution

Status: accepted design. Architecture v5; behavior follows a validated architecture-only checkpoint. Checkpoint scaffolds are not capability implementation or device acceptance.

## Requirement and scope

Use the actual BPI-R4 installation as the initial reference without assuming that a release name proves installed APIs. The dated inventory in [reference-target.md](../reference-target.md) is OpenWrt 25.12.5 with an rpcd revision newer than the base release. Package replacement, optional plugins, ACLs and downstream variants can change behavior independently of the release. Full management remains the product goal in [implementation-plan.md](../implementation-plan.md); v5 adds discovery, compatibility gating and support status, not mutations or complete coverage.

An operation needs four separate facts: reviewed implementation, operator authorization, fresh target capability observation, and validation evidence. A successful signature lookup does not prove a safe effect, successful invocation, a particular interface/service instance, or the response contract. A generic command is not feature coverage.

## Ownership and interfaces

- `core::capability` owns pure reviewed-object identifiers, required method signatures, observations and compatibility verdicts. Operation definitions require capability metadata without a serde default. A Ubus method contract binds its exact object, method, transmitted argument types and a versioned response-contract identifier. Catalog validation checks it against the actual action and declared parameters. Explicit unverified contracts cannot masquerade as reviewed Ubus contracts.
- `features` owns each builtin's immutable prerequisites and response contract. Capability metadata does not grant permission or replace effect classification.
- `runtime::capability` owns the probe port contract, private snapshots, expiry and use cases. The existing immutable `Backend` owns both probe and execution; there is no independently configured prober. The dispatcher alone holds a bounded cache for its backend instance and immutable configuration. An opaque backend epoch binds observations to the same connection/authentication generation; invalidation or an unavailable epoch prevents reuse. Snapshots are not caller-supplied values and cannot be shared across dispatchers.
- New portable `device-codec` owns fixed probe/action command encoding and bounded parsing over supplied bytes. It depends on core, not runtime or infrastructure, and cannot perform I/O. Both local and SSH backends use it. Router POSIX paths remain target data on every host.
- `adapters` and `backend-ssh` execute a closed probe on their already selected target, using the same process/connection limits and parser. SSH discovery uses the pinned/authenticated connection that executes the operation; no second credential or discovery connection is selected. Raw probe output never reaches MCP or audit.
- `mcp` maps the `operation_capability` metadata tool to the dispatcher use case. It has no probe implementation, target selection, policy override or backend access. `server` only composes the existing target.

Default backend probe behavior is unknown, never assumed present. A newly implemented adapter cannot inherit compatibility merely by implementing execute. The local adapter has one process-lifetime verified target context and expiring observations. The initial SSH adapter does not reconnect after uncertain failure; a failed/poisoned connection invalidates its observation epoch. Any future reconnection must advance the epoch. Host key, authentication source and configuration are immutable backend authority, so their private cache binding is by ownership rather than exposing pins or usernames in status.

## Closed probes and untrusted output

The initial request is `DescribeUbusObject(ReviewedObject)`, restricted to exactly system, network.device, network.interface, network.interface.lan, network.interface.wan, iwinfo and service. The command is `/bin/ubus` with `-v`, `list`, and the exact reviewed object. No wildcard, `-S`, method call, shell template, client object name or device mutation is a probe. The machine-readable registry records this reviewed set; tests tie the encoder and enum to it. Additional probe families require review and normal architecture evolution when their contract cannot fit.

Verbose ubus listing is text containing an object header and method-signature fragments, not one JSON document. The codec validates the requested object, header shape, bounded method/argument names and types, uniqueness, complete lines and complete transport output. Reject malformed UTF-8, oversized output, duplicate/foreign object headers, duplicate methods/fields, nested unexpected shapes and truncated fragments. Unknown future type labels must not be coerced into a supported type. Bound bytes before parsing and collection sizes before accumulating. Complete transport success is separate from parser success.

Only the fields an operation actually transmits must match the observed input signature. Extra target arguments are permitted; their presence does not add them to the operation. Optional omitted fields are not transmitted and must not cause a positional substitution. The same object can contain read and destructive methods, and a method may contain setters: discovery must never infer permissions from signatures or method names.

The approved scalar Ubus profile excludes literal null, which has no matching scalar signature type. Its Integer is signed 32-bit: libubox emits a different blob type for JSON integers outside that range. Enforce the range in checked Ubus templates, inputs, allowed-value definitions, schema and defensive compatibility evaluation. Process argument validation retains its existing integer domain but cannot execute under the unverified profile. Wider or structured input types need explicit contracts instead of being silently coerced.

## Facts, freshness and failure

Compatibility is compatible, incompatible or unknown. A missing object/method in ACL-filtered listing is unknown (`not_observed_or_hidden`), not proven absence. A missing transmitted argument is unknown (`incomplete_signature`), not proof that the handler rejects it. A conflicting observed supported type is incompatible; an unrecognized type is unknown. Timeout, authentication failure, nonzero status, malformed/truncated output and missing epoch cannot establish compatibility. Return bounded safe reason codes, not raw remote errors. Explicit transport/authentication errors can retain their safe existing codes while the cache remains unavailable.

Pre-implementation clarification after checkpoint 943dcf2: an isolated official 25.12.5 ARM64 image advertised an empty signature for global `network.interface.status`, while a separately controlled read-only call with an interface argument succeeded. This corrects the original inference that a missing argument proves incompatibility. Both classifications still deny execution; the v5 boundary, closed probes and unknown/fail-closed contract are unchanged. Do not override a missing signature with a release/source/test assertion or silently fall back. The generic interface operation may remain unavailable until a separately reviewed adapter can provide its function using a verifiable API (for example a bounded dump-and-select workflow). Emulator evidence and BPI-R4 hardware acceptance remain distinct.

Cache entries are private, bounded by the closed object set, use monotonic time and expire after 30 seconds. Check epoch and expiry both when retrieving and immediately before execution. Capture freshness conservatively across the probe, not from arbitrary device wall-clock time. Forced refresh first invalidates the relevant entry and never falls back to a stale success after failure. Concurrent refreshes are serialized/bounded; a caller cannot create unbounded probes or cache keys. A detected target/session change invalidates observations. Future package/firmware workflows must invalidate them on observed changes. External changes within the TTL are not instantly detected; discovery cannot eliminate the inherent time-of-check/time-of-use race with a privileged external writer.

No supplied release, UID, profile, fixture, `force` argument or client annotation can create a positive observation. Generic Process extensions remain explicit unverified and are blocked by the dispatcher until a reviewed probe contract exists. Custom Ubus actions need matching metadata and a reviewed object; unknown objects are unavailable, without falling back to ungated execution. This deliberately narrows the original administrator extension surface. Even a signature-compatible privileged extension remains an administrator capability, not a sandbox or a claim of protected-resource isolation.

## Audited lifecycle and offline behavior

1. Resolve immutable operation and authorize every category/effect. Reject unknown names and denied policy with no key read, connection, probe or execution.
2. Validate input and obtain bounded dispatcher capacity.
3. Record the start audit event. Failure prevents all device I/O, including discovery.
4. Under one bounded device-work deadline, get/refresh the required observation and evaluate the operation's exact transmitted signature. Unknown and incompatible prevent execution with distinct safe errors.
5. Check freshness/epoch, execute once, project approved output, and record the finish outcome. Completion audit failure retains the existing uncertainty semantics. Do not replay a submitted command.

`check`, `catalog` and `tools/list` remain offline: they show configured, authorized operations, not availability guarantees. No background polling or startup scan is introduced. `operation_capability` accepts only an exact installed operation name and optional refresh boolean. It authorizes that operation using the same policy and audits the attempt under its trusted name; it cannot inspect denied operations. It returns only bounded compatibility/reason/freshness/response-contract metadata, never raw signatures, host identity, object inventories or payloads. It does not execute the target method. Invalid metadata arguments are audited and cannot smuggle execution arguments or target overrides. Reserve its name against custom catalog collision. The tool may be omitted from an empty authorized catalog; invocation enforcement remains mandatory.

The metadata tool's annotations are hints, not authority. Audit distinguishes a capability-only query from normal execution without recording client-controlled names or arguments. Probe and command work share configured capacity/deadlines; audit retains its separately bounded writer/deadline. Cancellation may leave only a start event; it never implies a successful command or rollback.

## Evidence, harness and acceptance

`architecture/spec.toml` names the probe registry, evidence manifest and mandatory feature/runtime/codec suites. `architecture/capability-probes.toml` records the closed commands and runtime ownership/freshness/offline/failure invariants. `compatibility/evidence.toml` separates source, synthetic, native, emulated and exact-device records with target and artifact scope. Targets distinguish physical and emulated identity, and record the actually observed apk or opkg manager. Inventory-only targets cannot claim operation acceptance. Emulated runs cannot claim physical exact-device validation. Source review has no execution host; synthetic tests may record their host, while native acceptance requires a native host. WSL always identifies Linux and cannot satisfy native Windows/macOS acceptance. The actual feature catalog, not a second handwritten operation list, validates operation references in evidence. Evidence is reviewable provenance, not runtime authorization or a cryptographic attestation of a router binary. The harness cannot prove a human actually performed a recorded test; misleading records remain a review failure.

The three new suites execute on native Windows/Linux/macOS CI alongside the existing required suites. Architecture-only scaffold tests assert the recorded contract and are explicitly not behavioral acceptance; after the checkpoint they gain real positive/negative behavior tests. Negative harness fixtures reject altered probe programs/arguments, wildcard or invocation probes, missing capability contract/suites, dependency/I/O leaks, invalid evidence scope and omission of native gates.

Behavioral acceptance includes metadata/action mismatch, missing metadata, policy and audit failures causing zero probes, hidden methods and invalid/truncated data, conflicting/unknown types, stale/foreign/invalidated epochs, failed refresh with no stale fallback, concurrency limits, total deadlines, connection reuse and no automatic replay. Protocol and real-binary fixtures must prove offline listing and capability gating. Parser fixtures are synthetic until separately tested against declared OpenWrt images. Native CI configuration is not a native test result; WSL is Linux only. QEMU userspace is not BPI-R4 hardware acceptance.

Typed bounded collection reads and later package-manager drivers follow their own response/probe contracts; release names select no unchecked fallback. Mutation, encrypted backup publication, device-owned recovery, secret-value channels and protected-resource effects require a later checkpoint. See the explicitly proposed [management-workflows.md](../management-workflows.md).

## Primary sources

- [ubus CLI output and list implementation](https://github.com/openwrt/ubus/blob/master/cli.c)
- [ubus daemon ACL-filtered discovery](https://github.com/openwrt/ubus/blob/master/ubusd_proto.c)
- [procd system method definitions](https://github.com/openwrt/procd/blob/master/system.c)
- [libubox JSON-to-blob scalar encoding](https://github.com/openwrt/libubox/blob/master/blobmsg_json.c)
- [reference rpcd revision](https://github.com/openwrt/rpcd/commit/e37ed9d814699098eb7e26c8b33c054840782dfb)

Links to moving branches explain the protocol family, not proof of a particular installed binary. Dated reference observations and version-pinned behavioral evidence stay separately recorded.
