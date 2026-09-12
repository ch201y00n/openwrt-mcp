# ADR 0006: Typed bounded read collections

Status: accepted design for the architecture-v6 checkpoint. Production implementation follows a separately validated architecture-only commit. Checkpoint tests are not collection behavior or device acceptance.

## Why this changes the architecture

Comprehensive management needs lists, named records and nested service instances. The v5 flat scalar projector cannot express these, cannot bind a local selector and cannot report a malformed response. Its privileged `Structured` option is not an acceptable substitute for reviewed built-ins. Ordinary JSON decoding also loses duplicate keys before projection can detect them.

The initial emulator revealed a concrete need: `network.interface.status` advertises an incomplete input signature, whereas the separately reviewed no-argument `dump` method returns records that can be selected locally. We will change the operation's fixed implementation, not retry a failed status call or override missing capability evidence. The existing seven exact-object probes already cover the first collection reads. No new probe, execution bypass, crate or production dependency is introduced.

## Owners and prepared invocation

- `core::projection` owns finite declarative shapes, validation, scalar types, budgets, selector binding and fallible projection. Use standard Rust modules with private helpers; core remains free of I/O and async dependencies.
- `features` owns reviewed source paths, output names, types, presence requirements, narrower limits, permission requirements and response-contract versions. The actual catalog is authoritative; fixture coverage must follow it rather than a duplicate operation list in xtask.
- Core prepares an immutable `PreparedInvocation` containing the action and a bound projection. Fields and construction are private; provide an action accessor and a fallible projection method. Bound projections are not deserializable, serializable or raw-debuggable client input. Backend receives only the existing `PreparedAction`.
- `runtime::Dispatcher` authorizes, validates and binds input before admission/audit/device I/O. It probes the same backend, executes once, applies the prepared projection, enforces normalized result bounds, then records completion. Projection failure is a failed outcome, never partial success or a retry.
- `device-codec` adds a duplicate-rejecting bounded decoder for supplied action-response bytes. Both local and SSH adapters use it before producing `Value`. No I/O moves into the codec. Complete transport success remains a separate prerequisite.
- `mcp` maps already normalized results and enforces the serialized tool-result bound including its text and structured copies. It cannot select fields, inspect raw responses or bypass runtime. Server continues to compose only.

Existing public scalar helpers may remain for source compatibility, but Dispatcher uses only the prepared invocation path. A collection cannot execute through an unbound legacy projection. Reviewed built-ins cannot select `OutputMode::Structured`; its existing privileged extension boundary remains explicitly separate. Typed and legacy projection metadata are mutually exclusive, not two outputs concatenated together.

Parameters must be used by action arguments or a declared root selector. An unused parameter is invalid. A selector-only parameter is required nonempty String input, bounded by its identity field, including declared allowed values and input-schema bounds. Identity fields and map identities are nonempty; ordinary non-identity Text may be empty if its contract permits it. Unknown/missing/invalid selectors fail before I/O. Capability metadata describes only actual transmitted arguments: local selection does not fabricate an ubus input field.

## Finite response forms

Use a finite composition of records and collections, not an arbitrarily recursive `Node`, JSONPath interpreter, callback or wildcard evaluator. A leaf record contains fixed scalar fields. An inner record may additionally contain named collections of leaf records. A root record/collection may contain those inner records. This permits at most two collection levels, sufficient for service -> instances, without permitting recursively nested configuration schemas before validation.

The approved forms are:

1. Record: statically named fields with exact relative JSON Pointers, scalar kinds and presence requirements; named collections have the same explicit presence policy.
2. ObjectArray: an array of object rows projected through a record. A unique identity references an existing required bounded-text field, not a second arbitrary client pointer.
3. ObjectEntries: a map of object values projected as record rows. A map key is emitted only through one explicit statically named bounded-text identity field. It counts toward the field limit and cannot collide with another field.
4. ScalarArray: exact scalar elements normalized into rows with one explicit fixed output field. A reviewed definition declares whether duplicates are rejected; the initial wireless-device list requires uniqueness.

Root results are objects: list results use a fixed `items` array; a root `ExactOne` returns its selected record. A fixed root record can expose named scalar/collection fields. No dynamic source keys become output keys. Empty source pointers are allowed only where the reviewed form means the current root/value, not raw subtree copying. Pointers have exact segment semantics, no interpolation, regex, prefix matching or wildcard expansion. Decode and validate pointer escapes; reject duplicate/overlapping source fields within a record and duplicate output aliases, including scalar/collection/key collisions.

Selection is `All` or root-only `ExactOne` using exact equality between a reviewed identity and a validated bound parameter. Nested collections are All-only. ScalarArray has no single-selection mode in this profile. A list or selector cannot change the program, object, method, policy, projection schema or limits.

## Types and failure semantics

Scalar kinds are Boolean; Text with an explicit UTF-8 byte bound and no NUL; SafeInteger with declared inclusive bounds inside +/-9,007,199,254,740,991; and explicit DecimalCounter. SafeInteger rejects floats, coercion and out-of-range values. DecimalCounter selects either an exact unsigned JSON integer or canonical unsigned decimal text as its source, range-checks u64, and always returns a canonical decimal string. It never changes output type based on magnitude. This avoids silently rounding large counters in IEEE-754-based clients. These stronger rules do not retrospectively describe untyped legacy scalar outputs.

Missing optional fields or explicitly optional nested collections are omitted. A required field/container that is missing, or a present null/wrong type/range, is a safe response-contract error. A valid empty collection is a successful empty observation, not proof of global absence or complete authorization visibility. A root selector with no match returns `selection_not_observed`; duplicate identities or ambiguous matches are malformed output. Do not choose the first match, invent healthy/stopped state, substitute another record, coerce a scalar, or return first-N/partial results after overflow.

Validate the entire bounded reviewed source structure, including row shapes, declared fields and identities, before exposing a selected result. Ignore undeclared sibling fields without cloning them. Array identities are unique across the whole visited collection, including unselected rows. A duplicate JSON key is rejected earlier at the raw decoder, including escape-equivalent keys; it is distinct from duplicate row identities.

An upstream service response may omit instances. That is omission, not an empty/stopped instance list. A string field remains untrusted device data and may contain instruction-like text; static reviewed fields are the primary disclosure boundary. Sensitive-name filtering is defense in depth, never a guarantee that arbitrary values contain no secrets.

## Bounded work and serialization

The versioned contract fixes hard ceilings; operation definitions may choose lower values, never raise them:

| Bound | Ceiling |
| --- | ---: |
| Collection nesting | 2 |
| Collection schema nodes across a definition | 8 |
| Schema levels / JSON Pointer segments | 8 / 8 |
| Source pointer bytes / output-name bytes | 512 / 64 |
| Items in one collection | 256 |
| Total visited items / total emitted collection items per invocation | 256 / 256 |
| Fields per record, including map identity and named collections | 64 |
| One declared text field | 1,024 UTF-8 bytes |
| Normalized result JSON | 65,536 bytes |
| Serialized MCP CallToolResult, text + structured copies included | 262,144 bytes |
| Action JSON decoder input | min(configured byte limit, 16,777,216 bytes) |
| Action JSON container depth / value nodes | 32 / 65,536 |

Budgets belong to one invocation and are shared by every nested collection, not reset for each service or row. Check source cardinality against the remaining scan budget before iterating, even if the desired row appears first. Count output rows/elements before appending and serialized scalar/key/container bytes before cloning. Use checked arithmetic and account for UTF-8 and JSON escaping; never estimate solely from Rust string length or input bytes. No core filesystem/stream handle or extra dependency is needed for pure byte accounting.

The decoder checks bytes before parsing, charges each value node and container nesting before descent, detects duplicate map keys before insertion, and requires one complete JSON document without trailing garbage. Empty/whitespace responses may preserve the legacy null behavior, but typed collections reject that shape. Syntax, duplicate, depth and node failures become fixed safe errors without raw text. A configured large backend cap does not enlarge the normalized or MCP response cap.

Runtime bounds every normalized result before finish audit, including legacy results; the prepared legacy path charges its budget before recursive cloning and stops bounded traversal on failure. Calling the old projector to create an oversized clone and only then measuring it is not permitted on Dispatcher's path. MCP verifies its actual serialized CallToolResult size before returning success. This is a tool-result bound, not a claim that an entire potentially large catalog listing is paginated or bounded to the same size. Protocol-envelope overhead and the incoming request-ID limit remain separate framing concerns.

## First feature contracts and migration

The initial five contracts are specified in [collection-read-contracts.md](../collection-read-contracts.md). Four are new: network_interfaces, wireless_devices, service_status and service_status_list. Existing network_interface_status changes to a fixed dump + exact selection with response-contract `network_interface_status.v2`. It intentionally changes its development-stage result from slash-prefixed projection keys to ordinary typed field names and includes the selected interface identity. Document the change; do not silently claim v1 compatibility. No published stable release currently depends on this contract.

Every operation remains an independently authorized read. Generic Services.Read includes service/instance names and process-running metadata, even for VPN/firewall-related daemons. It does not imply access to their configuration, command lines, environment, health semantics or lifecycle execution. Operators needing to hide generic metadata can deny the two generic service tools and permit only the existing fixed service operations. Do not guess security categories from daemon names.

Projected observations are not transaction baselines or protected-resource graphs. Future mutations must separately resolve full private identity/dependencies and protect lan3 and indirect effects. An omitted field is never proof that a protected dependency does not exist.

## Harness and evidence

The spec names the projection/decoder/transport owners, hard limits, failure rules and five mandatory native suites: core collection projection, feature collection contracts, runtime collection projection, action-response codec and MCP bounded results. All run on Windows/Linux/macOS CI without OS exclusions, ignore or replacement no-op targets. The checkpoint includes explicitly named declaration scaffolds only; replace/extend them with behavior after committing the architecture.

Negative harness tests reject missing/weakened bounds, unreviewed forms, wrong owners, absent required suites, I/O/dependency leaks, skipped native jobs and incomplete evolution evidence. Fixture coverage derives operation/response-contract IDs from the actual feature catalog. Semantic tests cover strict metadata, type/range/presence, duplicate/ambiguous identity, escaped duplicate JSON keys, whole-input scanning, exact/over limits, nested aggregate budgets, UTF-8/escaping, denied/invalid/unaudited zero I/O, safe completion errors, unchanged legacy scalar guards and both serialized response copies. The gate is not a proof of arbitrary library behavior; independent review and distinct synthetic/emulated/exact-device evidence remain mandatory.
