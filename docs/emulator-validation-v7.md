# v7 package observation emulator acceptance

Date: 2026-09-13. This is an isolated OpenWrt-image test, not BPI-R4 hardware or whole-device inventory acceptance.

The actual stdio MCP executable used its native Rust SSH backend against the official OpenWrt 25.12.5 ARM64 initramfs image in QEMU. The environment and image trust/download caveats remain those in [emulator-environment.md](emulator-environment.md). QEMU had 256 MiB RAM, no host disks or shares, no bridged network, restricted user networking and only a loopback SSH forward. Authentication used public deterministic test credentials and a per-boot host pin obtained over the controlled serial console. No production key or Vault was accessed.

- UTC: **2026-09-13T07:42:13.224734Z–07:43:12.457219Z**.
- Host: Linux under WSL2 Ubuntu, not native Windows or macOS.
- Target: armsr/armv8, OpenWrt 25.12.5 r33051-f5dae5ece4, kernel 6.12.94, APK 3.0.5 (aarch64).
- Image SHA-256: `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`.
- Debug executable SHA-256: `9501209e00958c133d66e3fc7016e775663dc6451c6b4e1fd94a94a03e06780b`, built from the post-`0521c7c` v7 implementation working tree. Later protocol annotation/help and host regression changes are not silently relabeled as this binary.

Only Packages.Read was enabled. Listing returned the package operation and the authorized metadata tool. Metadata reported `capture_required` without claiming an input signature. The fixed version/query recipe completed and **205 records** were validated and enumerated across **13 pages** with no repeated `(layer,name)` identity and with the exact captured count. Both text and structured MCP copies matched and retained their byte bounds. Every page declared APK-visible non-atomic scope and whole-device completeness false.

A continuation replay returned identical data. A new capture invalidated the previous cursor. Three malformed/unsupported client inputs were rejected, including attempts to choose another manager or executable. **39 audit events** used only the approved keys; no package payload, cursor, identity or SSH host pin appeared in the logs. The private evidence file retains safe counts/metadata only, not the package list. The MCP process exited cleanly. The owned VM process group and loopback listener were stopped; an independent process lookup found no remaining lab QEMU process.

This accepts `packages_apk_installed` for this source profile and test image only. It does not test opkg, populated uvol layers, concurrent real package writers, privileged installation/removal scripts, on-device Rust execution, firmware, physical BPI-R4 peripherals or production deployment. The 281-record completeness case is a separate synthetic host test, not this VM's installed count. Earlier [v6 read acceptance](emulator-validation-v6.md) and later passive wireless fixture tests remain separately scoped.
