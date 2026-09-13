# Supplied backup-archive validation

Implemented internal codec, not a backup/restore MCP tool or a completed-backup
receipt. Architecture-only `6c3e2564459ab78fce7ff418c07bebf6062d2631` passed full
Windows GNU / Linux-on-WSL gates before implementation (475 / 496 distinct tests,
92 harness regressions). See [ADR 0016](adr/0016-bounded-backup-archive-validation.md).

## API and guarantees

`device-codec::archive` contains `ExpectedFile`, `ExpectedArchive` and
`ArchiveValidator`. The immutable supplied manifest requires unique safe relative
names and exact sizes, with no file/descendant conflict. A validator owns that
manifest and accepts bounded `feed` chunks. Only consuming `finish` can return an
`ArchiveSummary`, containing file, payload-byte and archive-byte counts. A successful
feed does not establish complete input. Every error permanently latches.

The closed regular-file profile checks exact POSIX/basic-GNU headers, unsigned
octal/checksum/string framing, expected path and size, zero body padding, every
manifest member exactly once and the complete aligned zero tail. Links,
directories, special files, extended/sparse/PAX records, aliases/traversal, duplicates,
unexpected files, ignored suffixes and concatenated archives reject. Header and
manifest names are private zeroizing buffers; bodies are skipped without copying.
No raw paths, metadata, payloads or content-derived hashes enter the summary/errors.

Limits: 64 KiB/feed, 72 MiB archive, 64 MiB payload, 8 MiB/file, 4,096 files,
256 bytes/path, 32 components/path, 100 bytes/component and 32 KiB terminal padding.
The 72 MiB ceiling is conservative: maximum valid framing under the other limits
already fits below it; it is not an advertised reachable valid archive size.
No file-body allocation scales with payload size. Manifest/index/header allocations
remain bounded but are not measured RSS. Zeroizing owned buffers does not erase
caller buffers, swap, crash dumps or every temporary copy.

These names have router-relative semantics, not native host path semantics.
Acceptance never permits direct extraction on any operating system. Some valid
tar names/metadata and all gzip-wrapped streams are outside this initial profile.
Expected names that cannot be represented in the admitted header forms cannot
produce a matching complete stream; there is no implicit long-name fallback.

## Validation evidence

Full implementation gates pass **490 distinct native Windows GNU tests**, **511
Linux-on-WSL tests**, zero ignored and **92 harness regressions** counted once.
Fifteen new archive tests extend the required action-response suite (26 total).
They exercise both formats, all two-piece splits, small chunk sizes, every
incomplete prefix for five payload sizes, all 254 excluded type bytes, every
header-byte checksum corruption, malformed fields, safe names/full-width prefixes,
missing/duplicate/unexpected members, ordering, ancestor conflicts, tail/padding,
latched failures and exact cardinality/payload/feed limits. The 64 MiB payload
fixture repeatedly feeds one 64 KiB buffer instead of allocating the whole stream.

Independent host producer acceptance on 2026-09-13 used two public synthetic files:
`fixture/a` contains seven known bytes, `fixture/empty` is empty. Only these files
were placed in a newly created ACL-restricted non-synced directory outside all
repositories. Tar output stayed in bounded processing memory; no backup or
decrypted configuration was created, read or extracted.

| Producer | Explicit formats | Validators | Result per stream |
| --- | --- | --- | --- |
| Native Windows bsdtar/libarchive 3.8.8 | ustar, gnutar | Windows GNU and Linux-on-WSL | 2 files, 7 payload bytes, 10,240 total bytes |
| Linux GNU tar 1.35 on WSL | ustar, gnu | Windows GNU and Linux-on-WSL | 2 files, 7 payload bytes, 10,240 total bytes |

All eight combinations passed using the development-only `archive_fixture_check`
example, whose expected names/sizes are fixed. This is not a production CLI,
OpenWrt BusyBox/sysupgrade acceptance, macOS acceptance, an encryption test or
proof that an arbitrary producer/manifest is complete. No raw archive output was
logged. Producer processes completed successfully; that evidence is external to
the codec, not something its structural summary asserts.

Native example: 285,184 bytes, SHA-256
`804f07db0a0288c392248531aa2bbd648da5a708022531435b4a02b609b63f9f`.
Linux example: 369,512 bytes, SHA-256
`a8e6cf33cb9079872f1cb695c73674f629b66c4cd975f2f36476fad223ee2711`.
Toolchains: Windows GNU Rust 1.95.0 and Linux Rust 1.97.1; workspace stripped
opt-level-s/thin-LTO release profile on the Intel Core Ultra 7 265K host.
No latency, allocation-count, peak-RSS or ARM-device performance measurement.

## What remains outside this codec

All production consumers are still blocked by the harness. Capture commands,
trusted/fresh complete manifest discovery, compression validation, same-target
producer success, authenticated transport/provenance, age orchestration, completed
ciphertext publication, restricted staging, extraction, guardian recovery and
mutations require separate accepted integration. Same-size body substitution is
not detected by a tar structure/size check. Full backup authenticity and content
integrity must not be inferred from an ordinary tar header checksum.
