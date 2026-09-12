# Capability coverage

Internal age/key-source/container primitives are fixture-tested library infrastructure, not additional MCP/device tools. Native Windows Vault access, password-encrypted archives, backup publication, secure restore staging and device rollback remain unimplemented. See [key management](key-management.md).

Catalog entries describe configured adapter support, not live target availability. All built-in calls target `/bin/ubus -S call ...` using typed parameters and fixed local argv or quoted remote SSH arguments. Targets are explicit; an unconfigured server never executes host programs. Only declared output fields reach the agent. Host portability, native optional facilities and actual test evidence are tracked separately in [platform support](platform-support.md).

Architecture v5 adds same-target bounded introspection and `operation_capability`, a metadata tool rather than another management workflow. Unknown/incompatible input prerequisites block execution; all listings remain offline. See [capabilities](capabilities.md) for freshness, safe reasons and response/hardware distinctions. In particular, the initial official emulator publishes an incomplete global interface-status signature, so `network_interface_status` is correctly blocked until a verifiable replacement is implemented.

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
| extensions | Operator-defined fixed definitions | Requires extensions.write AND extensions.execute plus declared effects and fresh reviewed Ubus prerequisites; unverified Process definitions are blocked |

The extension registry can represent additional installed package operations through fixed ubus methods or absolute programs. No wildcard shell, arbitrary executable selected by a client, dynamic permission classification, or automatic grant follows from discovering an unknown method.

Ten built-in reads are implemented. An actual MCP/SSH run against isolated official OpenWrt 25.12.5 ARM64 QEMU validated seven successful reads and three explicit unavailable/error cases; see [emulated acceptance](emulator-validation.md). None has full BPI-R4 hardware acceptance. The five post-architecture-v2 contracts, source evidence, safe projections and fixture tests are recorded in [read-contracts.md](read-contracts.md). Missing fields are not synthesized into a healthy/stopped status, and a successful empty projection does not certify target compatibility. This is selected observability coverage, not complete read coverage of those categories.

Next coverage work: bounded record responses and typed collection adapters, broader probe/driver families (including observed apk/opkg and variants), continued emulator acceptance, then separately authorized hardware acceptance. State-changing adapters require encrypted backup, appropriate validation and recovery before being advertised as supported. Current discovery is only the closed Ubus input-signature family, not complete package/hardware capability inventory.
