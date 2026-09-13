# Package inventory design work

Status: proposed, not an accepted architecture revision or implemented MCP operation. Architecture v6 and its limits remain unchanged. A separately validated requirements/ADR/spec/harness checkpoint must precede pagination or a new capture port.

The reference inventory recorded 281 package lines, beyond the existing global 256-item projection budget. Raising that shared budget or returning the first 256 entries would not meet bounded, complete enumeration. The proposed solution is one private, immutable, validated observation with small pages, not repeated reads that can skip or duplicate entries as the device changes.

## Proposed ownership and bounds

- Core owns bounded package records and a closed source/scope identifier. Features owns Packages.Read operation definitions; clients cannot choose a program, path, package manager or parsing profile.
- Device-codec parses supplied bounded bytes and encodes only reviewed fixed capture recipes. The same immutable local/SSH backend performs capture; no protocol I/O path or generic Process exception.
- Dispatcher owns one snapshot, one capture admission and continuation checks. Every page repeats authorization and safe start/completion audit. No key, raw database, upstream error or cursor is logged.
- Candidate ceilings: 4 MiB default/16 MiB maximum source bytes, 4,096 records, 2 MiB retained field bytes plus bounded indexing overhead, 64 records/page and the existing 64-KiB normalized limit. Names/versions/architectures retain exact values, including ABI suffixes; reject duplicates and malformed records before the first page.
- Candidate absolute TTL: 120 seconds, never extended by paging. Bind continuations to this dispatcher/principal, backend epoch, operation, snapshot generation and offset. A replay returns the same page; foreign/expired cursors fail, never trigger automatic recapture. A new capture invalidates the old snapshot before attempting I/O; cancellation/failure cannot restore stale results.

These numbers and owner modules are design candidates, not validated implementation or performance measurements.

## Source scope must precede implementation

APK has database layers, including root and uvol. Reading only its root installed database cannot establish an all-layer inventory. Its query command can enumerate installed packages, but configuration loading, initialization side effects and variant behavior must be reviewed before admitting a fixed command. See the pinned [APK database implementation](https://github.com/alpinelinux/apk-tools/blob/v3.0.5/src/database.c), [query implementation](https://github.com/alpinelinux/apk-tools/blob/v3.0.5/src/query.c) and [configuration loading](https://github.com/alpinelinux/apk-tools/blob/v3.0.5/src/apk.c).

An opkg adapter must likewise declare whether it covers the default status database or every configured destination. Concurrent package writers can invalidate a consistency claim: a successful read and syntactically complete prefix do not by themselves prove an atomic database snapshot. Counts must describe the captured scope, not current whole-device state. These observations must not become mutation baselines.

Do not infer manager absence from a nonzero command exit or an ambiguous RPC not-found result. Do not select APK/opkg by release string, executable-name precedence, or client assertion. A root-only database view may be useful if its name and response explicitly declare that narrower scope and non-atomic consistency; it must not be called a complete installed-package inventory.

Before accepting an architecture checkpoint, choose and source-review an implementable admission recipe: either a strictly bounded, positively observed read-only package-manager query with declared layer/destination coverage, or a target-side capture helper with an explicit protocol and deployment/security contract. A helper is not authorized for deployment to BPI-R4 by repository work. Closed source views, complete-manager discovery and mutation authority remain distinct capabilities; no convenient generic shell/file tool can substitute for them.

Mandatory future negative harness/behavior tests must cover ownership and new portable suites, weakened limits, stale/foreign cursors, cancellation, source overflow, parse failure after the first page's would-be records, changing backend epochs, multiple managers/destinations, hidden permissions and zero-I/O denials. No package workflow or new capability has been enabled by this design note.
