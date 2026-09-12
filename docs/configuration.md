# Configuration

Config is TOML with strict keys. Unknown names/values fail startup rather than silently broadening access. The CLI accepts an explicit config path; it never reads credentials from chat or a project `.env`.

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

`logging` controls safe application lifecycle messages on stderr. It does not turn off audit events. Raw SDK debug logging is deliberately not enabled. `audit.enabled = false` explicitly disables usage recording. For syslog, the destination is the local Unix `/dev/log` datagram socket, not a user-provided network endpoint. File/syslog support is platform dependent; unsupported configurations fail rather than pretending to provide secure storage. Create a trusted log directory on a suitable RAM/persistent volume; avoid excessive flash writes. Rotation is size-based, not time-based. Time rotation, remote log delivery and compression can be managed by the operating system and are not built-in options.

## Limits

```toml
[limits]
timeout_ms = 10000
max_output_bytes = 65536
max_concurrent = 2
```

The runtime rejects zero and unreasonable bounds. Overflow, timeout and saturation return safe errors. Device calls are never automatically retried.

## Key sources and encryption

Optional `[protection]` settings select age and exact named sources for internal encryption/decryption primitives. No keys are read during `check`/`catalog`, and those settings do not enable backup MCP tools. See [key management](key-management.md) for file, environment, archive and Vault boundaries, source references and platform limitations.

## Operator action extensions

An action has a stable name, description, exact permission requirements, scalar parameter definitions, a fixed ubus or process target, and explicit result JSON pointers. Extensions always additionally require extensions.write and extensions.execute. Definitions are loaded once at startup and cannot be installed by a tool call.

Placeholders are whole strings of the form `{parameter}`; interpolation within strings is not supported. Ubus placeholders preserve JSON scalar types. Process placeholders become one argv element, never shell code. Parameter values may still have program-specific effects; the operator must constrain them with allowed_values where appropriate. Only JSON stdout is accepted (or empty stdout); use reviewed wrappers for programs with non-JSON output. Empty output_fields returns no backend payload.

Custom programs and fields are an operator trust decision. Do not put passwords in definitions or expose secret-producing programs. Adding a package-specific adapter with checked semantics and tests is the preferred route to product coverage. See architecture.md for the public data types.

`output_mode = "scalars"` is the default for every operation, including custom actions: approved pointers whose values are objects/arrays are omitted. An operator can explicitly select `output_mode = "structured"` for a reviewed extension that needs nested results. Sensitive-key redaction still applies, but does not guarantee arbitrary subtrees contain no secrets. The architecture-v2 migration removes the old behavior that inferred projection safety from a built-in name; structured extensions must now opt in explicitly.

The tested [example-extension.toml](../config/example-extension.toml) runs only `/usr/bin/printf` with one exact synthetic JSON payload. It demonstrates registration and permissions without router changes. Process placeholders must be required; optional ubus parameters omit the whole key/value. Output pointers must not overlap (such as `/a` together with `/a/b`).
