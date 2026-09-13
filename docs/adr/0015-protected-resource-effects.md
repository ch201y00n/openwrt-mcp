# ADR 0015: bounded protected-resource effect analysis

Status: accepted for an architecture-only checkpoint before implementation.
Scope: one pure domain predicate, not a mutation API or a deployed protection
mechanism. The remaining transaction proposal in management-workflows.md stays
proposed. This decision does not authorize live router changes.

## Motivation and boundary

Safe mutations need more than checking whether an input contains "lan3". A bridge,
VLAN, route, zone or service change can influence protected connectivity indirectly.
Removed/renamed relationships and bulk plans must be considered too. Implement the
bounded graph predicate first, independently of topology discovery, authorization,
backup, guardian recovery and actual execution. A clear result is explicitly not
permission to mutate and not proof that the supplied topology matches a device.

Use the existing pure core owner and conventional module directory
`crates/core/src/management/`, with deliberate public types under
`core::management` and private representation/algorithm modules. No new crate,
dependency, device command, capability probe, action variant, MCP tool, background
task, key access or native API is admitted. All production consumers currently
remain forbidden from importing this namespace; later dispatcher integration
requires a separate architecture checkpoint. Integration tests may exercise it.

The harness checks the exact v15 declaration, mandatory native core security suite,
consumer namespace bans and producer re-exports. Management production modules
cannot import serde/serde_json, use JSON macros or acquire existing core action or
policy APIs. The existing pure/core and portable-source restrictions remain.
No client deserialization, raw graph response or graph-derived authorization token
is introduced. Semantic correctness is verified by executable cases, not inferred
from the static scanner.

## Canonical identities and supplied knowledge

A resource has a finite typed kind and exact stable identifier. Initial kinds:
PhysicalPort, NetworkDevice, Bridge, Vlan, Interface, Routing, Firewall, DhcpDns,
Multicast, Service, Storage, Vpn, System, Packages and Firmware. A kind is a domain
label, not a permission category or claim that an adapter exists.

Identifiers contain 1–128 ASCII bytes from letters, digits, underscore, hyphen,
period, slash, colon and at-sign. Equality is exact and case-sensitive; kind is part
of identity. Do not trim, lowercase, normalize paths, split strings or infer aliases.
A later trusted resolver must canonicalize real aliases and make current identity
stable. These identifiers are opaque labels, never filesystem paths or commands.

Each immutable graph contains unique typed nodes and directed influence edges:
A -> B means that changing A can affect B. Edges may form cycles or self-loops.
Duplicate nodes, duplicate edges and dangling endpoints fail construction. Empty
graphs and edge lists are structurally valid; analysis still requires protected
identities in both graphs. A node
declares its outgoing effect knowledge as Complete or Unknown. "Complete" describes
the caller's supplied model only; it is not attestation or automatically assigned
from empty adjacency. A topology adapter must not silently turn unobserved effects
into Complete. Unrelated Unknown nodes need not block an independent change.

Graphs and protected sets keep private validated storage. Do not derive raw Debug
or serialization for graph/identity/edge containers; safe failures contain fixed
codes without labels, paths or input excerpts. No credential material belongs in
this model.

## Analysis contract

Inputs are the before graph, proposed-after graph, a nonempty explicitly bound
protected set and a declared change scope. Local scope has nonempty unique roots;
global scope is explicit and cannot be converted into an empty local list.
Protected identities must exist in both graphs. Missing protected identity denies
analysis rather than treating it as an empty disconnected resource.

Analyze the union of both graphs' nodes and influence edges. An edge that is
removed still counts; a newly added edge also counts. Knowledge is Unknown when
either supplied occurrence of that node is Unknown. A node present only before
(removal) or after (addition) remains in the union. Local roots must exist in the
union. Cross-snapshot paths may conservatively combine old/new edges even if that
exact chain is not present simultaneously; this can deny extra cases, never admit
an omitted influence.

Traverse the entire reachable influence closure iteratively with bounded visited
state, never recursive depth or repeated path enumeration. If any reachable node
is protected, return ProtectedImpact. If reachable effects are Unknown, return
IncompleteEffects. Do not return a partial clear result at a truncation/budget
boundary. Global effects always deny under this nonempty protection profile.

For reproducible safe diagnostics, validate all inputs/union limits first; after
traversal ProtectedImpact takes precedence over IncompleteEffects. A successful
result is named NoKnownProtectedImpact and contains only the affected resource
count. It is neither an authorization decision nor a list of potentially sensitive
topology identities. No graph-only result can bypass permissions, freshness,
exclusive admission, encrypted backup, armed recovery or postcondition validation.

## Hard resource ceilings

Each graph and the combined union admit at most 4,096 unique nodes and 8,192
unique directed edges. Duplicate edges across the two valid snapshots coalesce;
duplicates inside either snapshot reject. Local changes have at most 256 roots;
protection has at most 128 identities; both selection sets must be nonempty and duplicate-free.
Identity text is bounded before copying. APIs validate slice lengths before
building bounded internal collections. Additional allocations for maps, indices,
edges and the two supplied graphs are not included in a text-byte budget; bounds
are cardinality limits, not an asserted RSS measurement.

The algorithm must remain deterministic and cycle-safe. No source silently
truncates to these limits, and callers cannot raise them through MCP arguments.
Existing read, package, cryptographic and MCP result limits are unchanged.

## Verification and subsequent integration

Extend the existing `crates/core/tests/security.rs` native suite through a separate
management test module. The architecture checkpoint makes that suite mandatory on
Windows/Linux/macOS before behavior is added; existing tests are not relabeled as
graph acceptance. Required behavior cases include all finite kinds, exact identity
rules, direct/indirect protection, both edge directions, before/after removals and
additions, cross-snapshot chains, cycles, duplicate/dangling input, missing protected
identities, unknown reached/unreached effects, global scope, combined bulk roots,
boundary/overflow inputs, unchanged graph/inputs and fixed safe failure output.

Add metamorphic comparisons with an independent small-graph reference traversal:
root order and node/edge order do not change outcomes; adding influence edges or
Unknown knowledge cannot turn denied protection into a clear result. Distinguish
valid new/removed ordinary nodes from missing protected identities.

This library is a necessary prerequisite, not implemented lan3 protection.
A future architecture must establish trustworthy effect extraction, current
identity/boot/profile/revision bindings, policy integration, device admission,
backup/recovery and verification before any mutation becomes callable. Unknown
third-party/global effects remain unavailable under protected-resource policy.
