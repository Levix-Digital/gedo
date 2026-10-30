---
id: 15
title: Packed batch writes and an event cache
status: accepted
date: '2026-10-30'
version: v0.3.0
subsystems: [store, events]
supersedes: []
superseded_by: []
amends: [14]
amended_by: []
---

# ADR-0015: Packed batch writes and an event cache

## Context

The event log of ADR-0014 stores one blob per event in `refs/gedo/events`. On the Windows reference machine (i7-12700KF) publishing 1,000 events took 1.31 s at p95 and filtering 10,000 events 1.34 s, above the budgets of 1 s and 500 ms (SC-004 and SC-006 of `specs/004-event-model`, BUG-009 and BUG-010); Linux took 107 and 408 ms. Under `strace`, one publish of 1,000 events created about 1,270 loose object files (the events, about 256 fan-out trees, the root trees and the commits of the three collections), each written to a temporary file and renamed, and one filter opened about 21,400 object files, two per event (header, then content) and the trees. Creating and opening a file costs far more on Windows than on Linux. With the same objects packed by `git repack`, the filter took a quarter of the time even on Linux. Gedo never runs `git` (ADR-0001), and gitoxide holds a fixed number of pack index slots per opened repository (32 by default when there are few packs) and cannot read a pack beyond them. Events published one commit at a time come in small batches, and Git itself keeps a batch of fewer than 100 fetched objects loose (`fetch.unpackLimit`).

## Decision

- **Batched blob writes.** `ObjectStore` gains `write_blobs`, which returns the ids of a batch in order; its default writes one blob at a time, and `Collection::put` writes its entries with it. The Git adapter (`crates/adapters/gedo-store/src/pack.rs`) writes a batch of at least 100 blobs not stored yet as one version 2 pack of undeltified, zlib-compressed objects, indexed by gitoxide (`Bundle::write_to_directory`, a temporary file renamed with its index last) and without the `.keep` file, so `git gc` repacks it; a smaller batch is written as loose objects. Trees and commits stay loose.
- **Object store slots.** Every repository is opened with at least 1,024 pack index slots, so a long-running process can keep writing packs.
- **Event cache.** The symbol cache gains an `events` namespace mapping an event id (16 bytes) to the record stored in `refs/gedo/events`. `EventLog::with_cache` connects it: `publish` writes the records of the tape after the index entry and before truncating the write-ahead log, and `event`, `events`, `events_of_commit` and `filter` read the cache first and the collection on a miss, keeping what they read. A cached record is decoded with every check of a stored one, its id included; a refused or unreadable entry is read from the collection again, and a cache failure is a warning, never an error. The namespace is not rebuilt on open: event ids derive from content, so an entry never becomes stale and a missing one is read again.
- **Shared cache.** `SymbolCache::put` and `clear` take `&self`, so the identity index and the event log of one process share one cache (the redb file can be opened once per process).

## Alternatives Considered

- **A label index in the cache**: a filter also matches the author kind, the agent and the fingerprint, and returns whole events, so it would still read every event.
- **One blob per tape holding its events**: a format change of ADR-0014 that does not help events received from other machines.
- **Packing every batch, whatever its size**: one pack per commit's publish would grow the number of packs, and so the cost of every lookup, much faster than `git gc` merges them.
- **Reading an object once, without the header lookup**: 30% faster for a filter over loose objects with an empty cache, but the size limit would only be checked after the whole object is in memory.
- **Running `git repack`**: Gedo runs no Git subprocess.

## Consequences

- On Linux a publish of 1,000 events takes about 65 ms without the cache and 150 ms with it (the cache transaction), and a filter of 10,000 events about 50 ms with the cache, from tapes of any size. A filter with an empty cache still reads every object, once; on Windows the first filter after a clone or a cache rebuild can stay above 500 ms over loose events.
- Repositories accumulate one pack per large publish until `git gc` merges them; the 1,024 slots bound a single process, not the repository.
- The cache is per machine and never pushed; deleting it only costs one slow read of each event.
- Verified by the object store contract tests, the pack tests of `gedo-store` (one pack and no loose object, `git fsck --strict`, a pack left without its index ignored, 40 packs in one process), the event log tests with a full, an empty and a corrupted cache and with a publish stopped after the cache step, and the `events` benchmark on both platforms.
