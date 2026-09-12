# Post-v2 read-adapter acceptance contracts

Designed with architecture v2, before feature implementation. Architecture-only checkpoint 8faf423 passed the complete repository gate before these reads were implemented and fixture-tested. They fit the existing pure feature definitions, PreparedAction port and scalar projection without new dependencies or I/O boundaries. Real OpenWrt release/device acceptance remains pending.

All examples are synthetic. Source review is against upstream on 2026-09-13, not release/device certification. Absent output fields stay absent; empty results must not be interpreted as proof that a service is stopped or a capability is supported. Object/array values at scalar pointers are discarded.

| Tool / category | Fixed object.method | Input contract | Safe response contract |
| --- | --- | --- | --- |
| network_interface_status / network | network.interface.status | One required string: interface | Selected up/pending/available/autostart/dynamic/uptime/proto/device/l3_device; no addresses, routes, DNS or data subtrees |
| wireless_radio_info / wireless | iwinfo.info | One required string: device | Radio mode, channel, frequency, power and scalar quality; no SSID/BSSID or key/configuration subtrees |
| service_logd_status / services | service.list | No client arguments; name=log, verbose=false | Only log.instances.logd running/pid/exit_code leaves |
| service_sysntpd_status / services | service.list | No client arguments; name=sysntpd, verbose=false | Only sysntpd.instances.instance1 running/pid/exit_code leaves |
| diagnostics_watchdog_status / diagnostics | system.watchdog | No client arguments; always an empty object | Only status/timeout/frequency/magicclose leaves |

## Source evidence and constraints

The [netifd interface handler](https://lxr.openwrt.org/source/netifd/ubus.c#L1147) accepts an interface selector on the fixed network.interface object. No object-name interpolation or universal ubus call is needed. Netifd and a matching logical interface must exist.

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

Existing adapter timeout/output bounds apply unchanged. No live-router call, configuration change or publication is part of these tests.

## Explicitly deferred contracts

Client-specific iwinfo statistics need strict MAC parsing because invalid selectors can fall back to returning every client. Hostapd objects need an operator-owned or typed selector rather than arbitrary object strings. Mount/lease/client inventories need bounded typed collection projection instead of selecting the first array element. These require a documented domain-contract extension and harness regression before implementation; no raw-output shortcut is allowed.
