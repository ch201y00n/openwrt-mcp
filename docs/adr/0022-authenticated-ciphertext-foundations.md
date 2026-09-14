# ADR 0022: Authenticated ciphertext foundations (P3)

Status: architecture checkpoint; validate and commit before implementation.

P2 did not admit native backup factories. Windows private logs lack durable
publication; macOS protected-file ACL validation is unavailable. Reusing either
as a private durable backup store would be an unsupported security claim.

Adopt the [ciphertext foundation contract](../ciphertext-foundations.md). Its
portable alternative is an authenticated, pre-provisioned record file: private
staging is RAM, persisted payloads are age ciphertext, and an independent HMAC
key authenticates provenance. Publication is atomic visibility of a complete
record through the store API, **not atomic filename rename**. File sync precedes
success. Operators must provision a durable filename in a controlled local
filesystem; startup neither creates nor repairs that authority. Missing/unknown
provisioning is unsupported. This is not the guardian journal or a general file API.

The separate 32-byte authentication key is necessary because anyone with an age
public recipient can produce valid ciphertext. Its custody uses existing KeySource,
not the archival/device recovery identity. It remains outside the store/repository.
HMAC/SHA256 dependencies belong only to crypto-age/provenance. Adapters receive an
authentication port; custody and encryption remain independent and replaceable.

Lower-level runtime/backups types are not Admission, ValidatedPlan or DurableBackup
capabilities and cannot authorize router work. P4 wraps them with P2 authenticated
admission and the same Dispatcher. P3 tests real bounded SSH capture, host storage,
crypto and inspection using synthetic data, not unguarded backup/mutation MCP tools.

Rejected alternatives: widening Windows ABI casually, calling POSIX modes macOS
ACL proof, substituting logs, accepting rename as durability, or adding a database
with an unbounded cache. Native private directory/rename stores remain a future
profile. The record alternative avoids normal namespace writes but adds explicit
operator provisioning and a separate custody secret.

No new crate, unsafe code, OS-dependent shared code, live key access, router change,
auto-deployment or permission bypass. Existing limits remain. Validate declaration,
namespace/dependency restrictions and negative cases before behavior; use existing
mandatory native composition and SSH suites for subsequent implementation evidence.
