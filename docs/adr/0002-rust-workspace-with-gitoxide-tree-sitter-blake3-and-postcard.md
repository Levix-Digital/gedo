---
id: 2
title: Rust workspace with gitoxide, Tree-sitter, BLAKE3 and Postcard
status: accepted
date: '2026-10-23'
version: v0.1.0
subsystems: [core, store, parsing, events, surfaces]
supersedes: []
superseded_by: []
amends: []
amended_by: [6]
---

# ADR-0002: Rust workspace with gitoxide, Tree-sitter, BLAKE3 and Postcard

## Context

Gedo runs next to every Git operation of a developer and in a background daemon that must react to file saves within milliseconds. It must read and write Git objects without spawning the `git` process, parse many languages incrementally, hash syntax structurally and serialize events compactly. It ships as native binaries for Windows and Linux first, macOS later. The Levix team works in Rust daily.

## Decision

- **Language**: Rust, stable toolchain, one Cargo workspace (layout in ADR-0001), edition 2024. Versions of every dependency are pinned in `[workspace.dependencies]`; the exact versions and the minimum supported Rust version are recorded in the technical specification.
- **Git access**: `gix` (gitoxide), a pure-Rust Git implementation, used only inside `gedo-store`. No `git` subprocess in production code.
- **Parsing**: `tree-sitter` with one grammar crate per supported language, used only inside `gedo-parse`, which translates Tree-sitter trees into the core's neutral syntax tree.
- **Structural hashing**: `blake3` over the normalized token stream (trivia, whitespace and comments removed; line endings normalized).
- **Serialization**: `postcard` with `serde` for event tapes, AST maps and WAL records, each record prefixed with a format version.
- **Identifiers**: `uuid` v7 for new `AstNodeId`s (time-ordered, good index locality); v4 accepted when reading.
- **Async runtime**: `tokio`, only in `gedo-net` and `gedod`. The core and the store, parse and CLI paths are synchronous.

## Alternatives Considered

- **libgit2 (`git2`)**: mature, but a C dependency that complicates static Windows builds and cross-compilation; gitoxide is pure Rust and faster on object access.
- **Spawning the `git` CLI**: simplest, but slow for many small object writes and contrary to the blueprint (no shell invocation).
- **Bincode for serialization**: the blueprint mentions it as an alternative; Postcard is smaller on the wire, `no_std`-friendly and has a documented stable format.
- **SHA-256 or xxHash for the structural hash**: SHA-256 is slower; xxHash is not cryptographic, and the hash is used as a content identity across machines.
- **async everywhere**: adds complexity to deterministic core code with no benefit for local, CPU-bound work.

## Consequences

- gitoxide's write and reference-transaction coverage must be validated early (Phase 1); `gedo-store` hides it behind the store port so a fallback stays possible.
- Each new language is a grammar crate plus a mapping in `gedo-parse` and a conformance suite.
- Format version prefixes allow evolving tapes and WAL records without breaking existing repositories.
