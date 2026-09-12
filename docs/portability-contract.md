# Portable host API contract (v4)

This design precedes implementation. Public APIs are library/composition boundaries, not new MCP tools. Shared host targets are Windows/Linux/macOS; the controlled target remains OpenWrt. No actual host/platform/SSH acceptance follows from this document.

## host-platform

`HostError` has fixed variants InvalidPath, Unsupported, Unavailable, Insecure, Limit and no raw OS causes/paths. This crate does not depend on runtime and never executes processes.

- `read_config(path: &Path, max_bytes: usize) -> Result<Vec<u8>, HostError>`: config integrity, at most 1 MiB; explicit relative paths may join the current directory lexically without following symlinks. Parent traversal is rejected. Trusted owner, no untrusted writes, regular single-linked files and opened trusted directory handles.
- `read_secret(path: &Path, max_bytes: usize) -> Result<Zeroizing<Vec<u8>>, HostError>`: absolute path, integrity plus confidentiality, at most 16 MiB; callers apply lower purpose limits.
- `PrivateLog::open(path: &Path, max_bytes: u64, retained: usize) -> Result<PrivateLog, HostError>` and `write(&mut self, bytes: &[u8])`: private leaf, trusted parent, handle-relative append/create/rotation. Existing operator-owned 0644 logs may be tightened only after validating ownership/handles. Hard caps 1 GiB and 100 generations; caller AuditConfig retains current lower bounds.
- `SystemLog::open()` and `send(&self, bytes: &[u8])`: supported native system log only, no assumption that every Unix has Linux /dev/log.
- `native_file_protection_supported()` and `system_log_supported()`: accurately report implementation capability, not live availability.
- `verify_openwrt_local()`: Linux plus bounded root-owned /etc/openwrt_release and explicit DISTRIB_ID='OpenWrt'; never execute/source that file. Other distributions/forks are not implicitly accepted.

Initial Linux implementation uses safe rustix opened directory/file handles. Windows/macOS named modules explicitly return Unsupported for unimplemented native protection. Config-from-environment, direct environment key sources and stderr logging remain common alternatives selected explicitly, never fallback. Native platform files and system log are optional facilities, not a prerequisite for the portable remote path.

## backend-ssh

`SshOptions { host: String, port: u16, username: String, host_key_sha256: String }` has strict Deserialize/validate and no raw Debug. Validation is offline and rejects invalid/bounded host/user/fingerprint syntax.

`SshBackend::new(options: SshOptions, identity: Arc<dyn KeySource>) -> Result<SshBackend, RuntimeError>` is offline. First authorized execute lazily obtains the exact source (64 KiB max), connects and authenticates with one unencrypted native OpenSSH Ed25519 private key. Only the configured pinned Ed25519 host key is accepted. File loaders, known-hosts auto-discovery, key-agent APIs and forwarding are not used.

One persistent connection, one active exec channel; overlap returns Busy. Total operation deadline includes bounded single source worker, connect, authentication and complete response. Own a TCP shutdown handle and an RAII failure guard; cancellation/timeout/uncertain session failure poisons the backend until restart. No replay/reconnect after uncertain submission. Zero/nonzero/missing exit status and stream/channel closure are distinguished. stdout/stderr are independently bounded; only valid JSON/empty output reaches runtime. Library errors and server text are not returned or logged.

Programs/arguments are target POSIX data. One bounded encoder quotes every program/argument for remote SSH exec, including embedded single quotes. Do not treat local argv arrays as preserving remote shell argument boundaries. No local process is spawned by this adapter.

Runtime adds safe TargetNotConfigured, UnsupportedTarget, HostKeyRejected and AuthenticationFailed codes. Existing policy/audit/limits remain authoritative; source acquisition never occurs for denied operations.

## server and local adapter

Operator `target.kind` is unconfigured by default, openwrt_local or ssh. SSH adds options, an identity-source alias, source declarations and key limits. Selection is immutable and never comes from MCP arguments. OpenWrt-local can be constructed only after host-platform verification; its verified state is private. Unit-only fixture execution must not create a public constructor bypass.

`--config <path>` uses protected config loading; `--config-env <variable>` reads only that exact operator environment variable with the same size/schema validation. Unknown flags, missing/both sources, invalid names and unsupported native protection fail with fixed codes. This does not unlock Vault or access other environment values. `check`/`catalog` never read keys, connect SSH or probe the router.

Native required suites cover real stdio binary lifecycle via controlled child environment and fake SSH endpoints. The fixture endpoint may bind loopback only, generate keys in memory and must never run router commands. Native OS CI and actual OpenWrt acceptance are separately reported.
