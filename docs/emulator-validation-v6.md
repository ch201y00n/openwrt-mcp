# Architecture-v6 emulated acceptance

Run: **2026-09-12T20:59:54.082158Z to 2026-09-12T21:00:58.462054Z**. Result: twelve successful reads, including five typed contracts, and two explicitly unavailable legacy calls. This is actual MCP stdio -> native Rust SSH -> official OpenWrt userspace in isolated ARM64 QEMU, not a synthetic SSH responder. See [image provenance and environment](emulator-environment.md).

The debug executable was built from the v6 working tree after architecture-only checkpoint `eccd3e7`, including the monotonic deadline fix. The driver copied the executable into its private lab before running it, avoiding concurrent build replacement; executed SHA256:

`40f9e15d47ed2b7ac8d4a42d4b68fe47d0f3b0c68b77e881cf8d165879a66e2d`

The image SHA256 was `f510b0c73c1ee70a64df384d7e2ad4404caf83e6bc7cce9ac13426f77b9ae3be`. Read-only metadata reconfirmed OpenWrt 25.12.5, r33051-f5dae5ece4, kernel 6.12.94, armsr/armv8, board/model linux,dummy-virt, apk 3.0.5 and the component revisions in the environment record. In particular, this rpcd/netifd package combination is not identical to the BPI-R4 installation.

## Results

An operator-owned read-only configuration exposed fourteen management reads plus capability metadata, with no execute or extension grant. Results were checked against approved keys/types without persisting raw action responses. Generic Services.Read intentionally includes service/instance names across functional categories, not configuration or lifecycle control.

| Typed operation | Signature | Result | Aggregate projected rows | Scalar fields |
| --- | --- | --- | ---: | ---: |
| network_interfaces | compatible | success | 2 | 20 |
| network_interface_status.v2 | compatible | exact selected interface | 1 | 10 |
| wireless_devices | compatible | valid empty list | 0 | 0 |
| service_status | compatible | exact selected service | 2 | 4 |
| service_status_list | compatible | bounded nested list | 28 | 50 |

Service row counts aggregate service records **and** nested instance records; 28 does not mean 28 services. An empty iwinfo device list is an observed response, not proof about physical radios on another device.

The interface v2 operation successfully uses the separately reviewed fixed `network.interface.dump {}` action and a local exact selector. It does not bypass the incomplete old `status` signature or retry a different action. This new evidence does not change the [historical v5 result](emulator-validation.md).

| Legacy operation | Result | Projected fields |
| --- | --- | ---: |
| system_board | success | 9 |
| system_info | success | 14 |
| network_device_status | success | 16 |
| network_lan_status | success | 10 |
| network_wan_status | backend_failed during observation; no action submitted | 0 |
| wireless_radio_info | matching API signature, backend_failed for unavailable radio | 0 |
| service_logd_status | success | 2 |
| service_sysntpd_status | success | 2 |
| diagnostics_watchdog_status | success with fixed empty arguments | 4 |

Missing-instance and authorization failures are not universally distinguishable from these errors. A matching advertised input signature does not establish hardware availability or response-schema compatibility; the response is validated separately.

## Negative cases and lifecycle

- Twelve invalid-input cases were rejected before a start event: missing/empty/wrong-type/overlong/extra interface selector, empty service name, attempted verbose override, extra list inputs, watchdog setter and extra capability metadata. Each rejection was audited.
- Two valid exact selectors with no returned match produced `selection_not_observed` and failed completion audits, not empty success or another resource.
- The same MCP client successfully reused observation state after a selection error, then forced a refresh and successfully invoked a typed read. No submitted action was automatically replayed.
- All **88 audit events** (39 capability, 49 invocation) passed exact safe-key checks and checks for prohibited material. Arguments, raw responses, SSH identities and age keys were not audit payloads.
- The owned VM process group and loopback SSH listener were stopped; a separate process check found no remaining QEMU process.

Host: Linux-on-WSL2 Ubuntu; target: QEMU ARM64 virt/cortex-a53 TCG, 256 MiB. No actual BPI-R4 setting was changed, no real key or Vault was accessed, and no code was published. This is scoped emulated acceptance, not native Windows/macOS, BPI-R4 hardware, package/firmware mutation, backup/restore or complete OpenWrt coverage. The [evidence manifest](../compatibility/evidence.toml) records successful and negative operation scopes separately and never supplies runtime authorization.
