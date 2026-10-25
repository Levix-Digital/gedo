# Roadmap

The releases of the product in delivery order, from the MVP onwards. Each release has a goal and the backlog items (`docs/BACKLOG.md`) it delivers; the roadmap is the only place that says which item goes into which release. Discovery starts from the open items of the current release (the first one not released), after the bugs that come first.

Each release: `## vX.Y.Z: <name>` with the fields `Status` (planned, in_progress, released), an optional `Target`, `Released` (the date, once released), `Goal` and `Items` (backlog ids separated by commas, `-` when empty). Edit this file with `plan-memory/roadmap.py` or by hand, and check it with `roadmap.py validate`.

## v0.1.0: Store foundation
- **Status**: planned
- **Goal**: Gedo data can be written to and read from the Git object store under refs/gedo/*, atomically, and travels through plain git push and fetch on GitHub, GitLab and plain remotes; CI runs on Windows and Linux.
- **Items**: FEAT-001, FEAT-002, FEAT-003, FEAT-004, FEAT-005, FEAT-006, FEAT-007, FEAT-119

## v0.2.0: Node identity
- **Status**: planned
- **Goal**: Rust code entities keep a stable, deterministic identity through edits, renames, moves and formatting, on any machine, including history made without Gedo.
- **Items**: FEAT-009, FEAT-010, FEAT-011, FEAT-012, FEAT-013, FEAT-014, FEAT-015, FEAT-016

## v0.3.0: Event log and convergence
- **Status**: planned
- **Goal**: Semantic events are recorded, survive rebases and squashes, and converge deterministically; work can be split into lanes and isolated in virtual streams.
- **Items**: FEAT-063, FEAT-064, FEAT-065, FEAT-066, FEAT-067, FEAT-068, FEAT-069

## v0.4.0: MVP
- **Status**: planned
- **Goal**: The Levix team uses gedo as its only Git command on a real Rust repository: semantic merges, collisions found by the shadow merge, merges that compile, trace, graft and split publishes, with the daemon running on Windows and Linux.
- **Items**: FEAT-070, FEAT-071, FEAT-072, FEAT-073, FEAT-074, FEAT-075, FEAT-076, FEAT-077, FEAT-078, FEAT-079, FEAT-081, FEAT-082, FEAT-083, FEAT-084, FEAT-085, FEAT-086, FEAT-087, FEAT-088, FEAT-089, FEAT-090, FEAT-091, FEAT-092, FEAT-120

## v0.5.0: Team workflow on GitHub
- **Status**: planned
- **Goal**: Gedo value is visible on every pull request, data survives server-side merges, the merge queue is checked semantically, no work is ever lost, Cargo.toml is merged structurally and Cargo.lock regenerated, and parallel coding agents work in hidden streams that see each other's intents.
- **Items**: FEAT-096, FEAT-097, FEAT-098, FEAT-093, FEAT-094, FEAT-095, FEAT-100, FEAT-057, FEAT-129, FEAT-130

## v0.6.0: Levix stacks
- **Status**: planned
- **Goal**: TypeScript and Python repositories get the full Gedo experience, with package.json merged structurally and their lockfiles regenerated on merge, collisions and contract breaks show up in any LSP editor, and signed Windows binaries are installable with winget.
- **Items**: FEAT-017, FEAT-018, FEAT-101, FEAT-102, FEAT-121, FEAT-122, FEAT-055, FEAT-131

## v0.7.0: Real-time coordination
- **Status**: planned
- **Goal**: Teammates see each other's intents within seconds through an optional self-hosted relay, peek into work in progress and use soft leases for binary files.
- **Items**: FEAT-104, FEAT-105, FEAT-106, FEAT-107

## v0.8.0: Symbol inbox
- **Status**: planned
- **Goal**: Colliding changes are negotiated asynchronously per symbol and agreed before the pull request.
- **Items**: FEAT-109, FEAT-110, FEAT-111, FEAT-112

## v0.9.0: Stacks and playback
- **Status**: planned
- **Goal**: Large changes ship as stacks of small pull requests that restack themselves, and any delivery can be played back event by event.
- **Items**: FEAT-113, FEAT-114, FEAT-115, FEAT-116

## v0.10.0: IDE integration
- **Status**: planned
- **Goal**: Collisions, threads, leases and node identity are visible in VS Code, with types refined by language servers.
- **Items**: FEAT-117, FEAT-118, FEAT-080

## v0.11.0: Mainstream languages
- **Status**: planned
- **Goal**: The most used languages after the Levix stacks get full Gedo support: JavaScript, Java, Shell, C#, C++, PHP and Go.
- **Items**: FEAT-019, FEAT-020, FEAT-021, FEAT-022, FEAT-023, FEAT-024, FEAT-025

## v0.12.0: Configuration formats
- **Status**: planned
- **Goal**: YAML and Markdown, present in almost every repository, are tracked and merged structurally (TOML and JSON arrive earlier, with their lockfiles).
- **Items**: FEAT-056, FEAT-058

## v0.13.0: More platforms and hosts
- **Status**: planned
- **Goal**: Gedo runs on macOS and Windows on ARM, installs from Linux package managers, works with GitLab and with SHA-256 repositories.
- **Items**: FEAT-124, FEAT-125, FEAT-123, FEAT-099, FEAT-008

## v0.14.0: Data and markup formats
- **Status**: planned
- **Goal**: SQL, HTML and CSS, HCL and XML are tracked structurally, and generated files are detected and regenerated instead of merged.
- **Items**: FEAT-059, FEAT-060, FEAT-061, FEAT-062, FEAT-103

## v0.15.0: Peer-to-peer
- **Status**: planned
- **Goal**: Teams in local or corporate networks coordinate in real time without any relay.
- **Items**: FEAT-108

## v1.0.0: Stable
- **Status**: planned
- **Goal**: Gedo's data formats are frozen and every later release is guaranteed to read them; Gedo is ready to be used by teams outside Levix.
- **Items**: FEAT-127

## v1.1.0: Pair programming
- **Status**: planned
- **Goal**: Two developers work live on the same symbols through the control plane.
- **Items**: FEAT-126

## v1.2.0: Languages, second wave
- **Status**: planned
- **Goal**: PowerShell, C, Kotlin, Lua, Assembly, Ruby, Dart and Swift are supported, through declarative language packs.
- **Items**: FEAT-128, FEAT-026, FEAT-027, FEAT-028, FEAT-029, FEAT-030, FEAT-031, FEAT-032, FEAT-033

## v1.3.0: Languages, third wave
- **Status**: planned
- **Goal**: R, Groovy, Visual Basic .NET, VBA, MATLAB, Perl, GDScript and Elixir are supported.
- **Items**: FEAT-034, FEAT-035, FEAT-036, FEAT-037, FEAT-038, FEAT-039, FEAT-040, FEAT-041

## v1.4.0: Languages, fourth wave
- **Status**: planned
- **Goal**: Scala, Delphi, Lisp, Zig, Erlang, Fortran, Ada, F#, OCaml, Gleam, Prolog, COBOL and Mojo are supported, completing the language list.
- **Items**: FEAT-042, FEAT-043, FEAT-044, FEAT-045, FEAT-046, FEAT-047, FEAT-048, FEAT-049, FEAT-050, FEAT-051, FEAT-052, FEAT-053, FEAT-054
