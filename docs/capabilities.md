# Observed capabilities and version differences

The server checks the selected target before executing an authorized operation. A release number alone is not proof: installed packages, downstream patches, optional plugins, remote ACLs and hardware can differ. The initial BPI-R4 software observation is recorded separately in [reference-target.md](reference-target.md).

## Four different questions

1. Is there an implemented, reviewed operation? The catalog and [coverage matrix](coverage.md) answer this.
2. Does operator policy permit it? Category grants, independent execution permission and exact operation restrictions answer this.
3. Does the target presently advertise a matching input signature? The bounded probe and private expiring cache answer this limited question.
4. Has this workflow actually been verified on this device/platform? [Validation](validation.md) and scoped compatibility evidence answer this; discovery cannot substitute for acceptance.

`check`, `catalog` and `tools/list` stay offline and do not read credentials or contact a target. Their operation lists are not availability guarantees. No background polling or automatic grants occur.

## Metadata tool

For an operation already permitted by the same policy:

```json
{"name":"operation_capability","arguments":{"operation":"system_info"}}
```

Use `"refresh": true` to discard the previous observation and probe again. Refresh invalidates previously issued internal copies too; a failed/cancelled refresh cannot restore old success. There is no caller-supplied profile, target, permission or force override.

The result includes `compatibility`, a fixed `reason`, `scope: "input_signature_only"`, the operator-owned `response_contract` identifier and optional `remaining_ttl_ms`. It never returns raw signatures, host keys, authentication identities or device payloads. Without invocation arguments, status conservatively checks all potentially transmitted fields, including optional ones. Normal invocation checks only the fields it actually sends.

Projection-only selectors are not transmitted fields. In v6, `network_interface_status` binds the required `interface` input locally while its fixed `network.interface.dump` call sends `{}`. Its capability prerequisite therefore describes `dump` with no input fields and response contract `network_interface_status.v2`.

| Result | Meaning |
| --- | --- |
| compatible / signature_matched | Advertised input signature matches; no promise of successful response or hardware availability |
| unknown / not_observed_or_hidden | Object/method not observed; remote ACLs can hide it |
| unknown / incomplete_signature | A transmitted field is not advertised; introspection may be incomplete |
| unknown / unrecognized_type | An observed input type has no approved interpretation |
| unknown / invalid_observation | Malformed, foreign or otherwise invalid metadata |
| unknown / unreviewed_probe | No reviewed probe exists for this definition |
| unknown / probe_unavailable or stale_observation | No usable same-target fresh observation |
| incompatible / argument_type_mismatch | A known advertised type conflicts with the operation's input contract |

Transport, target selection, authentication, capacity and audit failures retain safe error codes such as `target_not_configured`, `authentication_failed`, `backend_failed`, `busy` or `audit_unavailable`; they never establish availability. In particular, nonzero introspection exit status does not prove a package is absent. Normal calls with an unknown or incompatible prerequisite return `capability_unknown` or `capability_unsupported` without invoking the target method.

Metadata requests pass through the dispatcher and audit with `kind: "capability"`. Normal calls use `kind: "invocation"`. A successful metadata-query audit means the query completed, not that its named operation executed or is supported. Denied and malformed queries are audited without accessing the target.

## Bounded, same-target observations

The initial closed set is system, network.device, network.interface, network.interface.lan, network.interface.wan, iwinfo and service. Probe commands only describe one exact object with `ubus -v list`; they never call a method as a presence test. Local and SSH modes share the same parser and command contract. Discovery and execution use the same immutable backend and authentication context.

Observations last at most 30 seconds, measured by the host's monotonic clock. A private cache has at most seven entries. Connection/authentication epoch changes or invalidation prevent reuse. Probe, cache waiting and command execution share one device-work deadline; byte limits cap each response and introspection has an additional 64-KiB ceiling. The start audit must succeed before probe/key/connection I/O. There is no automatic operation replay.

External configuration or package changes within the TTL may not be immediately detected. A privileged external writer can race any check; this is not transaction isolation. Later package/firmware workflows must invalidate their observations on managed changes. Response contracts remain separate: a known method may have no requested interface, radio or service instance.

## Known initial variant cases

The historical v5 official 25.12.5 ARM64 emulator run advertised `{}` for global `network.interface.status` even though a controlled call with an interface selector succeeded. At the reviewed revisions, netifd modifies the object's dispatch methods while libubus publishes the original object's type methods. Missing metadata is therefore **unknown**, not proof of unsupported behavior; the old v1 operation was correctly blocked. [netifd implementation](https://github.com/openwrt/netifd/blob/cbb83a18/ubus.c), [ubus object registration](https://github.com/openwrt/ubus/blob/24864e78/libubus-obj.c)

The current v2 operation always uses a separately reviewed fixed `dump {}` implementation and an exact local typed selector. It does not attempt `status`, retry after a failure, or treat source/emulator evidence as a positive runtime observation. A target without a matching fresh `dump` observation is still blocked. V2 uses ordinary output names and includes the interface identity instead of v1's slash-prefixed scalar keys. See [the collection response contracts](collection-read-contracts.md). The historical v5 run is not v2 device acceptance.

The same emulator can advertise iwinfo without having a physical radio, and can have LAN without WAN. Those are distinct API, instance and hardware facts, not grounds for synthesizing a healthy or stopped status. BPI-R4's actual netifd/rpcd package revisions differ from the base emulator; matching the release does not make their acceptance evidence interchangeable.

Generic Process extensions are now explicitly unverified and blocked, even with administrator grants. Custom Ubus definitions need matching required metadata and a reviewed object. This is an intentional narrowing of early extension behavior, not an undocumented fallback. See [configuration](configuration.md) and [ADR 0005](adr/0005-capability-observations.md).

## Response validation is separate from capability discovery

V6 implements finite typed collection responses for eight contracts, including three [passive wireless views](wireless-observation-contracts.md) added after the first five. After a compatible method executes, the prepared projection still checks required fields, exact scalar types/ranges, nonempty identities, uniqueness and bounded collection sizes, including unselected rows. A compatible signature cannot turn malformed output into success.

`wireless_station_status` transmits only `device` to `iwinfo.assoclist`; its MAC selector stays local and does not require or use rpcd's optional MAC filter. `assoclist` and `countrylist` still require a fresh observed String `device` signature. Empty arrays may reflect driver failures upstream and do not establish absence or health.

Typed shape/type failures use `invalid_output`; an exact selector with no observed match uses `selection_not_observed`; exceeded byte or item budgets use `output_limit`. None returns a partial list, chooses the first duplicate or retries the device operation. Optional fields/instances stay omitted, while a valid empty list is only an observation of returned entries, not proof of complete visibility or absent hardware.

Both target backends share strict action JSON decoding with duplicate-key rejection and byte/depth/node bounds. Normalized results are capped at 64 KiB and complete serialized MCP tool results at 256 KiB, including text and structured copies. Synthetic tests and the [v6 architecture](adr/0006-bounded-read-projections.md) define these boundaries; current emulator/native-host/physical-device acceptance is separately recorded, never inferred from a signature match.
