# Data Model: Bounded Detached-Tape Walk

## Commit generation record (symbol cache, namespace `generations`)

| Field | Encoding | Rule |
|---|---|---|
| key | the commit id bytes (20 for SHA-1, 32 for SHA-256) | one record per commit |
| format | 1 byte, `1` | any other value is refused |
| generation | 8 bytes, little-endian `u64` | ADR-0014: 1 for a root, one more than the highest parent |

- Local derived data: never pushed, not rebuilt on open, recomputed when missing or refused.
- Immutable for a commit: a record never becomes stale; it is only refused when it fails the check of research R4.

## Search state (in memory, per call)

- `pending`: sorted set of `(generation, commit)` of indexed commits not yet seen.
- `frontier`: max-heap of `(generation, commit)` still to visit.
- `seen`: commits already visited.
