# Windows private rotating audit files (v19)

The existing `audit.destination = "file"` now has a native local-NTFS implementation
on Windows. It uses the same portable audit configuration, safe event rendering,
bounded writer/deadline and fail-closed dispatcher as Linux. This is host audit
storage, not a router operation, encrypted backup store, Vault facility or durable
journal. The management catalog remains unchanged.

## Configuration

Provision a private, non-synced folder outside repositories under your user profile,
using the Windows Security settings described in [protected files](windows-protected-files.md).
Do not change permissions on an entire profile, drive or existing shared folder.
Use its exact absolute long path in operator-owned TOML; this is a placeholder:

```toml
[audit]
enabled = true
format = "json" # or "text"
destination = "file"
path = 'C:\Users\YOUR_USER\OpenWrtMcpPrivate\audit.log'
max_bytes = 1048576
retained_files = 3
```

The folder must already exist and pass the trusted ancestor/volume checks. A new
active file receives a protected owner/DACL at creation, granting only the current
process user full access without inherited grants. Existing files must already
meet the secret-grade ACL profile; unsafe files are rejected without repairing
their owner, permissions or contents. Failed initial admission can leave a new
empty private file, never intentionally cleaned up through an unvalidated path.

The operator configuration accepts 1 KiB..1 GiB per file and 1..100 retained files;
the lower-level internal host API also tests smaller file sizes. Retention counts
old generations, in addition to the active file. Records are never split across
generations. A record larger than the selected file bound fails. Writes are
delegated in at most 64 KiB chunks without copying the whole record here. Empty
records do not write or rotate. Reducing a limit after restart rotates an older
larger active file before the next nonempty record, subject to the 1 GiB hard cap.

## Protection and failure behavior

The [v8 path/volume/ACL restrictions](windows-protected-files.md) still apply:
absolute DOS paths on local NTFS, bounded UTF-16 components, no reparse points,
short aliases, streams, extra hard links, offline/recall or delete-pending objects.
Every derived `audit.log.1` through `.N` name is checked before initial creation.
Process-user identity, held ancestors and active file security/name/exact size are
rechecked before and after each record. Directory traversal rights make the held
ancestor no-delete-sharing restriction effective on the tested host; metadata-only
handles were insufficient. The same corrected walk also protects config/key reads.

Active data access is append-only, without write-data/truncate authority; its handle
and retained generation handles exclude other ordinary write/delete opens. All
existing generations are validated and held before rotation changes anything.
Only exact object-name-not-found means absent; a sharing or permission failure does
not. Even an oldest generation scheduled for removal must first be safe.

Rotation deletes the exact held oldest file, renames held generations in descending
order and the active file to `.1`, then creates a fresh private active file. Native
renames use a simple relative name and the verified parent handle, with replacement
disabled. No overwrite, collision retry, alternate destination, directory scan or
automatic reopen. An ordinary reader can leave a deletion pending and cause a safe
rotation failure. A later failure can leave partially advanced generations: this
multi-file operation is **not atomic**. Any error permanently disables that writer
instance and releases its handles; the audit layer reports only `audit_unavailable`.

No fsync/crash durability, signed/tamper-evident records or hard cancellation of OS
calls is claimed. This is not protection against the trusted process user,
LocalSystem, administrators, compromised kernel/driver, existing writable mappings
or process compromise. Do not externally rotate a live file. Choose an appropriate
operator-managed volume; Windows system-log delivery, macOS private files and
Personal Vault remain explicitly unsupported, with no fallback to stderr or another
custody profile after a configured file destination fails.

## Implementation and native evidence

Architecture/harness checkpoint `f3059d3d308309153d793e6235a98f029d900adc`
(formerly `15dae37`, message-only normalization) preceded behavior, with 551 native
Windows GNU and 572 Linux-on-WSL tests. The safe `windows/log.rs` facade exposes only
an opaque write port to the existing `windows/native.rs` SDK bridge. SDK version,
features, dependencies, portable layers and public host API are unchanged.

Native synthetic tests cover private creation under an inheritable readable parent,
append/reopen, exact limits, 100 generations, smaller reopened limits, Unicode and
multi-chunk records, unsafe ACLs/hard links/reparse/short names, generated-name
overflow, conflicting leaf/ancestor/generation handles, ACL drift and permanent
failure. Server composition additionally covers JSON/text, name sanitization,
disabled logging, error propagation, and the actual MCP binary's private rotation
with no raw arguments or client names in its audit output. Only exclusively created
private fixture directories are removed; no real log, Vault, key or router is used.
See [Windows validation](windows-validation.md) for full executed gates/artifacts.

Two native failures were reproduced and corrected without weakening the profile:

- The Win32 rename wrapper returned error 87 for the selected relative-directory
  form on Windows build 26200; `NtSetInformationFile` succeeded with equivalent
  no-replacement relative arguments. Production uses the native call directly,
  with no retry, fallback, bypass-access-check class or raw error logging. This is
  scoped host evidence, not a universal statement about every Windows release.
- Metadata-only ancestor handles permitted a conflicting DELETE open. Requesting
  `FILE_TRAVERSE` made the existing no-delete-sharing requirement effective; native
  tests now cover both an already-held deleter and a later delete attempt, while
  repeated rotation and protected reads continue to pass.

Primary API contracts: [native rename names/rights](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information),
[user-mode NtSetInformationFile](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-ntsetinformationfile),
and [share-access compatibility](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-iocheckshareaccess).
These describe APIs; the failures and correction results above are local fixture
observations. MSVC, macOS, other Windows releases/filesystems and remote CI remain
unverified; Windows success is not evidence for those environments.
