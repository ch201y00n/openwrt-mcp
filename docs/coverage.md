# Capability coverage

The [full feature specification](management-feature-spec.md) records the target
backlog and maps all 51 current built-in reads to stable feature IDs. Use the
[staged plan](implementation-plan.md) for future work. The current v20 checkpoint
adds only architecture/harness for four more UCI recipes; those four operations
are not implemented. The milestones below retain their scoped implementation evidence.

The v19 [Windows private log profile](windows-private-logs.md) implements protected
append/rotation through the existing audit configuration. Native synthetic files
and actual MCP binary composition have scoped evidence. This adds no router tool,
Vault/system-log support, durable backup store or macOS acceptance.

The v18 [provided-stream sealing flow](validated-archive-sealing.md) adds internal orchestration and synthetic age/gzip composition, not a callable backup tool. It requires complete input, validated counts, producer completion and acknowledged one-shot publication with explicit uncertainty. Real capture/storage, production consumers and mutation/recovery authority remain absent.

The v17 [gzip wrapper](gzip-archive-validation.md) extends supplied archive validation only. It is the sole internal consumer of v16, without application/device integration. Complete single-member corruption/size checks and independent host fixtures pass; no new MCP operation, actual OpenWrt backup, encryption/publication or restore workflow is covered.

The v16 [archive validator](backup-archive-validation.md) is another internal prerequisite, not backup capture, publication or restoration. It checks bounded supplied regular-file tar bytes against every expected name/size, without retaining payloads or exposing paths. Synthetic tests and eight independent Windows/Linux producer-validator combinations pass; no actual OpenWrt archive or production consumer is covered.

V14 adds the separate [opkg root-status view](opkg-observations.md), using an exact version/file recipe without ordinary opkg initialization writes. The [24.10.4 emulator run](emulator-validation-v14.md) enumerated 196 records in 13 pages and checked shared-slot invalidation and 50 safe audit events. APK absence on this image is recorded as a failure, not fallback or merged inventory.

V13 adds eighteen [base-service UCI views](base-uci-observations.md): NTP, network devices/bridge VLANs/routes/rules, firewall zones/forwardings/rules/redirects/NAT, DHCP pools/hosts/domains/CNAMEs and storage globals/swaps. There are now 24 closed UCI recipes with unchanged category authorization, probes and global bounds. These are selected configuration observations, not complete options, effective state, arbitrary UCI access or mutations. The category table below lists earlier tools; this supplement identifies the additional scope. The separate [v13 emulator run](emulator-validation-v13.md) exercises all eighteen using fixed pending RAM fixtures and verifies 198 safe audit events. Only fields actually observed in that report have emulated evidence.

Internal age/key-source/container primitives are fixture-tested library infrastructure, not additional MCP/device tools. Native Windows Vault access, password-encrypted archives, backup publication, secure restore staging and device rollback remain unimplemented. See [key management](key-management.md).

The v15 [protected-resource effect model](protected-resource-effects.md) is also internal library infrastructure, not an additional tool or enforced router protection. It analyzes supplied graphs only; every production consumer remains forbidden until reviewed workflow integration. Actual topology extraction, freshness, encrypted backup and recovery remain gaps.

Catalog entries describe configured adapter support, not live target availability. Forty-nine reads use fixed `/bin/ubus -S call ...` actions; two additional package workflows use a closed APK version/query recipe or opkg version/root-status-file recipe. Both use fixed local argv or quoted remote SSH arguments. Targets are explicit; an unconfigured server never executes host programs. Only declared output fields reach the agent. Host portability, native optional facilities and actual test evidence are tracked separately in [platform support](platform-support.md).

Architecture v5 adds same-target bounded introspection and `operation_capability`, a metadata tool rather than another management workflow. Unknown/incompatible input prerequisites block execution; all listings remain offline. Architecture v6 adds finite typed projections and bounded response decoding. `network_interface_status.v2` now uses fixed `network.interface.dump {}` plus exact local selection, rather than the former global `status` call with incomplete introspection. Matching fresh `dump` metadata and a valid response are still required. See [capabilities](capabilities.md).

| Category | Current built-in operations (host fixtures) | Device prerequisites / limitations |
| --- | --- | --- |
| system | system_board, system_info, system_configuration | procd board/info and selected scalar UCI system options |
| network | network_device_status, network_lan_status, network_wan_status, network_interface_status, network_interfaces, network_interface_addresses, network_interface_routes, network_interface_neighbors | netifd; fixed lan/wan names may be absent. Typed dump/exact selection caps source interfaces at 128. Scoped active/inactive addresses/routes/managed neighbors, not complete kernel FIB/ARP/NDP. No raw configuration or protocol-data subtrees |
| wireless | wireless_radio_info, wireless_devices, wireless_stations, wireless_station_status, wireless_countries | rpcd-mod-iwinfo/libiwinfo; bounded device/station/country observations and selected radio scalars. Station MAC identities are explicitly disclosed under Wireless.Read; no SSID/BSSID/keys or active scanning. Driver failures may appear as empty lists |
| firewall | firewall_defaults_configuration | Selected scalar UCI defaults; effective firewall4/nftables rules and transactional writes remain planned |
| dhcp_dns | dhcp_v4_leases, dhcp_v6_leases, dhcp_interface_dns | Optional luci-rpc lease-file observations; duplicate rows and false/integer expiry preserved. Separate netifd interface DNS/search observation, not effective resolver configuration or queries. Explicit client-identifier disclosure; non-atomic, not complete leases/DNS or connectivity. Configuration, reservations, RA and active DNS adapters remain planned |
| services | service_logd_status, service_sysntpd_status, service_status, service_status_list | Fixed standard-instance views plus typed service/instance names and running/pid/exit_code; generic metadata spans categories, never command/env/data; missing instances stay omitted |
| packages | packages_apk_installed, packages_opkg_status | Closed APK 3.0.5 visible installed-query or opkg 38eccbb1 root-status-file observation; 16-record immutable pages share one profile-bound slot. Non-atomic, no health interpretation or whole-device completeness. Manager discovery and privileged mutations remain planned |
| storage | storage_mounts, storage_block_devices | Optional LuCI-visible bounded mount/block/swap metadata with explicit paths and labels/UUIDs; non-atomic and not complete inventory. No configuration, shares, file contents or mutations |
| vpn | Planned | WireGuard/OpenVPN/IPsec package adapters; key exclusion |
| firmware | Planned | image identity, compatibility, backup, irreversible-action handling |
| diagnostics | diagnostics_watchdog_status | procd system.watchdog with fixed empty arguments; no setters; hardware availability varies. Ping/resolve/log adapters remain planned |
| extensions | Operator-defined fixed definitions | Requires extensions.write AND extensions.execute plus declared effects and fresh reviewed Ubus prerequisites; unverified Process definitions are blocked |

Additional category-owned v11 tools are `network_interface_configuration` (network), `wireless_radio_configuration` (wireless), `dhcp_dnsmasq_configuration` (dhcp_dns), and `storage_mount_configuration` (storage). Together with the system/firewall tools above, these six [closed UCI reads](uci-observations.md) expose only reviewed scalar options and exact section metadata. They do not implement complete configuration, reservations, shares, credentials, committed-only snapshots or effective state.

V12 extends the network-interface and dnsmasq configuration responses to v2 with ten explicitly selected text/list options. Nested terminal TextOption preserves representation, empty forms, order and duplicates with fixed kind/values output. All option values and section rows share unchanged global bounds. No new tool, UCI recipe, probe, mutation or arbitrary configuration access is introduced; the other four UCI response IDs remain v1.

The extension registry can represent additional installed package operations through fixed ubus methods or absolute programs. **Every custom Ubus uci action is rejected**, even for privileged extensions. No wildcard shell, arbitrary executable selected by a client, dynamic permission classification, or automatic grant follows from discovering an unknown method.

Fifty-one built-in reads are implemented: forty typed response contracts, nine legacy scalar projections and two closed package observation workflows. Contracts include [UCI observations](uci-observations.md), [collection reads](collection-read-contracts.md), [interface IP](interface-ip-observations.md), [wireless](wireless-observation-contracts.md), [scalar reads](read-contracts.md), [DHCP](dhcp-observations.md), [storage](storage-observations.md), [APK](package-observations.md) and [opkg](opkg-observations.md). V11 adds exact finite text enums without changing v6's shared 256-item budget, 64-KiB normalized result or 256-KiB whole MCP result cap. V10 retains ordered non-identity rows, false/integer sentinels and root error guards. V7/v14 privately validate bounded source-specific captures before returning 16-record pages. No silent truncation or synthesized state follows from malformed/missing data.

Current [v6 emulated acceptance](emulator-validation-v6.md) covers twelve successful reads (including the five typed contracts) and two explicit unavailable/error cases through actual MCP/SSH on official 25.12.5 ARM64 QEMU. The [historical v5 run](emulator-validation.md) remains separate and does not validate v2's new response contract. [Native Windows GNU host fixtures](windows-validation.md) also pass; MSVC, macOS and BPI-R4 hardware acceptance remain pending. This is selected observability coverage, not complete read coverage of those categories.

Generic Services.Read includes daemon/instance names and running/PID/exit metadata even for services belonging to other categories. Deny `service_status` and `service_status_list` if only the fixed logd/sysntpd views should be exposed; these reads never authorize lifecycle execution or configuration access.

The three passive wireless operations have synthetic host/MCP acceptance, not radio or emulator acceptance. Deny `wireless_stations` and `wireless_station_status` to withhold client MAC identities. The separate [v7 package emulator run](emulator-validation-v7.md) enumerated 205 captured records in 13 pages, with replay and refresh-invalidation checks; it does not validate whole-device inventory or mutations.

Next coverage work: additional reviewed typed collection adapters, broader probe/driver families (including observed apk/opkg and variants), current-version emulator acceptance, then separately authorized hardware acceptance. State-changing adapters require encrypted backup, appropriate validation and recovery before being advertised as supported. Current discovery is only the closed Ubus input-signature family, not complete package/hardware capability inventory.

The [v10 emulator run](emulator-validation-v10.md) adds exact scoped evidence for interface-IP observations, mount rows and empty block/lease collections. Empty cases do not prove populated routes/neighbors/DNS/leases/disks; the evidence record names each tested shape and response version.

The [v11 emulator run](emulator-validation-v11.md) validates selected system/network/firewall/dnsmasq scalar configuration rows. Wireless/fstab configuration returned safe backend failures despite matching signatures, not successful or empty observations. Unobserved fields and remaining configuration families are not promoted to device-tested coverage.

The separate [v12 emulator run](emulator-validation-v12.md) validates all ten new network/dnsmasq text-option fields using two fixed synthetic pending RAM sections and existing baseline rows. It verifies actual string/list preservation through MCP/SSH without committing or applying those settings. Empty, malformed and boundary cases remain host-fixture evidence; this does not establish full configuration or hardware support.
