# Gedo

Gedo (Git Event-Driven Overlay) is a collaboration overlay built directly on Git's object store. It lets a team collaborate on code entities (functions, types, modules) instead of lines of text: conflicts merged by meaning, collisions found while they are born, and history per entity, without a mandatory server, without annotations in source files and without breaking any existing Git tool. See [the vision](docs/product/VISION.md) for the full picture.

## Status

Version 0.1.0 is a **library only**: the store foundation that every later feature builds on, and the syntax model of the next release. There is no `gedo` command or daemon yet.

- `gedo-core`: the ports, commit-chain collections, in-memory adapters and the contract suite.
- `gedo-store`: the real adapters. Gedo data lives as commit chains under `refs/gedo/*` in the repository itself, next to a crash-safe local write-ahead log and a rebuildable redb cache in `.git/gedo/`. Nothing in the working tree, the index or any branch is touched.
- `gedo-ast` and `gedo-parse`: a Rust source file becomes its code entities (functions, types, modules, implementations), each with a qualified path and structural hashes that ignore formatting and comments and are identical on every machine. This is the base of node identity (v0.2.0).
- Gedo data travels through ordinary Git remotes (file, SSH, HTTPS; GitHub and GitLab are validated weekly). A plain `git fetch` brings it once the remote is set up, and branch pushes behave exactly as without Gedo.

Platforms: Linux and Windows. SHA-1 repositories only; SHA-256 repositories are refused.

## Build and test

Requires Rust 1.95 or later, Git 2.34 or later and a C compiler for the Tree-sitter grammar (MSVC on Windows, `cc` on Linux).

```text
cargo build --workspace
cargo test --workspace
cargo xtask arch-check            # the core depends on no adapter or infrastructure crate
cargo deny check                  # licenses, duplicate versions and RustSec advisories
cargo bench -p gedo-store --bench wal_append
cargo bench -p gedo-parse --bench parse_rust
cargo xtask parse-sweep           # parse every Rust file of the workspace and its dependencies
cargo xtask format-corpus         # regenerate the rustfmt corpus of the parser tests
cargo xtask scale-study           # push and fetch cost of large collections
```

The SSH remote test needs `openssh-server` on Linux and runs with `GEDO_TEST_SSH=1 cargo test -p gedo-store --test transport -- --include-ignored`. The GitHub and GitLab round trip runs from the `host-validation` workflow.

To see the entities of a Rust file and their hashes, run `cargo run -p gedo-parse --example syntax_map -- <file.rs>`. To see the store at work on a scratch repository, run `cargo run -p gedo-store --example roundtrip -- <repository>`, then `git status` and `git for-each-ref refs/gedo`.

## A note on `git log --all`

Tools that list every reference, such as `git log --all` or `gitk --all`, show Gedo's chains as separate histories with commits like `gedo: ...`, unrelated to your code. Branch views, `git log --branches --remotes`, hosting UIs and IDEs do not show them. See [ADR-0011](docs/adr/0011-gedo-references-as-commit-chains-with-a-separate-fetch-names.md).

## Documentation

- [Technical documentation](TECDOC.md) and [architecture](docs/architecture/)
- [Decisions](docs/adr/)
- [Roadmap](docs/ROADMAP.md)

## License

Licensed under either of the MIT license or the Apache License, Version 2.0, at your option.
