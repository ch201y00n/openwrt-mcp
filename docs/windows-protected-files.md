# Windows protected files (v8)

Windows protected **reading** is implemented for local NTFS. This includes operator config, restricted key files and a restricted ZIP from which the existing container adapter selects one exact member without extraction. It does not implement Personal Vault recognition/locking, private audit-file writes, general filesystem MCP access or a backup/restore workflow. Native acceptance currently uses Windows 11 x64 with the GNU Rust toolchain and synthetic files; MSVC/other Windows versions remain unverified.

## Use

Choose a non-synced private directory outside repositories, for example a new `OpenWrtMcpPrivate` directory directly under your user profile. In its Windows Security / Advanced settings, retain only your account and trusted system/administrator access as appropriate, and prevent unsafe inherited grants. Do not change permissions on a whole drive, profile or existing shared folder. This tool checks permissions but never repairs them. A private file below a shared writable ancestor is rejected; select a different trusted location instead of weakening verification.

Use the exact absolute long path; no environment expansion, shortcut, junction, symbolic link, short-name alias, mapped/substituted drive, UNC share or alternate data stream. Korean and other valid Unicode names are supported when they exactly match the opened name. ASCII case differences are accepted; other case-folding aliases are conservatively rejected. The profile admits only native `HarddiskVolume` mappings whose opened root matches the current drive mapping and a volume GUID root, with NTFS and persistent ACL support. Other local storage providers need a reviewed profile.

For a configuration file already provisioned securely, run `openwrt-mcp check --config 'C:\Users\YOUR_USER\OpenWrtMcpPrivate\config.toml'`. Substitute your actual path. Offline check validates configuration, not the presence or usability of referenced keys or a router connection. The server checks key sources only when the authorized workflow needs them. There is no fallback to an environment variable after a rejected path.

Within trusted TOML, direct and archive sources use the same shape as Linux, with literal Windows paths (placeholders only):

```toml
[protection.sources.keyring]
kind = "restricted_file"
path = 'C:\Users\YOUR_USER\OpenWrtMcpPrivate\keyring.zip'

[protection.sources.restore_identity]
kind = "archive_entry"
source = "keyring"
format = "zip"
entry = "router/identity.txt"
```

The ZIP itself must meet secret-file protection; ZIP is not encryption. Do not place it in an ordinary synced folder. `personal_vault_file` deliberately remains unsupported, even if the same pathname would pass a normal restricted-file check. Do not relabel Vault as a restricted source to imply unimplemented Vault assurance. Environment sources remain an explicit cross-platform alternative, with their separately documented exposure risks.

## What is verified

The process user, LocalSystem and Builtin Administrators are trusted; the fixed TrustedInstaller SID is also trusted only at the volume root. Arbitrary token groups are not automatically trusted. Thread impersonation is rejected. These checks cannot protect against the same user, an administrator, compromised kernel/driver, existing writable mappings or a compromised process.

Each ancestor is opened relative to its held parent, without following reparse points or sharing deletion. Owner and effective DACL are inspected from the handle. Untrusted directory changes/deletion are rejected, apart from add-subdirectory at the volume root; each existing descendant must independently be trusted. Null/absent DACLs and unknown ACE formats fail closed. Deny ACEs cannot rescue unsafe allow grants. Inherited effective grants count; inherit-only grants do not apply to the current object.

The leaf must be a regular disk file with one link, no delete-pending/reparse/offline/recall state and bounded size. Its handle shares reading only, so an existing writer/deleter normally causes rejection. Config may be readable by others but not modifiable; keys may grant others only metadata/control/synchronize, not contents, EA data or execute. Reads use that same handle, and metadata/security/length are rechecked before returning. All secret error buffers are zeroizing; OS caches, swap and dumps are outside that guarantee.

Bounds: 4096 UTF-16 path units, 128 components, 255 units per component, 64 KiB descriptor, 4 KiB token metadata, 1 MiB config and 16 MiB secret public ceilings. Purpose-specific key/container limits may be lower. Errors are fixed `host_*` / `key_source_*` codes, without paths or bytes. Kernel calls can block; this is not hard-real-time or protection against trusted concurrent administrators.

## Validation scope

Tests cover native private files, config-versus-secret access, unsafe write/execute/inherited grants, null DACL, untrusted ancestors, hardlinks, junctions, sharing conflicts, empty/oversized files, Unicode, exact ZIP selection, key-purpose limits and Vault separation. Supplied-byte tests cover malformed/overlapping descriptors, owner/ACE/SID bounds, unknown ACE kinds, conservative deny handling and normalized-name/volume-map rejection. Native short-alias checks exercise an alias only when the NTFS volume actually provides one; they do not alter 8.3 settings. No drive mappings, existing ACLs, real keys, Vault archives or router state are modified by acceptance tests.

The SDK bridge is confined to one private file by [ADR 0008](adr/0008-windows-protected-reads.md), with an exact purpose-specific interface. [QueryDosDevice](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-querydosdevicew) and handle-bound final paths establish mapping consistency; the remaining primary API references are in the ADR. See [native validation](windows-validation.md) for executed gates and outstanding platforms.
