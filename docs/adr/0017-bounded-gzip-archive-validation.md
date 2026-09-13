# ADR 0017: bounded single-gzip archive validation

Status: accepted for an architecture-only checkpoint before implementation.
Scope: supplied compressed bytes only, no capture, key access, encryption,
publication, extraction, live operation or new MCP authority.

## Ownership and deliberate extension

OpenWrt's sysupgrade stream is gzip-wrapped; the v16 regular-tar validator must not
silently guess or decompress formats. Add a separate `device-codec::gzip` namespace
with private modules in `crates/device-codec/src/gzip/`. A GzipArchiveValidator owns
the existing ExpectedArchive/ArchiveValidator and feeds only expanded bytes to it.
The base archive contract now admits this one internal consumer, never application,
device or protocol consumers. Keep every external archive namespace ban and add
equivalent gzip bans. No producer root alias, raw data return or alternate tool.

Unlike v16's tar framing, DEFLATE inherently retains a bounded plaintext history
window. Accept that explicit difference here, without changing the base validator's
no-payload-retention contract. Owned input-header/output buffers are zeroizing;
the compression backend's internal allocation/window is not guaranteed scrubbed.
Neither reset nor deallocation is described as secure erasure. Future sensitive
workflow integration must evaluate process-memory/swap/crash-dump protections;
this prerequisite does not claim complete plaintext-residue protection.

Reuse locked flate2 1.1.10 with default features disabled and only zlib-rs enabled,
whose locked backend is zlib-rs 0.6.7. Do not implement DEFLATE or CRC ourselves.
Only its low-level Decompress/Status/FlushDecompress API is admitted in gzip
production modules; no Read/Write wrappers, native C backend, dictionary, reset,
compression, OS API, core action/policy, serde/JSON, crypto or key custody. Pin the
workspace dependency and validate Cargo metadata features/version/target/owner.
No new crate owner; only one reviewed normal dependency edge after this checkpoint.

## Header and framing profile

The explicit `single_gzip_regular_tar_v1` profile accepts one gzip member using
DEFLATE (CM=8), exact ID bytes and zero reserved flag bits. FTEXT is accepted but
does not cause text conversion. MTIME, XFL and OS are not interpreted or exposed.
Before passing a header to the backend, incrementally bound its entire size to
4,096 bytes. Support the optional FEXTRA, FNAME, FCOMMENT and FHCRC fields in their
specified order; XLEN is little-endian and at most 4,084 bytes, names/comments at
most 1,024 bytes each including their terminating NUL. The global header limit
still applies to combinations. Optional metadata is opaque, never a path, policy
value or output, and is not retained after header admission. Use a linear parser,
not repeated scans of an expanding prefix. Reject missing terminators/truncation.

Pass the complete bounded original header to Decompress::new_gzip(15), then the
remaining bytes. The backend must validate FHCRC when present, DEFLATE structure,
payload CRC32 and ISIZE before StreamEnd. Use initialized bounded output buffers,
account total_in/total_out before and after each call, and never emit raw errors.
No user-selected window size, dictionary, format fallback or header stripping that
bypasses gzip integrity checks. Need-dictionary, invalid data and stalled progress
with unconsumed input fail. Drain pending output when necessary without spinning
after no progress. No compressed suffix may be silently discarded.

Only StreamEnd followed by exact end of supplied input is structurally complete.
Reject another member, concatenated archives, trailing zero padding or any byte
after the member, including bytes supplied in a later feed. `finish` consumes the
outer validator and succeeds only after backend completion, expansion checks and
inner tar-manifest completion. It returns only compressed-byte and inner count
summaries. It must not expose the inner validator or a premature tar summary.
First error latches permanently, including later empty feeds/finish; no reset.

CRC32 is corruption detection, not authentication. A malicious replacement can
have a correct CRC. Neither gzip nor tar validation establishes producer success,
full backup scope, trusted manifest, ciphertext authenticity or restore authority.

## Budgets and memory

Accept at most 64 KiB per feed and 72 MiB total compressed bytes. Expand through a
fixed 4 KiB zeroizing buffer into the unchanged v16 byte/file/path/tail limits.
Never retain the full compressed or decompressed archive or return payload bytes.
The backend allocates bounded inflate state and a 32 KiB history window plus
implementation overhead. These are inspected bounds, not measured RSS.

After complete member consumption, require expanded archive bytes <= 1 MiB +
128 times consumed DEFLATE bytes (excluding the known gzip header and 8-byte
trailer). Check this at completion so read-ahead/chunk sizes cannot change the
ratio decision. Hard expanded-byte limits remain enforced during decoding; the
ratio is an additional acceptance rule, not prevention of all earlier work.
Use checked arithmetic and keep budgets independent of client arguments.

Byte/output bounds do not guarantee hard-real-time execution or cancellation of
a synchronous feed. One small compressed chunk may cause many output blocks.
No clocks/workers/background tasks are introduced; future runtime integration
must enforce its deadline between bounded calls and measure worst-case work.

Clear owned temporary buffers on progress/error/drop, release backend state on
failure/completion, and never claim this scrubs its internal history. No raw Debug
or serialization for wrapper/header/decoder containers. Existing age/key-source,
MCP and base archive behavior remain independent and unchanged.

## Harness and acceptance

Commit this declaration/requirements/negative harness before dependency or feature
implementation. Require the existing native action-response suite on all three
host targets. Preserve old-version codec dependency restrictions and v16 consumer
semantics in downgrade fixtures. Check exact bounds/profile, namespace bans,
native test requirement, low-level-only source access and pinned Cargo metadata.

Then test valid stored/fixed/dynamic DEFLATE, every optional header combination,
FHCRC/CRC32/ISIZE failures, missing terminators, reserved flags/CM/magic, header and
source/output/ratio bounds, large expansion through small buffers, malformed raw
DEFLATE, all truncated prefixes, same/later-feed trailing bytes, concatenated
members, inner tar failures, poison persistence, all split/chunk sizes and
count-only diagnostics. Use independent fixture CRC/producer bytes as well as
library-generated fixtures. No ignored/OS-skipped shared cases or real backups.

Independent native gzip/tar producer checks remain fixture-format evidence, not
OpenWrt backup, macOS acceptance or publication/recovery. Full sysupgrade archives
can still contain v16-excluded entries, so this wrapper does not establish their
unrestricted compatibility. No live producer/crypto/store/guardian integration.

## Reviewed sources

[RFC 1952](https://www.rfc-editor.org/info/rfc1952/) specifies gzip framing and
corruption checks; this profile intentionally rejects multiple members and applies
additional finite limits. The [flate2 Decompress documentation](https://docs.rs/flate2/latest/flate2/struct.Decompress.html)
was version 1.1.10 when inspected. Locked source checks covered its mem/ffi zlib-rs
adapter and zlib-rs 0.6.7 stable/inflate allocation, reset and error behavior. No
implementation source is copied. Public low-level APIs and fixed feature selection
are used, not private layout/FFI access or a claimed backend secure-erasure API.
