# Paged APK observations

`packages_apk_installed` requires `packages.read`, never write or execute. It discloses exact package names, versions, architectures and APK layer numbers. Deny the category or this exact operation to withhold that metadata. The opt-in `config/observability.toml` includes this read.

Call with `{}` to capture, then pass the returned `next_cursor` as `{"cursor":"..."}` until it is absent. Each page contains at most 16 records. Use the cursor verbatim: it is local to this server process, operation, connection generation and snapshot, not a credential. Authorization and audit apply again on every page. There is no client-selected page size, package filter or executable.

A capture replaces the previous observation before attempting work. Repeating a capture is therefore not advertised as idempotent. Repeating a valid continuation gives the same page without another query. A failed/cancelled refresh, invalid admitted continuation, expired snapshot or changed connection invalidates its retained state. Do not automatically recapture after a continuation error; start again explicitly if a fresh observation is wanted.

Every result states `scope=apk_query_installed_visible`, `consistency=non_atomic_observation` and `whole_device_complete=false`. `captured_count` counts the one validated query result, not the current device. APK may omit unreadable non-root layers; concurrent writers and other managers are not isolated. This is not a configuration/package transaction baseline, a vulnerability scan or proof that all installed packages are healthy. Unknown APK versions fail closed rather than falling back to opkg or a raw database read.

The initial profile admits only an exact APK 3.0.5 version banner plus a successful fixed query and complete strict response validation. `operation_capability` reports `unknown/capture_required` with scope `closed_query_response` without running the inventory query. A successful signature from an unrelated ubus method cannot authorize this workflow. Source and architectural rationale are in [ADR 0007](adr/0007-paged-package-observations.md).

## Bounds

The existing backend-output setting also limits each source command: 64 KiB by default, hard package ceiling 4 MiB. An oversized capture is rejected, not truncated. Operators may raise `limits.max_output_bytes` within its existing allowed range; this cannot increase the package ceiling or normalized result cap. At most 4,096 records and 2 MiB of retained field bytes are admitted. Name/version are limited to 256 UTF-8 bytes, architecture to 64; fields must be nonempty without control characters. Duplicate `(layer,name)` identities, unknown fields, invalid types, unknown layers and malformed late records fail before any page.

Snapshots expire 120 seconds after capture started, not after the latest page. Only one capture/page holds the private slot at once; contention fails Busy. Whole normalized JSON remains limited to 64 KiB and the serialized MCP tool result including both copies to 256 KiB. Memory for decoding, record allocation and indexing is additional to the retained-field budget. No snapshot files, background refresh or polling are created.

Both SSH and verified OpenWrt-local adapters implement the same fixed recipe. Windows/Linux common paths have host-fixture evidence. The [isolated OpenWrt acceptance](emulator-validation-v7.md) is Linux-host SSH evidence, not Windows-to-router, on-device binary or BPI-R4 hardware acceptance. A separate [opkg root status-file observation](opkg-observations.md) now shares this private snapshot slot: starting either capture invalidates the other's cursor, and cross-profile continuations are rejected. Package installation/removal/upgrade and automatic manager discovery remain unimplemented.
