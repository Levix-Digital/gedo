---
plan_id: gedo_bounded_detached_tape_walk
project: gedo
title: Bounded detached-tape walk
kind: feature
version: v0.3.0
status: completed
author: LeviGuilherme
created_at: '2026-10-31T13:39:26'
updated_at: '2026-10-31T14:28:07'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- IMP-006
spec_dir: specs/008-bounded-detached-walk
milestones:
- name: M1 Setup
  done: true
  done_at: '2026-10-31'
  by: LeviGuilherme
- name: M2 Kept generations
  done: true
  done_at: '2026-10-31'
  by: LeviGuilherme
- name: M3 Bounded search
  done: true
  done_at: '2026-10-31'
  by: LeviGuilherme
- name: M4 Hardening and docs
  done: true
  done_at: '2026-10-31'
  by: LeviGuilherme
status_log:
- at: '2026-10-31T13:39:26'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-10-31T13:47:05'
  by: LeviGuilherme
  status: active
  note: Plan updated
- at: '2026-10-31T14:13:26'
  by: LeviGuilherme
  status: active
  note: Approved by Levi on 2026-10-31
- at: '2026-10-31T14:27:56'
  by: LeviGuilherme
  note: Milestone 'M1 Setup' completed
- at: '2026-10-31T14:28:03'
  by: LeviGuilherme
  note: Milestone 'M2 Kept generations' completed
- at: '2026-10-31T14:28:05'
  by: LeviGuilherme
  note: Milestone 'M3 Bounded search' completed
- at: '2026-10-31T14:28:07'
  by: LeviGuilherme
  note: Milestone 'M4 Hardening and docs' completed
- at: '2026-10-31T14:28:07'
  by: LeviGuilherme
  status: completed
  note: All milestones completed
---

---
plan_id: gedo_bounded_detached_tape_walk
project: gedo
title: Bounded detached-tape walk
kind: feature
version: v0.3.0
status: draft
author: LeviGuilherme
created_at: '2026-10-31T13:39:26'
updated_at: '2026-10-31T13:39:26'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- IMP-006
milestones: []
status_log:
- at: '2026-10-31T13:39:26'
  by: LeviGuilherme
  status: draft
  note: Plan created
---

# Bounded detached-tape walk

Envelope of the Spec Kit feature [`specs/008-bounded-detached-walk/`](../../specs/008-bounded-detached-walk/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Improvement of release v0.3.0 Event log and convergence: finding detached tapes (`detached`, `reattach_new`) costs the part of the history Gedo has data for, not the whole history. Measured before the change: p50 612 ms per call with 100,000 commits below the Gedo data, linear in the history.

## Decisions

- **Generations kept in the symbol cache** (research R2, R4, new ADR-0019): a `generations` namespace of local derived data, read before any history walk and checked against the parents' generations on read, written in batches; cache failures are warnings. Git's commit-graph is not read (needs a port change, often absent); it may become a faster source later.
- **Generation-bounded search** (research R3): commits visited by descending generation; the search stops when the highest commit still to visit is below the lowest indexed commit not yet seen; indexed commits missing from the object database are unreachable.
- Results, formats, ports and commands unchanged; `record`, `publish` and `converge` get cached generations too.

## Backlog items

### IMP-006: Bound the history walk that finds detached tapes
- **Priority**: low
- **Found**: 2026-10-30, PR #18, protocol review
- **Description**: EventLog::detached and reattach_new walk the history from the tips until every indexed commit is found; when some tape is really detached they walk the whole history on every call. Fast on the benchmark (a few hundred commits) but linear in the history on large repositories. Keep commit generations as local derived data (or use the commit-graph) and stop the walk below the lowest generation of the indexed commits still unseen; measure on a large repository first.

## Milestones

Taken from the phases of [`tasks.md`](../../specs/008-bounded-detached-walk/tasks.md): M1 Setup, M2 Kept generations, M3 Bounded search, M4 Hardening and docs.

## Results

Benchmark on the Linux development container (release build, Git store adapter, `cargo bench -p gedo-app --bench detached`, 2026-10-31): a history without Gedo data, then 200 commits with tapes and one tape detached from a side branch; the first call on an empty cache, then 12 calls each through a new event log on the same cache.

| History without Gedo data | Before (per call) | First call, empty cache | Generations cached, p50 / p95 | Budget |
|---|---:|---:|---:|---|
| 10,000 commits | p50 58 ms, p95 66 ms | 159 ms | 21 ms / 23 ms | |
| 100,000 commits | p50 612 ms, p95 726 ms | 0.86 to 1.19 s | 13 ms / 15 ms | SC-001: p95 under 50 ms (reference machine); SC-002: first call at most twice one call before |

Tests: 12 tests in `crates/core/gedo-app/tests/generations.rs` (generations over merges, reuse by a new event log with the commits read counted, a cleared cache, four kinds of bad records refused and rewritten, a failing cache, the search reading fewer than 100 commits of a history of 541, a reachable side branch, an indexed commit missing from the store, no index, a missing tip, and two properties of 256 cases: the bounded search against the full walk over generated histories with merges, roots and missing commits, and cached against computed generations), and the `detached-tape search done` and `generation record refused` events in `tests/event_tracing.rs`.

Deviations from the plan: the scenario tests of the bound and the property run in `tests/generations.rs` with the cache tests, on histories of empty-tree commits, instead of `tests/reattach.rs`; generations are written in transactions of 100,000 records instead of 1,000, since each transaction is a durable commit of the cache (the first call took 2.1 s with 1,000); the search reads an indexed commit's generation directly and treats a not-found error for that commit as unreachable; the CI bench runs at its default depth of 100,000 commits; the search keeps the parents read to check a cached generation, so the commits it visits next are not read twice (the cached p50 at 100,000 commits went from 16 to 24 ms to 13 ms).

CI on PR #25 passed on Windows and Linux. On the Windows CI runner, before keeping the parents, the bench gave 1.17 s for the first call and p50 65 ms, p95 66 ms with generations cached.
