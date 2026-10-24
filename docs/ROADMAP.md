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
- **Goal**: Gedo value is visible on every pull request, data survives server-side merges, the merge queue is checked semantically, no work is ever lost, and Cargo.lock never conflicts.
- **Items**: FEAT-096, FEAT-097, FEAT-098, FEAT-093, FEAT-094, FEAT-095, FEAT-100

## v0.6.0: Levix stacks
- **Status**: planned
- **Goal**: TypeScript and Python repositories get the full Gedo experience, with their lockfiles regenerated on merge, and signed Windows binaries installable with winget.
- **Items**: FEAT-017, FEAT-018, FEAT-101, FEAT-102, FEAT-121, FEAT-122

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
