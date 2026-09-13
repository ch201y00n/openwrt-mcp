# ADR 0018: validated supplied-stream sealing

Status: accepted for an architecture-only checkpoint before implementation.
Scope: application-owned orchestration over injected, pre-bound trusted ports;
synthetic integration only. No router capture command, real store, protected
staging, deployment, dispatcher consumer or new MCP authority is admitted.

## Why and ownership

The tar/gzip and age primitives alone cannot ensure a producer completed, that
encryption actually consumed the full input, or that a store published complete
ciphertext. Add `runtime::sealing` in `crates/runtime/src/sealing/` to own that
sequence, stream accounting and failure state. Keep implementation modules private.
Do not move source/cipher custody, codecs, filesystem access or policy into it.
All external production consumers and runtime siblings remain prohibited until
a separately reviewed authorized workflow adopts the ports.

The existing EncryptionSession implements a small supplied-stream SealCipher port
inside this module, using only the public-recipient session. No identity source,
decryption, raw key material, registry lookup or algorithm implementation is added.
The algorithm remains replaceable through a trusted compiled port, not a dynamic
client plugin. No new normal dependency, crate owner or OS branch is required.

Tests in runtime's existing mandatory protection suite use fake ports. Server's
existing protection-composition suite becomes mandatory on every native host and
may use device-codec as a **development-only** dependency for actual age + gzip
fixture composition. Codec namespace bans in production remain unchanged; no
production adapter is added merely to connect fixtures.

## Ports and state

The application owns fixed safe enums, bounded limits and count-only reports:

- `SealSource: Read`: a supplied binary source with `complete` returning its
  successful producer byte count and `cancel` returning a cleanup state. EOF is
  separate from producer completion. Implementers are responsible for actual
  producer exit/transport completion; a count is not source attestation.
- `SealCheck`: incremental `feed` and consuming boxed `finish`, returning supplied
  source/expanded/payload byte and regular-file counts after complete validation.
  No raw payload, path, inner-validator escape or early completion report.
- `SealCipher`: encrypt supplied Read/Write streams and return input/output counts.
  The existing EncryptionSession is the first bridge. Providers are trusted code;
  accounting cannot prove that an arbitrary dishonest provider encrypted data.
- `SealStage: Write`: already-bound private ciphertext staging, with declared
  private-staging/atomic-publication/durability capabilities, `publish` and `abort`.
  Require all three before source/key/cipher work. A capability is the adapter's
  contractual claim, not an independent OS proof. No fallback or retention action.
- `SealClock`: supplied monotonic milliseconds for deterministic deadline tests;
  normal calls use a private Instant-based clock. Backwards time fails closed.

Reports and errors contain only bounded counts and fixed states. Source/validator/
cipher/stage containers have no raw Debug or serde. The flow receives pre-bound
ports, not source paths, commands, keys, targets, manifests or client-selected store
locations. Real adapters must retain their artifact/correlation handles outside
the count-only report for future reconciliation.

Publication states distinguish NotAttempted, NotPublished, Published and Unknown.
Cleanup states distinguish NotRequired, Cleaned and Unknown. Provider errors never
carry paths, raw I/O text, source errors or backend diagnostics into these states.
`publish` explicitly returns Published, NotPublished or Unknown; an ambiguous
transport/error response cannot be relabeled NotPublished. A normal success result
requires Published before the deadline. An acknowledged late publication is an
error with state Published, not a claim that publication did not occur.

## Required sequence

1. Validate limits and required stage capabilities before reading source/key data.
2. Wrap the supplied source and stage with independent counters, bounded calls,
   monotonic deadline checks and a shared first-error latch.
3. Encrypt through the existing public-recipient session. Before any source bytes
   are returned to the cipher, feed them to the validator. The cipher can write
   only to private ciphertext staging; there is no plaintext staging port.
4. Require an actual nonempty-buffer EOF read, no latched stream failure, nonzero
   input/output and exact agreement with cipher-reported counts. Never drain an
   early-returning cipher's input afterward to pretend those bytes were encrypted.
5. Finish archive validation and compare its source byte count with independent
   input accounting; enforce finite summary counts. Independently require successful
   producer completion and its matching byte count. Flush staging and recheck the
   deadline before attempting publication.
6. Attempt publication once, passing only the validated count summary. Distinguish
   confirmed, absent and uncertain publication. Never retry publication or convert
   an uncertain/late result into permission for a later device change.

Every pre-publication failure cancels an unfinished source and aborts unpublished
staging once. Cleanup failures remain Unknown and must not erase the primary cause.
Confirmed producer completion needs no cancellation. A definitely NotPublished
result allows abort. Never abort/retry after Unknown publication or delete a known
published artifact, including one acknowledged after the deadline.

Use an RAII guard for best-effort cleanup on unwinding before publication, with
publication conservatively marked uncertain before invoking a publishing port.
Injected ports must not panic in cleanup. Process abort/power loss cannot be made
safe by Rust Drop, and synchronous port calls can block; no hard cancellation,
crash durability or guaranteed cleanup is inferred from these application rules.

## Budgets and memory

Use at most 64 KiB per delegated read/write. Default source/ciphertext ceilings are
64/65 MiB; hard caps are 72/73 MiB, with independent checked counters. At the exact
input cap, use only a one-byte zeroizing EOF probe; a byte beyond the cap rejects
without delivering it to the cipher or checker. Empty-buffer reads do not mark EOF.
Reject impossible delegated read/write counts, nonempty zero writes and all I/O
failures, including Interrupted, with a latched safe error. No full-stream
ReadToEnd/ReadToString, growing payload Vec or implicit copy convenience path.

Default cooperative deadline is 30 seconds, hard maximum 300 seconds. Check before
and after provided callbacks and stream operations, and after synchronous cipher /
validator work. Do not claim this interrupts a blocked callback or bounds its own
internal allocation. Publication acknowledged after expiry is separately reported
as known published; missing acknowledgement is Unknown regardless of elapsed time.
Cleanup is still attempted after expiry and can itself block under this sync port
contract. Future runtime workers require separate bounded lifecycle design.

The application retains only counters/control state and a one-byte probe, not full
source/ciphertext. Source/cipher/checker/store providers retain their own separately
bounded state. Gzip's backend history still lacks guaranteed scrubbing; age also
has internal buffers. Tests use generated synthetic identities and public fixture
content only. Sensitive real-workflow adoption still requires memory/swap/core-dump
review and actual platform protection evidence. This checkpoint does not satisfy it.

## Harness and acceptance

Commit exact requirements, this ADR, machine profile and negative regressions after
both full host gates, before implementing the module or actual development edge.
The harness confines ownership, no-I/O/key/decryption/policy/serde dependencies,
external/sibling namespace bans, exact limits, required native suites and the sole
server development-only codec edge. Old-version fixtures retain previous rules.

Behavioral tests must exercise order, empty/early cipher return, source vs EOF
completion, wrong counts, exact input/output ceilings and overflow probes, short
writes, impossible counts, latched/ignored errors, validator failures, failed key /
cipher/flush/source completion, cleanup uncertainty, missing stage capabilities,
every deadline boundary, time reversal, publication uncertainty and late ack,
no retry or abort-after-publication and panic-path best-effort cleanup. No ignored
or OS-skipped shared cases. Real age + strict gzip + memory-backed store-model tests
must verify decrypt-to-staging round trips and rejected truncation/corruption without
ever publishing failed synthetic input. They are not actual store durability tests.

## Explicit non-goals

This seal result is not a complete router-backup receipt, authenticated provenance,
trusted/fresh manifest discovery, a restore test or mutation authority. It does not
bind a live target/boot/plan, audit an MCP invocation, open a Vault, implement native
storage, use the JSON backend for binary data or arm a device guardian. These
remain requirements of [management-workflows](../management-workflows.md) and must
be accepted and tested separately before their production consumers are enabled.
