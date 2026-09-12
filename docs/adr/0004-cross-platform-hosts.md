# ADR 0004: Portable host execution, explicit OpenWrt targets and platform protection

Status: accepted design. Architecture v4; functionality follows a validated architecture-only checkpoint.

## Requirement

Windows, Linux and macOS are first-class intended MCP host platforms across the product, not only age. Host portability is distinct from the OpenWrt Linux target and CPU/ABI packaging. A compiling crate, Unix cfg, WSL test or configured CI job is not evidence of native platform acceptance. Native platform integrations may differ, but shared router features must use the same policy/catalog/use cases regardless of host.

## Decision

Keep domain, catalog, runtime, MCP, age and the new SSH backend portable. Add `backend-ssh` for native persistent SSH and `host-platform` for purpose-specific protected host files/system logs. Both are infrastructure layers, not new policy authorities. The server composes providers and never executes router commands. All invocations still use dispatcher authorization and audit.

The machine contract names all three required hosts, portable crates, mandatory non-OS-skipped suites and the native CI workflow. Harness negative tests reject platform cfg/API leaks in portable production and hiding required tests. Native CI executes the full gate and explicit portable suites on Windows, Linux and macOS, with no allow-failure. WSL is an explicit Linux development choice, not native Windows acceptance. No actual CI execution or publication is implied by adding the workflow.

### Host versus target

Operator target selection is explicit: unconfigured (default), openwrt_local or ssh. Unconfigured never executes anything. Local mode must verify Linux and an OpenWrt marker before allowing any local target process. There is no remote-to-local fallback and no desktop-host extension execution. POSIX router paths remain POSIX data even on Windows; do not broaden domain action validation to host-native paths.

The SSH adapter receives validated options and an injected key source, not key paths/environment access. It uses russh 0.63.3 with unused features disabled and ring as crypto backend (not a pure-Rust dependency-tree claim). Only explicit Ed25519 authentication and pinned host keys are initially approved. No trust-on-first-use, automatic host-key updates, interactive password prompts, SSH configuration discovery, forwarding or local subprocess is introduced. Authentication keys and age identities have separate bindings.

Reuse one authenticated connection and bounded exec channels. Apply a deadline from connection acquisition through response completion; bound stdout and stderr independently, require exit status and channel completion, and never replay a submitted operation. A dropped/timed-out SSH channel does not guarantee remote termination or rollback. SSH exec still invokes a remote shell: encode every fixed program/argument with one bounded POSIX quoting function; reject NUL/invalid prepared actions. This is target command encoding, not authorization or a host shell.

The initial SSH adapter permits one active channel per backend, returns Busy for overlap and reuses the connection sequentially. A cancellation/timeout/uncertain connection failure poisons the backend until process restart, with an owned TCP shutdown handle rather than assuming dropping russh's handle stops its driver. Source loading uses at most one blocking worker and a failure latch; repeated requests cannot accumulate blocked reads. Accept one unencrypted OpenSSH Ed25519 private key, bounded to 64 KiB, from the exact injected source. No source or key is cached beyond the connection authority that needs it. Safe failure codes distinguish missing target, unsupported local target, host-key rejection and authentication failure without upstream details.

### Host facilities across all features

`host-platform` owns protected config reads (integrity), secret reads (confidentiality and integrity), private append/rotation and native log delivery. Linux, Windows and macOS implementations are explicit modules; Unix is not a security equivalence class. Existing key-source file access becomes a façade over that implementation. Audit serialization stays in adapters; policy parsing and lifecycle wiring stay in server. No generic arbitrary-file MCP tool or unsafe/FFI exception is added.

Linux implementations verify opened directory/file handles, trusted owner, appropriate permission bits, regular files and links, including rotation parents. Windows requires DACL/owner/inheritance/reparse/opened-handle/volume validation. macOS requires ACL and volume-ownership semantics in addition to BSD mode. Until these native protection profiles are implemented and tested, return explicit unsupported instead of silently omitting checks. Planned optional facilities are not required for the portable baseline: operator-supplied environment configuration/key sources and stderr logging provide a common path. Secret/private configuration values are never printed.

Explicit configuration from a named environment variable is an alternative to protected-file loading. It has the same strict schema/size bounds and grants no extra permissions. It does not auto-discover `.env`, weaken a failed file read, or erase the original OS environment. Supported native file facilities are a feature matrix; unavailable optional facilities do not cause substitution. Personal Vault remains a user-unlocked optional platform integration, not a requirement on every OS.

### Directories and migration

- `crates/host-platform/src/{lib,linux,windows,macos}.rs`: explicit OS dispatch and safe purpose-specific I/O, with fixed safe errors. No process execution.
- `crates/backend-ssh/src`: persistent remote connection/authentication/channel execution; no filesystem, environment or process APIs.
- `crates/adapters/src`: router-local process adapter (explicit OpenWrt opt-in), audit rendering, common backend adapters.
- `crates/key-sources/src`: source/container composition; protected-file access delegates to host-platform.
- `crates/server/src`: typed target selection, host-independent configuration parsing, native/env configuration sources and composition.

The existing v3 host I/O paths are migrated within their infrastructure/composition owners only after the v4 checkpoint; portable layers never acquire native responsibilities. Future native platform implementations and any new dependency/unsafe boundary require the normal architecture-first evolution.

## Acceptance

Preserve policy/protocol/crypto/container regressions. Add portable real-binary stdio startup/list/call-denial/shutdown tests using controlled child environment, and fake in-memory SSH tests for authentication/pinning, argument encoding, connection reuse, bounds and no replay. Do not use a real router or secrets. Keep Linux-only process/file tests explicitly Linux; replace shared shell-program fixtures with Rust fixtures when those tests exercise host-portable behavior. Unsupported native capabilities must have explicit tests; none may report success without operating.

Three native host gates plus separately authorized OpenWrt acceptance are required before broad support claims. This increment can implement the common remote baseline while native protected files/Vault and device testing remain planned. Record exact tested hosts, fixtures, limits and unresolved capabilities.

## Primary sources

- [OpenSSH exec command handling](https://man.openbsd.org/ssh.1)
- [russh client API](https://docs.rs/russh/0.63.3/russh/client/index.html)
- [SSH channel signal semantics](https://www.rfc-editor.org/rfc/rfc4254.html#section-6.9)
- [Windows file security](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights)
- [Apple permission/ACL model](https://developer.apple.com/library/archive/documentation/Security/Conceptual/AuthenticationAndAuthorizationGuide/Permissions/Permissions.html)
