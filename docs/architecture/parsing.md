---
title: Parsing Adapter
summary: Turns source files into the core's neutral syntax tree with Tree-sitter.
code: [crates/adapters/gedo-parse]
---

# Parsing Adapter

Turns source files into the core's neutral syntax tree with Tree-sitter.

## Responsibilities

- Implements the parser port with Tree-sitter, incrementally when a previous tree is available.
- Maps each language's grammar nodes to the core's entity kinds and qualified paths.
- Holds one grammar crate per supported language and its conformance suite.

## Structure

One module per language: grammar, node-kind mapping and qualified-path rules.

## Behavior

Languages are added in this order: Rust; then TypeScript and Python (the Levix stacks); then about 30 languages ordered by worldwide usage (GitHub Octoverse and Stack Overflow rankings combined with Reciprocal Rank Fusion), limited to mature grammars. A language is declared supported only when its conformance suite passes.

## Interfaces

- Implements the parser port of `gedo-core`; used by `gedo-app` through it.

## Decisions

- [ADR-0002](../adr/0002-rust-workspace-with-gitoxide-tree-sitter-blake3-and-postcard.md): Tree-sitter, isolated in this crate.
