# ADR 0016: bounded supplied backup-archive validation

Status: accepted for an architecture-only checkpoint before implementation.
Scope: a pure codec prerequisite, not capture, publication, restore, authorization
or permission to operate a live router.

## Boundary

Age finalization does not prove complete source production. A tar terminator does
not prove that required files were included. Validate an explicit nonempty supplied
path/size manifest and the complete uncompressed stream independently. This does
not attest the manifest's completeness, live state, content integrity, producer
exit status or provenance; those require later workflow integration.

Use the existing pure device-format owner in `crates/device-codec/src/archive/`.
Its public namespace contains validated expected-file/manifest constructors, a
streaming validator, a count-only summary and fixed errors. No entry iterator,
payload getter, extraction, host path conversion, Read/Write implementation,
device command or MCP API. Only pure standard APIs and the existing workspace
zeroize dependency are admitted. No file payload is retained.

Keep twelve owners and existing invocation/crypto boundaries. The codec dependency
allowlist admits zeroize; its Cargo edge is added after this checkpoint. Every
production direct codec consumer forbids `openwrt_mcp_device_codec::archive`, with
producer root re-exports blocked. Later integration needs a separate checkpoint.
Archive modules forbid serde/JSON, core actions/policy, sibling command/response
APIs, all std I/O/native/path APIs, crypto and custody. The already required native
action-response suite gains a separate archive module after the checkpoint;
existing tests are not archive acceptance. Static gates supplement semantic tests.

## Closed profile

`regular_tar_manifest_v1` accepts uncompressed 512-byte tar records with exact
POSIX magic/version `ustar\0` + `00`, or exact basic GNU `ustar  \0`. These may
coexist per header; no heuristic format fallback. Only regular type `0` or NUL is
accepted. Reject links, directories, devices, FIFO, sparse, GNU long names/links,
PAX local/global and unknown types. GNU bytes 345..512 must be zero. POSIX
prefix/name combine with one slash, and bytes 500..512 must be zero. Link-name
bytes must all be zero. This deliberately excludes many otherwise valid archives.

Validate the unsigned header checksum, treating its field as spaces. Mode, uid,
gid, size, mtime and checksum are nonempty unsigned ASCII octal: optional leading
spaces, digits, then only spaces/NUL padding. Reject signs, base-256, overflow,
all-padding required fields and digits after a terminator. Mode <= 0777, uid/gid
<= u32::MAX. Device fields may be entirely NUL or octal zero, never nonzero.
Name/prefix/uname/gname fields may fill their entire width; otherwise a first NUL
must be followed only by NULs. Owner names are empty or ASCII graphic text, never
interpreted or returned.

Manifest and reconstructed paths contain 1..256 ASCII bytes from letters/digits,
underscore, hyphen, period, plus, at-sign and slash. At most 32 nonempty components
of at most 100 bytes; reject leading/trailing/repeated slash, dot/dot-dot, backslash,
colon, spaces, controls, NUL and non-ASCII. Equality is exact and case-sensitive,
with no normalization. These are router-relative archive names, never host paths;
future extraction requires its own native handle/protection policy.

The immutable supplied manifest has 1..4,096 distinct regular files with exact
sizes, including zero-length files. Reject any file that is another file's path
ancestor regardless of ordering. Require every expected member exactly once with
the exact size and no unexpected member; archive order is arbitrary. This detects
missing expected members, not an incomplete manifest or same-size substitution.

## Stream lifecycle and budgets

Construct the validator from an owned validated manifest. Feed arbitrary chunks
of at most 65,536 bytes. Success is only progress, not completion. Copy at most the
current 512-byte header and bounded path data; skip payload without copying and
verify zero member padding. No extraction or persistent storage is introduced.

At a header boundary the first zero block starts terminal padding. Require two
complete zero blocks, no later nonzero byte, a 512-aligned whole tail and at most
32,768 tail bytes including both terminators. Consume the complete supplied tail;
reject concatenation, ignored suffixes, single terminators and truncated final
blocks. `finish` consumes the validator, checks all expected members and returns
only file, payload-byte and total-archive-byte counts. Empty archives fail.

First error permanently latches; no reset, skip or later finish success. Clear
owned header buffers on errors and after processing. Owned header/name buffers
use zeroize on drop, without claiming protection from swap, core dumps, caller
buffers or all compiler-created temporary copies. No raw Debug/serde on sensitive
containers; errors contain fixed codes only, summaries no paths or content hashes.

Hard ceilings: 72 MiB archive, 64 MiB total payload, 8 MiB/file, 4,096 files,
256 bytes/path, 32 components/path, 100 bytes/component and 32 KiB tail. Check
length/count/sums before copying or allocating. Manifest path capacity is at most
1 MiB plus structural allocations. These are not RSS or time guarantees. No
clock, worker, background task or decompression; existing crypto/transport/operator
limits remain independent and may reject below these maxima.

## Verification and integration

Required tests: both header forms; cross-chunk header/body/padding/terminators;
zero/nonzero files; full-width names/prefixes; missing/duplicate/unexpected members
and wrong sizes; file ancestors and unsafe paths; every excluded type; malformed
checksum/numbers/string padding; nonzero/excessive tails and member padding; all
incomplete prefixes; exact limits/overflow; latched errors; fixed diagnostics;
count-only output; order/chunking invariance. Fixtures are synthetic and in memory.
Add independent producer-format acceptance when available, without live backup or
plaintext extraction. No compatibility claim follows from handcrafted fixtures.

Future integration must bind a fresh trusted manifest, same-target producer success,
authenticated transport, gzip framing/CRC/expansion limits, complete age authentication
and finalization, target/transaction provenance, ciphertext publication, protected
staging and device-owned recovery. This predicate establishes none of those facts.
No management operation or production consumer is added in this checkpoint.

## Primary source review

Offsets, normal header forms and checksum were checked against pinned upstream
libarchive [ustar writer](https://github.com/libarchive/libarchive/blob/v3.8.1/libarchive/archive_write_set_format_ustar.c),
[GNU writer](https://github.com/libarchive/libarchive/blob/v3.8.1/libarchive/archive_write_set_format_gnutar.c)
and [reader](https://github.com/libarchive/libarchive/blob/v3.8.1/libarchive/archive_read_support_format_tar.c).
The stricter admission rules are project decisions; no implementation code is copied.

OpenWrt [25.12.5 sysupgrade](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/sbin/sysupgrade)
wraps composed members in gzip, may include links, runs hooks and shares temporary
lists. This validator does not accept arbitrary sysupgrade .tgz directly, establish
complete-device backup, or authorize its extraction path. A future producer needs
its own reviewed compatibility and effect profile.
