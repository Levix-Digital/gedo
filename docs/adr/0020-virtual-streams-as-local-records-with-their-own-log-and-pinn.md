---
id: 20
title: Virtual streams as local records with their own log and pinned work commits
status: accepted
date: '2026-11-06'
version: v0.3.0
subsystems: [events, store, surfaces]
supersedes: []
superseded_by: []
amends: []
amended_by: []
---

# ADR-0020: Virtual streams as local records with their own log and pinned work commits

## Context

FEAT-068 asks for virtual streams: named units of work that switch or isolate work without `git stash` or `git checkout`, carry an optional agent author and opaque labels set at creation (ADR-0013), and give both to every event recorded inside them, so a tool such as the dev-cycle protocol opens one stream per milestone or per agent and finds it again by label. Until now all live work went into one write-ahead log under one recording context. Streams are not derived data, so they cannot live in the symbol cache, which may be dropped at any time (ADR-0004, ADR-0012); `refs/gedo/*` is pushed and fetched (ADR-0011), and in-progress work stays on the developer's machine (clarification of 2026-11-06). `git gc` prunes objects that no reference reaches, so a saved work tree that only a Gedo file names would be lost after `gc.pruneExpire`. A commit's id depends on its time, so a publish interrupted after its commit is written could end the stream's events on another commit when retried.

## Decision

- **Local records through a new port.** `StreamStore` (in `gedo-core`) keeps, per stream, an opaque record (format 1: name, agent, labels, base, saved work tree, commit of a publish in progress) and its own write-ahead log; per working tree, the active stream and the outside work. The file adapter puts each stream in `.git/gedo/streams/<key>/` (`stream` and `wal.log`), `<key>` being the hexadecimal form of the name's bytes, so every valid name is storable on every file system; the working tree state is `gedo/stream-state` in that working tree's own Git directory. A record is created with an exclusive create (a hard link of a complete temporary file) under the stream's log lock, which clears what an unfinished removal or creation left, and replaced with a rename; a stream is removed by deleting its record while its log is held.
- **Names**: 1 to 64 bytes of `/`-separated segments of `[a-z0-9._-]`, each starting with a letter or digit; lower case only, so no two names differ by case.
- **Inherited context.** Events recorded inside a stream are recorded by `EventLog` over the stream's log, on its base, with its agent (or the person) and labels; `GEDO_AGENT` and `GEDO_LABELS` apply only outside streams. Labels with the reserved prefix `gedo.` are refused at creation.
- **Work as trees, pinned.** Switching saves the caller's current work tree into the stream left (or as the outside work) and returns the target's. Saved work is kept alive by a work commit (the tree on the stream's base), written only when the work changed, pointed to by a local reference, `refs/gedo-local/streams/<key>` or `refs/gedo-local/worktrees/<id>`, outside `refs/gedo/` and therefore never pushed or fetched. Gedo writes no working file, index, `HEAD` or branch.
- **Publish recorded before its tape.** Publishing writes the commit, records it in the stream (`publishing`) and pins it, publishes the stream's log as its tape with `EventLog::publish`, then moves the stream onto it. Every later operation on the stream completes a publish in progress with the same commit. No branch moves; the stream stays open until discarded.
- **One lock.** Every change of a stream's record is made while holding the stream's log, which has one writer at a time.
- **Atomic, not synced.** Records and states are written as a complete temporary file renamed or linked into place, so a crash of the process leaves the old or the new content. They are not synced: like Git's references, which hold the tapes and pins these records point to, the last update may be lost to a power failure. A sync costs 6 to 8 ms on Windows, more than the whole budget of a switch.
- A stream is a side of the convergence of ADR-0017 (its base and pending events). Discarding refuses to drop pending events or saved work unless forced.

## Alternatives Considered

- **Streams in the symbol cache**: derived data that may be deleted or rebuilt at any time; work would be lost.
- **Streams under `refs/gedo/streams`**: pushed and fetched with every collection, against keeping work in progress on the machine.
- **One file and one log for all streams**: two agents in two streams would wait on each other's writes.
- **Readable directory names**: `CON` and `nul` are refused on Windows, `A` and `a` are one directory on Windows and macOS, and `/` would nest directories.
- **Pinning trees directly**: a second reference would be needed to keep the base; a commit keeps both, as `git stash` does.
- **Syncing every record and state write**: durable against power failures, but create, switch and publish missed their 10 ms budget on the Windows reference machine (create p95 21 to 39 ms), while the references they depend on are not synced either.
- **Closing the stream on publish**: the clarification keeps it open on the new commit.
- **Merging the environment's labels into a stream's**: a stream's events would differ by process, and tools filter by the stream's labels.

## Consequences

- Tools can create one stream per milestone or agent and list them by label and agent; their published events carry the stream's author and labels, so the event filters of FEAT-063 find them.
- Work can be put aside and resumed without stash or checkout once the CLI (v0.4.0) writes the returned trees; hidden checkouts for agent streams (FEAT-130) build on the same records.
- New local formats (stream record, working tree state) freeze at v1.0.0 like every stored format; the event format does not change.
- `.git/gedo/streams`, the state files and `refs/gedo-local/` are local data: copying a repository without them loses its streams, not its published history.
- Measured on the Linux development container (`cargo bench -p gedo-app --bench streams`): create p95 0.8 ms, switch 1.9 ms, get 4 µs, listing 1,000 streams 6.5 ms; publishing a stream of 1,000 events p50 7.6 ms against 4.8 ms for `EventLog::publish` of the same events.
- Verified by `crates/core/gedo-app/tests/streams.rs`, the `StreamStore` contract on both adapters, `crates/adapters/gedo-store/tests/streams_gc.rs` (pins through `git gc`, linked working trees, reserved names, pins not pushed) and `tests/event_tracing.rs`.
