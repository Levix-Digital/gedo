---
plan_id: gedo_syntax_model
project: gedo
title: Syntax model
kind: feature
version: v0.2.0
status: active
author: Levi Silveira
created_at: '2026-10-24T17:40:00'
updated_at: '2026-10-24T17:30:00'
session_id: null
tags:
- parsing
- tree-sitter
- structural-hash
- rust
supersedes: null
superseded_by: null
parent_plan_id: null
backlog_items:
- FEAT-009
- FEAT-010
- FEAT-016
spec_dir: specs/002-syntax-model
milestones:
- name: M1 Setup and core model
  done: true
- name: M2 Rust entities
  done: true
- name: M3 Formatting invariance
  done: true
- name: M4 Determinism
  done: false
- name: M5 Error tolerance
  done: false
- name: M6 Rust support
  done: false
- name: M7 Documentation and acceptance
  done: false
status_log:
- at: '2026-10-24T17:40:00'
  by: Levi Silveira
  status: draft
  note: Plan created
- at: '2026-10-24T17:30:00'
  by: Levi Silveira
  status: active
  note: Plan approved
---

# Syntax model

Envelope of the Spec Kit feature [`specs/002-syntax-model/`](../../specs/002-syntax-model/): the specification, plan, research, data model, contract, quickstart and tasks live there and are read with this plan. This body holds only the goal, the decisions and the milestones.

## Goal

First of the two features of release v0.2.0 Node identity: a source file becomes the list of its code entities (kind, qualified path, byte range, container) with structural, signature and body hashes that ignore formatting and comments and are identical on every machine. Rust is the first supported language. Feature `003-node-identity` (FEAT-011 to FEAT-015) builds matching, identities and deltas on it.

## Decisions

- **Three places**: the neutral model and the `Parser` port in `gedo-core`; normalization and hashing as pure functions in the new core crate `gedo-ast`; Tree-sitter and the Rust mapping only in the new adapter `gedo-parse` (research R3).
- **Tree-sitter 0.27 with tree-sitter-rust 0.24**, verified compatible, MIT, MSRV 1.90, about 2.4 ms per 1,000 lines (research R1).
- **Trivia**: whitespace, comments and documentation comments (clarification 1); line endings normalized; trailing separators and the order of `use` and `mod foo;` declarations ignored (research R5).
- **Hash format V1**: BLAKE3 derive-key over class, length and text of each significant token; any rule change needs V2 (research R6).
- **Entities**: items and associated items; enum variants and fields stay in their parent (clarification 2); qualified paths start with `crate::` without the crate name (clarification 3); trait impl items are `<Type as Trait>::item` (research R7).
- **Error tolerance and limits**: errors flag only the entities that contain them; 16 MiB, nesting 512, 2 s per file (research R8).
- **No incremental parsing** until measurements need it (research R10).

## Backlog items

### FEAT-009: Neutral syntax tree model and parser port
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Language-independent syntax tree in gedo-core and the parser port implemented by gedo-parse.

### FEAT-010: Trivia normalization and structural hash
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Separate significant syntax from trivia and compute the BLAKE3 structural hash with line endings normalized.

### FEAT-016: Rust support
- **Priority**: high
- **Found**: 2026-10-24
- **Description**: Grammar mapping, qualified paths and conformance suite for Rust.

## Milestones

Taken from the phases of `tasks.md`:

| Milestone | Tasks |
|---|---|
| M1 Setup and core model | T001–T012 |
| M2 Rust entities | T013–T020 (US1) |
| M3 Formatting invariance | T021–T024 (US2) |
| M4 Determinism | T025–T026 (US3) |
| M5 Error tolerance | T027–T030 (US4) |
| M6 Rust support | T031–T033 (US5) |
| M7 Documentation and acceptance | T034–T039 |
