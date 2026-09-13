# Bounded gzip archive validation

Implemented supplied-byte codec, not backup capture, age orchestration, restoration
or a new MCP operation. Architecture-only `e72e685361e86f90516d3985c06ef749ce124bb2`
passed native Windows GNU / Linux-on-WSL gates (495 / 516 distinct tests, 97 harness
regressions) before the dependency or behavior changed. See
[ADR 0017](adr/0017-bounded-gzip-archive-validation.md).

## Profile and boundaries

`device-codec::gzip::GzipArchiveValidator` owns the supplied expected manifest and
the [v16 regular-tar validator](backup-archive-validation.md). Bounded `feed` calls
never prove completion; consuming `finish` returns only file, payload-byte,
expanded-archive-byte and compressed-byte counts after every check succeeds.
It does not expose the inner validator, raw metadata or payloads. Failures latch;
inner malformed tar becomes a fixed invalid-archive code and its resource failures
remain limit errors. No source-controlled messages reach diagnostics.

The profile admits one DEFLATE gzip member, exact magic/CM, no reserved flags,
bounded optional extra/name/comment/header-CRC fields and exact stream end.
Original headers are passed intact to locked flate2 1.1.10 / zlib-rs 0.6.7 for
FHCRC, compressed structure, payload CRC32 and ISIZE validation. No header stripping,
reset, dictionary, alternate format, concatenated member or trailing padding is
accepted. FTEXT does not transform bytes; names/comments are opaque, never paths.
CRC32 detects corruption, not malicious replacement or authenticity.

Limits are 64 KiB/feed, 72 MiB compressed input, 4 KiB total gzip header, 4,084 extra
bytes and 1,024 bytes each for name/comment including the terminating NUL. Their
combined header still must fit 4 KiB. Expanded output passes through a fixed 4 KiB
buffer into unchanged v16 archive/file/manifest/tail limits. At complete end, require
expanded archive bytes <= 1 MiB + 128 * consumed DEFLATE bytes; header metadata and
the eight-byte trailer do not inflate that denominator. Final checking keeps the
ratio decision independent of input chunking; it does not avoid all prior work.

Owned header/output buffers zeroize and the full archive is never retained.
The backend retains bounded inflate state and a 32 KiB plaintext history window;
its allocations are **not guaranteed scrubbed** when released. This is not measured
RSS or complete protection from swap/core dumps/temporary copies. A synchronous
feed can expand many output blocks; no hard execution-time/cancellation guarantee
or new performance measurement is claimed. Sensitive workflow integration must
review process memory protection before adopting this component.

The wrapper is the only admitted internal consumer of the base archive namespace.
All application/device/protocol consumers of both namespaces remain prohibited.
The normal flate2 edge is exact-version, default-feature-free, Rust-backend-only
and confined to the gzip module. Keys, crypto providers and runtime are independent.

## Verification

Full architecture/evolution, formatting, strict all-target Clippy, workspace tests
and release builds pass: **510 native Windows GNU / 531 Linux-on-WSL distinct
tests**, zero ignored, **97 harness regressions** counted once. Fifteen gzip tests
bring the mandatory action-response suite to 41 cases. They cover all three block
types, 32 optional-flag combinations, every two-piece split and incomplete prefix
of selected fixtures, FHCRC/CRC/ISIZE corruption, malformed headers/blocks,
same/later-feed suffixes, manifest/tar failures, poisoning and exact bounds.
A 72 MiB empty-block source-limit test reuses one 64 KiB buffer. Large expansion,
ratio invariance and metadata-excluded denominator cases also pass.

Test sources include independent stored-block/bitwise CRC fixtures (checked against
the standard `123456789` CRC vector), an immutable Python tarfile + zlib 1.3
Z_FIXED fixture, and flate2-produced dynamic streams. The independently generated
fixed fixture contains one seven-byte public file, not a real backup. A transcription
error in its initial literal was detected and corrected against the producer.

Independent host producer checks on 2026-09-13 used only the existing two public
fixture files in a private ACL-restricted non-synced directory outside repositories.
All archive/compressed bytes stayed in bounded processing memory; no archive was
persisted or extracted. Producers exited successfully. Both host-native validators
ran the development-only fixed-manifest `gzip_fixture_check` example.

| Producer | Formats | Compressed bytes | Both validators |
| --- | --- | --- | --- |
| Windows bsdtar 3.8.8 tar, then .NET 10.0.12 GZipStream | ustar / gnutar | 149 / 143 | Accepted; 2 files, 7 payload bytes, 10,240 expanded bytes |
| Linux GNU tar 1.35 + gzip 1.12 on WSL | ustar / gnu | 150 / 151 | Accepted; same expanded counts |
| Windows bsdtar 3.8.8 direct gzip stdout | ustar / gnutar | 10,240 each | Expected rejection: trailing_gzip_data |

Eight positive and four expected-negative combinations pass. The direct bsdtar
case has external compressed-output padding, matching its documented stdout
behavior; see the [upstream 3.8.8 manual](https://github.com/libarchive/libarchive/blob/v3.8.8/tar/bsdtar.1).
The codec was not relaxed, and no bytes were trimmed to make it accept. This is a
deliberate interoperability restriction, not a blanket claim that such archives
are corrupt according to every gzip reader. Producer versions and compressed sizes
describe these fixtures, not stable generic archive sizes.

Native example: 334,848 bytes, SHA-256
`635823a943b23433d7328421c99e3ea751916d17006104c7819af2db90069bfc`.
Linux example: 426,856 bytes, SHA-256
`fa75837465108c22540eed1884bb8cf00dd99bf0856cb346c5593f038b39f8d9`.
Host toolchains remain Windows GNU Rust 1.95.0 and Linux Rust 1.97.1.

No BusyBox/sysupgrade, physical BPI-R4, macOS/MSVC, crypto pipeline, safe extraction,
publication/recovery or full-management acceptance follows. Arbitrary sysupgrade
archives can contain entries outside v16's regular-file profile. Capture provenance,
trusted/fresh complete manifests, age authentication, ciphertext durability and
device-owned recovery remain separate unfinished workflows.
