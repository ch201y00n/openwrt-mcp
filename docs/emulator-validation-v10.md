# v10 interface-IP and LuCI emulator acceptance

Date: 2026-09-13. Actual stdio MCP -> native Rust SSH -> official OpenWrt 25.12.5 ARM64 QEMU. This is isolated userspace acceptance, not BPI-R4 hardware, Windows/macOS-to-router testing, native OpenWrt binary deployment or full management coverage.

The environment remains [the private emulator profile](emulator-environment.md): QEMU ARM64 virt/cortex-a53 TCG, 256 MiB, no host disk/share, no bridge, restricted user networking with IPv6 disabled and only a loopback SSH forward. The fixture's network/authentication setup is confined to disposable RAM. Public deterministic test authentication and a per-boot host pin from the controlled serial console were used. No actual router, key or Vault access.

- Strict-contract run UTC: **2026-09-13T09:43:52.134289Z–09:44:50.878626Z**.
- Host: Ubuntu Linux under WSL2; target: armsr/armv8, OpenWrt 25.12.5 r33051-f5dae5ece4, kernel 6.12.94, APK 3.0.5 aarch64.
- Installed metadata: netifd 2026.02.26~cbb83a18-r1, rpcd 2026.06.04~28faf640-r1, ubus 2026.06.28~24864e78-r2, procd 2026.03.13~58eb263d-r1, Dropbear 2025.89-r3. Exact optional LuCI package identities were not collected; do not infer them from a branch review.
- Image SHA-256: `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`.
- Release executable: **4,795,504 bytes**, SHA-256 `f03f6620e9490e4b43397e62680830cdf61beb35a8af9e5db27a286c76f573c3`. Built from `e071f3e` plus the interface-IP implementation and Cargo-root harness correction; preserved privately and rehashed after the run. Later documentation-only edits do not relabel this source scope.

## Observed results

| Tool / response | Actual tested scope |
| --- | --- |
| network_interface_addresses.v1 | loopback and lan each returned one IPv4 row; all four active/inactive address collections validated. Loopback address/mask checked independently. No populated IPv6, point-to-point or lifetime acceptance |
| network_interface_routes.v1 | Both interfaces returned valid empty active/inactive route collections; not populated FIB or policy-route acceptance |
| network_interface_neighbors.v1 | Both interfaces returned valid empty netifd-managed neighbor collections; not kernel cache, discovered clients or nonempty-neighbor acceptance |
| dhcp_interface_dns.v1 | Both interfaces returned valid empty server/search and inactive collections; no actual DNS request or effective resolver proof |
| storage_mounts.v2 | Three mount rows validated against the exact field/type/identity bounds; unsigned capacities remained canonical decimal strings |
| storage_block_devices.v2 | Valid empty block-device observation; not real disk/swap/signature acceptance |
| dhcp_v4_leases.v1 and dhcp_v6_leases.v1 | Valid empty lease arrays; no populated leases, false-expiry row, DUID or IPv6 prefix acceptance |

All eight tools reported compatible input signatures and exact current response IDs on this target. That does not attest all output variants or future versions. Separate host fixtures exercise populated/duplicate/malformed/oversized cases; they are not reclassified as emulator observations. The [interface-IP](interface-ip-observations.md), [storage](storage-observations.md) and [DHCP](dhcp-observations.md) limitations continue to apply.

Discovery exposed 26 read tools plus the authorized metadata tool with read-only annotations. For each of the four interface tools, missing/empty/extra arguments were rejected, an unknown exact selector returned selection_not_observed, and a subsequent known-interface call succeeded using the same client. Forced capability refresh succeeded. Both MCP result copies matched and satisfied the 64-KiB normalized/256-KiB whole-result limits.

**68 audit events** had exactly the approved keys and correlated with all calls. No response fields, client selector marker, credentials, host pin, payload or argument values appeared in logs. The evidence report retains safe counts/metadata only. The MCP process exited cleanly; the owned VM process group and loopback listener stopped, and an independent exact-process lookup found no remaining lab QEMU process.

An earlier run at 09:41:28Z–09:42:29Z validated the interface tools but checked only optional LuCI envelopes; it is superseded for LuCI field validation by the strict run above, not silently relabeled. Neither run mutates a production router, installs optional packages, persists decrypted backups or publishes a release.
