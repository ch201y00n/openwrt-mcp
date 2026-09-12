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

The official 25.12.5 ARM64 emulator advertises `{}` for global `network.interface.status` even though a controlled call with an interface selector succeeds. At the reviewed revisions, netifd modifies the object's dispatch methods while libubus publishes the original object's type methods. Missing metadata is therefore **unknown**, not proof of unsupported behavior. This tool remains blocked under that incomplete signature until a verifiable typed replacement is implemented; source or test success does not bypass the gate. [netifd implementation](https://github.com/openwrt/netifd/blob/cbb83a18/ubus.c), [ubus object registration](https://github.com/openwrt/ubus/blob/24864e78/libubus-obj.c)

The same emulator can advertise iwinfo without having a physical radio, and can have LAN without WAN. Those are distinct API, instance and hardware facts, not grounds for synthesizing a healthy or stopped status. BPI-R4's actual netifd/rpcd package revisions differ from the base emulator; matching the release does not make their acceptance evidence interchangeable.

Generic Process extensions are now explicitly unverified and blocked, even with administrator grants. Custom Ubus definitions need matching required metadata and a reviewed object. This is an intentional narrowing of early extension behavior, not an undocumented fallback. See [configuration](configuration.md) and [ADR 0005](adr/0005-capability-observations.md).
