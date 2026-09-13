# v14 opkg root status-file emulator acceptance

Date: 2026-09-13. Scoped Linux-host SSH/userspace evidence, **not BPI-R4 hardware,
Windows/macOS-to-device, on-device Rust binary or complete package management**.

## Executed environment

Identity and selected package provenance are recorded separately in
[the v14 emulator environment](emulator-environment-v14.md).

- Final run: **2026-09-13T12:06:32.125694Z–12:07:30.712095Z**. An earlier run at
  12:03:56.136953Z–12:04:53.585815Z also passed the same workflow; the final report
  additionally records five selected component versions from the observed rows.
- Ubuntu on Linux WSL2; QEMU ARM64 virt/cortex-a53 TCG, 256 MiB, one CPU.
- Official OpenWrt **24.10.4**, **r28959-29397011cc**, armsr/armv8, kernel **6.6.110**.
  This is an explicitly reviewed opkg-family release, not a latest-release claim.
- Official initramfs: **39,465,472 bytes**, SHA-256
  `d0d8fe1e908902c99a055fba1f2a979be10e538f71f18cf0f78010718c7e04be`, verified before
  boot against the [release checksum list](https://downloads.openwrt.org/releases/24.10.4/targets/armsr/armv8/sha256sums).
- Exact manager banner: opkg version
  `38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)`.
- Selected status-file component versions: opkg `2024.10.16~38eccbb1-r1`,
  ubus `2025.10.17~60e04048-r1`, rpcd `2025.09.01~bba95191-r1`,
  netifd `2025.05.23~7901e66c-r1`, procd `2024.12.22~42d39376-r1`.
  These are source-file observations, not package health or binary attestation.
- Source: architecture-only `2233f3e83a63313e98bcf9413c3efa78ea6ee042` plus the v14
  implementation. Release executable **4,889,712 bytes**, SHA-256
  `f55f254b5e2d53a7cf36afea834f6b6b0a823993f1683dc36c215af95086fcd1`, checked before
  and after execution from a private copy.

Actual MCP stdio used native Rust persistent SSH, an explicit loopback-only target,
per-boot host-key pin and a public deterministic synthetic fixture identity. Only
Packages.Read was granted; write and execute were disabled. The VM had restricted
user networking, no host data mounts or persistent disk. SSH fixture authorization
and a test IP alias existed only in the owned RAM VM. No package database, feed or
UCI fixture was changed. No physical router, real key, Vault or backup was accessed.

## Observed results

`packages_opkg_status` captured **196 records in 13 pages**, with exact name,
version, architecture and opaque status fields. Names were globally unique and
sorted; two distinct status strings occurred. No installed-only filtering or
status-health inference was applied. Every page declared the root-status-file,
non-atomic scope and whole-device completeness false. Both MCP result copies
matched and remained within the normalized and serialized byte ceilings.

Discovery exposed exactly three tools: the two package operations and capability
metadata. The metadata call returned unknown/capture_required and the exact v1
response ID with closed_file_response scope; it did not claim that a query had
already succeeded. Four forbidden argument cases (path/program/manager/page_size)
and two invalid cursor representations were rejected.

Repeated continuation returned an identical page. A cursor submitted to the other
manager failed invalid_cursor. An explicit fresh opkg capture invalidated the old
cursor. Explicit APK capture failed backend_failed because that executable was
unavailable; it did not fall back to opkg and invalidated the prior opkg snapshot.
A subsequent explicitly requested opkg capture succeeded again. These are separate
positive and negative cases, not successful APK support on this image.

The run verified **50 correlated audit events**, each with exactly the permitted
metadata keys; no checked private identity, target, argument or payload material
occurred. The report retains only safe metadata/counts/outcomes and the five
selected package versions. Raw serial, status-file contents and MCP payloads were
not persisted. MCP shutdown completed normally. The owned VM process group and
loopback listener stopped; independent process/listener checks confirmed cleanup.

Empty files, other status values, malformed/duplicate/late records, byte/line/field
limits, resource exhaustion, capture cancellation and transport fault injection
remain [host-fixture evidence](validation.md). No performance measurement, package
mutation, automatic manager discovery, other opkg revision, alternative configured
destination or whole-device inventory guarantee follows. See the
[exact opkg contract](opkg-observations.md).
