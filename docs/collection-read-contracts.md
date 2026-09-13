# First typed collection read contracts

Status: implemented after the validated architecture-only ADR 0006 checkpoint, with synthetic behavioral tests and [v6 actual MCP/SSH emulator acceptance](emulator-validation-v6.md) for the first five typed contracts documented here. The later [three passive wireless contracts](wireless-observation-contracts.md) bring the catalog to seventeen reads/eight typed contracts; the earlier emulator run does not validate those additions. [Native Windows GNU host fixtures](windows-validation.md) also pass; MSVC, macOS and physical-device acceptance remain pending. Historical v5 evidence is scoped to the v5 executable, not these new or changed response contracts. All source responses are untrusted and may include sensitive fields that must not reach the result or audit.

| Operation | Fixed action / input | Permission | Response contract |
| --- | --- | --- | --- |
| network_interfaces | network.interface.dump {}; no input | Network.Read | network_interfaces.v1 |
| network_interface_status | network.interface.dump {}; required local selector `interface` | Network.Read | network_interface_status.v2 |
| wireless_devices | iwinfo.devices {}; no input | Wireless.Read | wireless_devices.v1 |
| service_status | service.list with required `name` and fixed verbose=false | Services.Read | service_status.v1 |
| service_status_list | service.list with fixed verbose=false; no input | Services.Read | service_status_list.v1 |

No execute permission is required or implied. Fresh matching method metadata is still required. Discovery does not prove the response contract, selected instance or device health. No fallback to old interface.status or a different target occurs.

Every returned resource identity and ExactOne selector is nonempty and NUL-free, with matching UTF-8 byte limits. Presence alone does not make an empty string a valid identity.

## Network records

At the reviewed [netifd revision](https://github.com/openwrt/netifd/blob/cbb83a1857407a28a63dc09412a1f209195914ef/ubus.c#L956), dump returns an `interface` array containing the logical name and status fields. Lists return `{items:[record,...]}`. Single status selects the unique matching name and returns that record directly.

Approved fields: required `interface` text and `up`, `pending`, `available`, `autostart`, `dynamic` booleans; optional `uptime` nonnegative SafeInteger and `proto`, `device`, `l3_device` text. Text limits are 256 UTF-8 bytes; at most 128 source interface records, all identities unique. Addresses, DNS, routes, data, jail and tags are not returned. The selector is required and equally bounded; it is not sent to ubus, so the observed input contract for dump is empty.

The v2 single-status result deliberately uses ordinary field names and includes `interface`, unlike v1's slash-prefixed scalar keys. This is an explicit development-stage response-contract change, not an invisible fallback. A missing match is `selection_not_observed`; malformed or duplicate records are errors, never an arbitrary first choice.

## Wireless device records

The [reference rpcd iwinfo implementation](https://github.com/openwrt/rpcd/blob/e37ed9d814699098eb7e26c8b33c054840782dfb/iwinfo.c#L887) returns a devices array. Normalize it to `{items:[{device:...},...]}` with required text at most 256 bytes, no duplicates and at most 128 items. These are network interfaces supported by iwinfo, not UCI radio or PHY names. A valid empty list means no entries were returned; it does not prove the absence of physical radios. It grants no scan, association lookup or configuration access.

## Service and instance records

The [service-list implementation](https://github.com/openwrt/procd/blob/58eb263d5abe03f8c1280bdfa65a3b052614215d/service/service.c#L483) returns a map keyed by service name. Each record may contain an instances map; [instance serialization](https://github.com/openwrt/procd/blob/58eb263d5abe03f8c1280bdfa65a3b052614215d/service/instance.c#L1668) includes fields beyond those approved here, even without verbose output.

Normalize the list to `{items:[service_record,...]}`. Each service has required `name` text and optional `instances` array. Each instance has required `name` text and `running` boolean, optional positive SafeInteger `pid`, and optional nonnegative SafeInteger `exit_code`. Names are bounded to 256 bytes. Exclude command/env/data/errors/respawn/bundle and paths. Missing instances remain omitted, never synthesized as empty or stopped. A returned running flag is not application-specific health validation.

The root service collection has at most 128 entries; each instances collection has at most 128 entries, with the independent 256-item whole-invocation scan/emission cap shared across services and instances. These are product limits, not OpenWrt maximums. A later need for larger snapshots/pagination requires an explicit snapshot contract, not silent truncation. Single service status binds the requested name both to the fixed action argument and exact returned-map identity, returning only a unique matching record. A server returning a different service cannot broaden the selection.

Services.Read intentionally exposes generic daemon names and running/PID/exit metadata, including daemons serving other categories. It does not authorize their settings, command lines, environment or lifecycle. For narrower visibility, deny the generic service tools and allow only the fixed logd/sysntpd operations; do not infer categories from names.

## Required tests

The behavioral suite in `crates/features/tests/collection_contracts.rs` derives the set of typed operation/response-contract IDs from the actual catalog and requires an exactly matching exercised fixture set. It checks fixed action/prerequisite metadata, deny-by-default/category isolation, selector bounds/schema, selected-field/type allowlists, secret-bearing siblings, missing/empty/malformed results, identity duplication, exact/excessive row/string/byte limits and shared nested budgets. The network contract is exercised at exactly 65,536 normalized JSON bytes and with an escaped-character overflow. Prior scalar privacy/rejection tests are retained and interface cases migrated to v2.

Core, runtime, codec and MCP suites separately validate finite metadata, prepared selector binding, pre-I/O validation and audit, strict JSON decoding, no replay and complete serialized tool-result limits. List and single-result shapes must match this record. Keep synthetic fixtures distinct from actual userspace or hardware acceptance; response-contract versions do not create positive runtime capability observations. Current full-gate results belong in [validation.md](validation.md), not in historical v5 evidence.
