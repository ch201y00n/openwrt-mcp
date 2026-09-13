# ADR 0019: Windows protected rotating audit files

Status: accepted for an architecture-only checkpoint before implementation.
Scope: the existing operator-selected PrivateLog facility on local Windows NTFS,
not router writes, Vault access, backup storage, durable logging or a new MCP tool.

## Ownership and compatibility

Keep the existing host-platform owner, Windows SDK 0.61.2 target/features, shared
PrivateLog API, audit-worker lifecycle, limits and all other host profiles. Add
`host-platform/src/windows/log.rs` for the safe private-log facade and its narrow
`LogWriter: Send` port; SDK calls and every concrete handle remain confined to the
existing `windows/native.rs`. No new normal/development dependency is needed.

The native bridge may expose exactly one additional parent-only function:
`open_log(path: &Path, max_bytes: u64, retained: usize) -> Result<Box<dyn super::log::LogWriter>, HostError>`.
The existing exact protected-read function remains admitted unchanged. The log
port exposes only `write(&mut self, bytes: &[u8]) -> Result<(), HostError>`; neither
raw handles, paths, native structs, generic filesystem operations nor security
descriptors leave the SDK boundary. No Debug/serde for concrete log containers.
No child module extends unsafe authority. The safe facade contains no SDK, unsafe,
filesystem, environment, process or network access.

Only Windows private_log_supported becomes true after implementation and native
acceptance. File-read, Personal Vault and system-log capabilities remain separate;
Vault and Windows system logging are still unsupported. No environment/stderr
fallback is chosen after a configured file destination fails. macOS and Linux
profiles are unchanged; Windows validation is not evidence for either.

## Path, object and creation rules

Retain v8's exact absolute DOS paths, bounded components, local NTFS volume and
drive mapping, no alternate stream/short alias/reparse/offline/recall objects,
trusted owner/DACL checks, no impersonation/privilege changes and held ancestors
without delete sharing. Validate generated names `base.1` through `base.N` against
the same path/component ceilings **before creating anything**. Retention is 1..100;
max active bytes is 1..1 GiB as in the existing public API. Existing active and
generation files cannot exceed the 1 GiB hard ceiling. A smaller configured limit
can rotate an older larger active file before the next nonempty append.

Open children relative to the verified parent handle, with reparse traversal
disabled. The active log receives append-only data access, read-control/attributes,
delete authority for its own rotation and synchronous access, sharing reads only.
This prevents another ordinary write/delete handle while it is owned. Generation
handles request only metadata/delete rights and share reads only. No write-data,
truncate, overwrite/supersede disposition, delete sharing, backup intent or generic
path-based mutation is admitted. External same-principal/admin writers remain
trusted authority, not an OS isolation guarantee.

Initial open-or-create never overwrites. New files receive an explicit protected
self-relative owner/DACL at creation: current process user as owner, one full-access
allow ACE for that user, no inheritance. Construct it from the bounded validated
token SID; validate it with the existing pure descriptor rules before supplying it
to the SDK. Check the resulting object's actual descriptor, type, link count,
non-delete-pending state, size and canonical name before writing. Existing files
must already satisfy secret-grade data-access restrictions; do not repair ACLs or
ownership. Failed admission may leave a new empty private file, never written audit
content; do not delete an unvalidated object to hide a failure.

## Append, rotation and failure

The safe facade enforces the same per-record/configuration limits without payload
copies. The native owner rechecks held ancestors and the active object's descriptor,
name, single-link regular type and exact expected size before each write. Append
also rejects thread impersonation and a changed process-user identity. Append
in bounded 64 KiB delegated chunks; validate resulting size/security again. Empty
records perform no append or rotation. No caller-controlled seeking or arbitrary
file content read is added. Partial write/validation/rotation failures permanently
latch the instance and release its handles; later calls cannot silently reopen.

Rotate only when the next record would exceed the configured limit. Before any
rotation mutation, open and validate every existing bounded generation and the
active file, retaining those handles. Only exact object-name-not-found means an
absent generation; permission/share/path errors are failures. Reject unsafe older
generations even when they would otherwise be discarded.

Delete only the validated oldest generation through its own held handle. Rename
held generations descending and active to `.1`, using relative destination names
under the same verified parent and **no replacement**. Then create a new active
file with create-new semantics and the same explicit private descriptor. A
destination collision or unexpected change fails; no replacement, retry, directory
scan, alternate destination or ACL repair. Preflight cannot make multi-file rotation
atomic: a later failure may leave a partially advanced generation set. Preserve it,
latch the error and let the existing audit layer reject further admitted work.

This is bounded synchronous audit-file I/O, not a write-ahead journal, fsync/crash
durability promise, all-or-nothing rotation or hard cancellation of OS calls. The
existing bounded writer/deadline/failure-latch policy remains authoritative. Do not
reuse PrivateLog as an implementation of the v18 ciphertext publication port.

## Harness and validation

Commit requirements, architecture, exact Windows-log machine contract and negative
tests after both full host gates, before adding native write/rotate behavior. Keep
all v8 protected-read rules and earlier-version bridge exports unchanged. Check
the exact new entry/port, SDK/unsafe confinement, facade ownership, finite bounds,
no handle-export/consumer escape and mandatory native Windows log coverage within
the existing `host-platform/tests/windows.rs` suite; no declaration scaffold is
counted as logging behavior. Existing required portable suites and CI remain.

Afterward test actual native synthetic files: private creation independent of
inheritable parent ACLs, append/reopen, exact limits/retention, repeated rotation,
empty/oversized records, unsafe active and old generations, hard links/reparse/name
aliases, other write/delete handles, unexpected size/descriptor changes, collision
and permanent failure latching. Existing protected-read tests must remain valid.
Use isolated non-synced fixture folders; never change real directory ACLs, existing
logs, Vault or credentials. Linux regression gates are separate from Windows native
acceptance. Record artifacts and explicitly outstanding macOS/MSVC evidence.

## Primary references reviewed

- [Microsoft Nt/ZwCreateFile contract](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-zwcreatefile): append-only rights, relative opens, create/open dispositions, sharing and user-mode Nt naming.
- [Handle-based file changes](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) and [FILE_DISPOSITION_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_disposition_info): deletion targets an opened object and requires DELETE access.
- [FILE_RENAME_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) and [native rename contract](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/ns-ntifs-_file_rename_information): directory-relative destinations and no-replacement collision behavior.
- [Security descriptors at creation](https://learn.microsoft.com/en-us/windows/win32/secauthz/creating-a-security-descriptor-for-a-new-object-in-c--): explicit creation-time descriptors, not post-write access repair.

These source contracts motivate the design; native fixtures must demonstrate this
implementation's selected NTFS behavior before support is claimed.
