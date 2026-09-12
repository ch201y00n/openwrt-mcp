# Capability coverage

Catalog entries describe configured adapter support, not live target availability. All built-in calls execute `/bin/ubus -S call ...` using fixed argv and typed parameters. Only declared output fields reach the agent.

| Category | v0.1 built-in operations | Device prerequisites / limitations |
| --- | --- | --- |
| system | system_board, system_info | procd system board/info ubus methods; selected board, release, uptime and memory data |
| network | network_device_status, network_lan_status, network_wan_status | netifd; fixed lan/wan logical names may be absent or renamed; device status requires name |
| wireless | Planned | hostapd/iwinfo schemas and secret-safe projections |
| firewall | Planned | firewall4/nftables typed reads and transactional writes |
| dhcp_dns | Planned | lease/state schemas and dnsmasq/odhcpd adapters |
| services | Planned | safe service list projection; scoped lifecycle methods |
| packages | Planned | detect opkg/apk on the actual target; privileged installation effects |
| storage | Planned | mount, disk, shares and file access constraints |
| vpn | Planned | WireGuard/OpenVPN/IPsec package adapters; key exclusion |
| firmware | Planned | image identity, compatibility, backup, irreversible-action handling |
| diagnostics | Planned | bounded typed ping/resolve/log adapters |
| extensions | Operator-defined fixed actions | Requires extensions.write AND extensions.execute, plus declared effects; trusted definitions only |

The extension registry can represent additional installed package operations through fixed ubus methods or absolute programs. No wildcard shell, arbitrary executable selected by a client, dynamic permission classification, or automatic grant follows from discovering an unknown method.

Next coverage work: target capability discovery that distinguishes available, unsupported and policy-denied methods; native response schemas; per-OpenWrt-version fixtures; emulator integration; then device acceptance. State-changing adapters require encrypted backup, appropriate validation and recovery before being advertised as supported.
