# v12 bounded text-option emulator acceptance

Date: 2026-09-13. Scoped userspace evidence, **not BPI-R4 hardware, complete configuration coverage, configuration application or native Windows/macOS-to-device acceptance**.

## Executed environment

- Run: **2026-09-13T10:44:52.949055Z–10:45:51.977691Z**.
- Host: Ubuntu on Linux WSL2. Official OpenWrt 25.12.5 ARM64 initramfs, armsr/armv8, r33051-f5dae5ece4, kernel 6.12.94; QEMU virt/cortex-a53 TCG, 256 MiB, one CPU.
- Image SHA-256: `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`.
- Observed rpcd `2026.06.04~28faf640-r1`, ubus `2026.06.28~24864e78-r2`, netifd `2026.02.26~cbb83a18-r1`, procd `2026.03.13~58eb263d-r1`, Dropbear `2025.89-r3`; APK 3.0.5. This is emulator inventory, not a fresh physical-router observation.
- Source: architecture-only `6c45623914ef327d6e8e007f8644e22905eda620` plus v12 text-option implementation. Release executable: **4,840,560 bytes**, SHA-256 `aba06d83d025c0461895b3d53e6887f23d4a86a58b3ca49573e9ea8c18212421`, checked before and after the run.
- Actual MCP stdio and persistent native Rust SSH, explicit loopback target, per-boot host-key pin and public synthetic fixture identity. MCP grants were read-only, without execute or write.

The owned VM had restricted user networking, no host data mounts or persistent disk. Before MCP acceptance, the private fixture runner created two named synthetic pending UCI sections in RAM, one interface and one dnsmasq section. Eighteen fixed setup commands populated public documentary addresses/domains/paths. **No commit, reload, restart, configuration application or MCP mutation occurred.** Nothing accessed the physical router, actual keys, Vault or backups. The configured hosts-file path was not read.

The first private fixture attempt joined the setup into one 1,301-character source line and failed before MCP acceptance. The runner was corrected to submit the same eighteen commands individually (maximum 82 characters); the subsequent complete run passed. The exact underlying line-handling failure was not established. No production check or acceptance requirement was weakened, and the first failure summary remains retained privately.

## Observed scope

All six operations had compatible input signatures. Network-interface and dnsmasq configuration advertised their exact v2 response IDs; the other four retained v1.

| Operation | Outcome | Text-option evidence |
| --- | --- | --- |
| system_configuration | 1 valid row | Existing hostname/timezone/zonename scalar fields only |
| network_interface_configuration | 3 valid rows, 11 total projected items | Baseline ipaddr lists; synthetic ipaddr string, ip6addr list, dns list and ifname string |
| dhcp_dnsmasq_configuration | 2 valid rows, 11 total projected items | Synthetic server list, address string, interface list, notinterface list, rebind_domain list and addnhosts string |
| firewall_defaults_configuration | 1 valid row | Existing input/output/forward scalar fields only |
| wireless_radio_configuration | Safe backend_failed | No successful radio-configuration evidence |
| storage_mount_configuration | Safe backend_failed | No successful fstab-configuration evidence |

Each operation was repeated once with the same success/error class. Successful responses passed exact envelope, unique bounded section identity, exact section type, boolean anonymous flag and u32 index checks. Every present scalar and text-option string passed its declared UTF-8/NUL bounds. Each representation wrapper had exactly kind/values; string kind contained exactly one value, list kind respected its per-option bound. Section rows and all values were counted against the shared 256-item budget. Both synthetic rows were compared in memory with the exact configured representation, values and order; no raw configuration was persisted in the report.

Normalized output and both MCP content copies matched and respected their byte ceilings. Discovery listed **33 tools**. Thirty invalid-argument cases (config/type/session/method/execute) returned unknown_argument. Six refreshed metadata checks, twelve actual calls and those rejections produced **66 correlated audit events**, each with the exact safe metadata keyset; checked prohibited material was absent. MCP shut down cleanly, and the owned VM process group and loopback listener were stopped.

Empty options, duplicate/Unicode/escaped values, malformed late values and exact resource-boundary cases retain [host fixture evidence](validation.md), not emulator acceptance. The unavailable radio/fstab cases are not relabeled as absent packages, empty configurations or successful reads. No atomic snapshot, durable commit, effective service state, mutation recovery, CPU/RSS/latency or full coverage claim follows. The retained private report contains metadata, field/form/count summaries and safe error classes only. See the [exact UCI contract](uci-observations.md).
