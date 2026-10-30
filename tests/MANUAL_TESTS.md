# Manual Tests

Curated manual test suite: one section per feature, each test describing how the software behaves **today**. Tests are edited in place when behavior changes and removed when they no longer apply. Only what cannot be automated is tested here: Gedo is a library without a `gedo` command yet, so these tests cover the build on a real machine and the developer examples on real repositories. Every rule of the core is covered by automated tests.

How to record a run: in each test of the current run, mark exactly one box of the `Result` line with `x` and, for `Fail` or `Blocked`, describe what happened in `Notes`.

## Installation

### MT-INST-001: Build and test from source on Windows

- **Covers**: build of the workspace, README installation steps
- **Prerequisites**: Windows 10 22H2 or later, Rust 1.95 or later, Git 2.34 or later, the Visual Studio Build Tools (C compiler)
- **Steps**:
  1. `git clone https://github.com/Levix-Digital/gedo`
  2. `cd gedo`
  3. `cargo build --workspace`
  4. `cargo test --workspace`
- **Expected**:
  - The build finishes without errors.
  - Every test result line ends with `0 failed`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-INST-002: Build and test from source on Linux

- **Covers**: build of the workspace, README installation steps
- **Prerequisites**: Linux x86_64 or aarch64 with glibc 2.31 or later, Rust 1.95 or later, Git 2.34 or later, `gcc` or `build-essential`
- **Steps**:
  1. `git clone https://github.com/Levix-Digital/gedo`
  2. `cd gedo`
  3. `cargo build --workspace`
  4. `cargo test --workspace`
- **Expected**:
  - The build finishes without errors.
  - Every test result line ends with `0 failed`.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Syntax Map

### MT-PARSE-001: Print the entities of a Rust file

- **Covers**: `gedo-parse` example `syntax_map`
- **Prerequisites**: the workspace built (MT-INST-001 or MT-INST-002 steps 1 to 3)
- **Steps**:
  1. `cargo run -p gedo-parse --example syntax_map -- crates/core/gedo-crdt/src/tree.rs`
- **Expected**:
  - One line per entity of the file, each with its kind, qualified path (such as `crate::tree::fold`), byte range and structural hash.
  - Running the command twice prints the same lines.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Store

### MT-STORE-001: Write Gedo data without touching the working tree

- **Covers**: `gedo-store` example `roundtrip`, references under `refs/gedo/*`
- **Prerequisites**: the workspace built; a scratch Git repository with one commit (`git init scratch`, add a file, `git commit`)
- **Steps**:
  1. `cargo run -p gedo-store --example roundtrip -- <scratch>`
  2. `git -C <scratch> status`
  3. `git -C <scratch> for-each-ref refs/gedo`
  4. `git -C <scratch> log --oneline --branches`
- **Expected**:
  - Step 1 prints `wrote entry <id> in commit <oid>` and the references of `refs/gedo/` with their entry counts.
  - Step 2 reports a clean working tree.
  - Step 3 lists `refs/gedo/roundtrip`.
  - Step 4 shows only the commits made in the prerequisites.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

## Event Log and Convergence

### MT-EVT-001: Record, publish and read the events of a commit

- **Covers**: `gedo-app` example `events`, collections `events`, `tapes` and `commit-tapes`
- **Prerequisites**: the workspace built; a scratch Git repository with two commits, the second editing a function of a `.rs` file
- **Steps**:
  1. `cargo run -p gedo-app --example events -- <scratch> HEAD`
  2. `git -C <scratch> for-each-ref refs/gedo`
  3. `git -C <scratch> status`
- **Expected**:
  - Step 1 prints `<n> events at <commit>` with one line per event, then `published tape <id> with <n> events` and `<n> stored events carry dc.milestone=M1`.
  - Step 2 lists `refs/gedo/events`, `refs/gedo/tapes` and `refs/gedo/commit-tapes`.
  - Step 3 reports a clean working tree.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-EVT-002: Converge two branches with a change on each and one conflict

- **Covers**: `gedo-app` example `converge`, tree CRDT
- **Prerequisites**: the workspace built; a scratch Git repository whose `main` has `src/lib.rs` with `pub fn total() -> u32 { 1 }` and `pub fn tax() -> u32 { 2 }`; a branch `left` from `main` that changes the body of `total` to `10` and adds `pub fn fee() -> u32 { 3 }`; a branch `right` from `main` that changes the body of `total` to `20` and adds `pub fn discount() -> u32 { 4 }`
- **Steps**:
  1. `cargo run -p gedo-app --example converge -- <scratch> main left right`
  2. Run step 1 again with the tips swapped: `... main right left`
  3. `git -C <scratch> for-each-ref refs/gedo`
- **Expected**:
  - Step 1 prints `side 0: 1 commits, 2 events` and `side 1: 1 commits, 2 events`, a `changed` line for `crate::fee` and one for `crate::discount`, and exactly one `conflict Part(Body)` line, on the entity of `total`, with one value on side `[0]` and another on side `[1]`.
  - Step 2 prints the same entities and the same conflict, with the side numbers swapped.
  - Step 3 prints nothing: the convergence writes no Gedo data.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-EVT-003: Re-attach the tapes of a squashed branch

- **Covers**: `gedo-app` example `reattach`
- **Prerequisites**: the workspace built; a scratch Git repository with one commit
- **Steps**:
  1. `cargo run -p gedo-app --example reattach -- <scratch>`
  2. `git -C <scratch> status`
  3. `git -C <scratch> branch`
- **Expected**:
  - Step 1 prints two `detached before` lines, then `<squash commit>: 2 tapes attached`, and ends with `detached after: 0`.
  - Step 2 reports a clean working tree and step 3 lists only the branches that existed before.
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
