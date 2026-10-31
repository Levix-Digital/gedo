---
plan_id: gedo_bounded_detached_tape_walk
project: gedo
title: Bounded detached-tape walk
kind: feature
version: v0.3.0
status: active
author: LeviGuilherme
created_at: '2026-10-31T13:39:26'
updated_at: '2026-10-31T14:13:26'
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
  done: false
- name: M2 Kept generations
  done: false
- name: M3 Bounded search
  done: false
- name: M4 Hardening and docs
  done: false
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
