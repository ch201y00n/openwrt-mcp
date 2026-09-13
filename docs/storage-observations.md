# Scoped storage observations

Implemented after architecture-only checkpoint `acf6532` (full Windows GNU and Linux-on-WSL gates before behavior). These are synthetic-contract-tested LuCI observations, not exact BPI-R4 acceptance, all storage management, or backup/restore support.

| Tool / response contract | Fixed target action | Returned fields |
| --- | --- | --- |
| storage_mounts / storage_mounts.v2 | luci.getMountPoints {} | mount, device, size_bytes, available_bytes, free_bytes |
| storage_block_devices / storage_block_devices.v2 | luci.getBlockDevices {} | source_key, device, size_bytes, filesystem; optional uuid, label, version, mount |

Both have no arguments, require Storage.Read without execute, and return `{items:[...]}`. Configured descriptions remain offline; actual calls require fresh matching same-target LuCI method metadata. Missing/hidden modules, failed probes or invalid output fail closed. No package installation, alternate command or local-host fallback. Use `policy.deny_operations` to withhold either tool even when storage read is enabled. Default policy still denies both.

Paths, UUIDs and labels are intentionally visible under this grant and may reveal topology or user-chosen names. Nothing in those strings is an instruction or a trusted host path. Only the named fields are projected; no options, credentials, configuration, file contents, SMART details or raw error text. Audit records contain only safe operation/lifecycle metadata, never these values.

After architecture-only checkpoint `4040ce3`, v10 strengthens both response contracts to v2: any present root `error` rejects before projection, even null/false or alongside plausible success data. Successful field shapes are unchanged. Prior v9 evidence remains historical v1 evidence, not silently relabeled v2 acceptance.

## Meaning and limits

Each collection accepts at most 128 source entries; v6's independent 256-item shared budget, 64-KiB normalized result and 256-KiB whole MCP result limits remain. Capacities must be unsigned JSON integers and become exact decimal text (including values above JavaScript's safe integer range). No string/float coercion or zero substitution. This preserves received numbers; it cannot repair upstream numeric loss or prove a disk's actual size.

Mount paths are unique, nonempty identities, bounded to 1024 UTF-8 bytes; repeated mount paths, including overmount ambiguity, reject the entire result. Block map keys are unique bounded nonempty source identities, not a unique physical-device assertion: a device can appear separately as a swap entry. Device and optional mount text cap at 1024, labels/UUIDs at 256, filesystem/version at 64. Required fields and present optional fields are strictly typed; unknown siblings are not returned. Exceeding any bound rejects rather than truncates. All accepted rows are checked before success is audited.

These are **non-atomic, LuCI-visible** observations. Upstream mount reporting skips failed/zero-block filesystem statistics. Block reporting can omit unreadable or unrecognized devices and may produce an empty map even when the block utility failed; its source key and text can retain upstream escaping. A successful empty response means only that no rows were returned, not that the machine has no devices or mounts. Missing capacity, paths or filesystem type is an error, not an invented value. Do not infer filesystem health or cross-row capacity invariants.

Block signature reads can spin up disks. Mount statistics can contact remote filesystems and block inside the target daemon/kernel. The MCP deadline bounds client waiting and adapter work, not a guarantee of aborting every target-side kernel operation. There is no mount/unmount, swap activation, fsck/repair, block detect, fstab write, share access or firmware operation in these tools.

## Review and tests

The [LuCI 25.12 implementation](https://github.com/openwrt/luci/blob/openwrt-25.12/modules/luci-base/root/usr/share/rpcd/ucode/luci) supplies these optional methods. The [fstools info dispatch/cache](https://github.com/openwrt/fstools/blob/master/block.c) reads signatures and mount metadata without entering mount, swap or repair handlers. Its [tiny probe](https://github.com/openwrt/fstools/blob/master/libblkid-tiny/libblkid-tiny.c) uses read-only device access; the optional [libblkid adapter](https://github.com/openwrt/fstools/blob/master/probe-libblkid.c) uses low-level probing, whose [default open is read-only](https://github.com/util-linux/util-linux/blob/master/libblkid/src/probe.c). These branch source reviews are not runtime version/variant guarantees.

Actual feature-catalog fixtures enforce exact actions/response IDs, all category/access/execute combinations, private siblings, required fields, malformed errors, duplicate identities, integer precision, exact/excessive row/text and byte limits. MCP fixtures exercise both tool outputs, direct denied calls, invalid inputs, same-target probe caching and payload-free audit. Closed codec fixtures cover every admitted object. Current executed host gates belong in validation.md; no hardware or emulated storage coverage is claimed here.
