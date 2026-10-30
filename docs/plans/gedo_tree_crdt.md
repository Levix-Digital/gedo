---
plan_id: gedo_tree_crdt
project: gedo
title: Tree CRDT with deterministic convergence
kind: feature
version: v0.3.0
status: completed
author: LeviGuilherme
created_at: '2026-10-30T15:23:02'
updated_at: '2026-10-30T17:10:30'
session_id: null
tags:
- crdt
- convergence
- merge
- events
- rust
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-066
spec_dir: specs/006-tree-crdt
milestones:
- name: M1 Setup
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M2 State and fold
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M3 Members as a set
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M4 Commuting changes
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M5 Conflicts
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M6 Determinism
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M7 Converge commits
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M8 Hardening and docs
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
status_log:
- at: '2026-10-30T15:23:02'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-10-30T15:23:03'
  by: LeviGuilherme
  status: draft
  note: Plan updated
- at: '2026-10-30T15:36:04'
  by: LeviGuilherme
  status: active
  note: Approved by Levi on 2026-10-30
- at: '2026-10-30T15:56:56'
  by: LeviGuilherme
  note: Milestone 'M1 Setup' completed
- at: '2026-10-30T15:57:00'
  by: LeviGuilherme
  note: Milestone 'M2 State and fold' completed
- at: '2026-10-30T15:57:01'
  by: LeviGuilherme
  note: Milestone 'M3 Members as a set' completed
- at: '2026-10-30T15:57:03'
  by: LeviGuilherme
  note: Milestone 'M4 Commuting changes' completed
- at: '2026-10-30T15:57:05'
  by: LeviGuilherme
  note: Milestone 'M5 Conflicts' completed
- at: '2026-10-30T15:57:06'
  by: LeviGuilherme
  note: Milestone 'M6 Determinism' completed
- at: '2026-10-30T15:57:09'
  by: LeviGuilherme
  note: Milestone 'M7 Converge commits' completed
- at: '2026-10-30T15:57:10'
  by: LeviGuilherme
  note: Milestone 'M8 Hardening and docs' completed
- at: '2026-10-30T15:57:10'
  by: LeviGuilherme
  status: completed
  note: All milestones completed
- at: '2026-10-30T17:10:30'
  by: LeviGuilherme
  status: completed
  note: 'Delivered in PR #20 (merged 2026-10-30)'
---

# Tree CRDT with deterministic convergence

Envelope of the Spec Kit feature [`specs/006-tree-crdt/`](../../specs/006-tree-crdt/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Third feature of release v0.3.0 Event log and convergence: concurrent work converges deterministically per entity. Members of a structure behave as a set, a move, a rename and an edit of the same entity commute, an event present on several sides applies once, the same base and sides give the same result in any arrival order on every machine, and incompatible changes are reported as conflicts with their sides and events, never resolved silently. These are the merge rules of ADR-0009 that the merge driver (FEAT-070), the shadow merge (FEAT-074), the sequencer (FEAT-069) and virtual streams (FEAT-068) will share.

## Decisions

- **Three-way convergence per part over a common base** (research R2, new ADR-0017 amending ADR-0009): a state-based join per part, not last writer wins and not an operation-based move CRDT; no version vectors, no format change.
- **Five parts per entity** (research R3): presence, place (unit, file, container path), name, signature, body; the qualified path is split at its last top-level separator.
- **Fold of a side** (research R4): events deduplicated by id, aliases resolved, total order of ADR-0014; neutral entities leave no trace; an unknown entity is an error.
- **What commutes and what conflicts** (research R6, clarifications): place and name with everything; signature and body with each other (the user chose it); removal against change conflicts; a removed container against a new member conflicts.
- **Entities added on several sides** (clarification, research R7): equal content is one entity with the lower id kept (ADR-0007); different content is an added-on-both-sides conflict.
- **Conflicts keep the base value** and name each distinct value, its sides by input position and the events that set it (research R8).
- **Use case on commits** (research R9): `EventLog::converge(base, sides)` reads the base state for the files the sides changed and the events of each side's commits through `events_of_commit`, plus optional live events.
- No new crate, port, collection, stored format or third-party dependency; nothing is written.

## Backlog items

### FEAT-066: Tree CRDT with deterministic convergence
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Set semantics for members of a structure, commutative move and edit, idempotent events; same result in any arrival order.

## Milestones

Taken from the phases of [`tasks.md`](../../specs/006-tree-crdt/tasks.md): M1 Setup, M2 State and fold, M3 Members as a set, M4 Commuting changes, M5 Conflicts, M6 Determinism, M7 Converge commits, M8 Hardening and docs.

## Results

Benchmarks on the reference machine (Windows 11, Intel Core i7-12700KF, release build, 2026-10-30):

| Requirement | p50 | p95 | p99 | Budget |
|---|---:|---:|---:|---|
| Converge two sides of 10,000 events over 10,000 entities (`cargo bench -p gedo-crdt --bench tree`, 25 runs, SC-004) | 27.5 ms | 29.0 ms | 30.7 ms | p95 under 50 ms: met |
| Converge two branches of 100 commits from their tips, Git store adapter (`cargo bench -p gedo-app --bench converge`, 25 runs, SC-005) | 136.5 ms | 143.7 ms | 146.7 ms | p95 under 500 ms: met |

Benchmarks on the Linux development container (release build, 2026-10-30), for comparison:

| Requirement | p50 | p95 | p99 | Budget |
|---|---:|---:|---:|---|
| Converge two sides of 10,000 events over 10,000 entities (`cargo bench -p gedo-crdt --bench tree`, 25 runs, SC-004) | 44 ms | 64 ms | 66 ms | p95 under 50 ms on the reference machine |
| Converge two branches of 100 commits from their tips, Git store adapter (`cargo bench -p gedo-app --bench converge`, 25 runs, SC-005) | 51 ms | 81 ms | 84 ms | p95 under 500 ms |

The container's p95 for SC-004 is above the 50 ms budget, which is set for the reference machine, where it is met. Event ids are now derived with a context hasher prepared once and a buffer kept per thread, with the same ids (pinned by the encoding snapshot).

Tests: 27 tests in `crates/core/gedo-crdt/tests/tree.rs` (one per acceptance scenario of User Stories 1 to 4, six properties of 256 cases each, a pinned convergence), 8 scripted branch tests in `crates/core/gedo-app/tests/converge.rs` (additions to one type, two bodies, removal against edit, a graft, published against reconstructed events, live events on top, a base not indexed, a pinned convergence) and the `sides converged` event in `tests/event_tracing.rs`.

Deviations from the plan: a rename takes its name from the qualified path it records, so `TreeError::InconsistentRename` was dropped; the container path is the parent of the qualified path, so a method's container is its type, and implementation blocks are left out of the added-on-both-sides rule (both in ADR-0017); the use case does not time itself (the core has no clock); `docs/architecture/core.md` needed no change, since `gedo-core` did not change.

CI on PR #20 passed on Windows and Linux (tests, deny, MSRV and the benches, including the new `tree` and `converge` benches).
