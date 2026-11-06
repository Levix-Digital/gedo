---
plan_id: gedo_virtual_streams
project: gedo
title: Virtual streams
kind: feature
version: v0.3.0
status: completed
author: LeviGuilherme
created_at: '2026-11-06T15:09:57'
updated_at: '2026-11-06T15:40:05'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-068
spec_dir: specs/009-virtual-streams
milestones:
- name: M1 Setup
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M2 Model and store
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M3 Labeled streams
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M4 Inherited context
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M5 Switching
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M6 Publish
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M7 Converge and discard
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
- name: M8 Hardening and docs
  done: true
  done_at: '2026-11-06'
  by: LeviGuilherme
status_log:
- at: '2026-11-06T15:09:57'
  by: LeviGuilherme
  status: draft
  note: Plan created
- at: '2026-11-06T15:16:32'
  by: LeviGuilherme
  status: active
  note: Plan updated
- at: '2026-11-06T15:22:22'
  by: LeviGuilherme
  status: active
  note: Approved by Levi on 2026-11-06
- at: '2026-11-06T15:39:58'
  by: LeviGuilherme
  note: Milestone 'M1 Setup' completed
- at: '2026-11-06T15:39:59'
  by: LeviGuilherme
  note: Milestone 'M2 Model and store' completed
- at: '2026-11-06T15:40:00'
  by: LeviGuilherme
  note: Milestone 'M3 Labeled streams' completed
- at: '2026-11-06T15:40:01'
  by: LeviGuilherme
  note: Milestone 'M4 Inherited context' completed
- at: '2026-11-06T15:40:02'
  by: LeviGuilherme
  note: Milestone 'M5 Switching' completed
- at: '2026-11-06T15:40:03'
  by: LeviGuilherme
  note: Milestone 'M6 Publish' completed
- at: '2026-11-06T15:40:04'
  by: LeviGuilherme
  note: Milestone 'M7 Converge and discard' completed
- at: '2026-11-06T15:40:05'
  by: LeviGuilherme
  note: Milestone 'M8 Hardening and docs' completed
- at: '2026-11-06T15:40:05'
  by: LeviGuilherme
  status: completed
  note: All milestones completed
---

# Virtual streams

Envelope of the Spec Kit feature [`specs/009-virtual-streams/`](../../specs/009-virtual-streams/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Sixth feature of release v0.3.0 Event log and convergence: work is isolated in named **virtual streams** kept by Gedo on the developer's machine, switched without `git stash` or `git checkout`. A stream has a base commit, an author kind (the person of the Git identity, or an agent), opaque labels set at creation, its own pending events and its saved work (a tree). Events recorded inside a stream inherit its author and labels; a tool finds its streams again by label. Publishing a stream writes its commit on its base with its events as the commit's tape.

## Decisions

- **What a stream isolates** (clarification, research R7): its context (author kind, labels), its own pending events and its work as a tree in the object store; switching saves the caller's current work tree into the stream left and returns the target's. Gedo writes no working file, index, `HEAD` or branch; the CLI (v0.4.0) writes the returned tree.
- **Local only** (clarification, R2, new ADR-0020): one record and one write-ahead log per stream in `.git/gedo/streams/<hex name>/`, the active stream and outside work per working tree, through a new `StreamStore` port; saved work kept alive by work commits pinned under `refs/gedo-local/`, which is never pushed or fetched (ADR-0011).
- **Publish** (clarification, R9): writes the commit, records it in the stream, publishes the events as its tape (`EventLog::publish`), then moves the stream onto it; an interrupted publish is completed with the same commit. No branch moves; the stream stays open until discarded.
- **Context** (R6): inside a stream its author and labels; outside, `GEDO_AGENT` and `GEDO_LABELS` as today; inside a stream the environment is ignored.
- Streams are convergence sides of feature 006 (R11); discard refuses to drop pending events or saved work without an explicit flag.
- The event format and every existing stored format are unchanged; new formats (stream record, working tree state) are versioned and freeze at v1.0.0.

## Backlog items

### FEAT-068: Virtual streams
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: gedo stream <name>: switch or isolate work in memory without git stash or checkout. A stream carries the same opaque labels as events (FEAT-063), set at creation, and an optional agent author (--agent <id>), so a tool can open one stream per plan milestone or per agent and find it again with gedo stream list --label key=value --json. Every event recorded inside a stream inherits its author and labels; outside a stream the author is the person of the Git identity, or the agent and labels named by GEDO_AGENT and GEDO_LABELS. The prefix gedo. is reserved for Gedo's own keys (ADR-0013).

## Milestones

Taken from the phases of [`tasks.md`](../../specs/009-virtual-streams/tasks.md): M1 Setup, M2 Model and store, M3 Labeled streams, M4 Inherited context, M5 Switching, M6 Publish, M7 Converge and discard, M8 Hardening and docs.

## Results

Tests: `crates/core/gedo-app/tests/streams.rs` (23 tests: one or more per acceptance scenario of US1 to US5, with the stores reopened as a new process would, a linked working tree, eight concurrent creations of one name, a publish interrupted at each of its three writes, a property of 256 cases that every switch returns the tree last saved for its target, and an `insta` snapshot of a scripted session), the `StreamStore` contract (3 checks) on the memory and file adapters, `crates/adapters/gedo-store/tests/streams_gc.rs` (pinned work through `git gc --prune=now`, unpinned and discarded work pruned, linked working trees with their own state, names Windows reserves, pins not pushed), unit and property tests of names, records, filters and contexts in `gedo-core::stream` and `gedo-crdt::streams`, and the stream events in `tests/event_tracing.rs`.

Benchmark on the Linux development container (release build, Git store adapter, `cargo bench -p gedo-app --bench streams -- --quick`, 20 runs each, 2026-11-06):

| Operation | p50 | p95 | Budget (SC-003, reference machine) |
|---|---:|---:|---|
| Create a stream | 0.51 ms | 0.84 ms | under 10 ms |
| Switch between two streams | 1.28 ms | 1.92 ms | under 10 ms |
| Read a stream | 4 µs | 4 µs | under 10 ms |
| List 1,000 streams (filter by label) | 3.6 ms | 6.5 ms | under 100 ms |
| Publish a stream of 1,000 events | 7.6 ms | 18.3 ms | adds under 10 ms to `publish` |
| `EventLog::publish` of the same 1,000 events | 4.8 ms | 6.7 ms | |

Publishing a stream adds 2.9 ms at p50 and 11.5 ms at p95 on the container (two record writes and one pin besides the commit); the reference machine run decides SC-003.

Deviations from the plan: a refused stream record is a `StoreError::Corrupted` naming the stream instead of a separate `StreamRecordError`; `stream_context` returns a `Result` (a reserved label, which creation and reading never let through); the example creates and lists streams instead of publishing `HEAD`'s tree (with no events, that is nothing to publish); examples need no `[[example]]` entry; the `git gc` checks run on the port in `gedo-store`, since `gedo-store` cannot use `gedo-app`.
