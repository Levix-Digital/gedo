---
id: 19
title: Commit generations in the symbol cache and a generation-bounded detached-tape search
status: accepted
date: '2026-10-31'
version: v0.3.0
subsystems: [events, store]
supersedes: []
superseded_by: []
amends: [14, 16]
amended_by: []
---

# ADR-0019: Commit generations in the symbol cache and a generation-bounded detached-tape search

## Context

Re-attachment without pairs and the retention report (ADR-0016) find the detached tapes by walking the history from the tips until every commit of the commit index is seen. When one tape is really detached, the normal case after a squash merge on the server, its commits are never seen and the walk goes on to the root commit, on every call. Measured on the Linux development container (release build, Git store adapter, 200 commits with tapes over a history without Gedo data, one detached tape): p50 58 ms with 10,000 commits and 612 ms with 100,000, linear in the whole history (IMP-006). Commit generations (ADR-0014) bound such a walk, but the event log kept them in memory for the life of one call, so computing one also walked to the root.

## Decision

- **Generations in the symbol cache.** The cache gains a `generations` namespace: key the commit id, value a format byte (`1`) then the generation as 8 little-endian bytes. `EventLog::generation` reads it before reading a commit's parents and writes every generation it computed at the end of the call, in transactions of up to 100,000 records. A cached value is used only when it equals one more than the highest generation of the commit's parents known in memory or in the cache (1 for a root); a value that disagrees or does not decode is refused with a warning and computed again. Cache failures are warnings (ADR-0015). The namespace is local derived data, never pushed and not rebuilt on open.
- **Bounded search.** The search visits commits by descending generation (a max-heap) and keeps the indexed commits not yet seen in a set sorted by generation. It stops when that set is empty or the highest commit still to visit is below the lowest of them: a commit only reaches commits of lower generation, so none of them can be reached any more. An indexed commit missing from the object database is unreachable.
- Every other user of generations (`record`, `publish`, `converge`, the ancestor test and the generation order of re-attachment) gets the cached values with no change.

## Alternatives Considered

- **Git's commit-graph file**: free when present, but it needs a new `ObjectStore` operation, Git does not write it on every clone or fetch, and its corrected commit dates (generation v2) are not ADR-0014's generations. It can become a faster source of generations later.
- **Generations in `refs/gedo/*`**: pushed data for something every machine computes from its own history.
- **Trusting cached values without a check**: generations become part of event clocks and therefore of event ids; a corrupted record would write wrong pushed data. The check costs one commit read per value, which the walks read anyway.
- **Writing generations in transactions of 1,000**: each transaction is a durable commit of the cache; the first call on a history of 100,000 commits took 2.1 s instead of 0.86 s.

## Consequences

- With generations cached, listing the detached tapes over 100,000 commits without Gedo data takes p50 16 to 24 ms on the development container, as for 1,000 commits. The first call with an empty cache walks the history once and writes every generation: 0.86 to 1.19 s there, against 0.61 to 0.73 s for one call before.
- The cache grows by about 50 bytes per commit of the history (about 50 MB for 1,000,000 commits); deleting it only costs one more full walk.
- Results do not change: the same tapes are detached, with the same commits, and generations, clocks and event ids are the same.
- Verified by `crates/core/gedo-app/tests/generations.rs` (reuse across event logs with commits read counted, refused and rewritten records, a failing cache, the bound on a deep history, a missing indexed commit, a property against the full walk over generated histories, cached against computed generations) and the `detached` benchmark.
