# opkg-family emulator identity and inventory provenance

Inventory metadata recorded during the isolated run ending
**2026-09-13T12:07:30.712095Z**. This file supplies identity provenance only; it is
not operation acceptance, live device state or a physical BPI-R4 inventory.

Official OpenWrt 24.10.4 ARM64 initramfs, armsr/armv8,
revision r28959-29397011cc, kernel 6.6.110. Release and kernel were observed using
fixed system metadata queries in the disposable VM. The compatibility manifest's
board label `qemu-virt-cortex-a53` describes the configured emulator machine, not
an asserted system.board property or physical board identity.

The [official target directory](https://downloads.openwrt.org/releases/24.10.4/targets/armsr/armv8/)
supplied the 39,465,472-byte initramfs. Its SHA-256,
`d0d8fe1e908902c99a055fba1f2a979be10e538f71f18cf0f78010718c7e04be`, matched the
published checksum before boot. The host was Ubuntu on Linux WSL2. QEMU virt used
cortex-a53 TCG, one CPU, 256 MiB, restricted user networking and one loopback SSH
forward. No persistent disk or host data mounts were attached.

The exact version option reported opkg
`38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)`. Five selected package
versions were recorded from validated root-status-file rows in memory:

| Component | Observed recorded version |
| --- | --- |
| opkg | 2024.10.16~38eccbb1-r1 |
| ubus | 2025.10.17~60e04048-r1 |
| rpcd | 2025.09.01~bba95191-r1 |
| netifd | 2025.05.23~7901e66c-r1 |
| procd | 2024.12.22~42d39376-r1 |

These selected file entries do not attest running binaries, configuration,
package health, other destinations or every installed component. The report's
legacy `installed_components` field is empty; the separate
`selected_package_versions` map contains only these five observations.

SSH authorization used a public deterministic synthetic identity and a per-boot
host-key pin. No actual router, personal key or Vault was accessed. Raw serial,
status-file and MCP response data were not retained. Both the owned process group
and listener stopped after the run. The separate
[v14 acceptance record](emulator-validation-v14.md) documents the tested
operations and failures without promoting this inventory into acceptance.
