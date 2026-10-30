---
plan_id: gedo_semantic_lanes
project: gedo
title: Semantic lanes and split publish
kind: feature
version: v0.3.0
status: completed
author: LeviGuilherme
created_at: '2026-10-30T17:52:01'
updated_at: '2026-10-30T22:39:12'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-067
spec_dir: specs/007-semantic-lanes
milestones:
- name: M1 Setup
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M2 Lanes
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M3 Refactor builder
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M4 Split publish
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M5 Safe fallback
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
- name: M6 Hardening and docs
  done: true
  done_at: '2026-10-30'
  by: LeviGuilherme
status_log:
- at: '2026-10-30T17:52:01'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-10-30T17:52:27'
  by: LeviGuilherme
  status: active
  note: Plan updated
- at: '2026-10-30T21:28:51'
  by: LeviGuilherme
  status: active
  note: Approved by Levi on 2026-10-30
- at: '2026-10-30T21:52:22'
  by: LeviGuilherme
  note: Milestone 'M1 Setup' completed
- at: '2026-10-30T21:52:24'
  by: LeviGuilherme
  note: Milestone 'M2 Lanes' completed
- at: '2026-10-30T21:52:26'
  by: LeviGuilherme
  note: Milestone 'M3 Refactor builder' completed
- at: '2026-10-30T21:52:28'
  by: LeviGuilherme
  note: Milestone 'M4 Split publish' completed
- at: '2026-10-30T21:52:29'
  by: LeviGuilherme
  note: Milestone 'M5 Safe fallback' completed
- at: '2026-10-30T21:52:31'
  by: LeviGuilherme
  note: Milestone 'M6 Hardening and docs' completed
- at: '2026-10-30T21:52:31'
  by: LeviGuilherme
  status: completed
  note: All milestones completed
- at: '2026-10-30T22:33:42'
  by: LeviGuilherme
  status: completed
  note: 'Delivered in PR #22 (merged 2026-10-30)'
---

# Semantic lanes and split publish

Envelope of the Spec Kit feature [`specs/007-semantic-lanes/`](../../specs/007-semantic-lanes/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Fourth feature of release v0.3.0 Event log and convergence: mixed work is published as a refactor commit followed by a feature commit. Each change of a publish has a lane (refactor or feature), the refactor commit holds the base plus only the refactor changes, the feature commit leads exactly to the developer's tree, and the events of each lane are the tape of their commit. The `gedo publish --split-lanes` command (FEAT-083, v0.4.0) calls this use case and moves the branch.

## Decisions

- **Lanes per part of an entity** (research R2, new ADR-0018): moves and renames are refactor; creations, removals, signature and body changes are feature, except reference follow-ups.
- **Reference follow-ups are refactor** (clarification, research R3, R4): body, signature and import changes whose only difference is the new name or path of a moved or renamed entity, and a module split whose members all moved in, so the refactor commit stands on its own.
- **Refactor builder** (research R5): token diff of the base and final files; equal and follow-up text from the final version, moved entities as blocks rebuilt recursively, every other change kept as in the base.
- **Verified by re-indexing** (research R6): the refactor and feature commits are indexed and their changed parts must equal the lanes; the refactor files must parse.
- **Fallback** (clarification): one lane, a build error or a failed verification publishes one commit with everything and the reason.
- **Generated refactor message** (clarification, research R8), unless the caller gives one; the feature commit takes the caller's message.
- **One tape per commit** (research R7), stored in the order of ADR-0014, the log truncated once.
- No branch moved, no new port, collection or stored format; one new dependency, `similar` (pure diff library, MIT or Apache-2.0).

## Backlog items

### FEAT-067: Semantic lanes and split publish
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Classify events into refactor and feature lanes; gedo publish --split-lanes creates a refactor commit then a feature commit.

## Milestones

Taken from the phases of [`tasks.md`](../../specs/007-semantic-lanes/tasks.md): M1 Setup, M2 Lanes, M3 Refactor builder, M4 Split publish, M5 Safe fallback, M6 Hardening and docs.

## Results

Benchmarks on the Linux development container (release build, 2026-10-30):

| Requirement | p50 | p95 | p99 | Budget |
|---|---:|---:|---:|---|
| Split-publish a rename whose callers span 100 files, a feature edit in each, 1,000 events, Git store adapter (`cargo bench -p gedo-app --bench split`, 25 runs, SC-003) | 215 ms | 246 ms | 282 ms | p95 under 1 s on the reference machine |

Tests: 12 tests in `crates/core/gedo-app/tests/split.rs` (the lanes of scripted work, a rename with its callers, a move across files with the paths that name it, a module split, one lane, nothing to publish, determinism, events per lane on each tape, the fallback for a refactor version that does not parse, feature work that leaves a syntax error, 32 generated mixes that always end at the final tree and split whenever both lanes are present, a pinned outcome), unit tests of the follow-ups and of the refactor message in `gedo-crdt`, and the `split published` event in `tests/event_tracing.rs`. The example `cargo run -p gedo-app --example split` was run on a scratch Git repository.

Deviations from the plan: the lane and builder scenarios run on scripted repositories in `gedo-app/tests/split.rs` instead of `gedo-crdt/tests/lanes.rs`, since they need identity results, and the generated property runs there (32 cases); equal units keep the base's layout with the final tokens, and the text after a follow-up or a moved block continues the final layout; a feature change that also carries follow-ups is feature and changes in both commits; a container's body is left out of the verification, since it changes with its members; whole `use` declarations added or removed for a move are kept as in the base (a known limit in ADR-0018); `publish` and the split share one storing step.

CI on PR #22 passed on Windows and Linux (tests, deny, MSRV and the benches, including the new `split` bench). On the Windows CI runner the split bench gave p50 1.07 s and p95 2.78 s, against 0.20 s and 0.28 s on the Linux runner. Most of the gap was the split reading the base and final trees again for every changed file (about 400 reads of loose objects per split of 100 files). After the merge, the trees are read once per split and the refactor files are written in one batch, so a large refactor is one pack (ADR-0015). The container's p95 after the fix is 260 ms.

Reference machine (Windows 11 Pro build 26200, Intel Core i7-12700KF, SATA SSD, rustc 1.98.1, release build, 2026-10-30):

| Requirement | Commit | p50 | p95 | p99 | Budget |
|---|---|---:|---:|---:|---|
| Split-publish a rename whose callers span 100 files, a feature edit in each, 1,000 events (`cargo bench -p gedo-app --bench split`, 25 runs, SC-003) | `main` after PR #22 | 258 ms | 373 ms | 515 ms | p95 under 1 s |
| The same | with the tree reads fix | 199 ms | 416 ms | 440 ms | p95 under 1 s |

SC-003 is met on both commits. The fix lowers the criterion median from 271 ms to 215 ms; with 25 runs, the p95 is set by the one or two slowest runs. Publishing a tape of 1,000 events (`--bench events`) takes p50 182 ms there, so most of the split's time is the publish it shares.
