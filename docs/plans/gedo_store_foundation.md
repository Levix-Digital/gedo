---
plan_id: gedo_store_foundation
project: gedo
title: Store foundation
kind: feature
version: v0.1.0
status: active
author: Levi Silveira
created_at: '2026-10-24T03:22:28'
updated_at: '2026-10-24T03:44:22'
session_id: null
tags:
- store
- gitoxide
- wal
- redb
- ci
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
spec_dir: specs/001-store-foundation
milestones:
- name: M1 Setup and core
  done: false
- name: M2 Objects and references
  done: false
- name: M3 Write-ahead log
  done: false
- name: M4 Transport
  done: false
- name: M5 CI
  done: false
- name: M6 Cache
  done: false
- name: M7 Scale study
  done: false
- name: M8 Documentation and acceptance
  done: false
status_log:
- at: '2026-10-24T03:22:28'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-24T03:44:22'
  by: Levi Silveira
  status: active
  note: Plan updated
---

# Store foundation

Envelope of the Spec Kit feature [`specs/001-store-foundation/`](../../specs/001-store-foundation/): the specification, plan, research, data model, contracts, quickstart and tasks live there and are read with this plan (`get_plan.py`, search, drift detection). This body holds only the goal, the decisions and the milestones.

## Goal

Release v0.1.0 Store foundation: the Rust workspace and Gedo's persistence foundation. Gedo data is written to and read from the Git object store under `refs/gedo/*` atomically, without touching the working tree, the index or branches; it travels through plain `git push` and `git fetch`; local work survives crashes; the local cache can always be rebuilt; and every pull request is checked on Windows and Linux.

## Decisions

- **Commit-chain collections** (ADR-0011, proposed with this plan): one reference per collection pointing to a chain of commits with fan-out trees, because a spike showed that remote references to blobs or trees can only be updated with a forced push.
- **Fetch into `refs/gedo-remote/<remote>/*`**, union-merged into the local chains; **no push refspec**; the push of `refs/gedo/*` runs from the pre-push hook (v0.4.0), fast-forward only (clarification 3; ADR-0011 amends ADR-0009's subprocess scope).
- **WAL**: framed records with CRC-32 and format version, 100 ms group commit, single writer with an OS lock and a 2 s wait (clarification 1).
- **Host validation**: file remote on every pull request (Windows and Linux), SSH remote on Linux, GitHub and GitLab in a manual and weekly workflow against private test repositories (clarification 2).
- **Architecture check in CI** from `cargo metadata` (`cargo xtask arch-check`); `check_design.py` run locally before the pull request.
- **Advisories through `cargo deny`** instead of a separate `cargo-audit` run (justified in the plan's Complexity Tracking).

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

## Milestones

Taken from the phases of `tasks.md`:

| Milestone | Tasks |
|---|---|
| M1 Setup and core | T001–T014 |
| M2 Objects and references | T015–T024 (US1, US2) |
| M3 Write-ahead log | T025–T029 (US3) |
| M4 Transport | T030–T032 (US5) |
| M5 CI | T033–T035 (US7) |
| M6 Cache | T036–T038 (US4) |
| M7 Scale study | T039–T040 (US6) |
| M8 Documentation and acceptance | T041–T048 |

## Benchmark results

`cargo bench -p gedo-store --bench wal_append`, 1 KiB records, 10,000 appends (SC-003: p95 under 1 ms):

| Machine | p50 | p95 | p99 | SC-003 |
|---|---:|---:|---:|---|
| Linux container, 4 vCPUs, Git 2.43 (2026-10-24) | 1.3 µs | 23.8 µs | 30.3 µs | pass |
| GitHub Actions `windows-latest`, `--quick` (2026-10-24) | 2.3 µs | 3.4 µs | 27.8 µs | pass |
| GitHub Actions `ubuntu-latest`, `--quick` (2026-10-24) | 1.4 µs | 3.2 µs | 6.0 µs | pass |

Every run is well under the 1 ms limit on both platforms. These are shared runners and a cloud container rather than the 4-core SSD reference machine; the CI bench job uploads its report on every pull request for trend tracking.
