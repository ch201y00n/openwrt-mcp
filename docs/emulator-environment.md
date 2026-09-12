# Isolated OpenWrt 25.12.5 environment

Observed 2026-09-12 UTC; this is an emulated ARM64 userspace, not BPI-R4 hardware. The MCP acceptance run is recorded separately in [emulator-validation.md](emulator-validation.md).

| Property | Observed value |
| --- | --- |
| OpenWrt | 25.12.5, r33051-f5dae5ece4 |
| Target / board / model | armsr/armv8; linux,dummy-virt; linux,dummy-virt |
| Kernel | 6.12.94 |
| Emulator | QEMU 8.2.2, ARM64 virt/cortex-a53, TCG, 256 MiB |
| Host | Ubuntu Linux under WSL2; not native Windows/macOS |
| ubus | 2026.06.28~24864e78-r2 |
| rpcd | 2026.06.04~28faf640-r1 |
| netifd | 2026.02.26~cbb83a18-r1 |
| procd | 2026.03.13~58eb263d-r1 |
| apk | 3.0.5 |
| Dropbear | 2025.89-r3 |

The base image has different rpcd and netifd package revisions from the actual [BPI-R4 inventory](reference-target.md). Matching release names must not erase that distinction. There is no physical wireless radio or reference WAN setup in this lab.

## Image provenance

The image is the official [25.12.5 ARM64 initramfs kernel](https://downloads.openwrt.org/releases/25.12.5/targets/armsr/armv8/openwrt-25.12.5-armsr-armv8-generic-initramfs-kernel.bin), 35,754,496 bytes. Its SHA256 was checked against the release manifest:

`f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`

The downloaded [sha256sums manifest](https://downloads.openwrt.org/releases/25.12.5/targets/armsr/armv8/sha256sums) was 182,306 bytes, SHA256 `2ba9c990e6f05b9875aa54992171f6a2cd447ab01c64ed11b02c6c7116cc31ec`. Its detached signature validated with signing fingerprint `92C561DE55AE6552F3C736B82B0151090606D1D9`, primary fingerprint `8A8BC12F46B836C0F9CDB36F1D53D1877742E911`.

The public key was newly retrieved from the [official OpenWrt keyring at a pinned commit](https://raw.githubusercontent.com/openwrt/keyring/6b42a5c8b7dc049b899869b2a1b94daf69ceb2f5/gpg/0x1D53D1877742E911.asc), not an already trusted local key. This is explicit trust on first retrieval, not pre-established offline release-key trust. Signature and checksum validation do not attest the physical router or every runtime package.

## Isolation and lifecycle

The disposable initramfs VM used no host disk, shared directory, physical interface or bridge. Its bounded SSH acceptance phase used restricted user networking with one loopback-only forward, synthetic VM-only LAN addressing, a public test authentication identity and this boot's host key obtained through the controlled serial channel. No user identity, Vault archive or real-router configuration was used.

VM helpers and extracted emulator dependencies were kept in a private, non-synced WSL directory outside both repositories. Emulator dependencies were extracted privately without system installation. Each VM run was bounded; the acceptance driver stopped the VM and verified its loopback listener was closed. Only safe software identity, test results and hashes are recorded here. Raw device responses and audit events were checked in memory, not published.
