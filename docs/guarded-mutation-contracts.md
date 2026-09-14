# Guarded mutation contracts — P2 / architecture v21

Status: **architecture-only**, not an implemented management workflow. The current
55 read tools are unchanged. Machine declaration: `guarded_mutation_contract` in
[spec.toml](../architecture/spec.toml). Decision: [ADR 0021](adr/0021-guarded-mutation-boundaries.md).
This document supersedes conflicting candidate choices in
[first-mutation-readiness](first-mutation-readiness.md) and
[management-workflows](management-workflows.md) only for this closed first profile.

## 1. First intent and compatibility (D1)

`guarded_hostname_v1` implements SYS-02's hostname scalar only. The client supplies
one new ASCII DNS label, 1..63 bytes, letters/digits/hyphen, alphanumeric ends.
No dots, whitespace, control characters, templates, paths, option names or caller
section indices. No-op equality is an observed no-change result, not a transaction.

Resolve exactly one existing `system` section in the **committed** system file.
The original hostname must be one scalar matching the same grammar; missing,
duplicate/list, ambiguous/multiple sections and unsupported UCI syntax deny.
Anonymous sections are supported by a private locator bound to the whole exact
preimage and unique section, never by a reusable client index. Reject foreign
pending changes, extra override/delta paths and unreviewed parser profiles.

The future pure target codec recognizes a bounded reviewed UCI grammar, preserves
every byte outside the one hostname token and verifies the resulting file again.
It is not a generic UCI editor. Unsupported escaping/continuations/duplicates must
fail closed, not be reserialized heuristically. P4 fixtures must cover the exact
supported grammar and compare it with pinned libuci interpretation. No libuci FFI
or new dependency is implicitly authorized here.

Effects are (1) replace the committed hostname token while preserving unrelated
settings and file metadata and (2) set the kernel hostname. There is no system
reload, config-change broadcast, DNS/DHCP/mDNS restart, network reload or reboot.
OpenWrt's [pinned system init](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/init.d/system)
also changes logging/timezone/kernel clock settings; invoking it is outside this
effect scope. A daemon may observe the kernel hostname without a restart: those
dependencies must be included in the installed profile's influence graph. Unknown
consumers or a path to protected resources deny. DNS/mDNS names are not promised
to change just because the hostname changed.

Use the nonempty operator-owned protected set and both before/after graphs;
clients cannot remove protected resources. BPI-R4's lan3 KT IPTV WAN passthrough
must remain protected, including indirect dependencies. A profile cannot claim
safety by omitting lan3 from its graph or checking only the hostname string.

The first emulator profile can exclude hostname-consuming optional services. The
recorded BPI-R4 profile includes optional services and is **not yet admitted**.
P4 must close its actual hostname dependencies or report unsupported; never
silently substitute an emulator profile. Release strings, historic backups and
permission to call `uci.get` do not establish this evidence.

## 2. Ownership and conventional Rust directories

These are admitted module locations, not newly implemented modules. Keep ordinary
Cargo `src`, `tests`, explicit visibility and `foo/mod.rs` for these boundaries.

| Owner / directory | Responsibility |
| --- | --- |
| `crates/core/src/transactions/` | Typed intent, immutable baseline binding, plan/state, required-permission union. Pure, private fields; no JSON, clock, I/O or authority constructor from client data |
| `crates/core/src/management/` | Existing non-authorizing before/after influence predicate, unchanged |
| `crates/features/src/categories/system/` | Closed hostname operation metadata and sanitized schema; no target execution |
| `crates/runtime/src/transactions/` | Methods on the existing Dispatcher, plan cache, coordination and verification. No filesystem/network/process work |
| `crates/runtime/src/mutation_ports/` | Six narrow ports below, private bindings/leases/receipts; `secrets/` owns non-printable purpose-specific secret values |
| `crates/device-codec/src/transactions/` | Pure bounded versioned guardian frames and closed hostname parser/patch, no host I/O |
| `crates/adapters/src/backups/` | Supplied archive/gzip-to-sealing bridge, bounded worker ownership and ciphertext-store adapters |
| `crates/adapters/src/guardian/` | OpenWrt-only admission/journal/hostname/recovery I/O behind reviewed ports; no universal command service |
| `crates/backend-ssh/src/transactions/` | Same-target authenticated binary/control channels; no host-side router commands |
| `crates/key-sources/src/secrets/` | Operator-configured secret reference resolution; never algorithm choice |
| `crates/crypto-age/src/sealing/` | Provided-stream session bridge only; no paths/custody |
| `crates/host-platform/src/{linux,windows,macos}/` | Purpose-specific protected storage primitives, subject to existing native/unsafe contracts |
| `crates/server/src/transactions/` | Trusted construction/configuration of services, no execution |
| `crates/server/src/bin/openwrt-mcp-guardian.rs` (P4) | Separate opt-in executable composition, runtime target check; no I/O logic here |
| `crates/mcp/src/` | Safe plan/job DTO mapping via Dispatcher only, no private namespaces |

The executable is not deployed by stdio startup. Host-common features must compile
and behave consistently on Windows/Linux/macOS. A guardian invocation on a desktop
or an unverified target returns unsupported; it never falls back to local commands.
Installed OpenWrt-side procd packaging/lifecycle is separate from desktop CI.
The twelve crate dependency allowlists are unchanged. Any new SDK factory or
dependency still requires an architecture checkpoint; logical port admission does
not override the current Windows native function allowlist or portable restrictions.

The exact allowed producer/consumer pairs are in `edges`. All other production
consumers and flattened public aliases remain denied. Existing core management,
archive, gzip and sealing producer rules and limits remain in force. In particular
transport cannot import mutation ports, private plans or ciphertext machinery.

## 3. Typed data and six ports

The exact Rust trait declarations are in the machine contract and syntax-checked
by xtask. They are declarations, not compiled production APIs or six implementations.
Traits use async futures for I/O and `Send + Sync` providers; injected ports are
trusted infrastructure. Implementations use the workspace's existing async-trait
convention when object-safe dispatch is needed. No generic JSON execute method.

| Port | Receives / returns | Obligations |
| --- | --- | --- |
| MutationObservation | TargetBinding + WorkBudget → PrivateBaseline | Same authenticated target; committed bytes, pending/override evidence, kernel hostname, boot/profile/effect evidence; bounded capture, no shared-delta snapshot passed off as committed |
| DeviceAdmission | ValidatedPlan + OwnerBinding → Admission | Device-wide durable exclusive owner, fresh complete recheck, no settings changed |
| BackupCapture | Admission → CaptureLease | Binary source from the admitted immutable scope, independent manifest/length/EOF/producer completion; no JSON/base64/stdout result |
| CiphertextStore | ArtifactBinding → StageLease; reconcile → PublicationState | Private creation, bounded sink, one finalize, no-replace publish, durability receipt, explicit published/not-published/unknown reconciliation |
| SecretSource | SecretReference + SecretPurpose → SecretValue | Only preconfigured exact reference, purpose/type binding, zeroizing bytes, no Debug/Display/serde; hostname profile does not resolve a secret |
| DeviceJobControl | Admission + DurableBackup → ArmedJob; apply_once/status/confirm/recover → typed receipts | Never generic action execution; durable owner/job binding, at-most-once submission, status reconciliation, device-owned recovery obligation |

`TargetBinding` includes authenticated target identity and connection epoch.
`OwnerBinding` is derived from operator-provisioned authentication, not a caller
label. Plan/job handles have 128-bit random nonces and are bound to owner, target,
boot generation, profile revision, policy revision, effect closure and expiry.
Knowledge of a handle is not ownership. Entropy failure denies admission.

PrivateBaseline retains exact admitted data privately; public IDs are random,
not deterministic hashes of secret-bearing config. ValidatedPlan, Admission,
DurableBackup, ArmedJob and VerifiedJob have private constructors with explicit
evidence. A client or deserializer cannot manufacture them. Receipts distinguish
observed evidence from success. Foreign/stale handles return bounded stable errors.

CaptureLease/StageLease own one finite stream session and its worker/control
handle; completion is independent of data EOF. Adapt existing supplied-stream
sealing ports, not a second crypto pipeline. WorkBudget carries an absolute
monotonic deadline, remaining bytes and cancellation token; nested calls do not
reset it. One bounded worker per host uses at most two 64-KiB transfer slots.
The adapter owns cancellation, source close and joining; dropping a future must
not orphan a `spawn_blocking` task. Non-interruptible codecs are byte-bounded, not
claimed to be hard real-time. P3 must test joining and shutdown under fault.

Public plan/job output contains only approved summary, state, category/effect
names, random ID, deadline and stable error/evidence codes. No baseline, raw
manifest, key/reference locator, payload, raw error or exception string. Logs add
only random correlation IDs, transition/decision/duration and bounded outcome.
Secret material travels through a purpose-bound binary channel, never argv/env,
ordinary action JSON, logs or MCP. Existing key-source/container and crypto-provider
abstractions remain separate; changing encryption needs a reviewed provider.

## 4. Permissions and the one dispatcher

Every client plan/status/apply/confirm/recover call resolves server-owned metadata,
checks exact operation denies and every required category, and audits admission
before any device I/O. Plan requires System.Read; status also requires job ownership.
Apply aggregates System.ReadWrite **and execute**, every affected category and
backup access. A backup is not a way to read an otherwise denied category. The
closed hostname scope needs only System; any additional effects must obtain their
own union or fail. Discovery and invocation use the same policy.

Confirm and requested recovery reauthorize the original union and job owner, plus
fresh observation requirements. Read-only users cannot initiate changes through
confirm/recover, and broad Services permissions do not imply System permissions.
Policy changes invalidate unapplied plans. Revoking client access after applying
cannot revoke the guardian's already-admitted obligation to recover. This is not
a client permission exemption and cannot authorize a new change.

Audit failure before the first effect prevents it. After durable apply intent,
audit failure is a failure outcome, never an excuse to abandon recovery, clear a
job or misreport success. The device retains its recovery record even when the
host audit sink is unavailable. No second direct MCP backend or auto-retry path.

## 5. Lifecycle and concurrency (D2)

The machine contract fixes 12 states and 21 guarded transitions. Normal path:

`planned → admitted → backup_complete → recovery_armed → applying → awaiting_confirmation → confirmed`

After apply intent, failure/expiry enters `recovering`; only fresh restored-state
verification plus durable terminal recording permits `recovered`. Recovery error
enters `recovery_failed`. Lost evidence enters `outcome_unknown`, never success or
implicit cancellation. Terminal confirmed/recovered/cancelled states cannot replay
apply. A no-reply apply request is reconciled by authenticated job status, not sent
again. Internal recovery writes may be resumed against journaled step preconditions;
this is not retrying a submitted client mutation.

The `failure_or_expiry` event groups an authorized caller abandoning the apply
through `recover`, verification/audit failure and device-timer expiry. Caller
recovery still requires the original permission union; the autonomous expiry
path fulfills the existing obligation, not a new client instruction.

Before apply intent, cancellation proves no settings write, durably disarms if
needed, then releases ownership. A published archival backup is not deleted by
cancel. After apply intent, client cancellation/disconnect does not release the
slot. An armed job that never receives apply expires safely with the no-write
proof. A crash before that proof blocks admission pending journal reconciliation.

The guardian binds an owner to provisioned SSH principal/forced-command or protected
local IPC peer credentials and its private journal. It holds one device-wide
admission slot across all hosts until proven terminal. Device admission is not
rpcd session ownership. Future IPC accepts a fixed protocol/version and typed
messages only; no supplied executable, ubus object/method, path or UID.

Operator must establish an exclusive management window covering LuCI, CLI,
scheduled jobs and other controllers. Recheck pending state, exact committed
preimage, boot/profile/policy and protected-resource closure at admission, just
before apply and before confirm. Root can race between checks or spoof state:
the lock is cooperative, **not root isolation**. Report this limitation prominently.
If a conflicting writer is detected after apply, never restore a whole stale
system file blindly. Restore only against a recognized journaled pre/postimage;
unrecognized state latches recovery_failed and retains evidence for the operator.

Confirmation is a device-serialized compare-and-swap on job generation, exact
verified postimage, owner and unexpired device deadline. Check expiry again before
persisting the terminal record. A concurrent expiry wins by the same ordering;
no host clock or duplicate request extends the 120-second window. Lost confirm
reply reconciles the durable terminal record; uncertain fsync/ack is not success.

## 6. Encrypted backup, restore and durable recovery (D3/D4)

Before any setting write, complete a public-recipient age archival backup and
confirm a durable ciphertext publication receipt. The first scope is the admitted
system file plus bounded recovery metadata; paths/manifests are server-owned.
Do not use sysupgrade hooks or raw full-router capture as a convenience. Validate
complete producer status, exact manifest, full stream/counts and cipher finalize.
The existing gzip/tar predicates provide structural checks, not trusted provenance.
Bind artifact/job/target/boot/profile/scope and ciphertext digest in the trusted
job record. Successful age decryption alone does not authenticate the producer.

The guardian creates its own recovery capsule from admitted observations; it does
not accept a client-supplied replacement capsule or journal. Protected storage,
authenticated admission and the retained binding establish provenance. A valid
age ciphertext made with a public recipient alone does not establish journal
authenticity. If an external store cannot preserve that provenance, a separately
reviewed authenticating store/manifest profile is required before use.

Device recovery uses a **different**, operator-provisioned age identity/recipient
and encrypted capsule. The off-device archival identity remains in its chosen
restricted file/env/archive/Vault source. Never install it on the router. The
dedicated device identity gives the guardian local recovery authority and must
be in explicitly approved root-only persistent storage; it is not magically safe
against a compromised root. Operators who cannot accept this trust/storage profile
cannot enable durable mutations. No key generation, key copy or ACL modification
is part of repository development or automatic startup.

The device journal and capsule have versioned bounded authenticated contents;
private job provenance is checked after full decryption. Atomic no-replace creation,
data sync, atomic journal generation switch and directory sync must succeed before
acknowledging recovery_armed. A retained dedicated identity is checked before arming.
Durable intent is written **before** the first configuration write. Journal state
orders each committed-file/kernel write so crash reconciliation recognizes preimage,
postimage or a supported partial state. Any storage ambiguity retains the admission
block and ciphertext. Ordinary rotating audit files are not this journal.

The initial restore capsule is bounded to 128 KiB plaintext; authenticate all of it
into private zeroizing memory before parsing/applying. No plaintext backup files
or general archive extraction. Future larger restores need their own private
staging profile. Configuration replacement necessarily creates a private temporary
**new live configuration** on the target filesystem for atomic replacement; that
is not a plaintext pre-change backup. It must preserve the approved owner/mode,
reject links, be bounded, be synchronized, and be cleaned without following aliases.
Recovery plaintext remains memory-only. OS paging/crash-dump secure erasure is not
guaranteed by zeroize; deployments must control swap and dumps accordingly.

On guardian crash, rpcd restart or router boot: inspect durable records before new
admission. Any unconfirmed apply intent recovers conservatively; don't restart its
timer. A durable confirmed record must not be rolled back just because its reply
was lost. Run boot recovery before services dependent on the changed hostname;
P4/P11 must test the concrete procd ordering. This does not promise failed-flash,
broken filesystem or failed-boot recovery. Those require separate P10 contracts.

D4 actual host/device ciphertext roots, restricted staging capability, key
reference and retention choices are operator configuration before deployment.
No default repository/OneDrive directory, environment fallback or path from MCP.
Supported stores must prove private creation, no links/reparse traversal,
no-replacement publication and sync semantics on their actual native platform.
An unsupported filesystem/profile fails explicitly. A rename acknowledgement and
durability are distinct. Unknown publication is retained and reconciled, not
retried under a fresh name or deleted. Native store acceptance is P3 work.

## 7. Resource and time ceilings

All ceilings are machine-checked declarations, **not measured achievements**.
Existing host read/parse/sealing caps and original resource targets are unchanged.

| Resource | Initial profile limit |
| --- | --- |
| Device active jobs / queued jobs | 1 / 0; busy is explicit |
| Host plans / workers | 8 plans, 60s absolute TTL / 1 joinable worker |
| Admission / apply / confirmation / recovery | 30s / 10s / 120s / 60s |
| Control message / binary frame / secret | 16 KiB / 64 KiB / 64 KiB |
| System file / authenticated recovery plaintext | 64 KiB / 128 KiB |
| Recovery ciphertext / journal total | 1 MiB each |
| Terminal job retention | at most 32, at most 7 days; evict only proven terminal |
| Guardian stripped binary / idle RSS / peak RSS | ≤10 MiB / ≤16 MiB / ≤32 MiB |
| Guardian idle CPU | <1% of one core over 60s |

Unresolved or failed-recovery records are never evicted to accept more work;
capacity exhaustion denies new jobs. Terminal metadata retention does not authorize
deleting archival backups; stores use separately configured retention outside
active-job cleanup. Plan-cache expiry cannot erase an admitted device obligation.
Record host+guardian aggregate memory/CPU/storage, cold/warm/max-input costs and
shutdown latency in P3/P4/P11. Do not increase a ceiling to hide a failed test.

## 8. Harness and required implementation evidence

P2 freezes declarations and tests rejection of missing/widened limits, permissions,
state guards, ports, unknown fields, extra consumer edges, old-version imports,
wrong directories, private serialization, action escapes, alias/macro/re-export
routes and bypasses in the assembled gate. Static checks cannot prove semantics
of arbitrary Rust bodies or an authorized maintainer editing the harness itself.

P3 starts with typed leases/ports and synthetic storage/capture/secret integration
inside these owners. Add independent failure injection for truncated input,
producer failure, altered provenance, cipher/authentication failure, full storage,
uncertain sync/rename, cancellation, bounded workers and all three native hosts.
Additional native ABI/dependency changes still precede their behavior in an ADR.

P4 then implements the finite domain/runtime/device state machine and actual MCP
admission, including read-only/execute separation, cross-category denial, expired
or foreign handles, pending/drift, before/after audit failure, disconnects, duplicate
submission, expiry/confirm races, process/reboot recovery and conflict-safe restore.
Use isolated OpenWrt; exact BPI-R4 and hardware acceptance remains separate.
Neither this diagram nor a passing declaration test establishes these behaviors.
