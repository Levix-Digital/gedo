---
id: 11
title: Gedo references as commit chains with a separate fetch namespace
status: proposed
date: '2026-10-24'
version: v0.1.0
subsystems: [store, net]
supersedes: []
superseded_by: []
amends: [3, 9]
amended_by: []
---

# ADR-0011: Gedo references as commit chains with a separate fetch namespace

## Context

ADR-0003 lays Gedo data out as one reference per object (`refs/gedo/ast-map/<blob-sha>`, `refs/gedo/tapes/...`) pointing to blobs or trees. A spike on Git 2.43 showed that a remote reference pointing to a non-commit object can only be updated with `--force` ("You cannot update a remote ref that points at a non-commit object ... without using the '--force' option"); a forced push would overwrite teammates' data. One reference per object also means tens of thousands of references, the risk FEAT-007 was created to measure. The clarified push and fetch behavior (spec `001-store-foundation`) requires that a plain `git push` keeps its branch behavior and that a fetch never overwrites unpushed local data.

## Decision

- **Commit chains.** Each Gedo collection is one reference, `refs/gedo/<collection>`, pointing to the head of a commit chain. Every update writes a commit whose tree holds the collection's entries in a two-level fan-out by entry id (as Git notes do) and whose parent is the previous head, and moves the reference by compare-and-swap. Divergent chains (another process, another machine) are joined by a merge commit whose tree is the union of both trees; entries are content-addressed, so a union never conflicts.
- **Fetch namespace.** The remote setup adds `+refs/gedo/*:refs/gedo-remote/<remote>/*` to each remote's fetch refspecs. A plain `git fetch` mirrors the remote chains there; Gedo merges them into the local chains. `refs/gedo-remote/` is outside `refs/gedo/`, so it is never pushed back.
- **Push.** `git push <remote> refs/gedo/*:refs/gedo/*`, fast-forward only, never forced; a rejection is resolved by fetch, merge and push again. No `remote.<name>.push` value is ever configured, so the branches a plain `git push` sends are unchanged.
- **Subprocess scope (amends ADR-0009).** Besides the CLI pass-through and the background fetch, the push of `refs/gedo/*` from Gedo's pre-push hook runs the user's `git`, for the same reason: the user's remotes, credentials and network settings. A `GEDO_INNER_PUSH=1` guard prevents the hook from recursing.
- **Chain commits** use the repository's configured user identity; Gedo refuses to write a chain commit when none is configured.
- ADR-0003's namespaces (`ast-map`, `tapes`, `threads`) become collections of this layout; the scale study of FEAT-007 confirms the layout with measurements before v0.2.0.

## Alternatives Considered

- **References to blobs or trees (ADR-0003 as written)**: cannot be updated on a remote without a forced push, which would erase other people's data.
- **One commit-chain reference per object**: fast-forwards, but creates tens of thousands of references that slow every fetch and advertisement.
- **Fetch refspec straight into `refs/gedo/*`**: a forced fetch would overwrite local chains that were not pushed yet.
- **A push refspec in the remote configuration**: makes plain `git push` carry the data, but replaces Git's `push.default` behavior for branches.

## Consequences

- Pushes and fetches of Gedo data are ordinary fast-forwards; hosts keep everything reachable from the chains, and the reflog keeps local history.
- Tools that list every reference (`git log --all`, `gitk --all`) show the chains as separate histories with no relation to the code; `--branches`, `--remotes`, hosting UIs and IDE branch views do not. This is documented in the README.
- Every write creates a commit; writers batch entries per operation to keep chains short, and later releases may compact old chain history.
- The scale study measures chains of 100 to 100,000 entries; if it fails SC-007's thresholds, a superseding ADR changes the fan-out or splits collections.
- **Scale study results** (`cargo xtask scale-study`, release build, file remote, Linux, Git 2.43, 2026-10-24; chains built 1,000 entries per commit):

  | Entries | Chain: clone + fetch | Chain: data added | Chain: incremental push / fetch | Reference per entry: clone + fetch | Reference per entry: incremental push / fetch |
  |---:|---:|---:|---:|---:|---:|
  | 100 | 0.08 s | 17 KiB | 0.03 s / 0.02 s | 0.14 s | 0.04 s / 0.02 s |
  | 1,000 | 0.09 s | 110 KiB | 0.04 s / 0.03 s | 0.57 s | 0.09 s / 0.05 s |
  | 10,000 | 0.21 s | 1.1 MiB | 0.06 s / 0.04 s | 2.93 s | 0.77 s / 0.45 s |
  | 50,000 | 0.95 s | 5.5 MiB | 0.07 s / 0.06 s | push fails | push fails |
  | 100,000 | 3.07 s | 12 MiB | 0.11 s / 0.09 s | push fails | push fails |

  At 10,000 entries a chain adds 0.16 s and 1.1 MiB over a plain clone, well inside SC-007's 5 s and 50 MB. Incremental cost stays near constant because only the changed fan-out directories travel. One reference per entry is 7 to 14 times slower from 1,000 entries up, and at 50,000 references `git push` over a local remote fails outright ("cannot exec 'fetch': Argument list too long"). **Decision:** keep the commit-chain layout with the two-level fan-out; no superseding ADR is needed. The study runs again against GitHub and GitLab with `--remote` once the host-validation secrets are configured.
