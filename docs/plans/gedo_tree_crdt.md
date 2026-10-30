---
plan_id: gedo_tree_crdt
project: gedo
title: Tree CRDT with deterministic convergence
kind: feature
version: v0.3.0
status: active
author: LeviGuilherme
created_at: '2026-10-30T15:23:02'
updated_at: '2026-10-30T15:36:04'
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
  done: false
- name: M2 State and fold
  done: false
- name: M3 Members as a set
  done: false
- name: M4 Commuting changes
  done: false
- name: M5 Conflicts
  done: false
- name: M6 Determinism
  done: false
- name: M7 Converge commits
  done: false
- name: M8 Hardening and docs
  done: false
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
