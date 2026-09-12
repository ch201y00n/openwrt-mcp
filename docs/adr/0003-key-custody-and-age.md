# ADR 0003: Independent key custody and age encryption boundaries

Status: accepted architecture, implementation follows a validated checkpoint. Contract version: 3.

## Requirement and decision

The owner selects age for backup encryption and requires interchangeable key access: a user-unlocked Personal Vault file, an archive holding multiple keys, an explicitly named environment variable, or an access-restricted file. Encryption should be replaceable without coupling custody to algorithms.

Separate **where bytes are obtained**, **how a container selects a key**, **public-recipient versus private-identity authority**, and **how streams are encrypted**. Use narrow application ports, not a general cryptographic framework, runtime-loaded plugins or arbitrary commands. Only the age X25519 v1 format is initially approved. Future algorithms or archive formats need a reviewed adapter and the normal architecture/harness evolution; no negotiation or downgrade driven by MCP input.

## Responsibilities and directories

- runtime::protection owns non-serializable, zeroizing key material, distinct recipient/identity bindings, key-source/container/cryptographic ports, limits and orchestration. Standard Read/Write trait objects describe caller-provided streams, not permission to open OS handles. Keep filesystem, environment, OS, process and stdin/stdout/stderr/pipe APIs forbidden.
- crates/key-sources implements named environment sources, restricted file access and a ZIP-entry decorator/decoder. It may depend on zip and safe rustix APIs, never age or the age adapter. Filesystem security belongs here, not in the cipher.
- crates/crypto-age implements the age encryption/decryption ports. It may depend on age, never key-sources, zip or filesystem/environment APIs. It receives purpose-specific material and already-open streams, never key paths or Vault credentials.
- server validates operator-owned configuration and composes these components. It must not print key material, load keys for offline check/catalog, or expose raw key-management/crypto calls as MCP tools.
- core/features stay unchanged and unaware of key bytes. mcp cannot import runtime::protection or either concrete adapter.

Separate crates enforce the two infrastructure responsibilities using existing Cargo metadata checks; this avoids weakening all of adapters to permit mixing key access and crypto. The nine-crate workspace retains the six logical layers from ADR 0002. Narrow streaming trait access is the only runtime source-boundary expansion.

## Key authority and lifetime

Encryption uses a recipient binding; decryption uses a separate identity binding. Encryption-only deployments omit private-key bindings entirely. Sources are chosen by exact operator-owned aliases, not client paths/environment names. No automatic searching, fallback, directory enumeration, key generation, key rotation or Vault unlocking is implicit.

Raw material has private storage, redacted Debug, no serialization or ordinary Clone, and zeroizes owned buffers on drop. Each operation loads only the configured material and releases it afterward. Zeroization does not erase the original environment, OS caches, library copies, swap, dumps or a compromised process; no memory-locking guarantee is made.

## Vault and platform policy

OneDrive Personal Vault requires the user's supported unlock/authentication flow. Treat it as a protected file source after that flow, not a cloud secret API. Locked, offline, unhydrated or unavailable states fail closed; do not copy the real Vault archive into the repository, another synced folder or an uncontrolled temporary location. No automatic unlock, keepalive, MFA bypass or silent fallback.

Unix restricted-file access must verify ownership, permissions, regular files, links and opened-handle consistency. Windows needs real owner/DACL/reparse/handle checks; the readonly bit and POSIX mode emulation are insufficient. Until a native Windows implementation is validated, the default file/Vault provider reports unsupported protection. Keep a protected-file access port so that implementation can be added independently. Environment and injected source fixtures must not be misrepresented as actual Windows Vault acceptance.

## Containers and bounds

An archive is a decorator over a source, not a separate cipher or secret store. Select exactly one named regular entry and never extract files. Initially support non-encrypted ZIP Stored/Deflate; other formats and password-encrypted archives fail explicitly. Future encrypted-container credentials need separate identity references and cycle checks.

Bound total archive bytes, central-directory metadata/count before parser allocation, entry size, actual decompression and compression ratio. Reject duplicate/ambiguous names, traversal/absolute/backslash paths, symlinks, overlapping entries, nested container chains and unsupported formats. Load only the selected entry's contents, then discard the bounded archive buffer. ZIP alone is not encryption.

## Streaming and authenticity

Use the age format and implementation; do not implement cryptographic primitives. Public recipients and private identities are parsed only by the age adapter. No passphrase/SSH/plugin/PQ identity types are silently accepted by the initial X25519 provider.

Enforce input/output limits and cooperative elapsed-time checks while copying bounded chunks. A synchronous caller-owned Read/Write can itself block: these primitives are not a hard-deadline I/O executor. Production backup orchestration must schedule bounded blocking work and handle cancellation explicitly before exposing device tools.

Encryption is successful only after age's finalization succeeds. Partial ciphertext must not be published as a completed backup. Decryption writes to **staging only**: later chunks or EOF can fail authentication after earlier plaintext was produced. Authentication success is not permission to restore; backup publication, protected staging, content validation and router application are separate future transactions. Do not claim automatic rollback: a router holding only public recipients cannot decrypt archival backups.

## Evolution and verification

Before implementing providers, add source/dependency regressions proving the new boundaries and run the full architecture checkpoint. Then test fake provider substitution, purpose separation, source failure without fallback, safe errors/debug, actual age round trips/wrong keys/tampering/truncation/finalization errors and malicious containers. Keep CLI check/catalog offline. Existing 81 tests must continue to pass. Only synthetic keys and archives are used; no user's real Vault or router is accessed.

## References

- [age stable specification](https://c2sp.org/age@v1.1.0)
- [age Rust API](https://docs.rs/age/0.12.1/age/) (upstream marks pre-1.0 releases beta; release review is required)
- [Personal Vault](https://support.microsoft.com/en-us/onedrive/protect-your-onedrive-files-in-personal-vault)
- [Windows access-control lists](https://learn.microsoft.com/en-us/windows/win32/secauthz/access-control-lists)
- [ZIP reader API](https://docs.rs/zip/8.6.0/zip/read/struct.ZipArchive.html)
