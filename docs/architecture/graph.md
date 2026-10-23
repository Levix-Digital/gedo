---
title: Symbol Graph
summary: Dependencies between symbols, minimum-cut graft and contract checks.
code: [crates/core/gedo-graph]
---

# Symbol Graph

Dependencies between symbols, minimum-cut graft and contract checks.

## Responsibilities

- Connects `AstNodeId`s in a directed dependency graph (A calls B, B implements C), across file boundaries.
- Computes the **minimum cut for graft**: when a symbol moves to another branch or stream, the minimal transitive closure of dependent nodes that must move with it to keep syntactic and type integrity.
- Detects silent API contract breaks before formal compilation.
- Does not run a language server; precise types come later through a port with an LSP adapter.

## Structure

Pure graph structures and algorithms over core types.

## Behavior

The graph is built from references found in the neutral syntax tree first; a type oracle port backed by language servers refines it after the MVP.

## Interfaces

- Called by `gedo-app` (`graft`, `status`, contract checks).
- Uses `gedo-core` types and the identity results.

## Decisions

- [ADR-0001](../adr/0001-hexagonal-architecture-over-a-bottom-up-crate-stack.md): graph algorithms are pure core logic.
