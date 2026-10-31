# Research: Bounded Detached-Tape Walk

## R1. Measurement before the change

- **Setup**: `crates/core/gedo-app/benches/detached.rs`: `git fast-import` writes a linear history of N commits without Gedo data; 200 commits on top each get a published tape; a side commit off the 100th of them gets one more tape; the tips are the last commit only, so the side tape is detached. `detached(tips)` is timed 10 times, each with a new event log on one cache (the way separate commands run).
- **Result** (Linux development container, release build, 2026-10-31): N = 10,000: p50 58 ms, p95 66 ms. N = 100,000: p50 612 ms, p95 726 ms. About 6 µs per commit, all of it reading commits from the pack.
- **Decision**: the improvement is worth doing; the same bench measures the result (SC-001, SC-002).

## R2. Where the generations come from

- **Decision**: computed by the event log as today and kept in a new `generations` namespace of the symbol cache.
- **Rationale**: the cache is already the place for local derived data shared by the use cases of a process (ADR-0012, ADR-0015); no port changes; works with every object store, including the in-memory one of the tests; the value of a commit never changes, so an entry is never stale.
- **Alternatives considered**: Git's commit-graph file (generation numbers v1/v2 written by `git gc` and `git commit-graph write`): free when present, but needs a new `ObjectStore` operation, is absent in many clones (Git does not write it on fetch by default), and its "corrected commit date" generation v2 is not ADR-0014's generation, so the event log would still need its own. It can become a faster source later. Keeping generations in `refs/gedo/*`: pushed data for something every machine can compute; rejected.

## R3. The bound

- **Decision**: replace the depth-first stack of `detached_set` by a max-heap of `(generation, commit)`. A sorted set holds the indexed commits not yet seen with their generations. Pop the highest commit; stop when the set is empty or the popped generation is lower than the set's lowest; otherwise mark it seen, remove it from the set when indexed, and push its parents with their generations.
- **Rationale**: a commit of generation g can only reach commits of lower generation. When the highest commit still to visit is below the lowest unseen indexed commit, no commit still to visit, nor any of their ancestors, can be one of them. A commit of equal generation that is not the commit itself is not an ancestor either, so the comparison is strict on the popped commit only.
- **Indexed commits missing from the object database** (FR-003): their generation cannot be computed; they cannot be reachable (Git keeps every reachable object), so they are left out of the set and stay unreachable.
- **Cost**: the generation of every indexed commit is needed for the bound; with the cache this is one read per commit, done once per call. Index sizes are in the thousands, far below the history.

## R4. Computing and keeping generations

- **Decision**: `EventLog::generation(commit)` keeps its memory map and its iterative post-order walk; before reading a commit's parents it asks the cache. Every generation it computes in one call is written to the cache in batches of 1,000 at the end of the call (one transaction each). Key: the commit id bytes; value: format byte 1 then the generation as 8 little-endian bytes.
- **Verification on read** (FR-005): a cached value for commit c is accepted only when c's parents (one commit read) have generations known in memory or in the cache and the value equals one more than their highest (1 for a root). A cached value of a parent used in that check is accepted without its own check: the check is local, it catches a corrupted or misplaced record, not a coordinated rewrite of the cache, which is local data of the user's own machine. A refused value is computed again from the history and logged as a warning.
- **Why the check**: the generation becomes part of event clocks and therefore of event ids on `record` and `publish`; a wrong cached value would write wrong pushed data. The check costs one commit read per value read from the cache, which the walks read anyway.

## R5. Other callers

- `record` (base generation of a live event), `publish`, `converge`, `ancestors` and `in_generation_order` all call `generation`; they get the cache with no change of their code. `ancestors` already stops below the lowest wanted generation.

## R6. Tests and properties

- Unit tests of the bound on the in-memory store: linear, branching and merging histories; tapes on reachable, unreachable and missing commits; tips that are indexed.
- A property (proptest, 256 cases): random DAGs, random indexed commits and tips; the bounded search equals a reference unbounded walk written in the test; generations with a cache equal those without; a commit-read counter shows that no commit below the bound is read.
- Cache tests: reuse across event logs, refusal of a corrupted value (wrong length, wrong format byte, wrong number), cache failures (a cache that fails every call).
