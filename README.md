# Gedo

Gedo (Git Event-Driven Overlay) is a collaboration overlay built directly on Git's object store. It lets a team collaborate on code entities (functions, types, modules) instead of lines of text: conflicts merged by meaning, collisions found while they are born, and history per entity, without a mandatory server, without annotations in source files and without breaking any existing Git tool. See [the vision](docs/product/VISION.md) for the full picture.

## Features

Gedo is a **library only** today: the store foundation that every later feature builds on, and the syntax model that node identity builds on. There is no `gedo` command or daemon yet.

- `gedo-core`: the ports, commit-chain collections, in-memory adapters and the contract suite.
- `gedo-store`: the real adapters. Gedo data lives as commit chains under `refs/gedo/*` in the repository itself, next to a crash-safe local write-ahead log and a rebuildable redb cache in `.git/gedo/`. Nothing in the working tree, the index or any branch is touched.
- `gedo-ast` and `gedo-parse`: a Rust source file becomes its code entities (functions, types, modules, implementations), each with a qualified path and structural hashes that ignore formatting and comments and are identical on every machine.
- Gedo data travels through ordinary Git remotes (file, SSH, HTTPS; GitHub and GitLab are validated weekly). A plain `git fetch` brings it once the remote is set up, and branch pushes behave exactly as without Gedo.

## Requirements

- Linux or Windows.
- Rust 1.95 or later, Git 2.34 or later and a C compiler for the Tree-sitter grammar (MSVC on Windows, `cc` on Linux). <!-- readme-ok -->
- SHA-1 repositories only; SHA-256 repositories are refused.

## Installation

Gedo is not published yet. Build it from source:

```text
git clone https://github.com/Levix-Digital/gedo
cd gedo
cargo build --workspace
cargo test --workspace
```

## Usage

To see the entities of a Rust file and their hashes:

```text
cargo run -p gedo-parse --example syntax_map -- <file.rs>
```

To see the store at work on a scratch repository, run the example, then look at the repository with Git:

```text
cargo run -p gedo-store --example roundtrip -- <repository>
git status
git for-each-ref refs/gedo
```

Development commands:

```text
cargo xtask arch-check            # the core depends on no adapter or infrastructure crate
cargo deny check                  # licenses, duplicate versions and RustSec advisories
cargo bench -p gedo-store --bench wal_append
cargo bench -p gedo-parse --bench parse_rust
cargo xtask parse-sweep           # parse every Rust file of the workspace and its dependencies
cargo xtask format-corpus         # regenerate the rustfmt corpus of the parser tests
cargo xtask scale-study           # push and fetch cost of large collections
```

The SSH remote test needs `openssh-server` on Linux and runs with `GEDO_TEST_SSH=1 cargo test -p gedo-store --test transport -- --include-ignored`. The GitHub and GitLab round trip runs from the `host-validation` workflow.

## Configuration

There is nothing to configure yet: the library takes the repository path from its caller.

## Troubleshooting

| Problem | Fix |
|---|---|
| The build fails in `tree-sitter` or `tree-sitter-rust` | Install a C compiler: the Visual Studio Build Tools on Windows, `build-essential` or `gcc` on Linux |
| `cargo xtask parse-sweep` reports missing files | Run `cargo fetch` first: the sweep reads the sources of the locked dependencies from the local Cargo registry |
| `git log --all` or `gitk --all` shows unrelated histories with commits like `gedo: ...` | These are Gedo's chains under `refs/gedo/*`. Branch views, `git log --branches --remotes`, hosting UIs and IDEs do not show them; see the [technical documentation](TECDOC.md) |

## Documentation

- [Technical documentation](TECDOC.md) and [architecture](docs/architecture/)
- [Decisions](docs/adr/)
- [Roadmap](docs/ROADMAP.md)

## License

Licensed under either of the MIT license or the Apache License, Version 2.0, at your option.
