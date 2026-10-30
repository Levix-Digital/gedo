---
plan_id: gedo_windows_event_publish_and_filter_performance_fixes
project: gedo
title: Windows event publish and filter performance fixes
kind: bug
version: v0.3.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-29T23:42:03'
updated_at: '2026-10-30T01:30:01'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-009
- BUG-010
milestones:
- name: M1 Packed batch writes
  done: true
  done_at: '2026-10-30'
  by: Levi Silveira
- name: M2 Event cache
  done: true
  done_at: '2026-10-30'
  by: Levi Silveira
- name: M3 Reference machine and documentation
  done: false
status_log:
- at: '2026-10-29T23:42:03'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-29T23:43:54'
  by: Levi Silveira
  status: active
  note: Plan updated
- at: '2026-10-29T23:44:05'
  by: Levi Silveira
  status: draft
  note: Waiting for Levi's approval
- at: '2026-10-29T23:44:15'
  by: Levi Silveira
  status: active
  note: Saved for plan approval
- at: '2026-10-30T01:13:18'
  by: Levi Silveira
  status: active
  note: Approved by Levi on 2026-10-30
- at: '2026-10-30T01:30:00'
  by: Levi Silveira
  note: 'Milestone ''M1 Packed batch writes'' completed: write_blobs and the pack writer; Linux publish p95 about 80 ms'
- at: '2026-10-30T01:30:01'
  by: Levi Silveira
  note: 'Milestone ''M2 Event cache'' completed: events cache namespace; Linux cached filter p95 52 ms'
---

---
plan_id: gedo_windows_event_publish_and_filter_performance_fixes
project: gedo
title: Windows event publish and filter performance fixes
kind: bug
version: v0.3.0
status: draft
author: Levi Silveira
created_at: '2026-10-29T23:42:03'
updated_at: '2026-10-29T23:42:03'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- BUG-009
- BUG-010
milestones: []
status_log:
- at: '2026-10-29T23:42:03'
  by: Levi Silveira
  status: draft
  note: Plan created
---

# Windows event publish and filter performance fixes

Bug plan for the two Windows misses of the event model benchmarks, fixed before feature 005 (bugs first: both items are `medium`). Levi asked on 2026-10-29 to follow the protocol, which puts this plan before the next feature.

Bug plan: discovery and planning of the backlog items below (`dev-cycle-protocol` Phases 1 and 2). Replace every instruction paragraph with the findings.

## Backlog items

### BUG-009: Publishing a tape of 1,000 events takes over 1 s on Windows
- **Severity**: medium
- **Found**: 2026-10-29, PR #15, Windows reference benchmark
- **Description**: On the Windows reference machine (i7-12700KF), cargo bench -p gedo-app --bench events gives p50 1.19 s and p95 1.31 s for publishing 1,000 events (1.49 s p95 into a chain of 100,000), above the 1 s budget of gedo publish (TECH_SPEC, SC-004 of 004-event-model); Linux p95 is 107 ms. Each event is a separate loose blob in refs/gedo/events, so a publish writes about 1,000 loose object files.

### BUG-010: Filtering 10,000 events by a label takes over 1 s on Windows
- **Severity**: medium
- **Found**: 2026-10-29, PR #15, Windows reference benchmark
- **Description**: On the Windows reference machine, EventLog::filter over 10,000 stored events gives p50 1.19 s and p95 1.34 s, above the 500 ms of SC-006 of 004-event-model; Linux p95 is 408 ms. The filter reads every event blob of refs/gedo/events; a label index in the symbol cache or fewer, larger objects would avoid it.

## 1. Triage

| Item | Expected (source) | Actual | Verdict | Severity |
|---|---|---|---|---|
| BUG-009 | Publishing 1,000 events under 1 s (SC-004 of `specs/004-event-model`, the `gedo publish` budget of TECH_SPEC) on the reference machine | p95 1.31 s, and 1.49 s into a chain of 100,000 events (Windows reference machine, `main` at 3f820f2) | bug | medium (confirmed): no data is wrong, but the first user command built on it would miss its budget |
| BUG-010 | Filtering 10,000 events under 500 ms (SC-006) on the reference machine | p95 1.34 s on the same run | bug | medium (confirmed): same reason |

Suspected code: `crates/core/gedo-core/src/collection.rs` (`Collection::put` writes one object per entry), `crates/adapters/gedo-store/src/objects.rs` (`write_blob` writes a loose object; `read` looks an object up twice, header then content), `crates/core/gedo-app/src/events/mod.rs` (`EventLog::filter` reads and decodes every event blob of `refs/gedo/events`).

## 2. Reproduction

- Windows reference machine (Levi's desktop, i7-12700KF), `cargo bench -p gedo-app --bench events -- --quick`, `main` at 3f820f2 (results in `docs/plans/gedo_event_model.md`): publish p50 1.19 s, p95 1.31 s; filter p50 1.19 s, p95 1.34 s. The same benchmark on Linux: publish p95 107 ms, filter p95 408 ms.
- Linux, the same benchmark under `strace -f` (one run: 2,000 records, 10 publishes of 1,000 events, 5 filters of 10,000 events):

  | Step | Files created (`O_CREAT`) | Renames | Object files opened for reading |
  |---|---|---|---|
  | 10 publishes | 12,758 | 12,668 | |
  | 5 filters | | | 107,218 |

  That is about 1,270 new loose object files per publish (1,000 event blobs, about 256 fan-out trees, the root tree and the commit, plus the tape and index collections) and about 21,400 object file opens per filter (two per event and the trees).

## 3. Evidence and root cause

- **Confirmed: both costs are per-file costs of loose objects.** Every object Gedo writes goes through `gix::Repository::write_blob` or `write_object`, which writes one zlib file under `.git/objects/xx/` through a temporary file and a rename. Every read goes through `GitObjectStore::read`, which calls `try_find_header` and then `try_find_object`: for a loose object each one opens the file and inflates it. On Windows, creating, renaming and opening a file costs far more than on Linux (NTFS metadata, the file system filter drivers of antivirus software): about 1 ms per created object here (1.31 s for about 1,270 files) against well under 0.1 ms on Linux.
- **Confirmed for BUG-010: packed objects make the filter about 4 times faster even on Linux.** Prototype, not kept: the same benchmark with `git repack -a -d` run between the publishes and the filter gives a filter p95 of 114 and 122 ms against 459 and 326 ms without it (two runs each, same container). Objects in a pack are read from one memory-mapped file, with no file open per object.
- **Hypothesis to confirm on the reference machine**: with the publish written as a pack and the filter reading through a cache, both are well under budget on Windows. The CI `bench (windows-latest)` job gives a first indication; the reference machine decides.
- Packing alone does not fix BUG-010 in real use: events published in small tapes (one commit at a time) stay loose (see the threshold below), and a filter reads all of them. The filter needs a read path that does not open one file per event.

## 4. Fix design (`software-design`)

- **Batched blob writes (BUG-009)**: the `ObjectStore` port gains `write_blobs(&self, blobs: &[Vec<u8>]) -> Result<Vec<GitOid>, StoreError>`, whose default implementation calls `write_blob` for each blob (the in-memory store keeps it). `Collection::put` writes its blobs with it. The Git adapter writes a batch of at least `PACK_THRESHOLD` (100) blobs as one pack with its index in `.git/objects/pack/` (built with `gix-pack`'s `Bundle::write_to_directory` from a pack stream of undeltified objects), and a smaller batch as loose objects, like Git's own `fetch.unpackLimit` default of 100. Objects already present are not written again. The pack is complete or absent: it is written to a temporary file and renamed with its index, so a crash leaves no half pack the object database would read. Git and gitoxide read the pack as any other; `git gc` repacks it.
- **Fan-out trees**: about 256 trees and the commit stay loose in a large publish (about 260 files against 1,270). If the Windows numbers after M1 are still above 1 s, `write_trees` is added the same way (two packs per large publish); otherwise it is left out.
- **Object slots**: gitoxide's object database holds at most 32 pack indices by default when it opens a repository with few packs, and a long-running process that writes more packs than that would fail to read new ones. `GitRepository` opens the object database with more slots (`Slots::AsNeededByDiskState` with a larger minimum, the number fixed in M1 after reading gitoxide's limits), and a test writes more packs than the default in one process and reads every object back.
- **One lookup per read (BUG-010)**: `GitObjectStore::read` reads the object once and checks the size from the header that lookup returns, when gitoxide gives it; the size limit still applies before the content is decoded. Measured in M2; kept only if it helps.
- **Event cache (BUG-010)**: a new `events` namespace in the symbol cache maps an event id to its stored record. `EventLog` takes the cache (`open(objects, refs, wal, cache)`); `publish` fills it in the same step as the WAL truncation (after the index entry, so a crash leaves at most a missing cache entry), and `event`, `events_of_commit` and `filter` read the cache first and the blob only on a miss, filling the cache. `filter` lists the collection's entries (the trees only) and looks the ids up in one read transaction (a `get_many` on the `SymbolCache` port, added if a lookup per id is measured too slow). Records read from the cache are decoded with the same checks as blobs, except the id derivation, which was checked when the entry was written; a cache entry that fails to decode is dropped and read again from the blob. The cache is local derived data and is never rebuilt on open: a missing entry is read from the collection, like the `identity` namespace. Event ids are derived from content, so an entry can never become stale.
- Alternatives rejected: a label index in the cache (a filter also matches author kind, agent and fingerprint, and the events must be returned anyway, so it would still read every event); one blob per tape holding its events (a format change of ADR-0014, and it does not help events received from other machines); running `git repack` (Gedo does not run Git subprocesses).

Deliberately left out: repacking or merging Gedo's own packs (Git's `gc` does it), deltas inside the packs (event records are small and distinct), and changes to the stored formats.

## 5. Affected surfaces and files

No surface in `architecture.surfaces` changes, and no stored format changes. Files: `crates/core/gedo-core/src/ports/object.rs` (`write_blobs`), `ports/cache.rs` (`get_many`, if needed), `collection.rs` (`put`), `contract_tests.rs`; `crates/adapters/gedo-store/src/{objects.rs, repository.rs}` and a new `src/pack.rs`, `Cargo.toml` (`gix-pack` with `streaming-input`, already in the dependency tree through `gix`); `crates/core/gedo-app/src/events/mod.rs` (cache), its tests, benchmark and example. Same defect elsewhere: every collection writes through `Collection::put` and gets the batch; the identity index writes to the cache, not to objects.

## 6. Tests (`quality-assurance`)

- BUG-009: object store contract tests for `write_blobs` on both adapters (ids equal to `write_blob`'s, order kept, duplicates, an empty batch, a batch at and above the threshold read back after reopening the repository); a Git adapter test that a batch of 100 blobs creates one pack and no loose object, and that a crash before the rename (the temporary file left behind) leaves the objects absent and the repository readable; the slots test above; `git fsck` on the scratch repository of the formats test (a developer check, as before).
- BUG-010: `EventLog` tests that `filter`, `event` and `events_of_commit` give the same results with an empty cache, a full cache, and a cache with a corrupted entry, and that publish fills the cache; crash injection extended to the cache step (a crash after the index entry and before the cache leaves a log that publishes nothing more and reads correctly).
- Regression guard: the benchmark itself. The Linux p95 of publish and filter must not grow; the Windows CI and reference machine numbers are recorded in this plan. Every existing test passes, the event fuzz target included.
- No manual test: there is no user surface yet.

## 7. Documentation (`architecture-docs`, `user-docs`)

New ADR-0015 "Packed batch writes and an event cache" (the threshold, the slots, the cache namespace; it amends the "no cache namespace" line of ADR-0014). `docs/architecture/store.md` (batch writes and packs, slots, the `events` namespace), `docs/architecture/events.md` (read path through the cache), `core.md` (the port methods). Regenerate the constitution (ADRs are a source). README: nothing changes for users.

## 8. Release, migration and rollback (`devops-release`)

Part of v0.3.0, not released. No stored format changes and no migration: packs are ordinary Git objects, and the cache namespace is new local data that older builds ignore. Rollback is a revert of the pull request; packs already written stay valid objects.

## 9. Execution steps and milestones

One branch `fix/windows-event-performance` from `main` (it carries the backlog commit of BUG-009 and BUG-010), one pull request into `main`.

1. M1 Packed batch writes: `write_blobs`, the pack writer, the slots, their tests, the Linux benchmark and the Windows CI benchmark of the pull request.
2. M2 Event cache: the `events` namespace, the read path, one lookup per read if it helps, their tests and benchmarks.
3. M3 Reference machine and documentation: the benchmark on Levi's Windows machine through Remote Control (Levi approves the session), section 7, the checkers; then BUG-009 and BUG-010 close, or the remaining gap goes to Levi as a decision.

## Results

Changes against section 4, found while implementing:

- **Shared cache**: the identity index held the cache as `&mut`, and redb opens a database file once per process, so the event log could not use the same cache next to it. `SymbolCache::put` and `clear` now take `&self` (redb transactions already do), `IdentityIndex::open` takes `&C`, and `EventLog::with_cache` takes a shared `&dyn SymbolCache`. No stored data changes.
- **Cached records keep every check**, the id derivation included: it costs about 1 µs per event, so the exception planned in section 4 is not needed.
- **No `get_many`**: one read transaction per event is fast enough (a cached filter of 10,000 events takes 50 ms on Linux).
- **One lookup per read rejected**: reading an object without the header lookup made a filter with an empty cache over loose events 30% faster on Linux (366 to 250 ms), but the 64 MiB limit would only be checked after the whole object is in memory.
- **Trees stay loose**: packing the blobs was enough on Linux; `write_trees` waits for the Windows numbers.

`cargo bench -p gedo-app --bench events` on Linux (the cloud container, p95; the container's timings vary by about 2 times between runs):

| Benchmark | `main` 3f820f2 | M1 packs | M2 packs and cache |
|---|---|---|---|
| Publish a tape of 1,000 events | 107 to 824 ms | 74 to 87 ms | 152 ms (the cache transaction) |
| Filter 10,000 events (tapes of 1,000) | 326 to 459 ms | 72 ms | 52 ms; 128 ms with an empty cache |
| Filter 10,000 events (tapes of 20, loose) | 348 ms | 348 ms | 53 ms; 390 ms with an empty cache |

The new benchmark case, 10,000 events in 500 tapes of 20, is the common shape (one publish per commit) and stays loose under the pack threshold; only the cache fixes it. A filter with an empty cache (a fresh clone, a deleted cache) still reads every event once.
