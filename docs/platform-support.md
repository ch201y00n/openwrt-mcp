# Host and target support

Windows, Linux and macOS are required **MCP hosts**. The **managed target** is OpenWrt. A Windows program uses Windows paths for its own configuration and POSIX paths for router actions; these must never be conflated. Shared policy, features, runtime, MCP, age and SSH code cannot introduce OS-specific behavior under architecture v4.

## Common execution path

Run the MCP executable on the workstation, explicitly supply configuration through `--config-env <NAME>`, select `target.kind = "ssh"`, supply an exact environment key source and use stderr audit output. No local SSH executable, shell, WSL, `.ssh/config`, key agent or automatic known-host discovery is used by the native SSH backend. Configuration/key environments are explicit operator authority, not preferred long-term secret storage. Process environments and crash dumps can expose their contents; do not paste private keys into commands or commit them. Operating systems can impose lower environment-size limits than the application's input bound.

The remote connection authenticates with an unencrypted OpenSSH Ed25519 private key and a separately pinned Ed25519 router host key. Obtain and verify the host fingerprint out of band. No trust-on-first-use, password prompt, automatic retry, remote-to-local fallback or auto-unlock is provided. One connection is reused sequentially; one active channel per backend, with overlap returning `busy`. Timeout/cancellation or uncertain session failure closes the socket and latches the backend until process restart. This does not prove that a submitted remote command stopped or rolled back.

`check` and `catalog` parse and validate without reading authentication/age sources or connecting to a target. A valid configuration is not evidence of router availability. The default target is unconfigured: authorized calls return `target_not_configured`, never execute host programs. `openwrt_local` requires an explicitly selected Linux host with a protected, root-owned OpenWrt release marker. Selecting it on an ordinary workstation is rejected.

## Capability and evidence matrix

| Capability | Windows host | Linux host | macOS host |
| --- | --- | --- | --- |
| Common stdio, policy, native SSH, age, environment sources, in-memory ZIP | Native GNU host/fixtures passed; MSVC/CI pending | Host fixtures on WSL; native CI pending | Portable implementation; native acceptance pending |
| Explicit environment configuration and stderr logs | Common path | Common path | Common path |
| Protected config/secret files, private rotating audit files | Explicitly unsupported pending native DACL/handle/volume profile | Owner/mode/opened-handle profile with synthetic tests | Explicitly unsupported pending native ACL/volume profile |
| Native system-log delivery | Explicitly unsupported | Local `/dev/log` adapter; actual daemon acceptance pending | Explicitly unsupported, no Linux-socket assumption |
| Personal Vault | Abstract profile only; native access unimplemented | Unsupported | Unsupported |
| Execute target programs locally | Rejected | Explicit, verified OpenWrt only | Rejected |

Unsupported optional host facilities do not block the explicitly selected common path, and never silently downgrade their security. Windows read-only attributes and macOS POSIX modes alone are not sufficient native protection. Add native profiles through the architecture-first workflow, including dependency review and platform-specific adversarial tests.

## Validation and deployment

The CI workflow requires Windows, Linux and macOS full gates plus explicit portable binary/SSH/crypto/runtime/MCP/package suites. The harness rejects missing hosts and OS-disabled/ignored required suites. Adding the workflow does **not** mean these jobs have run. Local full gates passed on native Windows GNU (355 distinct tests) and Linux under WSL (387 distinct tests), including v7 package observations and native entropy. Windows used the native gate branch without WSL; Linux explicitly selected `tools/Test-Repository.ps1 -UseWsl`. MSVC, macOS and configured CI jobs remain unverified. Do not claim all-host acceptance. See [native Windows evidence](windows-validation.md) and [validation](validation.md).

The Linux `/proc` footprint example reports Linux-only measurements and fails explicitly elsewhere. OpenWrt CPU/ABI cross-compilation, SDK/musl linking, packages and actual router acceptance are separate from workstation support. No router deployment or public push is authorized by these tests.
