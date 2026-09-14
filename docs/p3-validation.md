# P3 ciphertext foundations: verification and limits

Architecture-only checkpoint: `84ea84f29432d95d03fdafa3245a42b257383e01`, following
P2 `742f8f6`. The checkpoint passed the native Windows repository gate and added
assembled v22 negative case/strict lint; the Linux-on-WSL gate also passed. There
are 124 architecture regression cases, counted once in workspace totals.

## Implemented scope

- Fixed binary SSH capture on one already-authenticated pinned target; no reconnect,
  key loading, arbitrary command/path or new MCP entry point. Independent subsystem
  acceptance, echoed binding, exact counts, EOF, zero exit and close are required.
- Tar/single-gzip validation and existing age sealing, with private capped RAM
  stages and a real native record-file adapter. Separate HMAC-SHA256 authenticates
  store/target/boot/job/scope/format/lengths/ciphertext. Publication is complete
  authenticated record visibility plus file sync, not filename rename.
- Reopen/reconcile and full age authentication before memory-only count inspection.
  Failed/truncated/foreign data never becomes an apply/extraction payload.
- Non-printable purpose-specific service SecretReference/SecretValue and exact
  file/env/ZIP provider reuse; one owned joinable synchronous worker, zero queue.

Behavior coverage includes actual temporary ciphertext files, native lock contention,
native sync/reopen, synthetic loopback SSH, zero/max payloads, manifest mismatch,
wrong key/binding, tag/header/cipher tampering, every truncation of a representative
record, duplicate IDs/capacity, failed finalization, partial/full writes, sync/lost
ack/cancellation, strict age authentication even under a valid store MAC, worker
drop/join/panic/capacity and independent environment-child/ZIP secret resolution.

## Verification status

Native Windows GNU full `tools/Test-Repository.ps1` passes: architecture v22,
124 harness cases, formatting, strict workspace/all-target Clippy, 609 workspace
tests (including the harness once and one compile-fail doctest), release build.
Rust/Cargo 1.95.0; no ignored cases. The separate Linux-on-WSL full gate also passes
with Rust 1.97.1; it is not native Windows or macOS evidence. Upstream
`proc-macro-error2 2.0.1` reports a future-compatibility warning, not suppressed.

P3 adds 10 mandatory server composition cases, 3 mandatory SSH capture cases and
one owned-worker lifecycle case. Three-native-host CI is the remaining merge gate;
record its exact outcomes before treating P3 verification as complete.

P2 main CI is complete on all three hosts:
[run 34873954647](https://github.com/ch201y00n/openwrt-mcp/actions/runs/34873954647).

## Explicit limits

This closes one bounded P3 infrastructure profile, not all SEC/TXN feature IDs.
P2's five remaining high-level mutation ports, actual guardian admission/journal,
reboot recovery, Dispatcher authorization/audit composition, and restore application
remain P4. The catalog remains 55 read operations. No baseline router, private
Vault/identity, live configuration, deployment or GitHub default-branch setting
is changed. Production readiness and BPI-R4/lan3 behavior are not established.

The operator must pre-provision a stable durable filename on a controlled local
filesystem and a separate authentication key. Filesystem type/stable namespace
are operator preconditions, not detected guarantees. Native record I/O does not
claim ACL privacy, root isolation, physical power-loss acceptance, automatic
directory fsync/provisioning, or native rename semantics. Unknown append/sync
outcomes retain artifacts and need reconciliation; missing records are not proof
that a device job never applied. P4 must persist expected provenance independently.

Plaintext <=128 KiB, ciphertext <=1 MiB, file payload <=64 KiB; store <=32 records /
33562688 bytes. No plaintext backup files or general extraction. Zeroization does
not scrub swap/dumps or third-party codec/crypto/SSH internals. OS-blocked file or
source calls can delay worker shutdown: one owned worker is not kernel preemption.
No router performance/RSS/CPU or aggregate guardian target is claimed from tests.
