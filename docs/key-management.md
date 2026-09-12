# Key custody and age

Architecture v3 separates independently replaceable responsibilities:

```text
operator-selected source     optional container       application purpose      stream provider
restricted file ────────┐
user-unlocked Vault ────┼──> exact ZIP entry ─────┬──> public recipients ───> age encryption
explicit environment ──┘    or direct bytes      └──> private identities ───> age decryption to staging
```

Sources/containers do not know age. The age adapter does not know paths, environment variables, ZIP or Vault. The application owns narrow ports and separate encryption/decryption sessions. The composition layer selects only approved adapters. Replacing a cipher does not require changing sources; adding a container does not require changing age. This is not a generic crypto toolkit, dynamic plugin loader or client-selected cipher negotiation. See [ADR 0003](adr/0003-key-custody-and-age.md) and [API contract and bounds](protection-contract.md).

## Current scope

These are internal library primitives and offline configuration validation, not backup/restore MCP tools. The ten device read tools remain unchanged. Backup publication, secure plaintext staging, durable recovery and authorized restore remain separate future workflows.

| Component | Scope |
| --- | --- |
| age | Native X25519, streaming encrypt/authenticated decrypt, bounded I/O, cooperative deadline |
| Restricted file | Unix ownership, permissions, regular-file and no-follow opened-handle checks |
| Environment | Only the exact configured variable; explicit opt-in, no fallback/enumeration |
| ZIP member | Plain Stored/Deflate, one exact entry, bounded metadata/decompression, no extraction |
| Personal Vault | Protected-file abstraction; no automatic unlock/account access. Native Windows ACL/Vault access is not yet implemented and fails closed |
| Encrypted ZIP / 7z / other cipher | Not implemented; needs an explicit reviewed adapter |

The age provider accepts native X25519 text documents, optionally with blank/comment lines, and writes binary age format. Passphrases, SSH/plugin/PQ identity schemes and other algorithms are not silently accepted. The established Rust age library performs cryptography; this project does not implement raw primitives. Upstream classifies its pre-1.0 releases as beta: this is not a production-readiness or external-audit claim. [Rust age documentation](https://docs.rs/age/0.12.1/age/)

## Separate public and private authority

Encrypt with public recipients. Private identities are needed only for decryption and should be omitted from encryption-only router deployments. Public recipients still need integrity protection: replacing them can send future backups to an attacker. Provision sources through trusted operator configuration, never MCP inputs. Multiple recipients provide recovery access; each can decrypt the whole backup.

Sources are re-read per operation, not cached indefinitely. Owned key buffers are zeroized on drop, non-clonable/non-serializable and Debug-redacted. This does not erase the original environment, OS caches, swap, dumps or third-party copies and cannot protect a compromised process. Memory locking is not implemented.

## Configuration

Configuration contains aliases and locations, never literal keys. Default configuration has no key authority. The [environment example](../config/protection-environment.toml) uses public recipients only and grants no device permissions. `check` and `catalog` remain offline: success does not prove source availability or platform protection.

Unix file plus optional archive-held identity (placeholder paths; no files are created):

```toml
[protection]
cipher = "age"
recipient_source = "recipients"
identity_source = "restore_identity" # Omit on encryption-only deployments.

[protection.sources.recipients]
kind = "restricted_file"
path = "/etc/openwrt-mcp/keys/recipients.txt"

[protection.sources.keyring]
kind = "restricted_file"
path = "/etc/openwrt-mcp/keys/keyring.zip"

[protection.sources.restore_identity]
kind = "archive_entry"
source = "keyring"
format = "zip"
entry = "router/identity.txt"
```

Use absolute operator-selected paths, trusted parent directories and private regular files, typically mode 0600 or 0400. Unsafe ownership/permissions, links and unsupported protection are rejected. Never put private keys or keyring archives in this repository, generic synced folders or uncontrolled temporary storage. ZIP alone provides no confidentiality.

An archive in a Vault changes only the base source to `personal_vault_file`; `archive_entry` remains identical. A direct key uses the Vault source without a container. There is no hard-coded Vault path or universal file-count assumption. **This configuration shape is defined, but native Windows protected-file access is unavailable.** Until owner/DACL/reparse/opened-handle validation is implemented and tested, it returns `key_source_protection_unsupported`. Windows read-only attributes or WSL permission emulation are not equivalent protection.

The user unlocks Vault through Microsoft's supported authentication flow. This tool does not unlock it, bypass MFA, keep it open, export its archive or search for substitutes. See Microsoft's [Personal Vault authentication and automatic locking documentation](https://support.microsoft.com/en-us/onedrive/protect-your-onedrive-files-in-personal-vault).

Environment sources are explicit alternatives, not preferred private-key storage. Set variables outside repository/config files; avoid command-history exposure. The environment is a process copy, not a vault. The operating-system lookup makes its initial value copy before our length rejection. Control inheritance, diagnostics and crash dumps; the original value cannot be erased by zeroizing our buffer. Existing router child-process execution clears inherited environment.

## Failure and authenticity

Errors expose fixed codes, not paths, aliases, entry names, parser excerpts or key bytes. Source failure never triggers fallback, generation, rotation or requests to export real keys. Unknown options, cycles and nested/unsupported containers fail validation.

Encryption succeeds only after age finalization: partial ciphertext is not a completed backup. Decryption may output plaintext before a later authentication failure. Write only to protected staging, discard all staging on any failure, and apply nothing until the entire stream authenticates. These primitive methods do not create or secure staging files.

Authentication does not authorize restore or prove backup origin/freshness. Anyone with a public recipient can create a valid encrypted file for it; backup identity/context needs separate validation. A public-key-only router cannot decrypt its archival backup for rollback, so recovery must be designed separately.

Byte limits cover input and output separately, including age overhead. age header parsing has a separate fixed 256-KiB read budget including nonce/buffered read-ahead; native key lines above 128 bytes are rejected before decoding. ZIP uses a conservative subset that also rejects ZIP64, multiple disks and ambiguous alternate directory/name metadata. Deadlines are cooperative: a supplied blocking reader/writer may block beyond the deadline. Future workflow integration must bound workers and cancellation behavior before exposing device tools.

Only synthetic fixture validation is in scope. No actual Vault, user key, router or backup is accessed.
