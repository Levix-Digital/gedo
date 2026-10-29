---
plan_id: gedo_event_model
project: gedo
title: Event model
kind: feature
version: v0.3.0
status: active
author: Levi Silveira
created_at: '2026-10-29T17:48:48'
updated_at: '2026-10-29T22:30:11'
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
  done: false
- name: M2 Event ids
  done: false
- name: M3 Authors and labels
  done: false
- name: M4 Clocks
  done: false
- name: M5 Record and publish
  done: false
- name: M6 Reconstruction
  done: false
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
