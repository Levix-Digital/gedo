---
plan_id: gedo_store_foundation
project: gedo
title: Store foundation
kind: feature
version: v0.1.0
status: draft
author: Levi Silveira
created_at: '2026-10-24T03:22:28'
updated_at: '2026-10-24T03:22:28'
session_id: null
tags: []
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-001
- FEAT-002
- FEAT-003
- FEAT-004
- FEAT-005
- FEAT-006
- FEAT-007
- FEAT-119
milestones: []
status_log:
- at: '2026-10-24T03:22:28'
  by: Levi Silveira
  status: draft
  note: Plan created
---

# Store foundation

Plan for the backlog items below (`dev-cycle-protocol` Phases 1 and 2). Replace every instruction paragraph with the plan.

## Backlog items

### FEAT-001: Object store and reference ports with in-memory adapters
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Ports in gedo-core for objects, references, WAL and cache, each with an in-memory adapter for core tests.

### FEAT-002: gitoxide blob and tree adapter
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Read and write blobs and trees in .git/objects with gix, without spawning git or touching the working tree.

### FEAT-003: refs/gedo reference store with compare-and-swap
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Create, list and update references under refs/gedo/* atomically, safe for concurrent CLI and daemon writes.

### FEAT-004: Crash-safe write-ahead log adapter
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Append-only .git/gedo/wal.log with a checksum per record, fsync on consolidation and torn-record recovery.

### FEAT-005: redb symbol cache with rebuild
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Local symbol_cache.db with a schema version, rebuilt from refs/gedo/* on mismatch or corruption (ADR-0004).

### FEAT-006: Refspec setup and transport validation
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Refspecs for refs/gedo/* and validation of push and fetch against GitHub, GitLab and plain remotes, including host retention of custom references.

### FEAT-007: Reference packing measurement and consolidation
- **Priority**: medium
- **Found**: 2026-10-24
- **Description**: Measure how many refs/gedo/* references hosts and fetch tolerate and consolidate into index trees if needed.

### FEAT-119: CI on Windows and Linux
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Build, tests, benchmarks, check_design.py, cargo-deny and cargo-audit on every pull request on both platforms.

## 1. Goal and decisions

What changes and why; decisions with alternatives become ADRs (`architecture-docs`).

## 2. Design (`software-design`)

Where each piece lives, interfaces, data flow, error strategy, dependencies, what is deliberately not abstracted.

## 3. Affected surfaces and files

Interfaces in `architecture.surfaces` that change; files and modules.

## 4. Tests (`quality-assurance`)

Automated tests to add, manual tests to add, change or remove, acceptance criteria.

## 5. Documentation (`architecture-docs`, `user-docs`)

Subsystem pages and ADRs, README sections.

## 6. CI, release and observability (`devops-release`)

Workflow changes, version impact, logs, diagnostics and telemetry to add and how they are tested.

## 7. Security, migration and rollback

Secrets, data migrations (backward compatible), how to roll back.

## 8. Execution steps and milestones
