# Quickstart: Node Identity

Validation guide for `specs/003-node-identity`. Contract: [identity.md](contracts/identity.md); types: [data-model.md](data-model.md).

## Prerequisites

- Rust 1.95 or later and a C compiler, as for feature 002.
- A clone with full history (`git fetch --unshallow` on a shallow clone) for the history test over the Gedo repository.

## 1. Build and run the tests

```sh
cargo test --workspace
```

Expected: the `gedo-ast` matching, derivation, delta and alias tests (unit and property-based), the `gedo-app` history tests and the identity corpus pass on Windows and Linux.

## 2. Read the identity accuracy report

```sh
cargo test -p gedo-app --test identity_corpus -- --nocapture
```

Expected: a table of accuracy and false continuations per edit category; overall accuracy at least 95%, no category under 90%, false continuations under 1%.

## 3. Index a repository and print its identities

```sh
cargo run -p gedo-app --example index_history -- . HEAD
cargo run -p gedo-app --example index_history -- . HEAD crates/core/gedo-core/src/ids.rs
```

Expected: the number of commits indexed; with a path, one line per entity of that file at `HEAD` with its id, kind and qualified path. Run it twice: the second run indexes nothing. Delete `.git/gedo/symbol_cache.db`, run again: the ids are the same.

## 4. Check a rename by hand

Rename a function in a scratch branch, commit, and run step 3 with the file: the function keeps its id, and `--changes HEAD` lists a `renamed` delta for it.

## 5. Benchmarks

```sh
cargo bench -p gedo-ast --bench match_change -- --quick
cargo bench -p gedo-app --bench index_history -- --quick
```

Expected: matching a 2,000-line file under 5 ms at p95 (SC-004); time per commit and total for the indexed histories (SC-005).
