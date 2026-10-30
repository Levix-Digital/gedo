---
plan_id: gedo_reattachment
project: gedo
title: Re-attachment after history rewrites
kind: feature
version: v0.3.0
status: active
author: LeviGuilherme
created_at: '2026-10-30T14:01:11'
updated_at: '2026-10-30T14:18:53'
session_id: null
tags:
- reattachment
- rebase
- squash
- retention
- events
- rust
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-065
- IMP-005
spec_dir: specs/005-history-reattachment
milestones:
- name: M1 Setup
  done: false
- name: M2 Matching rules
  done: false
- name: M3 Local rewrites
  done: false
- name: M4 Server squash
  done: false
- name: M5 Determinism
  done: false
- name: M6 Retention
  done: false
- name: M7 Cache filling
  done: false
- name: M8 Hardening and docs
  done: false
status_log:
- at: '2026-10-30T14:01:11'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-10-30T14:15:16'
  by: LeviGuilherme
  status: draft
  note: Plan updated
- at: '2026-10-30T14:18:53'
  by: LeviGuilherme
  status: active
  note: Approved by Levi on 2026-10-30
---

# Re-attachment after history rewrites

Envelope of the Spec Kit feature [`specs/005-history-reattachment/`](../../specs/005-history-reattachment/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Second feature of release v0.3.0 Event log and convergence (scope chosen by the user on 2026-10-30: FEAT-065 with IMP-005, which the user assigned to v0.3.0; FEAT-066 follows): Gedo data survives local rebases and amends and server-side squash and rebase merges. Rewritten and fetched commits get the tapes of the commits they replace, so their events are the original live events again; tapes no longer attached to the history are reported with their age for the 90-day retention; the event cache is filled when new events arrive.

## Decisions

- **Two entry points** (research R2): Git's rewritten pairs (`post-rewrite`) and fetched commits without pairs; tips of the history are inputs, so the `RefStore` port stays limited to `refs/gedo/*`.
- **Semantic-signature fast path** (research R3, new ADR-0016 amending ADR-0008): equal semantic changes of the old and new commit attach the old tapes directly, instead of Git's patch-id.
- **Matching by entity state** (research R4, R5): candidate events fold into each entity's last state; a change matches when the states are equal, earlier events are superseded; neutral entities (created then deleted, change then revert) and identity merges are accounted for.
- **Attachment rule** (clarification 1, research R7): with a pair, one matched or superseded event suffices; without a pair, every event must be matched, superseded or neutral.
- **Identity links** (clarification 2, research R6): an entity created on the branch is linked to its new id in the squash as a reconstructed identity merge, lower id kept (ADR-0007), derived on demand, never stored.
- **Events of a commit with tapes** (research R8): its tapes, then the reconstructed remainder of the changes they do not account for and the links.
- **Retention date is local** (clarification 3, research R9): first time this machine saw a tape detached, one record in the symbol cache; removal stays in `gedo gc` (FEAT-087).
- **Cache filling** (IMP-005, research R11): `fill_cache` reads every event the cache lacks, called by the fetch path.
- No new crate, port, collection, stored format or third-party dependency.

## Backlog items

### FEAT-065: Re-attachment after rebase, amend and squash
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Rebuild the commit index after local and server-side rewrites by semantic diff and event matching; 90-day retention of unreachable events.

### IMP-005: Fill the event cache when Gedo data is fetched
- **Priority**: low
- **Found**: 2026-10-30, fix/windows-event-performance, Windows reference benchmark
- **Description**: With an empty event cache, the first EventLog::filter reads every event object once (1.5 to 1.7 s at p95 on the Windows reference machine for 10,000 events); filling the events namespace when a fetch or merge brings new events removes the one slow first filter.

## Milestones

Taken from the phases of [`tasks.md`](../../specs/005-history-reattachment/tasks.md): M1 Setup, M2 Matching rules, M3 Local rewrites, M4 Server squash, M5 Determinism, M6 Retention, M7 Cache filling, M8 Hardening and docs.
