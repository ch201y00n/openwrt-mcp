# Product requirements

Architecture v16 requires bounded incremental uncompressed archive validation against every supplied expected path and size, with strict tar framing, no links/extensions/special entries, complete zero tail, no ignored suffix, safe paths and latched failure. Owned header/path memory is zeroizing; payloads are not retained. Count-only structural completion does not prove provenance, full scope, source success or safe extraction. Keep this codec outside production consumers until a separate integration checkpoint; no backup/restore tool is introduced. See ADR 0016.

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

Architecture v15 first adds a pure bounded protected-resource effect predicate, not a mutation tool. Analyze directed influence across both before/after graphs, including removed/new edges, bulk roots, reached unknown effects and explicit global scope. Missing protected identities, uncertain reachability or exceeded budgets deny; a count-only NoKnownProtectedImpact result is not authorization or proof of actual topology. Keep the private validated model in core::management, outside every production consumer until separately reviewed integration. No serde/JSON, action or policy escape. Native core security tests and architecture-only checkpoint precede behavior; see ADR 0015. Device effect extraction, fresh state binding, backup, guardian recovery and dispatcher integration remain separate requirements.

Architecture v14 requires an explicit opkg root-status observation alongside APK without automatic selection, fallback or merged completeness. Read only a fixed reviewed status file after an exact version option that exits before initialization; ordinary opkg list/status commands have initialization writes and are not admitted under Packages.Read. Return selected name/version/arch/status with truthful file-only, non-atomic scope. Preserve one bounded shared snapshot and check its finite manager profile. No raw database/excluded fields or arbitrary file paths. Architecture/harness checkpoint precedes implementation; see ADR 0014.

Architecture v13 extends closed configuration observation to reviewed timeserver, device/bridge-VLAN, IPv4/IPv6 route/rule, firewall zone/forwarding/rule/redirect/NAT, DHCP pool/host/domain/CNAME and fstab global/swap sections. The exact eighteen additions retain fixed config/type/get, category Read, guarded typed output, unconditional custom UCI denial, current budgets and sessionless shared-delta scope. Selected identifiers/topology are explicitly disclosed; credentials, arbitrary extra options/commands and effective-state claims stay excluded. These reads do not prove a complete protected-resource dependency graph or authorize mutations. Validate the separate architecture/harness checkpoint before implementation; see ADR 0013 and base-uci-observations.md.

Architecture v12 requires bounded faithful UCI text/list observation without arbitrary JSON. Preserve missing, empty string, empty list and list ordering/duplicates distinctly through a nested terminal TextOption form with fixed kind/values output. No splitting, joining, coercion, caller selection, root use or secret-containing arbitrary options. Charge existing shared row/item/byte budgets and keep current closed UCI recipes/categories/guards. Exact selected fields and new response versions require feature contracts; architecture/harness must pass a separate checkpoint before implementation. See ADR 0012.

Architecture v11 requires closed non-secret UCI configuration observations, not arbitrary configuration access. Admit only six fixed config/type get recipes with their exact read category; reject all custom UCI operations and all other UCI methods/arguments before I/O. Typed section maps must reject root errors and wrong section types via bounded exact TextEnum values. Preserve scalar text and omitted options without converting them into effective state; lists need separate reviewed support. Clearly identify the sessionless shared-delta non-atomic source. Add exact uci introspection only after these safeguards are designed; preserve dispatcher/cipher/custody/host boundaries. See ADR 0011 and the architecture-only checkpoint requirement.

Architecture v10 requires faithful ordered observation rows where upstream cannot provide unique resource identity, a finite false-or-integer sentinel without coercion, and bounded fixed root absence guards before projection. Preserve DHCP row multiplicity, false versus zero expiry, optional client metadata and nested address budgets. Explicit mixed error/success payloads must fail the guarded contract without raw error output. Do not infer connected clients, full lease visibility or DNS state. These additions stay inside portable core/feature/runtime/codec/MCP ownership and require an architecture-only checkpoint before behavior; see ADR 0010.

Architecture v9 admits closed LuCI introspection for reviewed storage/lease families, not arbitrary rpcd calls or automatic tool generation. Preserve v5 authorization/audit/freshness and v6 bounded typed output. Storage observations must state their LuCI-visible non-atomic scope, possible omissions and read-side device activity. DHCP scalar unions/composite identities require a further reviewed contract rather than guessed uniqueness or coercion. See ADR 0009; this checkpoint adds no management coverage by itself.

Architecture v6 requires typed bounded list/record responses rather than raw structured subtrees. Fixed field/type/presence declarations, finite two-level collections, private pre-I/O selector binding, whole-input identity validation and shared cardinality/serialized-byte budgets must be enforced. Missing/malformed/empty/unobserved results remain distinct; no coercion, first-match selection, truncation or unchecked fallback. Both backends reject duplicate JSON keys and bounded-parser violations before Value erases them. Runtime bounds normalized results before completion audit; MCP accounts for text and structured copies. Generic service metadata visibility is an explicit Services.Read scope, not guessed daemon categories. See ADR 0006 and collection-read-contracts.md. This checkpoint does not implement mutation or claim complete read coverage.

Architecture v5 requires fresh, bounded, same-target capability observations before invoking configured operations. Release/version strings, administrator UID and client-selected profiles are not compatibility proof. ACL-hidden or unobserved methods are unknown and fail closed; conflicting observed signatures are separately incompatible. Probe and invocation share dispatcher policy, start-before-I/O audit and deadlines. Operation capability metadata must match action and transmitted arguments. Keep check/catalog/tools-list offline and offer an explicitly authorized metadata-status use case. Private snapshots expire after 30 seconds or backend epoch invalidation; unverified extensions have no execution bypass. See ADR 0005 and the machine-readable probe/evidence contracts.

The initial target is the actual BPI-R4 installation recorded in reference-target.md, including installed component revisions rather than just the base release. Track every management family in implementation-plan.md and the coverage matrix. Separate source review, synthetic fixtures, native host tests, emulated userspace and exact-device acceptance; inventory observations alone do not validate all operations. New package/variant surfaces remain explicit gaps until reviewed adapters and appropriate acceptance exist.

Architecture v4 makes Windows/Linux/macOS first-class intended hosts for ALL shared MCP functionality. Host OS and target OpenWrt OS are separate concerns. Shared policy/catalog/protocol/crypto/use cases and remote router features must not depend on host-native router commands. Use explicit unconfigured/OpenWrt-local/remote target selection, OS-specific protected config/key/audit adapters, and a portable environment+stderr alternative without automatic fallback. Unknown/unimplemented platform capabilities fail explicitly. Native OS builds, required portable executable suites and separate OpenWrt acceptance are mandatory; WSL is Linux evidence only. See ADR 0004 and the native support matrix; no present claim of complete platform parity follows from this requirement.

Full coverage is a product target, not a v0.1 completion claim. OpenWrt packages add arbitrary services and commands, so a fixed finite tool list cannot guarantee every future feature. Maintain three separate states: built-in tested adapter; privileged custom action; unsupported/planned. A generic command mechanism never counts as tested feature coverage.

Categories: system, network, wireless, firewall, dhcp_dns, services, packages, storage, vpn, firmware, diagnostics, extensions. Operations can require multiple categories. The catalog is the source of truth for parameters, required permissions and exposed output. Unknown operations, missing grants and unknown config keys fail closed.

The next passive wireless increment follows [wireless-observation-contracts.md](wireless-observation-contracts.md) within architecture v6: reviewed station/country metadata, explicit station-identity disclosure under Wireless.Read, no active scan or mutation, exact local selection and unchanged shared bounds. Package inventory pagination remains a [separate proposed design](package-inventory-design.md), not an exception to the current projection or capability contract.

## Security invariants

Architecture v8 requires Windows protected config/secret reads on a reviewed local NTFS profile. Validate every opened ancestor and leaf, actual owner/DACL, volume and reparse state; hold handles through reading. Reject untrusted write grants, and for secrets untrusted data/execute grants. Never interpret readonly attributes as ACL proof, repair operator permissions, unlock Vault, follow a failed profile with an environment fallback, or expose raw Windows errors. Native tests use only synthetic private fixtures. The approved unsafe SDK bridge is confined by the harness to one private host-platform file; shared layers remain safe and portable. See ADR 0008 for exact bounds and intentionally unsupported paths.

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
