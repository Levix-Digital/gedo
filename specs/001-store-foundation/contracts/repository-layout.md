# Contract: Repository Layout and Git Configuration

What Gedo writes into a repository in this release, and nothing else.

## References

| Reference | Points to | Written by |
|---|---|---|
| `refs/gedo/<collection>` | Head commit of the collection's chain (tree with two-level fan-out by entry id) | Gedo, compare-and-swap only |
| `refs/gedo-remote/<remote>/<collection>` | Mirror of the remote's chain | `git fetch`, through the refspec below |

Never written: `refs/heads/*`, `refs/tags/*`, `refs/remotes/*`, `HEAD`, the index, the working tree.

## Local runtime (never pushed)

| Path | Content |
|---|---|
| `.git/gedo/wal.log` | Write-ahead log (format in `data-model.md`) |
| `.git/gedo/wal.lock` | Exclusive lock of the log writer |
| `.git/gedo/symbol_cache.db` | redb symbol cache with `meta.schema_version` |

## Git configuration

Added once per remote by the remote setup, idempotently:

```ini
[remote "origin"]
    fetch = +refs/gedo/*:refs/gedo-remote/origin/*
```

Never added: `remote.<name>.push`. Existing `fetch` values and every other setting are kept.

## Push

```text
git push <remote> refs/gedo/*:refs/gedo/*
```

Fast-forward only. When rejected because the remote chain moved: fetch, merge the chains, push again. Run by the pre-push hook from v0.4.0 (with `GEDO_INNER_PUSH=1` as a recursion guard).

## Refusals

- SHA-256 repositories: `UnsupportedObjectFormat` before anything is written.
- No `user.name` or `user.email`: `MissingIdentity` before a chain commit is written.
