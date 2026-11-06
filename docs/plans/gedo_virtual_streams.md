---
plan_id: gedo_virtual_streams
project: gedo
title: Virtual streams
kind: feature
version: v0.3.0
status: active
author: LeviGuilherme
created_at: '2026-11-06T15:09:57'
updated_at: '2026-11-06T15:22:22'
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
  done: false
- name: M2 Model and store
  done: false
- name: M3 Labeled streams
  done: false
- name: M4 Inherited context
  done: false
- name: M5 Switching
  done: false
- name: M6 Publish
  done: false
- name: M7 Converge and discard
  done: false
- name: M8 Hardening and docs
  done: false
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
