# Paged opkg root status-file observations

`packages_opkg_status` requires only `packages.read`. It discloses package name,
version, architecture and the exact opaque status text. Deny this operation or
the category to withhold that metadata. The observability policy includes both
package reads; availability is checked during capture, not assumed from policy.

Call with `{}` for a new observation and pass each returned `next_cursor` verbatim
as `{"cursor":"..."}`. No path, manager, package filter, executable or page-size
argument is accepted. Each page has at most 16 records, sorted by unique exact
package name. Status is not filtered to installed packages or interpreted as
health. The response contract is `packages_opkg_status.v1`:

- `source`: `opkg_38eccbb1`
- `scope`: `opkg_root_status_file`
- `consistency`: `non_atomic_observation`
- `whole_device_complete`: `false`
- `captured_count`, `offset`, `items`, optional `next_cursor`
- Each item has exactly `name`, `version`, `arch`, `status`; no APK layer field.

The recipe first requires the exact opkg banner for
`38eccbb1fd694d4798ac1baf88f9ba83d1eac616 (2024-10-16)`. Only then does the same
backend read `/usr/lib/opkg/status`. Both commands clear inherited environment
and supply only a fixed PATH and LANG. Unknown banners, missing files, nonzero
exit, incomplete transport or malformed content fail closed. There is no
automatic manager fallback, configuration search or query retry.

This distinction matters: the reviewed opkg initializer can create lock, temporary
and destination paths even for ordinary query commands. This operation uses
`--version`, which exits before configuration loading, then a fixed file read;
it does not execute `opkg status` or `list-installed`. See the pinned
[OpenWrt package definition](https://github.com/openwrt/openwrt/blob/v24.10.4/package/system/opkg/Makefile),
[opkg entry point](https://github.com/openwrt/opkg-lede/blob/38eccbb1fd694d4798ac1baf88f9ba83d1eac616/src/opkg-cl.c)
and [configuration initializer](https://github.com/openwrt/opkg-lede/blob/38eccbb1fd694d4798ac1baf88f9ba83d1eac616/libopkg/opkg_conf.c).

The file is not an atomic package-manager snapshot. Other configured destinations,
unrecorded software, scripts, effective runtime state and completeness are not
inferred. Neither this observation nor its version banner is software attestation,
a backup or a transaction baseline. Metadata-only `operation_capability` returns
`unknown/capture_required`, scope `closed_file_response`, without reading the file.

## Validation and resources

The decoder validates the entire bounded UTF-8/LF document before the first page.
Nonempty input must terminate with a blank line. Mandatory selected fields must
occur exactly once per stanza, be nonempty and single-line. Header names and
duplicates are checked case-insensitively. Unknown headers and their continuations
are framing-validated but discarded, including configuration paths and hashes.
Continuations of selected fields, duplicate identities and malformed late records
reject the entire capture. An empty file is a valid empty observation, not proof
that the device has no installed software.

The shared package source ceiling is the smaller of operator output limits and
4 MiB (default 64 KiB). Limits also include 4,096 records, 2 MiB retained selected
field bytes, 8,192 bytes per physical line, 64 headers per stanza and 64 bytes per
header name. Name/version/architecture/status maxima are 256/256/64/128 UTF-8
bytes. Selected controls are rejected. Normalized JSON is at most 64 KiB and the
whole MCP result with both copies at most 256 KiB. There is no silent truncation.

APK and opkg share one private snapshot slot, not one allocation per manager.
Every new capture invalidates either previous snapshot before entropy or device
I/O. Continuations bind to source profile, operation, connection epoch and random
nonce, with an absolute 120-second lifetime from capture start. Cross-manager
cursors are invalid. Authorization and safe metadata-only audit apply on every
page; overlapping work is Busy. Failed/cancelled refresh, invalid admitted cursor,
expiry or lost connection cannot restore the old snapshot. Explicitly request a
fresh capture after an error; do not automatically continue or recapture.

The implementation uses the existing portable core/features/runtime/codec
boundaries with purpose-specific SSH and verified OpenWrt-local backend ports.
An absent backend implementation is unsupported, never host-local fallback. See
[ADR 0014](adr/0014-opkg-status-observations.md) and the separate
[APK contract](package-observations.md). Package installation, removal, upgrades,
repository edits and manager discovery remain separate work.
