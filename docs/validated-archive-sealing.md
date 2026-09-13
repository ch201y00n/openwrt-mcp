# Validated supplied-stream sealing

Implemented internal application flow under `runtime::sealing`, following the
architecture-only checkpoint `47cf2e40e8f82bd28eabe9ffa16018369f0c1443`.
That checkpoint passed 515 native Windows GNU / 536 Linux-on-WSL distinct tests,
including 102 harness regressions counted once, before production implementation
or the server development-only codec dependency changed.

This is not a router backup tool, real storage adapter, restore flow or permission
to change a device. There is no external production caller. See
[ADR 0018](adr/0018-validated-archive-sealing.md) and
[remaining management workflows](management-workflows.md).

## Sequence and ownership

`ArchiveSealer` accepts already-bound trusted source, incremental checker, cipher
and private ciphertext stage ports. The default clock uses monotonic elapsed time;
tests inject deterministic time. Limits are validated before ports are accepted.
Required stage declarations are checked before any source or recipient access.

The read wrapper validates each piece **before** returning it to the cipher.
Independent wrappers count successful reads and ciphertext writes. The first stream
error latches across both, including errors a cipher ignores. All I/O errors, even
Interrupted, are terminal here. Provider text is discarded; only fixed safe states
and bounded counts leave this module.

Success requires real EOF from a nonempty-buffer read, nonempty input/output,
matching cipher counts, consuming checker completion, finite archive counts,
successful producer completion with the same byte count, stage flush and one
acknowledged publication. Consuming a prefix or exactly all bytes without checking
EOF is not completion. Never drain an early-returning cipher afterward. The existing
public-recipient `EncryptionSession` implements the cipher port; the flow cannot
load an identity, decrypt, open a file or select a device.

A confirmed producer exit needs no cancellation even if its late acknowledgement
or wrong count rejects the seal. Ports are trusted: counters cannot prove arbitrary
providers encrypted data, truthfully described producer success or delivered
durable private storage.

## Publication and cleanup

| Result | Seal result | Cleanup / retry |
| --- | --- | --- |
| Failure before publish | Error, NotAttempted | Cancel unfinished source and abort private stage once |
| Definitely not published | Error, NotPublished | Abort stage once; completed producer needs no cancellation |
| Publication uncertain | Error, Unknown | No abort, deletion or automatic retry |
| Published before deadline | Success, count-only summary | No abort or retry |
| Published acknowledgement after deadline | Error, Published | Preserve artifact; no abort or retry |

Each cleanup result is independently NotRequired, Cleaned or Unknown; uncertain
cleanup does not replace the original failure. A best-effort unwind guard handles
pre-publication panics. Publication becomes Unknown **before** calling the port,
so an unwind during publication cannot trigger deletion. Cleanup ports must not
panic. Process abort, power loss and blocked callbacks are not solved by Drop.

NotRequired means this flow does not request cleanup, including when publication
is uncertain and abort is forbidden. It does not prove that no staging remains;
interpret it together with the separate publication state.

One-shot means one attempt in this invocation. The future workflow/storage adapter
must prevent improper reuse, retain its own artifact/correlation handle and
reconcile uncertain outcomes. This is not a durable receipt, target/boot/plan
binding, source attestation, freshness proof or mutation permit.

## Resource and security boundaries

Defaults: 64 MiB source, 65 MiB ciphertext, 30,000 ms. Hard ceilings: 72 MiB source,
73 MiB ciphertext, 300,000 ms. Delegated reads/writes are at most 64 KiB; short
writes are honored. Exact input ceilings use one zeroizing EOF-probe byte; excess
bytes never reach the checker/cipher. Failed reads scrub the delegated caller-buffer
region. Empty reads do not establish EOF. Impossible returned counts and nonempty
zero writes fail closed.

Checker summaries allow at most 4,096 files, 64 MiB payload and 72 MiB expanded
archive, with payload no larger than expanded data. The checker separately enforces
actual format/manifest relationships. This application retains counters/control
state and the one-byte probe, never a whole-stream buffer.

Deadlines are checked before and after callbacks, stream operations and synchronous
cipher/checker work. Backwards clocks reject. Checks are cooperative, not hard
cancellation; cleanup runs even after expiry and can block. Provider allocations
and timing need independent limits/review. Gzip backend history and age internals
are not guaranteed scrubbed; real sensitive adoption still needs memory, swap,
core-dump and platform-protection review.

## Fixture verification

Full architecture/evolution, formatting, strict all-target Clippy, workspace tests
and release builds pass **546 native Windows GNU / 567 Linux-on-WSL distinct
tests**, zero ignored, including **102 harness regressions** counted once.
The mandatory protection and composition suites contain 38 and six tests.
No new package download or normal dependency was required.

The mandatory runtime protection suite includes 26 new sealing tests: ordering,
stage declarations, independent EOF/counts, all callback/clock deadline boundaries,
time reversal, bounds, short I/O, first-error latching, failed recipient access,
cleanup uncertainty, publication states and unwind paths. The maximum-source test
processes 72 MiB with a fixed 128 KiB fake-cipher buffer and count-only source/stage
models, not a 72 MiB fixture allocation.

Five new server composition tests use actual age and the strict gzip/tar checker,
with freshly generated identities and public synthetic content entirely in memory.
The 139-byte Python tarfile/zlib fixture describes one seven-byte regular file in
a 10,240-byte tar. Nine input/output chunk combinations authenticate and round-trip;
identity access occurs only in separate test decryption. All 139 incomplete prefixes,
64 trailer-bit corruptions, a suffix and concatenation reject without publication.
Failed producer completion, ciphertext flush, actual final age-record write after
EOF and output ceiling also prevent publication.

An initially copied fixture contained 152 bytes and failed; it was corrected from
the already validated 139-byte codec fixture without changing acceptance rules.
Strict lint findings in test scaffolding were corrected without suppressions.
In-memory stage declarations do not validate OS atomicity or durability. No real
keys, Vault, archive file, router state or external publication is involved.
Native macOS/MSVC, OpenWrt backup/restore, production storage, measured end-to-end
resource use and comprehensive management acceptance remain outstanding.
