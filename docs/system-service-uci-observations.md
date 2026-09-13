# System-service UCI observations (v20)

Contract for the four reads admitted by architecture-only checkpoint `a711593`.
Feature IDs: SYS-04, SYS-06, SYS-07, DNS-06 (read slices only). No architecture
revision, dependency, probe, output form or resource ceiling changes are needed.
All operations take `{}` and send only their fixed `uci.get` config/type pair.
Their response IDs are `<tool>.v1`; previous response IDs are unchanged.

## Exact selected fields

Numbers in parentheses are maximum UTF-8 bytes, not numerical value ranges.
Every option is optional. Scalar fields accept only strings; TextOption fields
preserve string/list kind, empty values, order and duplicates without splitting.
Missing fields remain omitted, never filled with service defaults.

| Tool / category / config:type | Scalar text fields | TextOption fields |
| --- | --- | --- |
| system_led_configuration / system / system:led | name (256), sysfs (256), trigger (64), dev (256), default (8), inverted (8), brightness (32), delayon (32), delayoff (32), interval (32), port_state (32), delay (32), gpio (32), port_mask (32), speed_mask (32) | mode (256), port (256) |
| system_dropbear_configuration / system / dropbear:dropbear | enable (8), PasswordAuth (8), RootPasswordAuth (8), RootLogin (8), GatewayPorts (8), LocalPortForward (8), RemotePortForward (8), Port (32), Interface (256), DirectInterface (256), SSHKeepAlive (32), IdleTimeout (32), MaxAuthTries (32), RecvWindowSize (32), mdns (8) | None |
| system_uhttpd_configuration / system / uhttpd:uhttpd | redirect_https (8), rfc1918_filter (8), max_requests (32), max_connections (32), script_timeout (32), network_timeout (32), http_keepalive (32), tcp_keepalive (32), no_symlinks (8), no_dirlists (8), no_ubusauth (8) | listen_http (256), listen_https (256) |
| dhcp_odhcpd_configuration / dhcp_dns / dhcp:odhcpd | maindhcp (8), loglevel (32), leasefile (1024), hostsdir (1024), hostsfile (1024), piodir (1024), piofolder (1024) | None |

The existing [UCI envelope](uci-observations.md) applies: at most 128 sections,
256-byte section identity, required exact `.type`, boolean `.anonymous`, u32
`.index`, at most 128 values per text/list option, shared 256-item accounting,
64-KiB normalized output and 256-KiB entire MCP result. Wrong selected types,
root errors, malformed later rows and budget overflows reject the whole result.
Unknown options are not returned. Bounds intentionally do not validate daemon
semantics or certify that stored settings will work when applied.

## Security and semantics

System.Read is required for LED, Dropbear and uHTTPd; DhcpDns.Read for odhcpd.
Services.Read cannot substitute for either. Deny individual operations to withhold
listen endpoints, interface names, authentication flags or storage paths.
The existing dispatcher checks authorization before probing and invocation,
uses the same target's reviewed UCI signature and records no arguments/results.
Custom UCI definitions and caller-selected config/type/section/option/session
remain forbidden, even with privileged extension grants.

Never expose `message`, trigger scripts, `ForceCommand`, `keyfile`, `rsakeyfile`,
`BannerFile`, uHTTPd `key`/`cert`/`config`/`home`/`realm`/`httpauth`, executable
CGI/Lua/ucode/interpreter/ubus handler options, or odhcpd `leasetrigger`.
The bounded raw UCI section can contain these excluded options before projection;
they are never returned or logged. No referenced key/file/lease contents are opened.
Observed paths and trigger names are text only: never opened, expanded or run.
Selected free text can itself contain target-supplied sensitive text; selection
is not a guarantee against an operator placing secrets in normally public fields.

These are sessionless shared-delta, non-atomic observations, potentially including
pending changes. They do not prove installation, daemon state, effective login
policy, live listeners, LED brightness or current leases. Empty sections do not
prove absence. Section indices are not durable mutation handles.

## Source review and variation

Field selection was reviewed against the pinned OpenWrt v25.12.5 sources below;
no upstream implementation is copied or executed by these tools.

- [LED consumer](https://github.com/openwrt/openwrt/blob/v25.12.5/package/base-files/files/etc/init.d/led): configured trigger/mode differ from sysfs state; dynamic color options and custom triggers remain gaps.
- [Dropbear validation](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/dropbear/files/dropbear.init): case-sensitive option names; observed flags do not establish effective authentication or interface binding.
- [uHTTPd defaults](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/uhttpd/files/uhttpd.config): listeners and request limits are distinct from secrets and handlers.
- [odhcpd migration](https://github.com/openwrt/openwrt/blob/v25.12.5/package/network/services/odhcpd/files/odhcpd.defaults): legacy hostsfile/piofolder and newer hostsdir/piodir are separately preserved; this read never performs migration.

A release string is not admission evidence. Missing/ACL-hidden signatures remain
unknown; incompatible signatures fail; optional fields may differ by variant.
Current installed package revisions and complete option sets require separately
scoped observations. Source/fixture evidence is not physical BPI-R4 acceptance.

## Verification contract

Independent field fixtures participate in the existing mandatory feature suite:
exact fields/limits, optional/empty/wrong types, text/list and aggregate bounds,
error guards, fixed actions/response IDs and excluded secret-containing options.
The required core suite checks all 28 recipes and rejects cross-category/custom
admission. Actual MCP tests cover all four through the dispatcher: discovery,
direct denied calls, malformed arguments/results, same-target probe, and audit
redaction. No fixture uses a real router, Vault or operator key.
