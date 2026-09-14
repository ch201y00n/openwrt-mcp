# Ciphertext foundations — P3 / v22

Bounded infrastructure, not a callable management operation. P2's six high-level
ports and all admission/recovery obligations remain. No P3 constructor creates
Admission or bypasses Dispatcher. First scope: independently admitted
`etc/config/system`, one regular file, 0..65536 payload bytes, strict tar or one
gzip member. No sysupgrade/hooks/shared UCI/shell/generic path or format detection.
Capture <=128 KiB, ciphertext <=1 MiB. Exact manifest and length come from a trusted
independent observation, never from the captured archive itself.

## Ownership

- runtime/backups: private binding, format, bounded capture data, authentication
  port and count-only inspection; no I/O or authorization.
- runtime/mutation_ports: WorkBudget, safe MutationError and exact SecretSource
  port; secrets/ owns purpose-bound non-printable material.
- backend-ssh/transactions: fixed `openwrt-mcp-capture-v1` SSH subsystem on the same
  pinned session, binary request/response, independent success/EOF/exit/close and
  exact counts. Bounded zeroizing capture, never arbitrary executable or host command.
- adapters/backups: exact manifest bridge to existing ArchiveSealer, record format,
  host store, full-authentication inspection and one owned worker.
- crypto-age/provenance: supplied-key HMAC-SHA256 only; no custody or I/O.
- key-sources/secrets: offline exact aliases mapped to purpose and existing sources.
- host-platform/ciphertext: preexisting local file, bounded reads/appends,
  try-exclusive-lock and sync_all. No create/delete/rename/truncate/chmod, keys,
  policy, raw handles or native ACL claims.
- server/transactions: trusted composition only. MCP receives none of these types.

External runtime/backups consumers are exactly adapters/backups,
backend-ssh/transactions, crypto-age/provenance and server/transactions. Only
adapters/backups consumes host-platform/ciphertext. HMAC/SHA2 belong only in
crypto-age/provenance. Existing P2 edges remain; flattened exports stay forbidden.

## Binding, provenance and publication

Trusted construction supplies nonzero 128-bit store/target/boot/job IDs, fixed
scope/profile revision, expected system-file size, format and source length.
These private infrastructure inputs are not permission evidence or secret hashes.
P4 derives them from authenticated admission, not clients. P3 capture requires an
already authenticated session (established during preceding observation), and an
operator-bound target ID; it never loads a new SSH key or reconnects. The subsystem request
contains only fixed binary binding, no paths or plaintext.

Operators pre-provision an authenticated header for a random store ID and a
separate random 32-byte HMAC key via KeySource. No default path, key generation or
initialization on open. Operators establish durable filename/ancestor persistence
and a controlled local regular-file profile. NFS, cloud-sync/Vault stores and
unknown locking/sync profiles are unsupported operator preconditions, not automatic
filesystem classification by this library. Ciphertext may be externally
readable: this profile **does not claim native ACL privacy**. Keys/plaintext and
private staging remain in RAM or separately protected custody.

Hold an exclusive native file lock through authentication, scanning and append.
Concurrent cooperating processes get busy. Reject known symlink paths/nonregular
files. Locks do not isolate root, namespace attackers or noncooperating writers.
Authenticate the header before any append so a misdirected open cannot modify an
unrelated file. Operator-controlled namespace stability is a prerequisite; HMAC
protects integrity, not availability or filesystem honesty.

Each record binds version, fixed-width provenance, exact lengths, age ciphertext
and a domain-separated full HMAC-SHA256 tag. Scan <=32 records /33562688 bytes.
Reject duplicates, malformed/truncated tails, oversized lengths and failed tags.
Never skip/truncate/repair/replace/delete or append past uncertainty. Retrieve only
against exact independent expected binding, not an untrusted store listing.

Private RAM ciphertext is appended only after complete source/manifest/producer/
cipher validation. Existing artifact IDs cannot be appended again. One append
attempt writes the whole record and calls sync_all. Atomic publication means
lock-serialized complete authenticated-record visibility to store API readers;
raw file readers may see partial ciphertext. It is not atomic rename or physical
sector writes. Write/sync/late-ack failures return unknown and retain bytes, without
retry. Reconciliation verifies and syncs an exact record before reporting durable.
Absent/invalid evidence remains unknown unless nonpublication was actually proven.
Tail removal can hide a record: P4 must persist expected provenance independently
in its trusted job journal. No rolling-back store is proof that a job never applied.

## Restore, secrets and worker ownership

Check binding/HMAC first, then decrypt completely into capped zeroizing RAM.
Enforce exact input/output counts, true EOF and age final authentication before
archive inspection. On any failure discard staging. Re-run strict independent
manifest validation and return counts only: no extraction/application/plaintext
files. Larger restore and a recovery-payload capability require guarded P4 work.
Zeroize does not erase swap/dumps or third-party cipher/decompressor history.

Service SecretReference maps exact alias/purpose separately from crypto custody.
SecretValue has no Debug/Display/Clone/Serde. Unknown/wrong-purpose aliases deny
before reading. Existing source/ZIP limits and unsupported Vault/macOS file
profiles stay explicit. Environment is selected, never a fallback.

One process-wide joinable synchronous worker, zero queue; <=two 64-KiB transfer buffers,
128-KiB capture/restore, 1-MiB cipher stage. Shared absolute WorkBudget <=30s,
cancellation before/after each bounded callback. Drop cancels and joins before
capacity is released; no detached spawn_blocking. Async SSH capture precedes the
synchronous worker and uses the same absolute budget. Do not make the worker
depend on an executor that its caller synchronously blocks while joining.
OS-blocked I/O cannot be forcibly
interrupted: shutdown may wait for the one worker. No hard real-time claim.

## Acceptance

Actual host files and loopback pinned SSH on Windows/Linux/macOS, synthetic keys
only. Test EOF/producer failure, manifest mismatch, limits, wrong key/binding,
cipher/tag/header tampering, truncated records, duplicates, concurrent opens,
failed finalize/write/sync, lost/late ack, cancel/drop/join and reopen/reconcile.
Fault injection complements actual native sync/lock/reopen tests. This profile
does not rename; namespace-publication fault acceptance belongs to a future native
rename store. No real BPI-R4, physical power-loss, guardian or P4 claim follows.

Primary references: [Rust File locking/sync](https://doc.rust-lang.org/std/fs/struct.File.html),
[HMAC verification](https://docs.rs/hmac/0.12.1/hmac/),
[SHA2](https://docs.rs/sha2/0.10.9/sha2/). Pin dependencies; do not implement a MAC.
