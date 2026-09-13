# Product requirements

OpenWrt MCP is an independent community project. It aims to give AI agents comprehensive OpenWrt management through MCP with explicit operator control. Rust is an implementation choice; correctness and measured cost take precedence over language-based performance claims.

## User goals and acceptance criteria

Architecture revision 2 adds a mandatory development constraint: architecture and directory ownership are designed before implementation. The executable harness checks that contract. Requirements that cannot fit it must first update the architecture, ADR and harness; neither production code nor feature convenience may bypass those controls. See ADR 0002 and docs/development.md.

| Goal | Concrete requirement | Verification |
| --- | --- | --- |
| Comprehensive control | Cover base services and installed packages with capability discovery, typed native adapters and reviewed extensions; publish a coverage matrix per OpenWrt version | Adapter contract tests plus emulator/device acceptance tests |
| Accurate and fast | Validate arguments, distinguish configuration from effective state, return structured bounded results, check postconditions after changes | Negative tests, fault injection, real-state verification and latency benchmarks |
| Low resource use | Single Rust process, on-demand work, bounded concurrency/output, minimal SDK features, no embedded model | Binary size, idle RSS/CPU, per-call p50/p95 measurements |
| Easy security settings | Readable category settings, read-only example, explicit execution flag, policy check command, deny wins | Permission matrix and bypass regression tests |
| Clear structure | Enforce core/features/runtime/adapters/mcp/server ownership and dependency direction; no direct device I/O in protocol handlers | Versioned contract, AST/metadata checks, negative fixtures, evolution gate and review |
| Configurable usage logs | Audit attempts, decisions and outcomes; JSON/text, stderr/file/syslog, size rotation and retention; no payloads/secrets | Rotation, failure and secret-leak tests |

## Meaning of full coverage

Architecture v6 requires typed bounded list/record responses rather than raw structured subtrees. Fixed field/type/presence declarations, finite two-level collections, private pre-I/O selector binding, whole-input identity validation and shared cardinality/serialized-byte budgets must be enforced. Missing/malformed/empty/unobserved results remain distinct; no coercion, first-match selection, truncation or unchecked fallback. Both backends reject duplicate JSON keys and bounded-parser violations before Value erases them. Runtime bounds normalized results before completion audit; MCP accounts for text and structured copies. Generic service metadata visibility is an explicit Services.Read scope, not guessed daemon categories. See ADR 0006 and collection-read-contracts.md. This checkpoint does not implement mutation or claim complete read coverage.

Architecture v5 requires fresh, bounded, same-target capability observations before invoking configured operations. Release/version strings, administrator UID and client-selected profiles are not compatibility proof. ACL-hidden or unobserved methods are unknown and fail closed; conflicting observed signatures are separately incompatible. Probe and invocation share dispatcher policy, start-before-I/O audit and deadlines. Operation capability metadata must match action and transmitted arguments. Keep check/catalog/tools-list offline and offer an explicitly authorized metadata-status use case. Private snapshots expire after 30 seconds or backend epoch invalidation; unverified extensions have no execution bypass. See ADR 0005 and the machine-readable probe/evidence contracts.

The initial target is the actual BPI-R4 installation recorded in reference-target.md, including installed component revisions rather than just the base release. Track every management family in implementation-plan.md and the coverage matrix. Separate source review, synthetic fixtures, native host tests, emulated userspace and exact-device acceptance; inventory observations alone do not validate all operations. New package/variant surfaces remain explicit gaps until reviewed adapters and appropriate acceptance exist.

Architecture v4 makes Windows/Linux/macOS first-class intended hosts for ALL shared MCP functionality. Host OS and target OpenWrt OS are separate concerns. Shared policy/catalog/protocol/crypto/use cases and remote router features must not depend on host-native router commands. Use explicit unconfigured/OpenWrt-local/remote target selection, OS-specific protected config/key/audit adapters, and a portable environment+stderr alternative without automatic fallback. Unknown/unimplemented platform capabilities fail explicitly. Native OS builds, required portable executable suites and separate OpenWrt acceptance are mandatory; WSL is Linux evidence only. See ADR 0004 and the native support matrix; no present claim of complete platform parity follows from this requirement.

Full coverage is a product target, not a v0.1 completion claim. OpenWrt packages add arbitrary services and commands, so a fixed finite tool list cannot guarantee every future feature. Maintain three separate states: built-in tested adapter; privileged custom action; unsupported/planned. A generic command mechanism never counts as tested feature coverage.

Categories: system, network, wireless, firewall, dhcp_dns, services, packages, storage, vpn, firmware, diagnostics, extensions. Operations can require multiple categories. The catalog is the source of truth for parameters, required permissions and exposed output. Unknown operations, missing grants and unknown config keys fail closed.

The next passive wireless increment follows [wireless-observation-contracts.md](wireless-observation-contracts.md) within architecture v6: reviewed station/country metadata, explicit station-identity disclosure under Wireless.Read, no active scan or mutation, exact local selection and unchanged shared bounds. Package inventory pagination remains a [separate proposed design](package-inventory-design.md), not an exception to the current projection or capability contract.

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

Architecture revision 3 selects age for archival backup encryption and introduces independent key-source, container-selection and encryption-provider ports (ADR 0003). Accept exact configured restricted files, explicitly named environment variables and keys within bounded archives; model user-unlocked OneDrive Personal Vault without automatic authentication or fallback. Separate public encryption recipients from private decryption identities. Windows protected-file/Vault support requires native ACL/handle validation and must remain explicitly unsupported until implemented and tested. Start with X25519 age and plain ZIP Stored/Deflate; other methods need reviewed adapters. Raw keys are never MCP inputs/outputs or logs. Cipher finalization and complete authentication precede backup publication/restore application. These primitives do not implement router backup/rollback transactions by themselves.

Use inspect -> plan -> encrypted pre-change backup -> authorize -> apply -> verify -> confirm/rollback. The device owns the rollback timer so losing a client/SSH connection does not prevent recovery. Protected resources must be checked after resolving actual objects and indirect dependencies, not string matching alone. Bulk calls and extensions do not bypass these checks. Firmware and package changes need dedicated workflows; a timer cannot make every action reversible.

## Performance targets (not yet measured on OpenWrt)

Initial targets on a declared aarch64 OpenWrt reference device: stripped binary <= 10 MiB; idle RSS <= 16 MiB; idle CPU < 1% of one core over 60 seconds; policy check p95 <= 100 microseconds at 1000 operations; dispatcher overhead p95 <= 2 ms excluding device I/O. Benchmarks must state CPU, OpenWrt release, Rust version, build flags, catalog size, input size and iteration count. Change targets only with recorded measurements and rationale.

Default call deadline 10 seconds, stdout/stderr cap 64 KiB each, two in-flight operations, bounded input frames. Mutations are not automatically retried. Initial implementation may use fixed CLI argv adapters; native ubus optimization must be justified by measurements without weakening boundaries.

## Delivery sequence

Architecture v7 acceptance: enumerate a complete bounded captured APK query response across immutable 16-record pages, including inventories larger than 256 records. Preserve exact names/versions/architectures and declared layer. Authorize and audit every page; no hidden truncation, client-selected command, generic execution exception or continuation recapture. Fail closed on unreviewed versions, invalid/oversized responses, missing entropy, stale/foreign cursor or epoch changes. A 120-second absolute snapshot lifetime and 4-MiB source/4,096-record/2-MiB retained-field ceilings bound resources. Declare APK-visible, non-atomic scope and whole-device completeness false on every response; opkg, hidden layers and package mutations are separate work. See ADR 0007 for the authoritative recipe, ownership, limits and portable negative/behavior suites.

1. v0.1 foundation: stdio MCP, policy engine, strict catalog, selected ubus reads, privileged fixed-action extensions, audit outputs, fake-device tests and local benchmarks. The initial three-crate split is historical.
2. Architecture-first prerequisite: migrate to the version-2 six-layer workspace plus xtask, enforce dependencies/source ownership/evolution and pass regression tests before adding device features. Then capability inventory, core read adapters, response schemas and OpenWrt emulator matrix.
3. Transactional UCI mutation, encrypted backup, durable device-side rollback and protected-resource rules.
4. Service, package, storage, VPN and firmware adapters with adapter-specific verification.
5. Optional authenticated remote MCP transport and OpenWrt package delivery; performance profiling on supported targets.

The first post-v2 feature increment follows [read-contracts.md](read-contracts.md): general logical-interface status, selected iwinfo radio data, standard logd/sysntpd instance state, and a fixed empty-argument watchdog query. It stays inside the existing feature/backend/projection contract; no new direct I/O path or dependency is permitted. These are fixture-validated reads, not the complete capability-inventory or device-acceptance milestone.

Every release must state implemented coverage and outstanding limitations. Production readiness requires real OpenWrt testing and security review; a successful host build does not establish either.

## Sources

- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [OpenWrt ubus](https://openwrt.org/docs/techref/ubus)
- [OpenWrt rpcd and ACLs](https://openwrt.org/docs/techref/rpcd)
- [OpenWrt UCI](https://openwrt.org/docs/guide-user/base-system/uci)
- [Cargo release profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
