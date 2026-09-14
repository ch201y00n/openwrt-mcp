# ADR 0021: Guarded mutation architecture (P2)

Status: accepted for an architecture-only checkpoint; no mutation behavior enabled.
Feature trace: SEC-01, QUAL-01; prerequisites for SYS-02 and TXN-01..11.

## Decision

Adopt the closed [guarded mutation contract](../guarded-mutation-contracts.md).
Its first intent is an existing system section's hostname, with persistent and
kernel hostname effects only. Reject a profile with unknown dependent effects.
Do not invoke the full system reload, generic UCI apply, shell, or an arbitrary
file editor. The current BPI-R4 record is not live admission evidence.

D1 is this deliberately narrow profile, not a claim of current BPI-R4 support.
D2 is an explicitly provisioned, authenticated device guardian with one durable
device-wide admission owner plus an operator-controlled exclusive management
window. External root/LuCI/CLI writers are not isolated by our lock. Detectable
drift denies and conflicting recovery stops for operator intervention.
D3 is durable encrypted recovery with a dedicated device recovery identity,
separate from off-device archival identities. Provisioning this authority is an
explicit deployment decision; absent authority means mutations are unsupported.
Do not copy a Vault, archival identity, or plaintext pre-change backup to a router.
No volatile-only fallback satisfies the profile.

Keep the twelve crate owners. New modules separate pure transaction models,
runtime coordination/ports, target codecs, storage/transport and guardian I/O.
A separately installed guardian executable is composed by the server package;
it is not a thirteenth generic manager library. The host and guardian have
different lifecycles/authorities even when built from shared packages. Neither
the library nor MCP auto-installs, starts, unlocks, or provisions it.

v21 supersedes earlier *blanket consumer* restrictions only at exact module and
namespace pairs in `guarded_mutation_contract.edges`. Retain crate-wide deny lists
as defaults. In particular only runtime/transactions consumes core/management;
only adapters/backups consumes supplied archive/gzip validators; only that backup
bridge and crypto-age/sealing consume the external sealing namespace. The
internal sealing caller is runtime/transactions, never a sibling read handler.
The predicates themselves retain every pure/no-action/no-key/bounded-input rule.
No adapter gains permission to choose a policy or client-selected command.

Six async purpose-specific port declarations, finite state/guard declarations,
requirements aggregation and numeric limits are frozen in architecture/spec.toml.
Read JSON, capture bytes, secret material and job status are separate channels.
The same Dispatcher remains the sole MCP admission path. A device-side recovery
obligation survives client cancellation, host loss and completion-audit failure.
Authenticated durable confirmation, not an SSH reply, ends that obligation.

## Alternatives and consequences

- Broad UCI/service reload: rejected; it enlarges effects and uses shared pending
  state. Full system reload also applies clock/log settings.
- Host-only timers or rpcd's temporary rollback: rejected as durable recovery.
- Archival private key on device: rejected; a separate recovery key has a smaller
  purpose, but adds explicit operator custody and storage obligations.
- Pretend device locks serialize LuCI/root: rejected; require an operational
  window, revalidation and truthful conflict/unknown outcomes instead.
- Crate-per-port or one universal backend: rejected; modules within existing
  owners keep Cargo direction clear without unnecessary package proliferation.
- Automatically approve all host stores: rejected. This checkpoint admits ports
  and scoped integrations, not new Windows SDK exports, macOS ACL claims or unsafe
  code. A native ABI/dependency that cannot fit current rules needs its own ADR
  and negative tests **before** its P3 implementation, not a temporary bypass.

## Enforcement and delivery

The development harness validates the exact contract, every transition/guard,
port syntax, budgets, ownership edges and required regression files. The assembled
source gate applies narrow namespace exceptions, keeps producer re-export bans,
rejects private JSON/action escapes and confines port declarations. Tests include
inert real Cargo/Git fixtures; old versions retain their old prohibitions.
These are architecture tests, not implemented transactions or device acceptance.

Commit this checkpoint after the repository gate. P3 implements bounded capture,
store, secret and authenticated restore adapters with synthetic fixtures. P4
implements and fault-tests the guardian and first complete workflow in an isolated
OpenWrt environment. No production Rust, Cargo dependency, tool catalog, router,
key, Vault or GitHub default-branch setting is changed by P2.

Primary source informing the effect decision: OpenWrt's pinned
[v25.12.5 system init](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/init.d/system).
This review is not installed-file attestation or a license to run that script.
