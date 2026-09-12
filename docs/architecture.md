# Architecture

Status: initial architecture for v0.1; see requirements.md for the product target.

## Dependency direction

`server (MCP/CLI) -> runtime (dispatcher, backend, audit) -> core (policy, catalog, validation)`

The Rust workspace enforces separate crates. Core may depend only on serde, serde_json, toml, and thiserror. It has no I/O. Runtime has no MCP dependency. Server never accesses the router except through the dispatcher. Architecture checks reject forbidden dependencies/imports. No convenience path may bypass these boundaries.

## Invocation lifecycle

1. Protocol parses a named operation and JSON argument object.
2. Dispatcher resolves immutable, operator-installed metadata.
3. Policy checks ALL operation requirements, explicit operation denials, and exact operation allowlists.
4. Input validator rejects unknown/missing keys, wrong types, size violations, and unapproved enum values.
5. Dispatcher records a start event before backend execution; audit failure prevents execution.
6. Backend executes a fixed program and argument vector without a shell, with deadline, output bounds, null stdin, and process cleanup.
7. Output is projected using operator-owned JSON pointers. Nothing is returned by default. Raw backend errors are never returned.
8. Dispatcher records a completion outcome, duration and request sequence. Failed outcome logging does not imply an operation was undone.

Read and action discovery are filtered through the same authorization as execution. Discovery is not evidence that a target provides that capability; calls report unavailable methods as backend failures. v0.1 describes the configured catalog, not complete live device discovery.

## Permissions

Category access is deny/read/read_write. Execute is an independent boolean, disabled by default. Each operation carries a list of category requirements: read, write, or execute. A mutation can require write AND execute; a cross-category action requires every affected category. Category names are a closed enum with a dedicated extensions category. A profile cannot make an unknown operation safe by calling it read-only. Exact operation allowlists can narrow category access, and explicit denials win.

The server process uses one operator-selected policy. MCP clientInfo and tool arguments cannot choose an identity or elevate a profile. Separate stdio processes/configs isolate principals. HTTP, multi-user authentication, and token management are future work.

## Coverage and extension boundary

Built-in operations cover selected structured ubus reads. Trusted local action definitions can expose additional ubus methods and fixed executable argv for installed packages. They have exact names, mandatory permissions, strict parameters, and explicit output projection. Custom actions always require extensions.write AND extensions.execute, plus their declared category requirements. This is a privileged operator extension mechanism, not a sandbox for untrusted plugin authors. Generic root programs, scripts, arbitrary file writes, and package installation can bypass category isolation; they must not be offered to restricted clients as a universal shell tool.

v0.1 supports an on-device/companion stdio process. It can be carried over an operator-configured SSH connection to a binary on the device; no in-server SSH transport is claimed. Standard MCP handling uses the official Rust SDK. No TCP listener, polling loop, database, or embedded language runtime is needed.

UCI mutation transactions, encrypted backup streaming, postcondition verification, durable rollback, protected resources, package/firmware workflows, native ubus socket integration and full package coverage require explicit later adapters. v0.1 has no built-in UCI write, firmware flash, or arbitrary shell tool. Defining a custom action is not equivalent to implementing these workflows.

## Internal interface contract

Core public types (re-exported by lib.rs):

- `Category`: System, Network, Wireless, Firewall, DhcpDns, Services, Packages, Storage, Vpn, Firmware, Diagnostics, Extensions; snake_case serde.
- `Access`: Deny, Read, ReadWrite; `Permission`: Read, Write, Execute.
- `Requirement { category: Category, permission: Permission }`.
- `Grant { access: Access, execute: bool }` (deny / false default).
- `Policy { categories: BTreeMap<Category, Grant>, allow_operations: Option<BTreeSet<String>>, deny_operations: BTreeSet<String> }` with defaults; `authorize(&self, operation: &Operation) -> Result<(), CoreError>`.
- `Parameter { kind: ParameterKind, required: bool, allowed_values: Vec<String> }`; kinds String, Integer, Boolean. No nested inputs. Strings max 1024 bytes and no NUL; parameter names are safe identifiers.
- `Action` tagged by `kind`: `Ubus { object: String, method: String, arguments: BTreeMap<String, serde_json::Value> }`, or `Process { program: String, args: Vec<String> }`. A value/string of exactly `{parameter}` is substituted; no interpolation within strings. Ubus values retain input JSON types; process args stringify scalar types. Programs must be absolute, and cannot be client-selected.
- `Operation { name: String, description: String, requirements: Vec<Requirement>, parameters: BTreeMap<String, Parameter>, action: Action, output_fields: Vec<String> }`. output_fields are nonempty JSON pointers; empty list returns only completion, never raw output.
- `Invocation { program: String, args: Vec<String> }` generated by `Operation::prepare(&Value) -> Result<Invocation, CoreError>`; ubus program is `/bin/ubus`, argv `-S call object method json`.
- `Catalog::new(custom: Vec<Operation>) -> Result<Catalog, CoreError>` installs built-ins and validated custom definitions; `operations(&self) -> &[Operation]`, `get(&self, name: &str) -> Option<&Operation>`.
- `Operation::input_schema(&self) -> Value`; `Operation::project(&self, output: &Value) -> Value`, with projection encoded as a map from JSON pointer to selected value. Projected objects/arrays are recursively redacted by sensitive-key heuristics (defense-in-depth, not complete secret detection).
- `CoreError`: stable safe codes/messages only. Config and client strings must not appear in errors.

Runtime public types:

- `Limits { timeout_ms: u64, max_output_bytes: usize, max_concurrent: usize }`, defaults 10000 / 65536 / 2; validate bounded nonzero values.
- async `Backend: Send + Sync` with `execute(&self, invocation: &Invocation, limits: &Limits) -> Result<Value, RuntimeError>`; `LocalBackend` uses tokio child processes, null stdin and bounded stdout/stderr, kills/reaps on deadline or output overflow; empty output becomes null, otherwise JSON only.
- `AuditEvent` contains timestamp, request sequence, phase, operation name (only trusted names), outcome, duration; never input/output. `AuditSink: Send + Sync` with `record(&self, event: &AuditEvent) -> Result<(), RuntimeError>`.
- `AuditConfig`: enabled bool, format Json/Text, destination Stderr/File/Syslog, optional path, max_bytes, retained_files. AuditWriter creates/reopens append-only permission-restricted files, rotates at configured size. Syslog uses Unix datagram /dev/log on Unix; unsupported platforms fail config validation, no silent fallback. No output on stdout.
- `Dispatcher::new(catalog: Catalog, policy: Policy, backend: Arc<dyn Backend>, audit: Arc<dyn AuditSink>, limits: Limits) -> Result<Self, RuntimeError>`; `available_operations(&self) -> Vec<Operation>`; `async invoke(&self, name: &str, arguments: Value) -> Result<Value, RuntimeError>`. Unknown names log a constant, not the untrusted name. Denials and invalid inputs are audited. Use a bounded semaphore with try_acquire for saturation, not an unbounded wait queue.

Runtime and protocol errors are safe codes. Post-dispatch cancellation is not rollback. Mutations with uncertain completion must be checked on the device before retry; automatic retries are forbidden.

Process parameters must be required so omitting a value cannot shift the meaning of following command flags. Ubus optional parameters omit their entire JSON key/value. Output pointers must be disjoint (after JSON Pointer decoding); overlapping ancestor/descendant projections are rejected to prevent response memory amplification.

Audit sink writes run off the async executor with one bounded in-flight writer per dispatcher, a deadline, and a permanent failure latch. A stalled logger must not stop device deadlines or grow an unbounded set of writer tasks. The executable bounds runtime shutdown waits because an OS-blocked write cannot be forcibly cancelled. This is not durable logging or an OS sandbox.
