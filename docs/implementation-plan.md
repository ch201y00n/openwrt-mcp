# Comprehensive management implementation plan

Status: active work, not a completion claim. The user requested continued implementation and validation until the project goals are met. The first reference is the actual BPI-R4 installation; no live router changes or remote publication are authorized by repository development.

## Completion model

For a recorded release/device/package/service inventory, every management surface must have explicit classification, typed input/output/effects, policy enforcement, capability prerequisites and evidence. Record read/configure/execute/verify/recover separately. A generic privileged command or a successful host build does not count as tested support. Unknown third-party surfaces remain explicit gaps until reviewed adapters are added. Version/variant/package changes must be detected and must not silently inherit old compatibility assertions.

The goals continue to include cross-platform Windows/Linux/macOS hosts, bounded performance/resource use, easy category access and independent execution controls, age encryption with abstract sources/containers, safe configurable audit logs and architecture-first development. Requirements that need new boundaries get an architecture-only validated checkpoint before implementation.

## Milestones

| Stage | Scope | State |
| --- | --- | --- |
| Foundation v2-v4 | Policy/dispatcher/MCP, ten initial conservative reads, age/key sources, explicit local/SSH targets, host platform separation, architecture harness | Implemented; historical milestone had 192 Linux-on-WSL tests; native-host and device acceptance incomplete |
| Reference inventory | Live safe software identity and management metadata, compared with official release/package source evidence | Initial selected metadata recorded; full family closure remains; see reference-target.md |
| v5 capability architecture | Typed bounded probes, operation prerequisites, target/auth-bound expiring observations, policy/availability/evidence separation, variant handling, harness rejection tests | Architecture-only full gate passed (208 tests, including four non-behavior scaffold checks); checkpoint precedes implementation |
| Capability implementation | Local/SSH probes on the same selected target, strict parsers, offline CLI preservation, existing tool gating, operator-visible safe support status | Implemented; historical v5 full gate passed with 260 distinct Linux-on-WSL tests; native-host acceptance remains |
| v6 bounded read architecture | Finite typed projections, bound local selectors, duplicate-rejecting response parser, global row/byte limits and five mandatory native suites | Architecture-only checkpoint eccd3e7 passed with 272 tests (51 harness, five declaration scaffolds), before functional work |
| First v6 typed read increment | network_interfaces, network_interface_status.v2, wireless_devices, service_status, service_status_list; shared decoder/projection/MCP result boundaries | Implemented; full Linux-on-WSL gate passes 347 tests; fourteen total reads, five typed contracts. See collection-read-contracts.md and validation.md |
| Native Windows common path | GNU native build, policy/age/key-source/SSH/MCP fixtures and actual binary stdio; no WSL substitution | Full native gate passes 315 distinct tests after a test-only SystemRoot fix; MSVC/macOS and protected file/Vault facilities remain pending. See windows-validation.md |
| Passive wireless observations | Station list/exact local selection and driver-reported countries; no active scan, mutation or raw configuration | Implemented within v6; 17 reads/eight typed contracts. Ten additional host/MCP regressions pass; no radio acceptance. See wireless-observation-contracts.md |
| Package inventory pagination | Bounded complete capture, explicit source scope and private expiring pages | Proposed only; source admission/layer/destination/consistency design must precede an architecture checkpoint. See package-inventory-design.md |
| Broader typed read coverage | Additional network/wireless/service surfaces plus packages, storage, DNS/DHCP, VPN, firewall and system/diagnostics | Planned; individual contracts/evidence required; existing bounded profiles do not prove coverage of a family |
| Isolated OpenWrt acceptance | Official 25.12.5 ARM64 QEMU userspace, then an opkg-family release and variant/negative fixtures | Current v6 actual MCP/SSH run passed twelve reads and two explicit unavailable cases; see emulator-validation-v6.md. Broader coverage and opkg-family acceptance remain |
| Mutation architecture | Device-owned transaction/recovery authority, encrypted streaming backups, guarded plans, protected-resource graph, secret references, explicit uncertainty | Proposed separately in management-workflows.md; not implemented |
| Typed mutation coverage | UCI, service lifecycle, package management, storage, VPN, DNS/firewall/network/wireless, credentials, firmware/recovery | Planned; no generic shortcut around transaction/authorization requirements |
| Full host facilities | Native Windows/macOS protected files/audit destinations, optional actual Vault integration, native OS acceptance | Planned; current unsupported facilities stay fail-closed |
| Completion/release assessment | Coverage gaps closed for recorded reference, variant handling verified, native-host evidence, resource measurements, security/license review | Not complete; external publication and live destructive tests require separate authority |

## Reference management families

System identity/time/NTP/LEDs/accounts/SSH/web management; interface/device/bridge/DSA/VLAN/address/route/protocol management; wireless radio/BSS/security/client/channel management; firewall zones/rules/NAT/sets/effective rules; DNS/DHCP/RA/leases; services/startup/cron; package inventories/repositories/signing/install/remove; storage/mount/swap/filesystems/shares; WireGuard and installed VPN providers; firmware/image provenance/backup/restore/reset/recovery; logs/processes/connectivity and active diagnostics; installed add-on services such as adblock, ACME, Avahi, Samba, nginx and Tailscale.

Optional OpenVPN/strongSwan/pbr/mwan3/SQM and other variants need detected packages and reviewed contracts; this list does not assert they are installed on the reference router. Active scans/ping, package lifecycle scripts and arbitrary startup scripts must not inherit a harmless read classification from a UI or upstream ACL label.

## Working safeguards

- Use synthetic keys and fake/isolated targets for development; never copy actual identities or Vault archives into the repository.
- Read-only reference queries may collect selected public software metadata, never raw network/wireless settings, credentials, QR data, MACs or decrypted backups.
- Preserve KT IPTV's lan3 and its indirect dependencies; no live network/service/package/firmware changes in this development work.
- Keep implementation, target availability, permission and validation status as separate facts.
- Run the full repository gate before commits; retain checkpoints and evidence. No public push or complete-goal claim while required work remains.
