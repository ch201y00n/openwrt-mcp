# P2 validation — guarded mutation architecture

Status: local full repository gates passed on 2026-09-15 (Asia/Seoul);
no production mutation acceptance claimed.
Baseline: P1 `71e10deaa4a44f02695c3873ebf6d8cf365068f1`.
Architecture: v21 / [ADR 0021](adr/0021-guarded-mutation-boundaries.md).

P2 changes documentation, the machine contract and the development-only xtask
harness/tests. Production Rust, Cargo dependency manifests/lockfile and the 55
management read tools are unchanged. No router, real key/Vault or backup access.

The declaration gate freezes the first intent/effect profile, 6 port signatures,
12 states/21 guarded transitions, requirement aggregation, recovery authority,
numeric budgets and exact module consumers. Negative tests mutate every contract
field and each array entry; source fixtures test unauthorized siblings, aliases,
macro tokens, old versions, private serialization and port/action escapes. Real
inert Cargo/Git fixtures exercise the assembled gate rather than just helpers.

These tests do not execute mutation, ciphertext-store publication, device guardian,
reboot recovery or a real deployment. Host/guardian resource numbers remain targets.
P3 must supply real synthetic adapter/worker/storage/authentication evidence; P4
must supply complete isolated OpenWrt workflow and failure/recovery evidence.

## Executed local evidence

| Environment | Full gate | Workspace tests (including harness) |
| --- | --- | --- |
| Native Windows 11 x64, Rust/Cargo 1.95.0 GNU, GCC 15.2 | PASS | 590 passed, 0 failed, 0 ignored |
| Linux on WSL, Rust 1.97.1 / Cargo 1.97.0 | PASS | 594 passed, 0 failed, 0 ignored |

Both ran `tools/Test-Repository.ps1` (Linux explicitly with `-UseWsl`): v21
architecture/evolution, 119 architecture regression cases, formatting, strict
workspace/all-target Clippy, workspace tests and release compilation. The first
harness run is repeated inside workspace testing; do not count those 119 twice.
Historical v11/v12 fixture downgrades were updated to remove the v21-only table;
no old restriction or failing check was bypassed. WSL is not Windows evidence.
The existing Linux proc-macro-error2 2.0.1 future-incompatibility warning remains
visible; it is not a test failure and has not been suppressed.

Native offline `check` and `catalog` with the repository's read-only observability
example returned exactly 55 operations without constructing a device target.
The Windows release executable is unchanged from P1: 4,417,024 bytes, SHA256
`65a2d4b2fe287e526c276eb7f2f84f5d21874d951f6f7b0eac26be4e981fe506`.
Production Rust and Cargo manifests/lockfile have no diff from the P1 baseline.
Local links in the 12 changed Markdown files were checked.

## Native CI and publication

The checkpoint is pushed only after these local gates. The unchanged
[three-native-host workflow](https://github.com/ch201y00n/openwrt-mcp/actions/workflows/ci.yml)
then validates its exact commit on Linux, macOS and Windows MSVC before main
integration. Consult that commit's run for native CI status; this document's
local results do not pre-claim a pending CI result or replace native macOS/MSVC
evidence. No release or router deployment is implied by a branch push/merge.
