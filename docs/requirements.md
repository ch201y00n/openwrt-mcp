# Product requirements

OpenWrt MCP is an independent community project. It aims to give AI agents comprehensive OpenWrt management through MCP with explicit operator control. Rust is an implementation choice; correctness and measured cost take precedence over language-based performance claims.

## User goals and acceptance criteria

| Goal | Concrete requirement | Verification |
| --- | --- | --- |
| Comprehensive control | Cover base services and installed packages with capability discovery, typed native adapters and reviewed extensions; publish a coverage matrix per OpenWrt version | Adapter contract tests plus emulator/device acceptance tests |
| Accurate and fast | Validate arguments, distinguish configuration from effective state, return structured bounded results, check postconditions after changes | Negative tests, fault injection, real-state verification and latency benchmarks |
| Low resource use | Single Rust process, on-demand work, bounded concurrency/output, minimal SDK features, no embedded model | Binary size, idle RSS/CPU, per-call p50/p95 measurements |
| Easy security settings | Readable category settings, read-only example, explicit execution flag, policy check command, deny wins | Permission matrix and bypass regression tests |
| Clear structure | Enforce core/runtime/server dependencies; no direct device I/O in protocol handlers | Automated architecture checks and review |
| Configurable usage logs | Audit attempts, decisions and outcomes; JSON/text, stderr/file/syslog, size rotation and retention; no payloads/secrets | Rotation, failure and secret-leak tests |

## Meaning of full coverage

Full coverage is a product target, not a v0.1 completion claim. OpenWrt packages add arbitrary services and commands, so a fixed finite tool list cannot guarantee every future feature. Maintain three separate states: built-in tested adapter; privileged custom action; unsupported/planned. A generic command mechanism never counts as tested feature coverage.

Categories: system, network, wireless, firewall, dhcp_dns, services, packages, storage, vpn, firmware, diagnostics, extensions. Operations can require multiple categories. The catalog is the source of truth for parameters, required permissions and exposed output. Unknown operations, missing grants and unknown config keys fail closed.

## Security invariants

- Permission metadata is operator-owned and immutable during a client session.
- Read permission does not imply execution. Read_write does not imply execution.
- Any execute grant is ineffective when category access is deny.
- Tool discovery and invocation use the same policy; hiding a tool is not enforcement by itself.
- Generic ubus, UCI, file, service and package paths cannot be allowed to bypass more specific category restrictions.
- Device outputs are untrusted data, not instructions. Raw UCI network/wireless configuration, private keys, passwords and QR credentials are never emitted by built-in tools or logs.
- No AI operation may edit this server's policy, action definitions, authentication or audit configuration.
- Encryption identities and decrypted backups must stay outside source repositories in restricted storage. Future backups persist only ciphertext.
- Category ACLs are not an OS sandbox. A principal allowed to install privileged custom actions or alter the daemon's files has operator authority.

## Mutation design target

Use inspect -> plan -> encrypted pre-change backup -> authorize -> apply -> verify -> confirm/rollback. The device owns the rollback timer so losing a client/SSH connection does not prevent recovery. Protected resources must be checked after resolving actual objects and indirect dependencies, not string matching alone. Bulk calls and extensions do not bypass these checks. Firmware and package changes need dedicated workflows; a timer cannot make every action reversible.

## Performance targets (not yet measured on OpenWrt)

Initial targets on a declared aarch64 OpenWrt reference device: stripped binary <= 10 MiB; idle RSS <= 16 MiB; idle CPU < 1% of one core over 60 seconds; policy check p95 <= 100 microseconds at 1000 operations; dispatcher overhead p95 <= 2 ms excluding device I/O. Benchmarks must state CPU, OpenWrt release, Rust version, build flags, catalog size, input size and iteration count. Change targets only with recorded measurements and rationale.

Default call deadline 10 seconds, stdout/stderr cap 64 KiB each, two in-flight operations, bounded input frames. Mutations are not automatically retried. Initial implementation may use fixed CLI argv adapters; native ubus optimization must be justified by measurements without weakening boundaries.

## Delivery sequence

1. v0.1 foundation: three-crate architecture, stdio MCP, policy engine, strict catalog, structured selected ubus reads, privileged fixed-action extensions, audit outputs, fake-device tests and local benchmarks.
2. Native capability inventory, core read adapters, response schemas and OpenWrt emulator matrix.
3. Transactional UCI mutation, encrypted backup, durable device-side rollback and protected-resource rules.
4. Service, package, storage, VPN and firmware adapters with adapter-specific verification.
5. Optional authenticated remote MCP transport and OpenWrt package delivery; performance profiling on supported targets.

Every release must state implemented coverage and outstanding limitations. Production readiness requires real OpenWrt testing and security review; a successful host build does not establish either.

## Sources

- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [OpenWrt ubus](https://openwrt.org/docs/techref/ubus)
- [OpenWrt rpcd and ACLs](https://openwrt.org/docs/techref/rpcd)
- [OpenWrt UCI](https://openwrt.org/docs/guide-user/base-system/uci)
- [Cargo release profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
