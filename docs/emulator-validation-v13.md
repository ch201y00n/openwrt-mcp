# v13 base-service UCI emulator acceptance

Date: 2026-09-13. Scoped userspace evidence, **not BPI-R4 hardware, complete configuration coverage, configuration application or native Windows/macOS-to-device acceptance**.

## Executed environment

- Run: **2026-09-13T11:15:44.660440Z–11:16:49.009639Z**.
- Host: Ubuntu on Linux WSL2. Official OpenWrt 25.12.5 ARM64 initramfs, armsr/armv8, r33051-f5dae5ece4, kernel 6.12.94; QEMU virt/cortex-a53 TCG, 256 MiB, one CPU.
- Image SHA-256: `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`.
- Emulator package reference: rpcd `2026.06.04~28faf640-r1`, ubus `2026.06.28~24864e78-r2`, netifd `2026.02.26~cbb83a18-r1`, procd `2026.03.13~58eb263d-r1`, Dropbear `2025.89-r3`; APK 3.0.5. This is not a fresh physical-router inventory.
- Source: architecture-only `fe677cb2674b900972b1a1b219cc4bb6e8d9013c` plus eighteen base-service UCI consumers. Release executable: **4,861,040 bytes**, SHA-256 `c1c495eee23e01262ba66501d227a57c735bc6aa5d954b502d49b1bc6af33b81`, checked before and after execution.
- Actual MCP stdio and persistent native Rust SSH, explicit loopback target, per-boot host-key pin and public synthetic fixture identity. MCP grants were read-only, without execute or write.

The owned VM used restricted user networking, no host data mounts or persistent disk. The private fixture runner submitted 76 fixed serial setup commands individually (maximum 69 characters), creating eighteen named pending UCI sections in RAM. An absent fstab file was created empty in the disposable VM, without overwriting an existing file. Public synthetic option values exercise representation only; they are not valid or complete daemon configurations. **No UCI commit, reload, restart, service application or MCP mutation occurred.** No physical router, actual key, Vault or backup was accessed.

## Observed scope

All eighteen new operations advertised compatible input signatures and their exact v1 response IDs. Each was called twice successfully. Present optional fields, forms and counts below are the actual tested scope; other documented fields remain host-fixture evidence.

| Configuration tool (prefix omitted only where clear) | Rows / total projected items | Present optional fields and forms |
| --- | --- | --- |
| system_timeserver_configuration | 2 / 10 | Scalars enable_server, enabled; server string/list, dhcp_interface list |
| network_device_configuration | 2 / 7 | Scalars name, type; ports list, ingress_qos_mapping string |
| network_bridge_vlan_configuration | 1 / 5 | Scalar device; ports string, alias list |
| network_route_v4_configuration | 1 / 1 | Scalar interface |
| network_route_v6_configuration | 1 / 1 | Scalar interface |
| network_rule_v4_configuration | 1 / 1 | Scalar in |
| network_rule_v6_configuration | 1 / 1 | Scalar in |
| firewall_zone_configuration | 3 / 10 | Scalars enabled, forward, input, masq, mtu_fix, name, output; network list, device string |
| firewall_forwarding_configuration | 2 / 2 | Scalars dest, enabled, src |
| firewall_rule_configuration | 10 / 50 | Scalars dest, enabled, family, limit, name, src, target; proto string/list, dest_port string, src_ip string, icmp_type string/list |
| firewall_redirect_configuration | 1 / 5 | Scalar enabled; proto string, src_mac list |
| firewall_nat_configuration | 1 / 4 | Scalar enabled; proto list |
| dhcp_pool_configuration | 3 / 7 | Scalars dhcpv4, dhcpv6, ignore, interface, leasetime, limit, ra, start; dns string, domain list |
| dhcp_host_configuration | 1 / 5 | Scalar name; mac list, duid string |
| dhcp_domain_configuration | 1 / 2 | Scalar ip; name string |
| dhcp_cname_configuration | 1 / 4 | Scalar target; cname list |
| storage_global_configuration | 1 / 1 | Scalar anon_swap |
| storage_swap_configuration | 1 / 1 | Scalar enabled |

The independent acceptance definitions follow the documented field table, not the production catalog. Successful responses passed exact envelope, unique bounded section identity, exact section type, boolean anonymous flag and u32 index checks. Present scalars and option strings passed their UTF-8/NUL bounds. Each option wrapper contained exactly kind/values and preserved string/list form; source list duplicates/order were checked against the synthetic fixture in memory. Rows and option values shared the unchanged 256-item limit. Normalized output and both MCP content copies matched within their byte ceilings. Raw serial/UCI responses and synthetic configuration values were not persisted in the report.

Discovery listed **51 tools** (50 reads plus capability metadata). Ninety invalid-argument cases (config/type/session/method/execute for each new tool) returned unknown_argument. Eighteen refreshed metadata checks, 36 actual calls and those rejections produced **198 correlated audit events** with exact safe metadata keysets and no checked prohibited material. MCP shut down cleanly. The owned VM process group and loopback listener were stopped; a separate process/listener check confirmed cleanup.

Empty, malformed, Unicode/escaped and exact resource-boundary cases remain [host-fixture evidence](validation.md). No baseline wireless or old mount-configuration call was made in this run; creating a synthetic fstab does not retrospectively convert the earlier v11/v12 failures into successes. This does not validate effective daemon configuration, complete options, protected lan3 dependencies, recovery, CPU/RSS/latency or physical-device support. The private report contains only metadata, field/form/count summaries and safe outcomes. See the [exact eighteen contracts](base-uci-observations.md).
