# Scoped DHCP lease observations

Implemented after architecture-only checkpoint `4040ce3`, whose full native Windows GNU and Linux-on-WSL gates passed before behavior. These are finite LuCI-visible lease-file observations, not complete DHCP/DNS management or BPI-R4 acceptance.

| Tool / response contract | Fixed target action | Required fields |
| --- | --- | --- |
| dhcp_v4_leases / dhcp_v4_leases.v1 | luci-rpc.getDHCPLeases {family:4} | ipaddr, expires_seconds |
| dhcp_v6_leases / dhcp_v6_leases.v1 | luci-rpc.getDHCPLeases {family:6} | ip6addr, expires_seconds, ip6addrs |

Both tools take no arguments and require DhcpDns.Read, never execute. Missing categories remain denied. The operator can deny either operation individually. The existing same-target signature check requires the reviewed method and integer family parameter; no guessed command, package installation or host fallback follows from absence. Other luci-rpc methods are not authorized by this grant.

Results are `{items:[...]}`. Optional interface, hostname, macaddr, duid and iaid fields are omitted when absent, not synthesized. IPv6 ip6addrs is a list of `{address_prefix:...}` records. These tools intentionally expose client addresses, names and identifiers under the explicit category grant. Audit events contain none of these values or raw errors. Treat returned text as untrusted data, never commands or trusted local paths.

## Source meaning

The reviewed [LuCI 25.12 source](https://github.com/openwrt/luci/blob/openwrt-25.12/libs/rpcd-mod-luci/src/luci.c) reads configured dnsmasq/odhcpd lease files, with default-path fallback. It can skip unreadable files and malformed lines, repeat configured files, derive a MAC from a DUID and report expired rows. Observation is non-atomic and time-dependent. A successful empty list is not proof that no leases, reservations or clients exist. The source review is not a claim about every installed version or variant.

`expires_seconds` preserves exactly JSON false or an integer from 0 through 4,294,967,295. False is the source's non-expiring/unspecified sentinel, not zero; zero can describe an expired lease. There is no coercion from strings, floats, null or true. A positive remaining lifetime is not proof that the client is connected. Addresses/prefixes and other strings are bounded reported text, not validated identities or evidence of current routing/DNS reachability.

Rows retain order and duplicates. IP, MAC, DUID and composite values are not assumed unique, so these tools have no exact-row selector and never silently merge observations. Any present root `error`, even null or false alongside a plausible success payload, rejects the entire result with a fixed `invalid_output` code and no raw error text.

## Bounds and validation

Each family allows at most 128 rows. IPv6 allows at most ten address-prefix entries per row. Outer and nested entries share the existing 256-item scan/emission ceiling; thus 128 rows with one prefix each fit, but additional prefixes beyond the shared budget do not. Missing required arrays, malformed unselected data and excessive bounds reject rather than truncate. Empty valid arrays remain empty.

UTF-8 byte ceilings are 15 for ipaddr, 45 for ip6addr, 49 for address_prefix, 256 for interface, 512 for hostname/duid, 17 for macaddr and 64 for iaid. NUL is rejected. The independent 64-KiB normalized and 256-KiB whole MCP result limits still apply, including JSON escaping and both protocol copies. A collection satisfying row counts can still exceed the byte limit.

Architecture v10 adds only finite ordered rows, the false/integer scalar union and up to four declared root-absence guards in the existing pure projection owner. Identity-bearing collections keep their duplicate rejection and selectors. No generic union, arbitrary predicate, raw output, deeper collection nesting, new I/O owner or increased resource ceiling was introduced. See [ADR 0010](adr/0010-typed-observation-rows.md).

Core, catalog, dispatcher and real MCP fixture tests cover sentinels, duplicate preservation, exact/excessive scalar/row/nested/byte bounds, malformed errors, metadata rejection before I/O, category isolation, fixed family arguments, cache reuse and payload-free audit. Host evidence belongs in [validation](validation.md). No emulator, physical lease-file, DNS, RA configuration, reservation, packet capture or mutation acceptance is claimed by these fixtures.
