# Native Windows validation

Date: 2026-09-13. This is local Windows GNU host/fixture acceptance, not WSL, MSVC, macOS, OpenWrt deployment or physical-router acceptance.

## Executed gate and scope

The subsequent **v7 package observation full gate** passes **355 distinct native Windows GNU tests**, zero ignored, including 54 harness regressions counted once, strict Clippy and release compilation. Source baseline is architecture-only `0521c7cf066958f7cea79817f6a4ca657c5db87e` plus v7 implementation. New tests include actual native OS-entropy generation, all five package suites, synthetic loopback SSH fixed-command capture and existing real-binary tests. This remains host/fixture acceptance, not execution against a router from Windows. The earlier artifacts and timestamps below retain their original scopes; no new RSS/CPU claim is made.

Subsequent passive wireless increment: the same native full gate passed at **2026-09-13T06:30:30Z–06:31:19Z**, with **325 distinct tests**, zero ignored, after adding seven feature and three MCP regressions. Source baseline `979029803271c81386118699b2669e372604ef15` plus the wireless changes; seventeen reads/eight typed contracts. This extends synthetic host acceptance only, not real-radio or emulator evidence. The original 315-test result and artifact below remain historical and are not silently relabeled as this build. See [overall validation](validation.md).

The unmodified `tools/Test-Repository.ps1` native branch passed architecture/evolution checks, all 51 harness regressions, formatting, strict all-target Clippy, workspace tests and release compilation. The recorded evidence run was **2026-09-13T06:08:47Z–06:09:03Z**, following an earlier successful full run. **315 distinct tests** passed with zero ignored; the harness is executed twice by the gate and counted once. Existing Linux-only local-process, protected-file and audit fixtures are not Windows tests. All required portable suites execute on this host, including all five actual-binary stdio cases and eighteen loopback SSH integration cases.

The production baseline is commit `8f6c09587cf34d627522b7da31c97df461e175c0`, plus the test-only environment fix described below. No production code, dependency, architecture contract, CI requirement or repository gate was changed to obtain this result. A separate Linux-on-WSL full gate also passed after the fix; its 347-test inventory is separate, not added to the native count.

The native tests cover the common environment-configuration/key-source and stderr-audit path, policy, age and bounded ZIP primitives, actual MCP pipes, pinned SSH against synthetic loopback endpoints, capability gating and bounded responses. Those endpoints do not run router commands. Windows protected files, rotating audit files, system-log delivery and Personal Vault remain explicitly unsupported. A portable implementation or this GNU run does not establish MSVC/macOS acceptance or execution of the configured GitHub CI jobs.

## Test-environment defect and regression

The first native gate failed two real-binary SSH tests. Their hermetic child launcher used `env_clear()`, which also removed `SystemRoot`. A small native Rust loopback reproduction observed successful TCP connections with the inherited environment, Winsock error **10106** with an empty environment, and success with only `SystemRoot` retained. Microsoft identifies 10106 as [service-provider initialization failure](https://learn.microsoft.com/en-us/windows/win32/winsock/windows-sockets-error-codes-2); the environment dependency is evidence from this host's reproduction, not a claim that the error has only one cause.

The fixture now retains just an existing `SystemRoot`, before setting explicitly supplied synthetic fixture variables. It still clears ambient PATH, configuration and keys. The previously failing actual-binary cases now pass within the full gate; no test was ignored or OS-disabled, and no production SSH fallback was added. The initial failure record was retained separately from successful transcripts.

## Native toolchain and artifact

- Windows 11 Professional x64, build 26200; PowerShell 7.6.6. The Windows API reports version 10.0.26200.
- Rust 1.95.0 (`59807616e1fa2540724bfbac14d7976d7e4a3860`), Cargo 1.95.0 (`f2d3ce0bd7f24a49f8f72d9000448f8838c4e850`), host/target `x86_64-pc-windows-gnu`.
- GCC 15.2.0, POSIX-SEH-MSVCRT, rt_v13-rev1; actual C compilation and linking, including the SSH crypto dependency.
- Release executable: **4,194,304 bytes (4.00 MiB)**; SHA-256 `0ffd87e3f17b3f1e45b4edc9880439e320cdaee2db94276ae0a31ad662cee8fe`. This is a local validation artifact, not a published release; native RSS, CPU and active-work latency were not measured.

The toolchain, caches, outputs and synthetic test scratch space were isolated in an ACL-restricted, non-synced temporary directory outside both repositories. Environment changes applied only to validation child processes; no global Rust installation, system PATH, Visual Studio components or registry settings were modified. Existing Visual Studio lacked the C++/SDK components needed for an MSVC run, so that target remains pending.

Archive sizes and SHA-256 digests were checked before extraction. The six Rust components came from the [official 2026-04-16 distribution](https://static.rust-lang.org/dist/2026-04-16/channel-rust-1.95.0.toml), with filenames `<component>-1.95.0-x86_64-pc-windows-gnu.tar.xz`. GCC came from the [MinGW-w64 toolchain provider's release](https://github.com/niXman/mingw-builds-binaries/releases/tag/15.2.0-rt_v13-rev1), file `x86_64-15.2.0-release-posix-seh-msvcrt-rt_v13-rev1.7z`.

| Component | Archive bytes | SHA-256 |
| --- | ---: | --- |
| rustc | 100389960 | `735c4b5c1459f82a8a1a5251f5899a958c4cf0d92ed3c992691cfff083d22623` |
| cargo | 11744584 | `32afca18643ed650250297614de7e13b2e03eae0ecde6fe250f96b795e6b63e0` |
| rust-std | 26327096 | `f57e045016a04130125fb43295d95f9ad2bebc296150eadb031dbf5167ad12bd` |
| rust-mingw | 5473684 | `ac0b67b7ee8a35045c26b8e39a524f171b2dc4a18838bb15844308e4442802f2` |
| rustfmt | 2910384 | `10adca16ab047fade3d39d594fe0b212e40e1edcea40d9acc8d153e56d2ad111` |
| clippy | 5129080 | `75a28d08651024743d5348e0890b2c0acbc2e7c55f6712394db13f6b8f47b7f9` |
| GCC/MinGW-w64 | 103355261 | `cd21f5b1fd07ca87aa02c6c48537eb8791a441393ca03f1b88ebce53987512c1` |

Toolchain archives total 255,330,049 bytes (243.50 MiB), within the bounded 300 MiB setup allowance. The separate Cargo cache contains 267 downloaded crate archives totaling 29,184,502 bytes; expanded files and build outputs consume additional disk space. These are observed setup sizes, not server runtime requirements.

No real key, Vault archive, router configuration or decrypted backup was used. No remote publication or live-router change occurred. See [platform support](platform-support.md) and [overall validation](validation.md) for remaining acceptance work.
