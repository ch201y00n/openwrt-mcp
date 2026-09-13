# Protected-resource effect model

This is an internal pure library under `core::management`, **not a callable MCP
tool, actual topology discovery, mutation permission or deployed lan3 protection**.
Architecture-only `8adc3278df83289bff7529fb483c90edbbe1db49` was validated on native
Windows GNU and Linux-on-WSL before implementation. All production consumers remain
blocked from this namespace until a separately reviewed integration checkpoint.
See [ADR 0015](adr/0015-protected-resource-effects.md).

## Model and result

`ResourceId` pairs a finite `ResourceKind` with exact validated ASCII identity.
It does not normalize names, resolve aliases or interpret paths. `Node` adds
explicit Complete/Unknown outgoing-effect knowledge. `Influence::new(from,to)`
means a change to the first resource can affect the second, not the reverse.
`EffectGraph::new` validates the entire supplied graph, copies bounded node data
and stores edges as indices instead of repeating identity strings.

`ProtectedResources::new` and `ChangeScope::local` reject empty or duplicate
selections. `ChangeScope::global` represents global effects explicitly. The pure
`analyze_effects` function receives before/after graphs and those two selections.
It unions old and new influence edges, including removed relationships, then walks
the full reachable closure. Cross-snapshot paths are deliberately conservative.
Cycles and shared descendants count once, without recursive path enumeration.

For example, service -> bridge -> VLAN -> lan3 blocks a change rooted at the
service when lan3 is protected. Removing the bridge/VLAN edge in the proposed
graph does not erase the old influence. An unknown reachable effect also denies.
An unknown node disconnected from all change roots does not by itself block an
independent modeled change. A missing protected identity on either side denies
instead of treating the removed resource as disconnected.

The only successful result is `NoKnownProtectedImpact { affected_resources }`.
This count describes the supplied model only; a caller can supply an inaccurate
model, so success is not trusted device evidence or authority. Failures contain
fixed codes only. Graphs, identities and edges have neither raw Debug output nor
serde/JSON interfaces. No source value, effect list or input excerpt is returned.

Limits are 4,096 unique nodes and 8,192 unique directed edges per graph **and in
the union**, 256 roots, 128 protected identities and 128 bytes per identity.
Duplicate edges within one graph fail; identical edges across snapshots coalesce.
Oversized input rejects, never truncates. The analysis builds compact adjacency
offsets and target indices, with bounded visited/stack state. Container/index
allocation is additional to identity text; these limits are not measured RSS.

## Verification and measurement scope

Fifteen synthetic behavioral tests extend the mandatory native core security
suite. They cover finite kinds, exact identities, duplicates/dangling edges,
direction, before/after chains, removed/new ordinary resources, protected identity
loss, reached/unreached unknowns, global effects, bulk roots, cycles and exact
individual/union limits. An independent adjacency-matrix fixed-point computation
checks 3,584 small models, order invariance and monotonic denial. These are supplied
fixture graphs, not a verified representation of an actual router.

The optimized union merges validated sorted node/edge streams directly, translates
each graph's node indices once and releases temporary index maps before traversal.
Strictly increasing mappings preserve edge order; another fixture checks differing
typed-node layouts, removed/new nodes, shared edges and unknown knowledge across
the two snapshots. This avoids per-edge string searches and tree allocations
without changing the admission rules or limits.

The optional synthetic microbenchmark constructs two maximum-cardinality graphs
with 13-byte identifiers, 256 roots, one protected resource and 4,095 affected
unprotected resources. It measures graph creation/drop and analysis separately,
with 16 warmups and 256 samples, in the workspace release profile:

```sh
cargo run --locked --release -p openwrt-mcp-core --example effect_benchmark
```

Record host/toolchain/load alongside results. The example performs no network,
key, file or router operation and does not measure the MCP dispatcher, topology
extraction, peak memory, OpenWrt ARM performance or mutation safety.

Exploratory measurements on 2026-09-13 used the Intel Core Ultra 7 265K workstation,
native Windows GNU Rust 1.95.0 and Linux-on-WSL Rust 1.97.1. Final runs were
sequential with no repository gate started by this task concurrently; overall
machine load was not controlled. Values below are nanoseconds, not device latency:

| Host | Create/drop p50 / p95 | Analyze p50 / p95 |
| --- | ---: | ---: |
| Native Windows GNU | 1683900 / 1815500 | 137200 / 198600 |
| Linux on WSL | 1376816 / 1521280 | 72134 / 83692 |

An earlier exploratory candidate using tree-based union and per-edge identity
searches measured analysis p50/p95 of 5967000/6983400 ns (Windows) and
5695802/5933782 ns (Linux). Those initial runs could overlap other build activity;
they motivated direct sorted merging, not a controlled speedup guarantee. Creation
times and all-model correctness must be considered separately from analysis. The
final benchmark's two graphs are identical maximum-cardinality supplied models;
other overlap/density/identity lengths and actual topology extraction need separate
measurement. No peak memory or allocation-count claim follows from these timings.

Final benchmark executables (development examples, not shipped MCP binaries):
Windows 307,712 bytes, SHA-256 `61339a06c6bd5f8627dfd0f7b003ed05e0e45b03c7bac6ddfb5ecd99abf04303`;
Linux 394,032 bytes, SHA-256 `5e471b9351582879a8d4463612087df0ab73e501029eed646de7bfee9cdb3232`.

Next integration must establish trustworthy complete effect extraction, canonical
live identity, boot/profile/revision freshness, all-category authorization,
exclusive device admission, finalized encrypted backup, armed recovery and verified
postconditions. Existing configuration observations do not establish those facts.
The remainder of [management workflows](management-workflows.md) is still proposed.
