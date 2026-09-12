# Security model

## Trust boundaries

The operator controls binary installation, configuration, action definitions and log storage. The MCP client controls only operation names and typed arguments. Device output is untrusted. The local process may run as root, so an operator-approved arbitrary executable is effectively privileged authority even when its metadata lists narrower categories.

The permission engine is an application authorization boundary, not an OS sandbox against malicious local operators, root, or trusted extension programs. A client with a separate unrestricted root SSH session can bypass this MCP server; use separate credentials and OS restrictions when MCP is intended as the exclusive management channel.

## Enforcement

All tool calls route through the dispatcher. No protocol method edits policy. Category deny wins; missing grants deny; execute is independent. Exact operation deny rules override allowlists and category grants. Extensions are never available under ordinary read-only grants. Built-ins cannot be overridden by custom actions.

Ubus method names are not safe classifications: `get`-like names may have side effects. Schema discovery is not authorization. MCP annotations describe tools but do not enforce access. Raw UCI configuration and universal ubus/file/shell access are not built-ins.

Audit logs omit argument values, responses, configuration text, backend stderr and parse error excerpts at all verbosity levels. Known operation names are sanitized/validated; unknown names are logged as a constant. Output projection is separate from auditing. Recursive sensitive-key redaction is defense-in-depth, not a guarantee for arbitrary operator-defined output schemas. Operators must review custom result fields.

## Audit behavior

When enabled, an unavailable audit sink prevents a device operation from starting. A completion audit failure is reported separately: it does not mean the operation was rolled back. A start record without a finish means interrupted or uncertain completion, including process crash/cancellation. There is no automatic mutation retry.

The dispatcher performs audit writes off the event loop with a bounded writer and deadline. Audit failure latches the dispatcher closed; recovery requires operator intervention/restart. A stalled OS write may leave one blocked writer until process exit; it cannot consume unbounded workers or stop the event loop. Logging delivery timeouts do not retract a record that the operating system writes later. File records use unbuffered OS writes, not fsync durability. Each file path has exactly one daemon writer; concurrent external rotation is unsupported.

File rotation requires a trusted directory writable only by the operator. Unix mode restrictions and symlink checks do not defeat a privileged attacker or an attacker who controls a parent directory. Audit records are not signed or tamper-evident. Sending to Unix syslog acknowledges delivery to the local socket, not durable downstream storage. Retention and remote forwarding then belong to the syslog service.

## Resource bounds and remaining work

Internal age primitives now have separate public-recipient/private-identity authority, independently bounded sources/containers, redacted non-serializable material and cooperative streaming limits. They are not device backup tools. Native Windows protected-file/Vault access remains unsupported; source availability never causes fallback. Read [key custody, staging and authenticity limits](key-management.md) before integration. Full-stream authentication does not prove backup provenance or authorize a restore.

Inbound newline-delimited MCP frames are capped at 64 KiB before SDK parsing. Process outputs and concurrency are limited. This does not promise resistance to every protocol-level flood; admission tests for aggregate sessions are needed before a remote listener is added. Currently stdio/SSH controls session access and there is no application network listener.

The child program is executed without a shell, with null stdin and a deadline. A timeout/cancellation is not proof a mutation did not happen. Descendant processes created by trusted custom programs require review; process containment is a later hardening area.

v0.1 does not implement encrypted backups, UCI transaction isolation, durable rollback, protected-resource dependency checks, fine-grained remote identities, or OS sandboxing. Do not use privileged custom actions as a workaround for those missing protections on a production router.

Future UCI mutation must account for rpcd `uci.commit` emitting config-change events: even commit can trigger service behavior and requires write plus execute. rpcd pending apply/rollback state is shared and tied to its originating session. Device mutations must be serialized and pending changes detected. [rpcd implementation](https://lxr.openwrt.org/source/rpcd/uci.c)
