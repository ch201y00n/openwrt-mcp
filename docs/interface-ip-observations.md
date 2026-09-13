# Scoped interface IP observations

Contract recorded before implementation, within architecture v10's existing finite response shapes, owners and limits. This increment requires no new probe, backend, dependency, projection form or permission mechanism. It is not complete network/DNS management or hardware acceptance.

All four tools perform fixed `network.interface.dump {}` and select one exact `interface` locally. The parameter is required, nonempty and at most 256 UTF-8 bytes; it never becomes a device argument. Fresh matching same-target dump metadata is required. No global status fallback, shell, active query, neighbor solicitation, route update or DNS lookup is performed.

| Tool / response contract | Required grant | Declared optional collections |
| --- | --- | --- |
| network_interface_addresses / network_interface_addresses.v1 | Network.Read | ipv4_addresses, ipv6_addresses, inactive_ipv4_addresses, inactive_ipv6_addresses |
| network_interface_routes / network_interface_routes.v1 | Network.Read | routes, inactive_routes |
| network_interface_neighbors / network_interface_neighbors.v1 | Network.Read | neighbors, inactive_neighbors |
| dhcp_interface_dns / dhcp_interface_dns.v1 | DhcpDns.Read | servers, search_domains, inactive_servers, inactive_search_domains |

Each selected record always contains interface and boolean up. Each collection is optional: absent stays omitted, empty stays empty, and present malformed values reject. Netifd normally emits these collections only when up. Omission is not an empty/healthy/comprehensive observation, even if up is true. Up is netifd's state flag, not an independent connectivity or service-health check. Error detail, arbitrary protocol data, credentials and configuration subtrees are excluded. A present root error rejects the complete response.

## Exact row fields

- Address rows: required address and mask; optional ptpaddress, preferred_seconds, valid_seconds and class. IPv4/IPv6 address and point-to-point text cap at 15/45 bytes; mask is an exact integer 0..32/0..128. Class text caps at 256. Lifetimes are exact unsigned 32-bit JSON integers, not recalculated or defaulted.
- Route rows: required target, mask, nexthop and source; optional type, proto, mtu, metric, table and valid_seconds. Target/next-hop text cap at 45 bytes, source address/prefix at 49. Mixed-family mask is bounded 0..128 without inferring a family from arbitrary text. Other numbers are unsigned 32-bit JSON integers. No kernel FIB, policy-rule or selected-route completeness claim.
- Neighbor rows: required address (45 bytes); optional mac (17 bytes), proxy and router (unsigned 32-bit JSON integers, not booleans). These are netifd-managed neighbor entries, not the kernel ARP/NDP cache or connected-client discovery.
- DNS collections contain `{address:...}` (45 bytes) or `{domain:...}` (256 bytes). These are netifd's interface DNS/server-search observations, not effective dnsmasq forwarding/resolver configuration, DNS cache, query results or validation of upstream resolver reachability. DhcpDns.Read intentionally includes the selected interface identity and up flag, without granting other network tools.

All text is bounded reported text, rejects NUL, and is not independently validated as an address, DNS name, trusted path or command. There is no coercion from null/string/float to integer. Received lifetime values can reflect source timing and integer behavior; they are not proof of validity or permanence.

## Source semantics and resource limits

The reviewed [netifd reference revision](https://github.com/openwrt/netifd/blob/cbb83a1857407a28a63dc09412a1f209195914ef/ubus.c#L518) concatenates configuration and protocol lists. Active/inactive classification follows netifd's enablement, default-route and DNS flags, not independent kernel observation. Repeated records and scalar values are retained in source order; no identity/deduplication is invented. Nested row selection is not exposed.

The outer dump caps at 128 unique interface identities; each declared nested list caps at 128. All source interfaces and every declared nested entry, including unselected interfaces, consume the existing shared 256-entry scan budget and are validated. Only the selected record is emitted. A malformed/unbounded unrelated interface therefore rejects the operation; no first-match partial success. Missing selection returns selection_not_observed. Duplicate interface identities reject even if the requested interface appears only once. A 64-KiB normalized result and 256-KiB whole MCP result remain separate limits.

This protects resource usage but is not arbitrary-size inventory support. No truncation or implicit pagination is added. IPv6 delegated-prefix assignment maps, policy routes outside netifd, kernel neighbor caches and full topology remain separate reviewed contracts. These tools do not change existing narrow interface-status/list response shapes.

Network.Read now intentionally exposes interface addresses, route endpoints and configured neighbor identifiers through the three named tools; DhcpDns.Read exposes interface DNS addresses/search domains. Operators can withhold any tool through deny_operations. These payloads never belong in audit logs. Existing default-deny and independent execution rules are unchanged.

Required tests use actual catalog definitions through core and MCP paths: exact actions/response IDs, all category/access/execute combinations, local-only selector, omitted/empty/present malformed collections, unknown private siblings, source errors, duplicate rows versus duplicate identities, numeric/text limits, nested/unselected global budgets, denied calls without I/O and payload-free audit. Real target acceptance must be separately recorded; source review and synthetic fixtures are not current BPI-R4 state.

The separate [v10 emulator run](emulator-validation-v10.md) passed all four tools for loopback and lan: populated IPv4 address rows and empty route/neighbor/DNS observations. Populated variants remain covered by host fixtures, not inferred from the emulator's empty lists.
