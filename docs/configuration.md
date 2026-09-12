# Configuration

Config is TOML with strict keys. Unknown names/values fail startup rather than silently broadening access. The CLI accepts exactly one explicit `--config <path>` or `--config-env <variable>` source; it never reads credentials from chat or discovers a project `.env`. Configuration is bounded to 1 MiB. Environment names use ASCII letters/underscore with digits after the first character, up to 128 bytes; the original OS environment copy cannot be erased. `--config` requires the native integrity-protection profile, currently Linux only, with trusted owner/parents and a regular single-linked, non-symlink file. A failed file read never falls back to the environment.

## OpenWrt target

No `[target]` means unconfigured: allowed tools return `target_not_configured` and cannot run programs on the host. Select `kind = "ssh"` using the [SSH environment example](../config/ssh-environment.toml), supply a verified Ed25519 host fingerprint and a separate authentication source. The SSH source registry uses the same file/environment/archive abstraction as age but has a separate binding. Only one unencrypted OpenSSH Ed25519 authentication key is accepted, bounded to 64 KiB. These are not age identities.

Alternatively, `[target] kind = "openwrt_local"` opts into local programs after protected OpenWrt marker verification. Windows/macOS and non-OpenWrt Linux hosts are rejected for local execution. SSH has no host-shell subprocess or local fallback. `check` and `catalog` validate targets offline without reading keys, probing host markers or contacting the router. See [platform support](platform-support.md) for optional facilities and native acceptance status.

## Categories and operation selection

`system`, `network`, `wireless`, `firewall`, `dhcp_dns`, `services`, `packages`, `storage`, `vpn`, `firmware`, `diagnostics`, `extensions`.

Each category has `access = "deny" | "read" | "read_write"` and `execute = false | true`. Default: deny/false. To narrow broad category grants:

```toml
[policy]
allow_operations = ["system_info", "system_board"]
deny_operations = ["system_board"]

[policy.categories.system]
access = "read"
execute = false
```

Only system_info is callable. An omitted allow_operations means no additional restriction; an empty list permits nothing. Unknown operation references fail validation. Denials always win.

## Audit log

```toml
[audit]
enabled = true
format = "json"          # json | text
destination = "file"     # stderr | file | syslog
path = "/var/log/openwrt-mcp/audit.log"
max_bytes = 1048576
retained_files = 3

[logging]
level = "info"           # off | error | warn | info | debug | trace
format = "json"          # json | text
```

`logging` controls safe application lifecycle messages on stderr. It does not turn off audit events. Raw SDK debug logging is deliberately not enabled. `audit.enabled = false` explicitly disables usage recording. Stderr is the common host path. File logging uses the host's private-file protection profile. Syslog currently means the Linux local `/dev/log` datagram socket, not a user-provided endpoint or a universal Unix facility. Unsupported profiles fail; no unprotected fallback occurs. Create a trusted log directory on a suitable RAM/persistent volume; avoid excessive flash writes. Rotation is size-based, not time-based. Time rotation, remote delivery and compression can be managed outside the application.

## Limits

```toml
[limits]
timeout_ms = 10000
max_output_bytes = 65536
max_concurrent = 2
```

The runtime rejects zero and unreasonable bounds. Overflow, timeout and saturation return safe errors. Device calls are never automatically retried. SSH additionally permits one active channel per backend and returns `busy` for overlap even when the dispatcher limit is higher. Its deadline includes key acquisition, connection, authentication and channel completion. Timeout/cancellation or uncertain connection failure latches that backend until restart; restarting does not establish whether an earlier operation completed.

## Key sources and encryption

Optional `[protection]` settings select age and exact named sources for internal encryption/decryption primitives. No keys are read during `check`/`catalog`, and those settings do not enable backup MCP tools. See [key management](key-management.md) for file, environment, archive and Vault boundaries, source references and platform limitations.

## Operator action extensions

An action has a stable name, description, exact permission requirements, scalar parameter definitions, a fixed ubus or process target, a required capability prerequisite and explicit result JSON pointers. Extensions always additionally require extensions.write and extensions.execute. Definitions are loaded once at startup and cannot be installed by a tool call. The name `operation_capability` is reserved for the dispatcher-owned metadata tool and cannot be installed as an action.

Ubus actions require an exact input-signature declaration that matches the action template, including fixed literals and optional parameter types. For example, an action targeting `system.info` with no arguments declares:

```toml
[actions.capability]
kind = "ubus_method"
object = "system"
method = "info"
arguments = {}
response_contract = "operator_system_info.v1"
```

`arguments` maps every template argument name to `string`, `integer` or `boolean`; it is mandatory even when empty. A `response_contract` is a bounded versioned implementation identifier such as `operator_system_info.v1`, not a device availability assertion or proof of output validity. Do not use credentials as identifiers. Ubus literal null and nested inputs are unsupported. Checked Ubus integers are restricted to -2147483648 through 2147483647 because larger JSON integers use a different ubus wire type. Input schemas, literal templates, allowed values and invocation validation enforce that range.

Execution also requires fresh observed prerequisites from the same backend. Only the closed reviewed objects `system`, `network.device`, `network.interface`, `network.interface.lan`, `network.interface.wan`, `iwinfo` and `service` currently support introspection. A matching custom action for another object can be configured but remains unknown and cannot execute. A missing object/method can be ACL-hidden; a missing advertised argument can be an incomplete signature. Both fail closed as unknown. A conflicting known argument type is incompatible. Extra arguments advertised by the target do not expand an action, and optional omitted inputs are not checked as transmitted fields.

Process actions must explicitly declare `[actions.capability] kind = "unverified"`. They are blocked by the dispatcher until a reviewed prerequisite is implemented; neither privileged category grants nor choosing a target bypasses this restriction. A Ubus action cannot use `unverified` as an escape hatch, and capability metadata never grants permission. `check`, `catalog` and tool listing stay offline and describe configured definitions, not target availability. The `operation_capability` metadata tool authorizes and audits an exact installed operation before observing it; without invocation inputs, its status conservatively checks every potentially transmitted argument.

Placeholders are whole strings of the form `{parameter}`; interpolation within strings is not supported. Ubus placeholders preserve JSON scalar types. Programs and arguments refer to the OpenWrt target, not the workstation. Local mode uses direct argv without a shell; SSH exec uses a bounded POSIX encoder quoting every program/argument because the remote SSH server invokes a shell. Parameter values may still have program-specific effects; constrain them with allowed_values where appropriate. Only JSON stdout is accepted (or empty stdout); use reviewed wrappers for non-JSON programs. Empty output_fields returns no backend payload.

Custom programs and fields are an operator trust decision. Do not put passwords in definitions or expose secret-producing programs. Adding a package-specific adapter with checked semantics and tests is the preferred route to product coverage. See architecture.md for the public data types.

`output_mode = "scalars"` is the default for every operation, including custom actions: approved pointers whose values are objects/arrays are omitted. An operator can explicitly select `output_mode = "structured"` for a reviewed extension that needs nested results. Sensitive-key redaction still applies, but does not guarantee arbitrary subtrees contain no secrets. The architecture-v2 migration removes the old behavior that inferred projection safety from a built-in name; structured extensions must now opt in explicitly.

The tested [example-extension.toml](../config/example-extension.toml) defines `/usr/bin/printf` with one exact synthetic JSON payload, leaves the target unconfigured and explicitly marks the process as unverified. It demonstrates registration, permissions and a blocked prerequisite, not an executable feature or workstation-command authority. Process placeholders must be required; optional ubus parameters omit the whole key/value. Output pointers must not overlap (such as `/a` together with `/a/b`).
