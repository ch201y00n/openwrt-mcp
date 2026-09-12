# Capability coverage

Internal age/key-source/container primitives are fixture-tested library infrastructure, not additional MCP/device tools. Native Windows Vault access, password-encrypted archives, backup publication, secure restore staging and device rollback remain unimplemented. See [key management](key-management.md).

Catalog entries describe configured adapter support, not live target availability. All built-in calls execute `/bin/ubus -S call ...` using fixed argv and typed parameters. Only declared output fields reach the agent.

| Category | Current built-in operations (host fixtures) | Device prerequisites / limitations |
| --- | --- | --- |
| system | system_board, system_info | procd system board/info ubus methods; selected board, release, uptime and memory data |
| network | network_device_status, network_lan_status, network_wan_status, network_interface_status | netifd; fixed lan/wan names may be absent; general interface status takes a required logical interface selector |
| wireless | wireless_radio_info | rpcd-mod-iwinfo/libiwinfo and supported driver; selected radio scalars only, no SSID/BSSID/keys/client list |
| firewall | Planned | firewall4/nftables typed reads and transactional writes |
| dhcp_dns | Planned | lease/state schemas and dnsmasq/odhcpd adapters |
| services | service_logd_status, service_sysntpd_status | Standard procd init instance names; selected running/pid/exit_code only, never command/env/data; missing fields mean unknown |
| packages | Planned | detect opkg/apk on the actual target; privileged installation effects |
| storage | Planned | mount, disk, shares and file access constraints |
| vpn | Planned | WireGuard/OpenVPN/IPsec package adapters; key exclusion |
| firmware | Planned | image identity, compatibility, backup, irreversible-action handling |
| diagnostics | diagnostics_watchdog_status | procd system.watchdog with fixed empty arguments; no setters; hardware availability varies. Ping/resolve/log adapters remain planned |
| extensions | Operator-defined fixed actions | Requires extensions.write AND extensions.execute, plus declared effects; trusted definitions only |

The extension registry can represent additional installed package operations through fixed ubus methods or absolute programs. No wildcard shell, arbitrary executable selected by a client, dynamic permission classification, or automatic grant follows from discovering an unknown method.

Ten built-in reads are implemented; none is yet device-validated. The five post-architecture-v2 contracts, source evidence, safe projections and acceptance tests are recorded in [read-contracts.md](read-contracts.md). Missing fields are not synthesized into a healthy/stopped status, and a successful empty projection does not certify target compatibility. This is selected observability coverage, not complete read coverage of those categories.

Next coverage work: target capability discovery that distinguishes available, unsupported and policy-denied methods; native response schemas; per-OpenWrt-version fixtures; emulator integration; then device acceptance. State-changing adapters require encrypted backup, appropriate validation and recovery before being advertised as supported.
