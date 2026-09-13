# Management workflows: proposed future architecture

Status: **PROPOSED**, for a later mutation architecture checkpoint after v6 bounded reads; no architecture number is reserved yet. This document is not an accepted ADR, a replacement for the current machine contract, an implemented API, or authorization to change a router. Capability discovery, bounded reads and these future mutation workflows are separate work.

One prerequisite has now been accepted separately: [ADR 0015](adr/0015-protected-resource-effects.md) and the [pure effect model](protected-resource-effects.md) implement bounded analysis of supplied before/after graphs. They do not implement live topology extraction, dispatcher integration, backup, guardian recovery or any mutation lifecycle below. All production consumers remain blocked pending that later architecture review.

Implementation must first update the requirements, ADR, architecture and versioned harness with negative tests, then pass an architecture-only checkpoint. Names below describe proposed responsibilities, not available Rust APIs or MCP tools. Existing [architecture](architecture.md), [development rules](development.md), [protection contract](protection-contract.md) and [portable host contract](portability-contract.md) remain authoritative until that evolution is accepted.

## Reference baseline and evidence

Archive structure is now a second separately accepted prerequisite: [ADR 0016](adr/0016-bounded-backup-archive-validation.md) and [bounded supplied-archive validation](backup-archive-validation.md). It does not implement any capture, crypto/publication, restore or guardian workflow described below; all production consumers remain blocked.

The reference-device inventory reported during development on 2026-09-13 was BPI-R4, OpenWrt 25.12.5 r33051-f5dae5ece4, kernel 6.12.94, mediatek/filogic, with apk 3.0.5 on aarch64. This is a dated observation, not a statement of current device state or proof of mutation support. Fresh device inventory is required before acceptance or execution.

Do not infer installed component behavior from the release number alone. The official [25.12.5 rpcd package definition](https://github.com/openwrt/openwrt/blob/v25.12.5/package/system/rpcd/Makefile) selects `28faf6403792d25b9826043aaf37880624c19568`; the reported installed rpcd and associated modules were `2026.07.19~e37ed9d8-r1`, resolving to `e37ed9d814699098eb7e26c8b33c054840782dfb`. That [upstream commit](https://github.com/openwrt/rpcd/commit/e37ed9d814699098eb7e26c8b33c054840782dfb) directly follows the release-base commit and changes only the file plugin's path/ACL handling. Git blob identities for `uci.c`, `include/rpcd/uci.h` and `session.c` are identical between the two commits. These source facts do not attest the running executable or exclude downstream patches.

### rpcd UCI limitations relevant to the design

At the installed reference commit, calls without a session use shared `/tmp/.uci` staging and omit rpcd session ACL checks. Session-bearing calls select separate delta directories. Applying requires a session and consumes that session's nonempty pending configurations, not a client-selected option subset. Pending apply ownership and its timer are daemon-global. Confirmation and explicit rollback require the initiating session. Commit/revert are rejected while a rollback is pending, but direct external UCI writers are not thereby isolated. Snapshot copying and application do not provide a reliable per-file success report. A non-rollback apply request can reach snapshot cleanup during another pending rollback. These are reasons to require a dedicated transaction owner rather than expose raw calls as independent transactions. [Reference rpcd UCI source](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/uci.c)

The snapshot directories are under `/var/run/rpcd`; the default timer is 60 seconds. Snapshots are ordinary configuration copies, not age ciphertext. A daemon-memory timer is not a reboot recovery mechanism. [Reference UCI constants](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/include/rpcd/uci.h)

### Backup, services and firmware limitations

The 25.12.5 sysupgrade script supports streaming a configuration archive to stdout with `-b -`; `-k` additionally records package names. This is not a full-disk image or a guarantee that package binaries, bootloader, calibration data and external volumes can be restored. Its restore path can extract directly into the root filesystem. Backup creation also uses shared temporary list paths, so parallel backup/upgrade jobs must not be assumed independent. [Reference sysupgrade implementation](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/sbin/sysupgrade)

Service reload, restart and boot enablement are different actions. The service framework permits reload implementations with restart-like behavior, while enablement changes startup links. A successful command is not a service-specific health check. [Service framework](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/rc.common)

Firmware validation checks image compatibility and reports a signature-related result, but absence of a signature or `ucert` can succeed unless signature enforcement is required. Consequently, `sysupgrade -T` or a true signature-test field alone cannot prove authenticated origin. [Image validator](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/usr/libexec/validate_firmware_image), [signature and compatibility implementation](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/lib/upgrade/fwtool.sh)

## Proposed invariants

- Every plan, apply, confirm, rollback request and job-status read passes through dispatcher authorization and audit. Device-side recovery is the previously authorized, bounded inverse of an admitted change, not a new arbitrary execution channel.
- Permission checks cover the union of direct changes, referenced resources, validation actions, service effects and recovery actions. Unknown effects fail closed. A client cannot choose a less restrictive category or recovery profile.
- Discovery distinguishes observed capability, implemented adapter, operator authorization and device-tested workflow. A discovered executable or ubus method is not approved management support.
- Device identity, boot generation, installed component/profile identities and baseline revisions bind a plan to one target. Internal revisions must not expose hashes of guessable secret values through MCP or audit.
- One device-wide management admission owner serializes conflicting changes and shared backup/upgrade facilities. A per-host mutex is insufficient when several MCP hosts manage one router.
- External LuCI, CLI and package scripts do not necessarily honor a guardian lock. The initial profile must require an exclusive management window and reject detected pending changes or drift. It must not promise isolation from a privileged concurrent writer.
- Never retry a submitted mutation automatically. Lost transport responses yield an uncertain state that is reconciled through a job-status channel, not a repeated apply call.
- No MCP argument or result carries private keys, configuration passwords, decrypted archives or unrestricted configuration subtrees. Operator-bound secret references are separate from key-custody purpose bindings.
- Generic administrator extensions are not a substitute for typed workflows or a sandbox. Unrestricted privileged programs cannot coexist with claims of enforced protected-resource isolation for that same principal.

## Smallest first mutation contract

The first candidate is one reviewed, non-secret scalar setting in one existing UCI section, for example a host-name setting after its installed service behavior has been verified. This is not permission to change the reference router's hostname. Even the stock system service processes other system settings when reloaded, so profile review must include the complete reload effect. [System service implementation](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/init.d/system)

The initial profile excludes arbitrary package/section/option addressing, section creation/deletion/reordering, anonymous index addressing, lists, wildcard selectors, arbitrary scripts and network/firewall/package/firmware mutations. A resolved stable section identity and its expected preimage are required. Unsupported shapes remain unavailable rather than falling back to generic execution.

Proposed lifecycle:

1. **Plan:** resolve a typed intent using fresh capabilities and configuration; calculate affected resources and a safe summary. Return an opaque plan identifier, required permissions, expiry and recovery guarantee. Plans alone do not write device configuration.
2. **Admit:** reauthorize, acquire device admission, check identity/boot/profile/revisions, verify empty owned staging and no conflicting pending work. Re-evaluate protected-resource effects and available memory/storage. Any drift invalidates the plan.
3. **Back up:** stream the approved backup scope into age and publish the completed ciphertext artifact. Backup source completion, encryption finalization and store completion must all succeed before the first configuration write.
4. **Prepare recovery:** obtain the explicitly configured recovery authority, record the bounded inverse and arm the device-owned recovery mechanism. Acknowledgement must prove that the selected recovery profile is ready before application begins.
5. **Apply:** execute exactly the immutable plan using private staging and bounded typed operations. Keep a device job identifier and state independently of the requesting SSH channel. Multi-file work is not described as atomic unless the implementation establishes that guarantee.
6. **Verify:** inspect committed configuration, effective service behavior and protected-resource invariants. A transport success or process exit code alone cannot satisfy this step.
7. **Confirm or recover:** confirm only the same target, plan, owner and verified postconditions before expiry. Failure, expiry or host loss triggers the already-authorized recovery action. Check recovery postconditions before reporting success.

States must distinguish planned, admitted, backup-complete, recovery-armed, applying, awaiting-confirmation, confirmed, rolling-back, rolled-back, recovery-failed and outcome-unknown. Define terminal-state idempotency and the confirm/expiry race before implementation. Do not infer confirmation from a vanished timer or absent rpcd pending data.

Start-audit failure prevents mutation. Audit failure after application must not disable the recovery timer or suppress the bounded inverse; a separately bounded device recovery record must permit later reconciliation without recording payloads. Normal MCP shutdown must not accidentally cancel required device recovery.

## Archival encryption versus recovery authority

Age public recipients permit encryption without access to their private identities. That separation is appropriate for archival backups, but an encryption-only device cannot decrypt those archives to restore itself. [Age project and usage](https://github.com/FiloSottile/age)

Proposed profiles are explicit, mutually described guarantees rather than automatic fallbacks:

| Profile | Retained state and authority | Host/SSH loss | Guardian restart or router reboot |
| --- | --- | --- | --- |
| Archival backup | Age ciphertext; decryption identity held outside the router | Archive remains available at its confirmed store | Restoration requires an available authorized decryptor; not autonomous rollback |
| Volatile guardian recovery | Bounded before-images in device process memory; no new plaintext backup files | May recover while the guardian and device remain healthy | Not guaranteed; must not be advertised as durable |
| Durable encrypted recovery, future | Encrypted journal/snapshot plus explicitly provisioned device recovery authority and boot recovery handler | Intended to recover independently | Requires tested key availability, crash consistency and boot integration; unsupported until implemented |
| Verified alternate boot slot, future | Validated known-good slot and board-specific boot-state authority | Depends on the board-specific profile | Requires actual bootloader/slot acceptance; never inferred from board family or image options |

Only ciphertext may be retained as backup/recovery artifacts. Existing live router configuration and tightly bounded processing memory are not an excuse to create unencrypted snapshot files. A volatile profile must address swap/core-dump exposure, lifetime, zeroization and memory ceilings; inability to establish its claimed protections makes it unavailable.

Do not place archival age identities on the router to make an incomplete recovery design appear durable. A durable device key, sealed key service or alternate-slot authority is a separate proposed security boundary requiring explicit operator provisioning and review. Power loss, storage failure and loss of required key authority remain explicit failure cases. There is no universal rollback guarantee for arbitrary device writes.

## Protected-resource graph

The reference operator constraint protects `lan3` as KT IPTV WAN passthrough. That name is an input to the protection profile, not sufficient proof of its current implementation. Resolve the current topology before admitting relevant changes.

The proposed graph includes physical ports and parent devices, DSA/bridge membership, VLAN membership/tagging, logical interfaces, protocol and tunnel dependencies, firewall zones/rules, routes, DHCP/DNS and multicast services, and service reload/restart scopes. A change to a shared bridge, parent device, firewall policy or global network service can affect a protected port without naming it in the request.

Evaluate both the existing and proposed graph, including removed and renamed edges. Resolve identifiers before checks; aliases and indirect references cannot evade policy. Union all affected resources for bulk plans. Unknown package-defined relationships or global effects deny the operation under a protected-resource profile.

Verification must cover the protected connectivity/service invariants, not merely the continued presence of a UCI section. An irreversible or inherently disruptive maintenance workflow may require a separately configured operator exception and out-of-band recovery plan. MCP inputs cannot turn protection off. Neither firmware preservation flags nor an unchanged `lan3` string prove preservation of the passthrough service.

## Backup streams, ciphertext stores and secret values

Proposed `BackupStream` carries bounded sensitive binary data privately between infrastructure and the encryption use case. It is not `serde_json::Value`, a normal backend result or an MCP resource. A source-completion receipt must distinguish complete successful production from clean-looking truncation. Require bounded structural completeness checks for the declared archive format in addition to producer exit status; an empty stream or plausible prefix is not a backup. Apply independent byte/time limits and backpressure; suppress raw producer stderr.

Proposed `CiphertextStore` owns begin/write/finalize/abort semantics for an operator-selected location. It publishes only after producer success and age finalization, binds ciphertext to a safe manifest and makes partial objects distinguishable from complete backups. Atomic visibility, durability, access control, retention and remote-store acknowledgement are separate declared capabilities. A successful write is not an fsync or restore test. Windows/Linux/macOS implementations must demonstrate their advertised guarantees; unsupported required protection prevents mutation without selecting a different destination.

The private manifest binds artifact identity, target identity, capture scope, component/profile versions, transaction identity and integrity metadata. Public summaries and audit omit private file lists and content-derived secrets. Encryption does not by itself establish trusted backup provenance: restoration also needs an authenticated association with an approved artifact and target.

Restoration authenticates the full encrypted stream into restricted, non-synced staging outside repositories before application. It then validates the archive and manifest, enforces exact allowed paths and size/count/expansion limits, and rejects traversal, links/special entries, duplicates and unexpected files according to a reviewed restore format. Never pass an arbitrary decrypted archive straight to sysupgrade extraction. Any restore is itself a typed, authorized mutation with its own pre-change backup and recovery plan.

Proposed `SecretValueSource` resolves operator-bound references for such values as wireless credentials or VPN material only after authorization and purpose validation. It is not automatically the existing cryptographic `KeySource`. Sensitive material requires non-serializable zeroizing wrappers and a bounded private transport channel, never command-line arguments, unrestricted JSON or client-selectable paths. No automatic Vault unlock, source fallback or secret discovery is introduced.

## Later workflow families

### Services

Use reviewed service aliases and separate operations for running state, start/stop/reload/restart and boot enablement. Include all dependent categories/resources, installed script identity and service-specific health predicates. Reinstating a running flag does not undo arbitrary service side effects or restore lost sessions. Unknown service scripts remain outside typed support.

### Packages

Select adapters by observed package-manager capabilities, not a release-number guess. Official releases use apk in 25.12 and newer and opkg in 24.10 and older, but installed variants still require probing. [Official package guidance](https://openwrt.org/packages/start)

Plans must bind package versions, authenticated repository metadata, dependency closure, installed baseline and storage requirements. Feed aliases are operator-owned; arbitrary client URLs, untrusted-package overrides and wildcard upgrades are excluded. Treat package maintainer scripts and dependencies as potentially system-wide effects. Removal or reinstalling an older package is not a general inverse of those scripts. Whole-system upgrades belong to the firmware workflow, not a promise of transactional package rollback.

### Firmware

Separate acquisition, provenance verification, device compatibility validation, pre-change backup, maintenance approval, flash submission and post-boot verification. Bind the exact artifact digest to an authenticated release manifest and approved origin, then to board, target/subtarget, storage/boot mode, installed profile and compatibility policy. Recheck the same stored image immediately before submission to avoid artifact substitution.

Do not treat a successful device-side compatibility test as independent signature verification. Do not expose force flags, compatibility bypasses or arbitrary URLs in the initial workflow. Preserve-config behavior is explicit and cannot imply package/binary preservation. Bootloader, partition-table, calibration and raw block writes need separate board-specific contracts and are not covered by a UCI transaction.

Flash submission is a point of possible irreversibility. Losing the connection is expected and does not authorize replay. Reconcile using the recorded job, authenticated target identity and a new boot-generation observation. Require separate acceptance for power loss, failed boot and alternate-slot recovery; do not imply BPI-R4 has a tested rollback layout merely because it is the reference board.

## Proposed ownership and ports

These paths and interfaces must be evaluated in the future ADR and harness before being created or imported by production code.

| Proposed owner | Responsibility | Boundary |
| --- | --- | --- |
| `core::management` | Typed intents, plan/state identifiers, revision comparisons, effect graph, declared recovery guarantees | Pure data/rules; no OS, crypto or execution |
| `features` category modules | Reviewed operation/profile definitions and verification expectations | No UCI/session orchestration or device I/O |
| `runtime::management` | Dispatcher-integrated workflow state machine and proposed `DeviceTransaction`, `BackupStream`, `CiphertextStore`, `RecoveryAuthority`, `SecretValueSource` ports | No concrete source, process, filesystem or network access |
| Proposed OpenWrt infrastructure owner | Version-specific UCI, service, package and firmware interpretation; typed guardian client | Depends on application ports; never becomes a policy bypass or MCP handler |
| Proposed device-guardian package | Device admission, bounded execution/recovery, timers and optional boot recovery | Explicit OpenWrt-only authority, separately installed; no automatic remote deployment or arbitrary command endpoint |
| SSH/local infrastructure | Authenticated bounded management/binary transport for the same semantics | No host-local fallback, no raw secrets in shell argv |
| `crypto-age` / `key-sources` | Existing stream encryption and independent cryptographic custody/container access | Keep the current separation; no router workflow logic |
| `host-platform` and proposed artifact-store adapter | Native protected staging/publication facilities and selected ciphertext destination | Advertise only implemented platform guarantees |
| `server` / `mcp` | Operator composition / safe request and status mapping | Neither executes device operations nor owns alternate authorization |

The future checkpoint must settle exact trait signatures, sync/async stream ownership, cancellation, bounded worker lifetimes, restart reconciliation, guardian authentication and deployment, dependency allowlists and module layout. The existing JSON-only backend and scalar action-template interfaces must not be stretched into hidden binary or transaction escape hatches.

## Required acceptance failure matrix

All initial tests use synthetic fixtures or a disposable emulator. Live acceptance requires separate authorization, a confirmed encrypted pre-change backup, fresh topology and an out-of-band recovery plan. Native Windows/Linux/macOS transport/lifecycle tests and OpenWrt device tests are distinct evidence.

| Injected condition | Required result |
| --- | --- |
| Unknown version, method shape, package manager or downstream behavior | Capability may be recorded; unsupported mutation remains unavailable |
| Denied category, extra affected category or unknown resource edge | No mutation, backup export or payload-secret acquisition; only separately authorized inspection may run |
| Stale plan, changed boot/identity, reordered section or changed script/profile | Reject as conflict; do not silently replan/apply |
| Foreign UCI pending state, competing guardian host or external configuration drift | Reject/reconcile without committing or reverting another writer's changes |
| Backup truncation, producer failure, size limit, cancellation, full store or failed publication | No application; partial ciphertext never reported as completed backup |
| Recovery authority unavailable, wrong profile or insufficient device storage/memory | Fail before first configuration write; no plaintext/fallback recovery |
| Host crash or SSH loss before and after submission | Distinguish not-submitted from uncertain; device recovery continues within its advertised profile |
| Lost apply/confirm response or repeated client request | Query recorded state; no duplicate mutation or contradictory terminal state |
| Confirmation races expiry or arrives from another owner/target/plan | One terminal transition; wrong-owner confirmation rejected |
| Audit stalls/fails before mutation or after partial application | Prevent initial mutation; preserve already-armed recovery and later safe reconciliation |
| rpcd/guardian restart, router reboot or power loss in each state | Verify the selected guarantee; unsupported durability is never reported as success |
| Apply writes only part of a plan or reload succeeds but health checks fail | Trigger bounded recovery; verify restored effective state or report recovery failure |
| Direct/indirect bridge, VLAN, zone or global-service effect on protected `lan3` | Deny before application; postconditions detect unexpected changes |
| Secret source failure or secret-looking device/error payload | Fixed safe error; no value in MCP, logs, argv, repository or ordinary temporary files |
| Wrong-target, truncated, tampered, nested or path-malicious restore artifact | Reject before extraction/application; authenticated restricted staging only |
| Package script failure or unavailable previous package version | Explicit partial/irreversible outcome; no claim that configuration restore undoes scripts |
| Unsigned/substituted firmware, mismatched board/storage or failed compatibility check | Reject even if a permissive device signature-test field is true |
| Connection loss during flashing, boot failure or unavailable alternate slot | No replay; outcome unknown/recovery-required until independent evidence resolves it |

## Acceptance decision still required

Before implementing mutations, decide the first concrete operation/profile and its verified effect set, allowed concurrency with other management tools, required recovery durability, approved recovery authority, ciphertext/staging destinations, guardian trust/deployment model and board-specific maintenance limits. Then record the accepted architecture and executable harness contract separately.

This proposal does not claim full OpenWrt coverage, transaction isolation from privileged external writers, general package/firmware reversibility, native platform protection parity or successful BPI-R4 mutation acceptance.
