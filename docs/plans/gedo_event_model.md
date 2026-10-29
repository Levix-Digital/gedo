---
plan_id: gedo_event_model
project: gedo
title: Event model
kind: feature
version: v0.3.0
status: in_progress
author: Levi Silveira
created_at: '2026-10-29T17:48:48'
updated_at: '2026-10-29T22:52:00'
session_id: null
tags:
- events
- clocks
- labels
- tapes
- rust
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-063
- FEAT-064
spec_dir: specs/004-event-model
milestones:
- name: M1 Setup and event model
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M2 Event ids
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M3 Authors and labels
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M4 Clocks
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M5 Record and publish
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M6 Reconstruction
  done: true
  done_at: '2026-10-29'
  by: Levi Silveira
- name: M7 Hardening and acceptance
  done: false
status_log:
- at: '2026-10-29T17:48:48'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-29T21:35:30'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-29T21:35:38'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-29T21:35:50'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-29T22:30:11'
  by: Levi Silveira
  status: active
  note: Approved by Levi on 2026-10-29
- at: '2026-10-29T22:51:53'
  by: Levi Silveira
  note: Milestone 'M1 Setup and event model' completed
- at: '2026-10-29T22:51:54'
  by: Levi Silveira
  note: Milestone 'M2 Event ids' completed
- at: '2026-10-29T22:51:56'
  by: Levi Silveira
  note: Milestone 'M3 Authors and labels' completed
- at: '2026-10-29T22:51:57'
  by: Levi Silveira
  note: Milestone 'M4 Clocks' completed
- at: '2026-10-29T22:51:59'
  by: Levi Silveira
  note: Milestone 'M5 Record and publish' completed
- at: '2026-10-29T22:52:00'
  by: Levi Silveira
  note: Milestone 'M6 Reconstruction' completed
---

# Event model

Envelope of the Spec Kit feature [`specs/004-event-model/`](../../specs/004-event-model/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

First feature of release v0.3.0 Event log and convergence (scope chosen by the user on 2026-10-29: FEAT-063 and FEAT-064; FEAT-065 follows with the CRDT): the semantic deltas and identity merges of feature 003 become immutable, content-addressed events with clocks consistent with the history, an author fingerprint, an author kind (person or agent) and opaque labels (ADR-0013), recorded live through the write-ahead log and published as tapes indexed by commit, or reconstructed deterministically for commits made without Gedo.

## Decisions

- **Content-derived ids for every event** (research R2): UUID v8 from BLAKE3 over the event's content, live and reconstructed alike; retried publishes and grafts are idempotent.
- **Clocks on commit generations** (research R3): `(generation, sequence)`, so reconstructed clocks are a function of the history and live events stay later than their base's ancestry; total order `(clock, id)`.
- **Author fingerprint** (clarification 3, research R4): BLAKE3 over the trimmed, lowercased Git e-mail; the e-mail is never stored. `CommitInfo` gains the author e-mail.
- **Author kind and labels in the format from the start** (research R5, R6): person or agent with an identifier; at most 16 labels, keys of 1 to 64 bytes, values of 1 to 256, `gedo.` reserved; `GEDO_AGENT` and `GEDO_LABELS` text forms parsed in the core.
- **Tapes keep every recorded event** (clarification 2, research R8): ordered event ids without commit ids, at most 65,536 per tape.
- **Three content-addressed collections** (research R9, new ADR-0014): `refs/gedo/events`, `refs/gedo/tapes`, `refs/gedo/commit-tapes` (entry = commit + tape); amends TECH_SPEC's data table and ADR-0003/ADR-0008 on the layout.
- **Reconstructed events are local derived data** (clarification 1, research R10), built from ids as recorded at the commit, never shared.
- **New core crate `gedo-crdt`** (research R1) for the pure event logic; `EventLog` use case in `gedo-app`; no new port and no new third-party dependency.

## Backlog items

### FEAT-063: Event model with Lamport clocks and content-addressed ids
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Event, SemanticOp, Lamport clock and author fingerprint, with deterministic ids for reconstructed events. Each event also carries the kind of author (person or agent, with an identifier) and a map of opaque labels (key and value) that tools on top of Gedo set and filter, for example plan=... or milestone=M2 from the dev-cycle protocol; Gedo stores and returns them without knowing what they mean. Both are in the format from the start, because formats freeze at v1.0.0 (FEAT-127).

### FEAT-064: Event tapes and commit index
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Content-addressed events grouped in tapes, with a commit-to-tape index (ADR-0008).

## Results

Benchmarks of `cargo bench -p gedo-app --bench events` (release build, Linux development container, 4 cores, Git store adapter, 2026-10-29); the Windows reference machine run is pending the user's go:

| Requirement | p50 | p95 | p99 | Budget |
|---|---:|---:|---:|---|
| Record one event (2,000 runs, SC-004) | 4.0 µs | 6.0 µs | 9.1 µs | p95 under 1 ms |
| Publish a tape of 1,000 events (10 runs, SC-004) | 90 ms | 107 ms | 107 ms | under 1 s |
| Publish 1,000 events into a chain of 100,000 (5 runs) | 240 ms | 275 ms | 275 ms | under 1 s |
| Reconstruct the events of one commit of this repository (20 runs, SC-005) | 0.11 ms | 0.29 ms | 0.41 ms | p95 under 100 ms |
| Filter 10,000 events by one label (5 runs, SC-006) | 331 ms | 408 ms | 408 ms | under 500 ms |

Deviation from the plan: commit generations are kept in memory by `EventLog` instead of a symbol cache memo, and reconstructed events are not memoized (research R3, R10), since `IdentityIndex` already borrows the cache and the numbers above leave no need for it.
