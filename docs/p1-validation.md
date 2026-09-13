# P1 verification record

Scope: repository-only P1/v20 increment after `161836f`; no router, Vault, key,
deployment or external account configuration changes. P1 is not full management
or production acceptance. The baseline is 51 read tools and 24 UCI profiles;
this increment has 55 read tools, 28 UCI profiles and 44 typed response contracts.

## Work and evidence

- Four closed system-service UCI definitions follow architecture-only `a711593`.
  Exact independent field fixtures, core recipe/category tests and four actual
  MCP/dispatcher tests cover commands/credentials exclusion, optional/empty/wrong
  values, row/string/list/shared limits, immutable actions, one same-target probe,
  discovery/direct-call isolation and argument/result-free audit.
- The new exclusion test was run before implementation and failed because the
  planned tool was absent. It passed after the definitions were implemented.
- Management inventory: 3 historical profiles, 31 feature mappings for 55 tools,
  20 retained package groups, 14 object names, 7 method groups and 9 unknown gaps.
  Strict development tests compare actual catalog, feature specification and
  reference records; negative cases reject omissions, fabricated mappings,
  unsafe/missing evidence, schema changes and unsupported completeness claims.
- First-mutation readiness records missing committed/boot/drift/effect/recovery
  evidence and the D1/D2/D3 decisions needed for P2. No mutation port is enabled.

## Verification status

Verified: 2026-09-14 (Asia/Seoul). P1's recorded-surface/read-contract completion
criteria pass; CTL/PKG and other feature families are not marked wholly complete.

| Host execution | Toolchain | Full workspace results |
| --- | --- | --- |
| Native Windows 11 x86_64 GNU | Rust/Cargo 1.95.0, GCC 15.2.0 | 581 passed, zero failed/ignored |
| Linux x86_64 on WSL2 | Rust 1.97.1, Cargo 1.97.0 | 585 passed, zero failed/ignored |

Both execute `tools/Test-Repository.ps1` (Linux explicitly uses `-UseWsl`):
architecture v20/evolution and 12 crate boundaries, 110 harness regressions,
formatting, strict all-target Clippy, workspace tests and release builds. The
110 harness cases are counted once within each workspace total, not counted
again for the gate's preliminary harness run. Ten tests were added beyond v20's
baseline: two independent field/document tests, four MCP tests and four inventory
tests; existing core/feature/MCP cases also exercise the additional recipes.

Both release executables passed offline `check` and exact catalog comparison
against the specification's 55 names, using the repository's synthetic read-only
example through a child-process environment variable and no configured target.
Changed document links and Git whitespace checks pass. The native Cargo cache
remains 267 downloaded archives; no dependency or lockfile change was introduced.

| Local release artifact | Bytes | SHA-256 |
| --- | --- | --- |
| Windows GNU | 4,417,024 | `65a2d4b2fe287e526c276eb7f2f84f5d21874d951f6f7b0eac26be4e981fe506` |
| Linux on WSL | 4,897,904 | `f108f016d75029eab221cd39ce227d996e8c2a43cb9fea49af06800c3fc55ef6` |

These artifact identities are not OpenWrt builds or resource-performance results.

## Failures resolved and remote CI scope

The first full P1 gate exposed the example policy test's stale 51-tool expectation;
it was updated to 55 and both full gates rerun. An inventory negative test initially
indexed a missing TOML field; the fixture now explicitly inserts that invalid field.
Neither failure was skipped or bypassed.

The previous published commit `161836f` had a failed
[three-host run](https://github.com/ch201y00n/openwrt-mcp/actions/runs/34770585714).
Its Linux/macOS and Windows logs identify Rust 1.98's new
`clippy::chunks_exact_to_as_chunks` lint, not a P1 runtime failure. All five constant
chunk iterators in the existing fixtures and Windows descriptor/SID helpers now
use equivalently bounded array chunks. The change preserves the 1.95 minimum
Rust version, security checks and strict lint gate; no warning suppression,
dependency change, platform exclusion or architecture exception was added.

This is the **pre-push local verification record**. Latest-stable MSVC/macOS/Linux
CI is separate: inspect the exact pushed commit in the
[native-host workflow](https://github.com/ch201y00n/openwrt-mcp/actions/workflows/ci.yml).
Local GNU/WSL passes do not assert that a later remote run passed. The existing
Linux `proc-macro-error2` future-compatibility warning remains visible.

## Limits

Evidence is synthetic host execution and existing separately scoped historical
profiles, not new emulator/hardware acceptance. No live inventory refresh, actual
backup/restore, guardian, mutation, complete package/API/driver auto-discovery,
global drift invalidation or whole-feature completion is claimed. Remote native
CI and physical BPI-R4 acceptance are separate evidence scopes. No new resource measurements are
inferred from the language, fixture count or successful build.
