---
id: 22
title: Stream pins as loose references written with Git's lock protocol
status: accepted
date: '2026-11-14'
version: v0.3.0
subsystems: [store]
supersedes: []
superseded_by: []
amends: [20]
amended_by: []
---

# ADR-0022: Stream pins as loose references written with Git's lock protocol

## Context

ADR-0020 keeps each stream's saved work alive with a pin, a local reference under `refs/gedo-local/`, written by a gitoxide reference transaction. On the Windows reference machine (i7-12700KF, Windows 11, Defender real-time protection on), `crates/core/gedo-app/examples/stream_steps.rs` timed each storage step of a switch that saves new work and of what a stream publish adds to `EventLog::publish` (200 runs, three times, commit 34992b5): the pin was the slowest step, p50 1.6 to 2.0 ms and p95 2.8 to 4.1 ms, against 0.5 ms for replacing the stream record by a rename and 0.8 ms for writing the work commit. A saving switch took p50 4.4 to 4.5 ms in storage, and `cargo bench -p gedo-app --bench streams` reported switch p95 8.7 to 24.8 ms and a publish overhead of 10 to 12 ms at p50 (BUG-012).

The transaction checks `packed-refs` and its lock, creates the reference's directory, creates the lock, opens and reads the old value, writes the new one, tries to open the reflog and renames the lock: a plain replacement does only the lock, the write and the rename. On Windows each extra open costs a scan. A pin needs none of the extras: Git keeps no reflog for references outside branches, remote-tracking references, notes and `HEAD`; the stream's log is held while its pin changes, so no expected old value is checked; and a loose reference takes precedence over a packed one.

## Decision

- `FileStreamStore` writes a pin (`StreamStore::pin` and `pin_outside` with a commit) as a loose reference in the repository's common directory with Git's own locking: `<reference>.lock` is created exclusively (another writer's lock, or on Windows a lock still pending deletion, is waited for with backoff up to 2 s, `REF_LOCK_WAIT_MS`), the commit's hexadecimal id and a newline are written, the lock is closed and renamed onto the reference. The reference's directory is created only when the lock cannot be created for lack of it. No reflog is written and the old value is not read.
- Deleting a pin stays a gitoxide reference transaction, which also removes a packed copy.
- `refs/gedo/*` references (tapes, index, collections) keep their transactions, which check the expected old value.

## Alternatives Considered

- **Keeping the transaction**: correct, but the pin alone takes 1.6 to 2.0 ms of a switch's 10 ms budget on the reference machine.
- **Pinning later, once per batch of switches**: work saved in between would have no reference and could be pruned by `git gc`.
- **One reference for every stream**: a commit with each stream's work as a parent would be rewritten on every switch of any stream and would serialize all streams on one lock.

## Consequences

- A switch that saves work and a stream publish each write one reference with three file operations (create the lock, write, rename) instead of eight.
- A pin written while `git pack-refs` runs follows Git's own rule: the loose reference is newer, and `pack-refs` deletes a loose reference only under its lock and when the value it packed is still current.
- A stale `.lock` left by a crash makes the next pin of that stream fail after 2 s, as a Git command would, until it is removed.
- Verified by `crates/adapters/gedo-store/tests/streams_gc.rs` (a pin over a packed one wins, `git fsck` passes, removal clears both; a pin waits for another writer's lock; pins through `git gc`) and the `StreamStore` contract.
