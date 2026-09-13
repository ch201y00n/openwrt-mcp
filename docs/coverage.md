# Capability coverage

Internal age/key-source/container primitives are fixture-tested library infrastructure, not additional MCP/device tools. Native Windows Vault access, password-encrypted archives, backup publication, secure restore staging and device rollback remain unimplemented. See [key management](key-management.md).

Catalog entries describe configured adapter support, not live target availability. All built-in calls target `/bin/ubus -S call ...` using typed parameters and fixed local argv or quoted remote SSH arguments. Targets are explicit; an unconfigured server never executes host programs. Only declared output fields reach the agent. Host portability, native optional facilities and actual test evidence are tracked separately in [platform support](platform-support.md).

Architecture v5 adds same-target bounded introspection and `operation_capability`, a metadata tool rather than another management workflow. Unknown/incompatible input prerequisites block execution; all listings remain offline. Architecture v6 adds finite typed projections and bounded response decoding. `network_interface_status.v2` now uses fixed `network.interface.dump {}` plus exact local selection, rather than the former global `status` call with incomplete introspection. Matching fresh `dump` metadata and a valid response are still required. See [capabilities](capabilities.md).

| Category | Current built-in operations (host fixtures) | Device prerequisites / limitations |
| --- | --- | --- |
| system | system_board, system_info | procd system board/info ubus methods; selected board, release, uptime and memory data |
| network | network_device_status, network_lan_status, network_wan_status, network_interface_status, network_interfaces | netifd; fixed lan/wan names may be absent; typed dump lists/exact selection cap source interfaces at 128; no address/route/DNS/configuration subtrees |
| wireless | wireless_radio_info, wireless_devices, wireless_stations, wireless_station_status, wireless_countries | rpcd-mod-iwinfo/libiwinfo; bounded device/station/country observations and selected radio scalars. Station MAC identities are explicitly disclosed under Wireless.Read; no SSID/BSSID/keys or active scanning. Driver failures may appear as empty lists |
| firewall | Planned | firewall4/nftables typed reads and transactional writes |
| dhcp_dns | Planned | lease/state schemas and dnsmasq/odhcpd adapters |
| services | service_logd_status, service_sysntpd_status, service_status, service_status_list | Fixed standard-instance views plus typed service/instance names and running/pid/exit_code; generic metadata spans categories, never command/env/data; missing instances stay omitted |
| packages | Planned | detect opkg/apk on the actual target; privileged installation effects |
| storage | Planned | mount, disk, shares and file access constraints |
| vpn | Planned | WireGuard/OpenVPN/IPsec package adapters; key exclusion |
| firmware | Planned | image identity, compatibility, backup, irreversible-action handling |
| diagnostics | diagnostics_watchdog_status | procd system.watchdog with fixed empty arguments; no setters; hardware availability varies. Ping/resolve/log adapters remain planned |
| extensions | Operator-defined fixed definitions | Requires extensions.write AND extensions.execute plus declared effects and fresh reviewed Ubus prerequisites; unverified Process definitions are blocked |

The extension registry can represent additional installed package operations through fixed ubus methods or absolute programs. No wildcard shell, arbitrary executable selected by a client, dynamic permission classification, or automatic grant follows from discovering an unknown method.

Seventeen built-in reads are implemented: eight typed response contracts and nine legacy scalar projections. Their source contracts and synthetic fixtures are documented in [collection-read-contracts.md](collection-read-contracts.md), [passive wireless contracts](wireless-observation-contracts.md) and [read-contracts.md](read-contracts.md). Typed results enforce nonempty resource identities, exact selection, per-collection limits, a shared 256-item budget and a 64-KiB normalized-result cap; MCP additionally caps the complete serialized tool result at 256 KiB. No truncation or synthesized healthy/stopped state follows from missing or malformed data.

Current [v6 emulated acceptance](emulator-validation-v6.md) covers twelve successful reads (including the five typed contracts) and two explicit unavailable/error cases through actual MCP/SSH on official 25.12.5 ARM64 QEMU. The [historical v5 run](emulator-validation.md) remains separate and does not validate v2's new response contract. [Native Windows GNU host fixtures](windows-validation.md) also pass; MSVC, macOS and BPI-R4 hardware acceptance remain pending. This is selected observability coverage, not complete read coverage of those categories.

Generic Services.Read includes daemon/instance names and running/PID/exit metadata even for services belonging to other categories. Deny `service_status` and `service_status_list` if only the fixed logd/sysntpd views should be exposed; these reads never authorize lifecycle execution or configuration access.

The three new passive wireless operations have synthetic host/MCP acceptance, not radio or emulator acceptance. Deny `wireless_stations` and `wireless_station_status` to withhold client MAC identities while retaining the other wireless reads. Package enumeration remains [proposed architecture work](package-inventory-design.md), not an implemented operation.

Next coverage work: additional reviewed typed collection adapters, broader probe/driver families (including observed apk/opkg and variants), current-version emulator acceptance, then separately authorized hardware acceptance. State-changing adapters require encrypted backup, appropriate validation and recovery before being advertised as supported. Current discovery is only the closed Ubus input-signature family, not complete package/hardware capability inventory.
