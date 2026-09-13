# ADR 0011: Closed UCI configuration observations and exact text enums

Status: accepted architecture declaration; pass and commit the architecture-only gate before implementation. This checkpoint is not UCI mutation support.

## Motivation and separation

Effective netifd/LuCI observations do not expose the operator's selected configuration. Add narrowly reviewed non-secret configuration reads while distinguishing rpcd's view from committed files and effective service state. Introspecting uci must not accidentally enable raw UCI extensions, setters, arbitrary configuration paths, sessions or unbounded configuration output.

The [reference rpcd source](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/uci.c#L600) implements get by loading a package and projecting sections after optional type filtering. Without ubus_rpc_session it reads the shared /tmp/.uci delta view and does not perform rpcd session ACL checks. The get path changes the daemon's in-memory cursor selection but does not commit/revert/apply configuration. Section metadata includes anonymous/type/name/index; options are strings or string lists. Therefore these reads are **sessionless shared-delta, non-atomic observations**, not committed-only snapshots, another LuCI session's staging, validation of option semantics or evidence that settings have taken effect.

MCP policy, configured SSH authority and the existing dispatcher remain the authorization boundary for this path. Do not manufacture a session, install rpcd, fall back to CLI files, guess pending-state isolation or claim hardware acceptance from source review.

## Closed read recipes

Core owns a pure `UciReadProfile` and fixed config/type/category mappings in `core::uci`; features owns the named tools and exact non-secret field definitions. There is no new process, binary stream, transaction use case or runtime bypass. Existing Ubus actions/capability metadata and the same strict decoder are reused only after recipe validation.

| Profile | Fixed uci.get arguments | Required category |
| --- | --- | --- |
| system | config=system, type=system | System.Read |
| network_interfaces | config=network, type=interface | Network.Read |
| wireless_radios | config=wireless, type=wifi-device | Wireless.Read |
| firewall_defaults | config=firewall, type=defaults | Firewall.Read |
| dnsmasq | config=dhcp, type=dnsmasq | DhcpDns.Read |
| mounts | config=fstab, type=mount | Storage.Read |

All six recipes take no MCP parameters. They have exactly the two fixed string arguments, no section/option/match/session/temporary path, and only get. No config inventory, arbitrary package, get-all-types, state, changes, set/add/delete/rename/order/commit/revert/apply/confirm/rollback or authentication method is admitted. The recipe's required read category is enforced before I/O, independently of annotations. Generic Process remains unverified and blocked.

Catalog construction rejects **every custom Ubus operation whose object is uci**, even with Extensions.Write/Execute. Built-in Ubus uci definitions must match one of the six recipes, have no parameters, contain the profile's read requirement and a typed root collection over /values. Require All selection, bounded unique section-map keys, no nested option collections initially, root /error guard and a required /.type field constrained to the profile's exact section type. Defensive Operation preparation validates the same recipe/projection requirements. Fixed non-secret option declarations belong to features and their exact catalog fixtures, not an alternative raw output contract in core. External callers constructing trusted built-in catalogs remain composition authorities, not MCP clients.

Initially expose bounded scalar option text only. Missing options stay absent; text such as '0'/'1' remains text, not effective booleans/numbers. A list where a scalar is declared fails, never joins/coerces or returns a raw subtree. Do not expose wireless BSS/SSID/keys, credentials, arbitrary mount options (which can contain passwords), custom service commands, VPN keys or raw network settings. Optional string/list configuration fields, ordered section workflows and all writes require separately reviewed evolution. Generated anonymous section names and indices are observation identities, not durable mutation handles.

## Probe and response evolution

The v11 capability profile `base_luci_uci_v3` adds exactly one description: `/bin/ubus -v list uci`. Registry schema 3 is exactly the prior nine objects plus uci; earlier schemas remain valid checkpoint subsets but cannot contain the new object. Neither a description nor a discovered setter authorizes invocation. Enforce the new custom/recipe rejection before making uci discoverable to production. Existing cache entries remain bounded by the closed object enum, with unchanged 30-second freshness, same backend/epoch, audit order and deadline.

The portable projection profile `typed_collections_v3` adds only `ScalarKind::TextEnum { max_bytes, values }`. Metadata must contain 1..16 distinct nonempty NUL-free UTF-8 strings, each within max_bytes (1..1024). Comparison is exact and case-sensitive. Return the received matching string; reject any other string, type or null. No trimming, pattern, wildcard, normalization, default, arbitrary union or client-selected allowed values. Definition limits are validated before I/O and byte accounting uses the existing borrowed scalar path. UCI /.type uses a one-value enum; it cannot silently accept another section type even when a target ignores filtering.

All previous scalar/row/root-guard semantics and collection/node/item/field/byte/depth ceilings stay unchanged. No production dependency is added. Older contracts reject the new UCI table, enum metadata/profile and schema-3 probe expansion.

## Layout and harness

Keep the existing twelve owners. Add pure core::uci under crates/core/src, category definitions under their existing features modules and a shared definition helper only for assembly. Runtime, device-codec and MCP continue their existing interfaces; the harness gets a separate uci_reads declaration validator, not production code. The versioned uci_read_contract fixes owners, six recipes, custom denial, fixed arguments, typed/guarded/type-constrained projection, source semantics and required suites.

Require native core capability and MCP read-contract suites in addition to already mandatory projection/feature/runtime suites. Architecture tests reject missing/unknown/weakened declarations, additional profiles, setters, raw custom UCI admission, missing guards, absent category requirements, false committed/effective-state claims, probe calls/wildcards, enum coercion/unbounded values and version downgrades. The architecture-only checkpoint retains current production behavior/registry; migration follows its successful commit.

Implementation tests must cover every profile/action/category, direct/custom/privileged bypass attempts before backend use, wrong section types, optional/malformed/private fields, scalar/list mismatch, anonymous metadata, exact text enum limits, row/byte bounds, prepared action binding, same-target capability checks, failed projection/audit and identical bounded MCP copies. Actual reference configuration is never used as a fixture. Emulated userspace and Windows/Linux/macOS native acceptance remain separately reported.

This is partial configuration observability. It does not resolve UCI transaction isolation, encrypted pre-change backup publication, device-owned recovery, protected-resource effects, privileged maintenance or the remaining full-management goal.
