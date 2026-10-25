---
plan_id: gedo_node_identity
project: gedo
title: Node identity
kind: feature
version: v0.2.0
status: active
author: Levi Silveira
created_at: '2026-10-25T07:53:15'
updated_at: '2026-10-25T08:36:00'
session_id: null
tags:
- identity
- matching
- deltas
- history-indexing
- rust
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-011
- FEAT-012
- FEAT-013
- FEAT-014
- FEAT-015
spec_dir: specs/003-node-identity
milestones:
- name: M1 Setup and identity model
  done: false
- name: M2 Matching
  done: false
- name: M3 Deltas
  done: false
- name: M4 Determinism and reconciliation
  done: false
- name: M5 History indexing
  done: false
- name: M6 Identity corpus
  done: false
- name: M7 Documentation and acceptance
  done: false
status_log:
- at: '2026-10-25T07:53:15'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-25T08:16:33'
  by: Levi Silveira
  status: draft
  note: Plan updated
- at: '2026-10-25T08:36:00'
  by: Levi Silveira
  status: active
  note: Approved by Levi on 2026-10-25
---

# Node identity

Envelope of the Spec Kit feature [`specs/003-node-identity/`](../../specs/003-node-identity/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

Second and last feature of release v0.2.0 Node identity: every Rust code entity gets an identity it keeps through edits, signature changes, renames, moves and formatter runs, derived deterministically so the same history gives the same ids on every machine and in every indexing order, including history made without Gedo; each change is described as semantic deltas, the input of the event log of v0.3.0. Identity accuracy is measured on a reference corpus (95% target).

## Decisions

- **Change-set matching** (clarification 1, research R3): the unit of matching is the set of Rust files changed by a commit, so an entity moved to another file keeps its id. Tiers: exact; same place, same content elsewhere and a scored similarity at 80% (integer score: signature, body and scope, research R5); spawn or tombstone.
- **Whole history** (clarification 2, research R10): indexing always walks every ancestor to every root, parses only changed files, and orders commits topologically with ties by id.
- **Identity results are local derived data computed per commit** (research R2, new ADR-0012): an identity depends on history, so a shared per-blob record is not deterministic; results live in the local symbol cache and are recomputed on each machine (cheaper than sharing). This amends TECH_SPEC's data table and ADR-0003; shared identity data travels with the events of v0.3.0.
- **Derivation** (research R7): UUID v8 from BLAKE3 over kind, unit, qualified path, occurrence and structural hash, with a deterministic collision counter; `MatchFormat::V1` versions all rules. No `uuid` crate yet.
- **Unit of an entity** (research R4): the parser reports the compilation unit's directory, which tells crates of a workspace apart without reading manifests.
- **Reconciliation** (research R11): identity merges resolve every id to the lowest id of its connected component; found at merge commits during indexing.
- **Syntax errors** (research R8): unmatched entities of a damaged file are carried, not tombstoned.
- **Accuracy** (clarification 3, research R9): measured on seeded scripted edits of real code with known outcomes; real histories prove determinism.
- **New core crate `gedo-app`** (research R1) with the indexing use case; no new port and no new third-party dependency.

## Backlog items

### FEAT-011: Three-tier node matching
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Exact match, signature and scope heuristics (80% threshold as a tuned constant) and spawn or tombstone fallback.

### FEAT-012: Deterministic identity derivation and alias reconciliation
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: UUID v8 ids derived from content, versioned matching and IdentityMerged aliases at merge time (ADR-0007).

### FEAT-013: Semantic delta emission
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: NodeCreated, NodeMoved, NodeRenamed, SignatureMutated, BodyModified and NodeTombstoned deltas.

### FEAT-014: Reference identity corpus and benchmarks
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Corpus of renames, moves and formatter runs measuring accuracy (95% target) and determinism across machines.

### FEAT-015: Indexing of history made without Gedo
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Index existing repositories and commits made by teammates without Gedo, with the same ids on every machine.
