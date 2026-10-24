# Quickstart: Syntax Model

Validation guide for `specs/002-syntax-model`. Contracts: [parser-port.md](contracts/parser-port.md); types: [data-model.md](data-model.md).

## Prerequisites

- Rust 1.95 or later, and a C compiler for the grammar (MSVC on Windows, `cc` on Linux).
- `cargo fetch` done once, so the real-world sweep finds the sources of the locked dependencies.

## 1. Build and run the tests

```sh
cargo test --workspace
```

Expected: the `gedo-ast` property tests, the `gedo-parse` conformance suite and the formatter, change and error-injection corpora pass on Windows and Linux.

## 2. Parse a file and print its syntax map

```sh
cargo run -p gedo-parse --example syntax_map -- crates/core/gedo-core/src/ids.rs
```

Expected: one line per entity with kind, qualified path (`crate::ids::GitOid::from_hex`, ...), byte range and structural hash.

## 3. Check formatter invariance by hand

```sh
cp crates/core/gedo-core/src/ids.rs /tmp/ids.rs
rustfmt --edition 2024 --config max_width=60,trailing_comma=Never /tmp/ids.rs
cargo run -p gedo-parse --example syntax_map -- /tmp/ids.rs
```

Expected: the same hashes as in step 2 (only byte ranges differ). Paths differ because the file is outside `src/`; compare the hashes.

## 4. Real-world sweep

```sh
cargo xtask parse-sweep
```

Expected: a report with files, lines, crashes (0) and files with syntax errors (0 outside `known_grammar_gaps.toml`).

## 5. Benchmark

```sh
cargo bench -p gedo-parse --bench parse_rust
```

Expected: p95 under 10 ms for the 2,000-line file on the reference machine (SC-001).

## 6. Fuzzing (Linux, nightly)

```sh
cargo +nightly fuzz run parse_rust -- -max_total_time=600
```

Expected: no crash, hang or out-of-memory in 10 minutes (SC-007).
