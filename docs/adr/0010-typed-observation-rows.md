# ADR 0010: Typed observation rows, finite sentinel values and root guards

Status: accepted architecture checkpoint; full validation and commit must precede behavior.

## Why v6's unique resource collections are insufficient

LuCI's DHCP lease reader visits configured files, which can overlap or contain repeated entries. IP address, MAC, DUID and even their tuple do not prove a unique row or connected device. Inventing identity, dropping duplicate rows or omitting expiry would misrepresent the observation. Preserve source order and every returned row instead, without exposing raw records. LuCI emits expiry as integer seconds or boolean false; false must not become numeric zero. Also admit fixed absence guards so an explicit upstream error cannot be hidden by a simultaneously present successful-looking result.

## Finite portable response extension

Keep core as the sole projection owner and the same private PreparedInvocation, dispatcher, strict decoder and MCP result boundaries. No I/O, new dependency, backend, command, probe, permission or mutation is introduced. The v10 `typed_collections_v2` contract adds exactly:

- `Collection::RowArray { source, max_items, record }`, generic over the existing finite record levels. It preserves order/duplicates, has **no identity or ExactOne selection**, invents no row ID, and validates every record. Resource ObjectArray/ObjectEntries retain their unique identity rules. All forms charge the same scan/emission/byte budgets, even when a parent is unselected. No third collection level, arbitrary JSON node, silent truncation or pagination is added.
- `ScalarKind::FalseOrSafeInteger { min, max }`. Definition bounds satisfy the existing safe signed-integer range and min <= max. Only JSON boolean false or an in-range JSON integer is accepted; true, null, floating numbers, strings and containers fail. Preserve false/integer representation exactly; source-specific meaning belongs to the feature contract. No general union, coercion or unbounded recursive schema.
- Root `reject_if_present` pointers on TypedProjection Record/Collection, default empty for old serialized definitions. At most four unique, decoded non-overlapping, valid non-root JSON pointers under existing depth/byte bounds. Validate operator metadata before I/O. Before projecting/allocating output, reject if any pointer exists, **regardless of its value**, including null/false/empty. Missing paths pass; malformed traversal fails. Return only the fixed invalid_output error, never the matching value. This is fixed metadata, not client-supplied query/expression logic.

All existing ceilings remain: 2 collection levels, 8 nodes, 256 scanned/emitted items, 128 per intended lease list, 64 fields, 1024-byte maximum text, bounded pointers, 64-KiB normalized and 256-KiB whole MCP results. A collection's narrower limits cannot raise aggregate ceilings. Missing/empty/malformed stay distinct. Older architecture versions reject the new contract fields/profile/forms. The architecture-only checkpoint changes declarations/harness, not the production projector; existing mandatory native suites are subsequently extended with actual behavior.

## First consumers

`dhcp_v4_leases` and `dhcp_v6_leases` use fixed `luci-rpc.getDHCPLeases {family:4}` / `{family:6}`. Both have no client arguments, require DhcpDns.Read alone and use the existing reviewed luci-rpc description probe. Project the corresponding required lease array as RowArray. Require expiry (false or integer 0..4294967295) and the reported primary address text. Preserve optional interface, hostname, macaddr, duid and iaid; IPv6 requires the nested ip6addrs string list (up to 10), preserving duplicates too. Strings are bounded fields, not claims of validated network identity, successful assignment or reachability. Client addresses/MACs/DUIDs/hostnames are explicitly disclosed by this category grant, never by audit.

These are non-atomic, LuCI-visible lease-file observations, not DNS answers, active-client detection, all configured reservations or complete DHCP coverage. Upstream may skip unreadable files/malformed lines, derive MACs from DUIDs, include expired entries and use a wall clock. Preserve source values and omissions; do not repair, synthesize activity or deduplicate. A false expiry is the producer's no-expiry sentinel, not proof of a permanent valid assignment; zero differs. The shared row budget can reject an IPv6 result below 128 outer rows because nested addresses count too. No file contents/configuration mutation, lease deletion, DNS flush, active lookup or package installation.

Both lease tools reject root `/error`. Strengthen storage_mounts and storage_block_devices with that guard and bump their response contracts to v2, preserving successful result shapes. This intentional stricter acceptance prevents mixed result/error payloads from appearing successful; existing v1 evidence remains historical. No generic heuristic rejects a field called error elsewhere: guards are exact reviewed metadata only.

## Harness and behavioral acceptance

Require exact profile, RowArray form, order/duplicate/no-selection policy, finite false/integer rule, pre-projection absence guards and four-guard ceiling. Negative declaration fixtures and assembled Cargo/Git checks must reject weakening/version downgrades. Keep architecture-only checkpoint before implementation.

Then extend mandatory core tests for new scalar boundaries, row duplicates/order, nesting/global budgets, rejection of selection and third levels, absent/present/malformed guards, escaped-pointer duplicate/overlap/path bounds and zero raw leakage. Feature fixtures must exactly cover all actual typed catalog/response IDs, permissions, fixed family arguments, duplicates, false/zero/numeric expiry, missing optional metadata, nested limits and private siblings. Runtime/MCP tests must prove preparation before I/O, failed projection before success audit, same-target capability admission and identical bounded text/structured outputs. Verify Windows/Linux native hosts available locally; do not relabel WSL or invent macOS/device evidence. Actual router changes remain unauthorized.

## Primary source

Reviewed [LuCI OpenWrt 25.12 lease reader and serialization](https://github.com/openwrt/luci/blob/openwrt-25.12/libs/rpcd-mod-luci/src/luci.c): configured read-only lease files, fixed fallback paths, 512-byte line buffer, up to ten IPv6 addresses, false/integer expiry and `family: Integer` metadata. Branch source review is not evidence of exact installed package behavior. Optional module absence and incompatible returned shapes remain explicit errors; no fallback bypass.
