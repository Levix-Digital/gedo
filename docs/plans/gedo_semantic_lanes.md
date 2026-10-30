---
plan_id: gedo_semantic_lanes
project: gedo
title: Semantic lanes and split publish
kind: feature
version: v0.3.0
status: active
author: LeviGuilherme
created_at: '2026-10-30T17:52:01'
updated_at: '2026-10-30T21:28:51'
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
  done: false
- name: M2 Lanes
  done: false
- name: M3 Refactor builder
  done: false
- name: M4 Split publish
  done: false
- name: M5 Safe fallback
  done: false
- name: M6 Hardening and docs
  done: false
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
