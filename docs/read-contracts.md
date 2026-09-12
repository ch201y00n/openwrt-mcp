# Post-v2 reads and their current contracts

Historical origin: architecture-only checkpoint 8faf423 passed the complete repository gate before the five post-v2 reads were implemented and fixture-tested. Four retain their original scalar projections. Architecture v6 deliberately replaces `network_interface_status.v1` with a bounded dump-and-select implementation and typed response v2; its authoritative current definition is in [collection-read-contracts.md](collection-read-contracts.md). This migration followed the separate v6 architecture-only checkpoint, not a projection bypass.

All examples are synthetic. The original source review is against upstream on 2026-09-13, not release/device certification. The separately recorded [historical v5 emulator run](emulator-validation.md) does not validate v6's changed interface contract. Absent legacy scalar fields stay absent; empty results do not prove that a service is stopped or a capability is supported. Object/array values at legacy scalar pointers are discarded. Typed v2 instead rejects missing required fields or malformed types and distinguishes a missing selection from an empty list.

| Tool / category | Fixed object.method | Input contract | Safe response contract |
| --- | --- | --- | --- |
| network_interface_status / network | network.interface.dump, fixed {} | Required nonempty `interface`, at most 256 UTF-8 bytes; exact local selector, not an ubus argument | Typed v2 field names including interface identity and approved state; no addresses, routes, DNS or data subtrees; see collection contract |
| wireless_radio_info / wireless | iwinfo.info | One required string: device | Radio mode, channel, frequency, power and scalar quality; no SSID/BSSID or key/configuration subtrees |
| service_logd_status / services | service.list | No client arguments; name=log, verbose=false | Only log.instances.logd running/pid/exit_code leaves |
| service_sysntpd_status / services | service.list | No client arguments; name=sysntpd, verbose=false | Only sysntpd.instances.instance1 running/pid/exit_code leaves |
| diagnostics_watchdog_status / diagnostics | system.watchdog | No client arguments; always an empty object | Only status/timeout/frequency/magicclose leaves |

## Source evidence and constraints

The original [netifd interface handler](https://lxr.openwrt.org/source/netifd/ubus.c#L1147) accepted an interface selector on the fixed network.interface object, but the v5 emulator exposed an incomplete advertised input signature. That historical implementation is no longer used by the built-in tool. V2 requires fresh matching `dump` metadata and validates/selects the returned interface rows locally. A missing match is `selection_not_observed`, not another interface or a synthesized state. No object-name interpolation, source-based compatibility override or universal ubus call is introduced.

[rpcd iwinfo](https://lxr.openwrt.org/source/rpcd/iwinfo.c#L318) supplies optional radio scalars. This requires rpcd-mod-iwinfo, libiwinfo and a supported driver. Selected pointers are /phy, /mode, /country, /channel, /frequency, /txpower, /quality, /quality_max, /signal, /noise, /bitrate and /encryption/enabled. Unsupported fields can be omitted.

[procd instance serialization](https://lxr.openwrt.org/source/procd/service/instance.c#L1872) can include command, environment and data even without verbose output. Never select parent objects. The expected instance names come from the standard [log init](https://github.com/openwrt/openwrt/blob/master/package/system/ubox/files/log.init), [sysntpd init](https://github.com/openwrt/openwrt/blob/master/package/utils/busybox/files/sysntpd) and [procd helper](https://github.com/openwrt/openwrt/blob/master/package/system/procd/files/procd.sh). Modified init scripts may not match. A missing instance is unknown, not running=false.

The [watchdog handler](https://lxr.openwrt.org/source/procd/system.c#L465) combines getters and setters. Only the fixed empty-argument call is classified as read; frequency, timeout, magicclose and stop must be rejected as client inputs. Hardware watchdog availability is device-dependent.

## Required tests

- Exact PreparedAction object, method and fixed/typed arguments; injection-shaped strings remain JSON data.
- Missing, wrong-type and unknown arguments fail before device dispatch. Setter fields are never accepted by read tools.
- Default denial and category-specific access; unrelated categories and read-only grants cannot gain execution.
- Projection removes synthetic credentials/identifiers/commands/env/data, including object/array substitutions under otherwise approved pointers.
- Missing result fields are not synthesized into healthy or stopped state.
- Real MCP dispatch with fake ports preserves authorization and secret-free audit output.

The original scalar rejection/privacy tests are retained, with their interface cases migrated to v2's fixed action and fallible typed projection. V6 also adds shared strict JSON decoding and normalized/MCP response limits. No live-router call, configuration change or publication is part of these synthetic tests.

## Explicitly deferred contracts

Client-specific iwinfo statistics need strict MAC parsing because invalid selectors can fall back to returning every client. Hostapd objects need an operator-owned or typed selector rather than arbitrary object strings. Mount/lease/client inventories remain unimplemented: use the bounded typed collection profile when it fits, with reviewed operation/effect/prerequisite contracts and evidence. Requirements outside that profile must first extend the architecture and harness; no first-element or raw-output shortcut is allowed.
