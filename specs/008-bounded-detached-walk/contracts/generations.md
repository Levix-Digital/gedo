# Contract: commit generations and the detached-tape search

Public interface: unchanged. `EventLog::detached`, `EventLog::reattach_new` and `EventLog::generation` keep their signatures and results.

Behavior added:

- `EventLog::generation(commit)`: with a cache, reads the `generations` namespace first (verified, research R4) and writes every generation it computes; without a cache, as today.
- Detached-tape search: visits commits by descending generation and stops at the bound (FR-001); an indexed commit missing from the object database is unreachable (FR-003).

Errors: unchanged. A tip or a commit on the walk that cannot be read is `EventError::Store` with context; cache failures are warnings only.

Logs: `detached-tape search done` (debug: commits read, stopped on the bound or not); `generation record refused` and `could not write generations` (warn).
