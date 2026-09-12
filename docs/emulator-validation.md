# Architecture-v5 emulated acceptance

Historical v5 executable evidence. The current v6 run, including typed interface dump/selection, is recorded separately in [architecture-v6 emulated acceptance](emulator-validation-v6.md).

Run: **2026-09-12T20:02:13.755357Z to 2026-09-12T20:03:11.817704Z**. Result: passed with three explicitly unavailable calls, not ten successful management operations. See [environment/provenance](emulator-environment.md).

The actual Linux debug executable communicated over MCP stdio through the Rust SSH backend to official OpenWrt 25.12.5 running in isolated ARM64 QEMU. This exercised real target commands rather than a fake SSH responder. The executable was built from the uncommitted v5 implementation following architecture checkpoints `943dcf2` and `2bf6b48`; SHA256:

`ab9a160986ab566cef234ece8434c07cb7374c060751e7be33ea382fe154c287`

This is not a release-binary benchmark, native Windows/macOS acceptance, BPI-R4 hardware acceptance, or complete OpenWrt feature coverage. No live router setting was changed.

## Operation results

All calls used an operator-owned read-only configuration without execution or extension grants. The client listed eleven tools: ten reads plus capability metadata. Successful outputs were checked against exact approved key/type sets without retaining raw network payloads.

| Operation | Input-signature observation | Actual call | Projected fields |
| --- | --- | --- | ---: |
| system_board | compatible | success | 9 |
| system_info | compatible | success | 14 |
| network_device_status | compatible | success | 16 |
| network_lan_status | compatible | success | 10 |
| network_wan_status | backend_failed during probe | backend_failed; no action submitted | 0 |
| network_interface_status | unknown / incomplete_signature | capability_unknown; no action submitted | 0 |
| wireless_radio_info | compatible | backend_failed; no physical radio | 0 |
| service_logd_status | compatible | success | 2 |
| service_sysntpd_status | compatible | success | 2 |
| diagnostics_watchdog_status | compatible | success with fixed empty arguments | 4 |

A successful signature observation proves neither an instance nor an output schema. Missing fields are not invented. The emulator's absent WAN caused a nonzero ubus describe result; that error does not prove absence on other targets or distinguish absence from every authorization failure. The iwinfo API was present although no physical radio was available.

The historical global interface-status limitation is deliberate fail-closed behavior: netifd advertises no `interface` argument in this method's introspection despite a handler that accepts it. Version labels or earlier manual calls cannot override a missing observation. V6 subsequently implemented a separately reviewed bounded dump-and-select response contract; that later evidence does not reclassify this v5 result.

## Negative and lifecycle assertions

- Extra system-info arguments and a watchdog setter were rejected with `unknown_argument`.
- Extra capability metadata fields were rejected with `invalid_arguments`; an uninstalled name returned `unknown_operation`.
- After missing-capability errors, the same MCP client performed a forced system observation refresh and another successful read. No retry of a submitted action was added.
- Forty-eight audit events were checked: 24 capability events and 24 invocation events. Exact safe key sets and exclusion of prohibited material passed; neither raw arguments nor target responses were audit payloads.
- The driver stopped the disposable VM and confirmed that its loopback SSH listener was closed.

These tests validate the listed responses and failure paths on this one image. Separate workspace regressions cover policy-denied zero I/O, failed audit zero I/O, stale/revoked observations, parser malformation, bounded transport, cancellation and shared deadlines. Results are scoped in [the compatibility evidence manifest](../compatibility/evidence.toml), not used as runtime authorization or permanent capability assertions.
