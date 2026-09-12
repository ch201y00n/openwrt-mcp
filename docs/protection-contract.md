# Protection API contract (architecture v3)

This contract is defined before provider implementation. These are internal library interfaces, not MCP tools and not a complete backup/restore workflow.

The source/provider ports remain current. Architecture v4 supersedes the original Unix-wide file-profile assumption: Linux native checks now belong to host-platform; Windows/macOS return explicit unsupported until their native DACL/ACL/volume profiles are implemented. See [portable host contract](portability-contract.md).

## Application-owned types: runtime::protection

Keep this namespace explicit; do not re-export its contents from runtime's root. Crypto and source adapters import it directly, while MCP is forbidden from doing so.

- `ProtectionError`: safe fixed variants/codes only; InvalidConfig, InvalidMaterial, SourceUnavailable, InteractionRequired, UnsupportedProtection, UnsupportedContainer, InvalidContainer, ResourceLimit, EncryptionFailed, DecryptionFailed, StreamFailed, DeadlineExceeded. Do not include source names, paths, keys, parse excerpts or external errors.
- `KeyMaterial`: private `Zeroizing<Vec<u8>>`, bounded construction, explicit borrowed byte access, redacted Debug, no Clone/Serialize/Deserialize. Construction rejects over-limit data while dropping a zeroizing buffer. This is an internal secret, never an MCP value.
- `RecipientMaterial(KeyMaterial)` and `IdentityMaterial(KeyMaterial)`: distinct non-serializable wrappers with explicit construction/access. Cryptographic parsing validates the intended purpose; never infer it from the source filename.
- `KeyLimits`: maximum key bytes 64 KiB, container bytes 4 MiB, entries 128, central-directory bytes 256 KiB, expansion ratio 100 by default; all nonzero and validated against documented hard caps.
- `CryptoLimits`: input 64 MiB, output 65 MiB, cooperative deadline 30 seconds, recipient count 16 by default. All nonzero, bounded; input/output refer to the direction of the individual operation.
- `CryptoReport`: processed input/output byte counts only. A successful encrypt report requires finalization; a successful decrypt report means the complete stream authenticated, not authorization to apply it.

## Ports and use cases

Hard caps: keys 1 MiB; containers 16 MiB; entries 4096; directory metadata 1 MiB (also no greater than the container limit); expansion ratio 1000; crypto input/output each 1 GiB; cooperative deadline 300 seconds; recipients/identities 64. KeyMaterial accepts bounded source/container buffers up to 16 MiB; sessions independently enforce the smaller key bound. Configuration locations may be cloned, private material may not.

`KeySource: Send + Sync` has `read(&self, max_bytes: usize) -> Result<KeyMaterial, ProtectionError>`. A source is pre-bound to an operator-selected location. There is no client-controlled location parameter, source enumeration, fallback, or process execution.

`KeyContainer: Send + Sync` has `read_entry(&self, container: &KeyMaterial, entry: &str, limits: &KeyLimits) -> Result<KeyMaterial, ProtectionError>`. It knows a container format, not a key algorithm. Key-sources composes a container decorator over a source.

`EncryptionProvider: Send + Sync` has `format_id(&self) -> &'static str` and `encrypt(&self, recipients: &RecipientMaterial, input: &mut dyn std::io::Read, output: &mut dyn std::io::Write, limits: &CryptoLimits) -> Result<CryptoReport, ProtectionError>`.

`DecryptionProvider: Send + Sync` has `format_id(&self) -> &'static str` and `decrypt_to_staging(&self, identity: &IdentityMaterial, input: &mut dyn std::io::Read, staging: &mut dyn std::io::Write, limits: &CryptoLimits) -> Result<CryptoReport, ProtectionError>`.

`EncryptionSession` owns a recipient source and an encryption provider; it has no identity-source field or argument. `DecryptionSession` owns a separate identity source and decryption provider. Both validate limits, load fresh bounded material once per call, convert to the correct purpose wrapper, call the provider and release material. The sessions do not open files or emit logs. Future audited router backup use cases must own authorization, completion publication and restoration policy around them.

## Concrete adapters

`crypto-age::AgeX25519` implements both provider traits under format id `age-x25519-v1`. Recipient documents contain bounded newline-separated native age X25519 public recipients; identity documents contain bounded native X25519 private identities. Blank/comment lines and CRLF are supported; trimmed native key lines are capped at 128 bytes before decoding. Decryption header construction has a fixed 256-KiB read budget including nonce and buffered read-ahead, with at most one overflow/EOF probe byte. Reject other schemes, passphrases and private material supplied as recipients. The provider has no knowledge of file/env/archive locations. Use the established age library with unused features disabled.

`key-sources` owns `SourceConfig` tagged by kind: restricted_file(path), personal_vault_file(path), environment(variable), or archive_entry(source alias, format zip, entry). Configurations contain locations/references, never literal private keys. A bounded `SourceRegistry` validates identifiers, referenced aliases, paths, member names, cycles and nested containers without reading any source. Each lookup creates the exact configured provider/decorator; there is no fallback. A file-access interface permits a future native Windows implementation without modifying the crypto provider.

Personal Vault is a user-unlocked protected-file profile, not automated Microsoft account access. Default provider support must reflect actual platform guarantees: Unix restricted files require verified owner/mode/no-follow handles; a platform without implemented protection reports UnsupportedProtection. Do not infer a Vault path from its spelling or silently downgrade its protection. No call unlocks, hydrates, extracts, exports or keeps a Vault open.

## Scope and acceptance

Implement and fixture-test the provider and source abstractions, age streams, explicit environment lookup, Unix restricted files and bounded non-encrypted ZIP selection. Validate optional operator source references offline. Do not claim native Windows Vault/ACL support, password-protected archives, complete backup publication, restore staging transactions, router rollback, authenticated backup provenance, hard deadlines for blocking I/O or a secret-management MCP API.

Future encryption implementations replace the provider without changing sources. Future key containers replace the decoder without changing age. Each added format remains an explicit approved selection, not a user-supplied dynamic module or command. Raw cryptographic primitives remain encapsulated by the established library.
