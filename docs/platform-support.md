# Host and target support

Windows, Linux and macOS are required **MCP hosts**. The **managed target** is OpenWrt. A Windows program uses Windows paths for its own configuration and POSIX paths for router actions; these must never be conflated. Shared policy, features, runtime, MCP, age and SSH code cannot introduce OS-specific behavior under architecture v4.

## Common execution path

Run the MCP executable on the workstation, explicitly supply configuration through `--config-env <NAME>`, select `target.kind = "ssh"`, supply an exact environment key source and use stderr audit output. No local SSH executable, shell, WSL, `.ssh/config`, key agent or automatic known-host discovery is used by the native SSH backend. Configuration/key environments are explicit operator authority, not preferred long-term secret storage. Process environments and crash dumps can expose their contents; do not paste private keys into commands or commit them. Operating systems can impose lower environment-size limits than the application's input bound.

The remote connection authenticates with an unencrypted OpenSSH Ed25519 private key and a separately pinned Ed25519 router host key. Obtain and verify the host fingerprint out of band. No trust-on-first-use, password prompt, automatic retry, remote-to-local fallback or auto-unlock is provided. One connection is reused sequentially; one active channel per backend, with overlap returning `busy`. Timeout/cancellation or uncertain session failure closes the socket and latches the backend until process restart. This does not prove that a submitted remote command stopped or rolled back.

`check` and `catalog` parse and validate without reading authentication/age sources or connecting to a target. A valid configuration is not evidence of router availability. The default target is unconfigured: authorized calls return `target_not_configured`, never execute host programs. `openwrt_local` requires an explicitly selected Linux host with a protected, root-owned OpenWrt release marker. Selecting it on an ordinary workstation is rejected.

## Capability and evidence matrix

| Capability | Windows host | Linux host | macOS host |
| --- | --- | --- | --- |
| Common stdio, policy, native SSH, age, environment sources, in-memory ZIP | Native GNU host/fixtures and MSVC CI passed | WSL host fixtures and native Linux CI passed | Native ARM64 CI passed for the common path, not optional file protection |
| Explicit environment configuration and stderr logs | Common path | Common path | Common path |
| Protected config/secret files | Local NTFS handle/DACL profile; native GNU and MSVC synthetic acceptance | Owner/mode/opened-handle profile with synthetic tests | Explicitly unsupported pending native ACL/volume profile |
| Private rotating audit files | Local NTFS private-DACL/handle-relative v19 profile; native GNU/MSVC files and MCP binary fixtures | Owner/mode/handle-relative rotation with synthetic tests | Explicitly unsupported |
| Native system-log delivery | Explicitly unsupported | Local `/dev/log` adapter; actual daemon acceptance pending | Explicitly unsupported, no Linux-socket assumption |
| Personal Vault | Abstract profile only; native access unimplemented | Unsupported | Unsupported |
| Execute target programs locally | Rejected | Explicit, verified OpenWrt only | Rejected |

Unsupported optional host facilities do not block the explicitly selected common path, and never silently downgrade their security. Windows read-only attributes and macOS POSIX modes alone are not sufficient native protection. Add native profiles through the architecture-first workflow, including dependency review and platform-specific adversarial tests.

The [Windows log profile](windows-private-logs.md) is independently admitted from
protected reads. It rejects insecure existing files, creates private files without
inherited grants, and latches any append/rotation failure. It is not a Vault,
system-log or durable ciphertext storage capability.

## Validation and deployment

The CI workflow requires Windows, Linux and macOS full gates plus explicit portable binary/SSH/crypto/runtime/MCP/package suites and a Windows-only protected-read suite. The harness rejects missing hosts and disabled/ignored required cases. Adding the workflow does **not** mean these jobs have passed. The v8 increment follows architecture checkpoint `9c0117f`; its [native Windows evidence](windows-validation.md) retains that historical scope. The later [P1 verification record](p1-validation.md) identifies exact local and successful remote runs for Windows MSVC, Linux and ARM64 macOS on code commit `07c3f51`. CI invokes the native gate on every runner; only the explicit local `-UseWsl` run counts as Linux-on-WSL. MSVC results are recorded separately rather than inferred from GNU. Host/fixture execution never implies all optional OS facilities, OpenWrt hardware or production acceptance.

Windows paths must meet the [protected-read profile](windows-protected-files.md): absolute DOS paths, a local native NTFS volume, trusted owner/ancestor ACLs, no reparse/short aliases or extra links. A private leaf below a shared writable ancestor is insufficient. No existing ACL is repaired, and no failed file read falls back to environment or Vault. Personal Vault remains unsupported.

The Linux `/proc` footprint example reports Linux-only measurements and fails explicitly elsewhere. OpenWrt CPU/ABI cross-compilation, SDK/musl linking, packages and actual router acceptance are separate from workstation support. No router deployment or public push is authorized by these tests.
