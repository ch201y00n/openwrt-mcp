# v11 closed UCI emulator acceptance

Date: 2026-09-13. Scoped userspace evidence, **not BPI-R4 hardware, full configuration coverage, mutation or native Windows/macOS-to-device acceptance**.

## Executed environment

- Run: **2026-09-13T10:18:47.557977Z–10:19:45.430856Z**.
- Host: Ubuntu on Linux WSL2. Official OpenWrt 25.12.5 ARM64 initramfs, armsr/armv8, r33051-f5dae5ece4, kernel 6.12.94; QEMU virt/cortex-a53 TCG, 256 MiB, one CPU.
- Image SHA-256: `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`.
- Observed rpcd `2026.06.04~28faf640-r1`, ubus `2026.06.28~24864e78-r2`, netifd `2026.02.26~cbb83a18-r1`, procd `2026.03.13~58eb263d-r1`, Dropbear `2025.89-r3`; APK 3.0.5. These are this emulator's components, not a fresh physical-router inventory.
- Source: architecture-only `0b581eaceedfe81a8fd214b5df6875cc9633ccce` plus v11 UCI implementation. Release executable: **4,820,080 bytes**, SHA-256 `1c704e781466c953b77dc09155165e1cdc43203b0be680598e2ccd07f412d1d8`, checked before and after the run.
- Actual MCP stdio and the application's persistent native Rust SSH backend. Explicit loopback target and per-boot host-key pin, public synthetic fixture identity. Read grants only; no execute/write grant.

The VM had restricted user networking, no host data mounts or persistent disk. Fixture boot setup used only private RAM state; no physical router, real keys, Vault or backup was accessed. No package installation, raw UCI output capture or configuration mutation through MCP occurred.

## Observed scope

All six operations advertised a compatible `uci.get` config/type input signature and the exact `.v1` response contract. This **did not guarantee a successful configuration read**:

| Operation | Actual outcome | Fields observed beyond required section metadata |
| --- | --- | --- |
| system_configuration | 1 valid row | hostname, timezone, zonename |
| network_interface_configuration | 2 valid rows | device, proto |
| firewall_defaults_configuration | 1 valid row | input, output, forward |
| dhcp_dnsmasq_configuration | 1 valid row | authoritative, boguspriv, cachesize, domain, domainneeded, expandhosts, local, localservice, rebind_protection |
| wireless_radio_configuration | Safe `backend_failed` | No successful response/physical-radio evidence |
| storage_mount_configuration | Safe `backend_failed` | No successful fstab/mount-configuration evidence |

The two backend failures are deliberately **not** relabeled as absent packages/files, empty configurations or successful reads: no raw stderr/configuration was disclosed to diagnose a more specific cause. Each operation was repeated once and retained its success/error class. Successful repeats also passed strict field validation; the run does not claim snapshot atomicity or stable configuration revisions.

Each successful response checked its exact envelope, unique nonempty bounded section keys, exact section type, required boolean anonymous flag and u32-range index. Every present optional field was an explicitly permitted NUL-free bounded UTF-8 string. Undeclared fields were forbidden in normalized output. The normalized JSON and both MCP content copies were checked against existing byte ceilings. Options not present here retain synthetic-host evidence only. No pending-delta/commit/restore semantics or effective-service behavior was exercised.

Discovery returned **33 tools**: 32 built-in reads plus capability metadata. Thirty invalid argument cases (config/type/session/method/execute supplied to parameterless tools) returned `unknown_argument`. Six refreshed metadata checks and twelve actual calls, together with those rejections, produced **66 audit events**. Every audit event had exactly the safe metadata keyset and correlated call sequence; arguments, payloads, fixture secrets, host pins and selected identifiers were absent. MCP shut down cleanly.

Only a private metadata/count/error summary was retained outside repositories and synced storage. The owned process group and loopback listener were stopped and independently checked. No RSS/CPU/latency or public-release claim is made. [Host validation](validation.md) and [UCI source contract](uci-observations.md) remain separate evidence.
