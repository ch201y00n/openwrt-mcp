# ADR 0009: Closed LuCI metadata probes and scoped storage observations

Status: accepted architecture checkpoint; validate and commit before changing production behavior.

## Need and boundary

The reference inventory includes `luci` and `luci-rpc`, but the v5-v8 seven-object probe enum cannot observe their method signatures. Admit exactly these two additional **descriptions**, not calls, wildcard enumeration, UCI, file, shell or general process probes. Registry schema 2 contains the original seven plus these two exact `/bin/ubus -v list <object>` entries. Architecture v9 requires capability `probe_profile = "base_luci_v2"`; schema 1 remains the closed pre-migration subset so this checkpoint can precede production migration. Earlier architecture versions reject schema 2. Unknown profiles, objects, options and future schemas fail the harness. Existing mandatory portable capability/collection/parser/runtime/MCP suites remain.

Ownership stays unchanged: core owns ReviewedObject, device-codec encodes/parses supplied bytes, local/SSH infrastructure executes, runtime owns same-target authorization/audit/freshness/deadlines, features owns category definitions. No dependency, host OS API, generic process exception, raw output or response form is added. A method signature does not authorize it, certify effects or prove a response schema. Missing/ACL-hidden modules remain unknown without installing packages or trying another method. Privileged operator extensions retain extra write+execute requirements; discovery creates neither tools nor grants.

## First consumer: storage observations

Review `luci.getMountPoints {}` and `luci.getBlockDevices {}` against OpenWrt 25.12 LuCI and the `block info` effect path before implementing. Reviewed non-mutating metadata requires Storage.Read. Calls have fixed empty arguments: no client paths, block detect, mount/unmount, swap activation, fstab/config access, file contents, shares or credentials. Device/mount paths and optional filesystem labels/UUIDs disclose topology under this grant; operation denials independently hide and reject these tools. Never audit those values.

Use unchanged v6 ObjectArray (mount path identity) and ObjectEntries (block source key identity). Require typed fields; capacity is an exact unsigned JSON integer rendered as decimal text, never float coercion. Collections cap at 128, text stays within existing ceilings, and shared 256-item/64-KiB normalized/256-KiB MCP limits remain. Errors, duplicate/ambiguous mount identities, missing required fields and overflows reject the whole observation. No truncation or raw fallback.

These are non-atomic LuCI-visible observations, not complete disks/mounts, health judgments, cross-row capacity invariants, repairs or backups. getMountPoints skips failed/zero-block statvfs results; getBlockDevices may yield empty/partial results on utility/device-read failure. Empty does not prove absence. Block signature reads may spin up media; statvfs may contact remote filesystems or block in target kernel/daemon. The MCP deadline bounds waiting, not every target-side kernel task. Document these effects and exclusions in tool descriptions and user contracts.

## DHCP follow-up, not implemented by this checkpoint

`luci-rpc.getDHCPLeases` advertises `family: Integer` and reads configured dnsmasq/odhcpd lease paths. It may skip unreadable files/malformed leases. `expires` is integer seconds **or boolean false**, while IPv6 identity may need DUID/IAID/interface/address context. Current v6 has no scalar union or composite identity. Do not discard expiry, coerce false to zero, assume unique MAC/DUID or weaken projection to fit. Review and extend architecture first for full typed leases. The introspection probe alone is not DHCP coverage.

## Acceptance

Commit an architecture-only checkpoint after negative profile/version/probe tests, including the assembled Cargo/Git gate. Then migrate registry/enum and exact encoding fixtures together. Exercise actual new catalog entries, policy denial/read/independent execution, fixed arguments, selected fields, source failures, bounds and transport/audit paths on portable hosts. Distinguish source review, synthetic hosts, optional LuCI emulation and exact BPI-R4 acceptance. No real router writes, Vault access or publication are authorized.

## Primary sources

- [LuCI 25.12 ucode methods](https://github.com/openwrt/luci/blob/openwrt-25.12/modules/luci-base/root/usr/share/rpcd/ucode/luci)
- [LuCI 25.12 C module and lease reader](https://github.com/openwrt/luci/blob/openwrt-25.12/libs/rpcd-mod-luci/src/luci.c)
- [fstools block dispatch and info implementation](https://github.com/openwrt/fstools/blob/master/block.c)

Branches can change; review is not proof the reference router has identical implementations or every optional dependency.
