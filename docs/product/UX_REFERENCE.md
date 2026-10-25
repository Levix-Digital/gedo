# UX Reference

Direction for how Gedo's main surfaces should read, captured during the improvement review of 2026-10-25. It is input for the discovery and specification of the features named below, not a specification: field names, glyphs and layout are decided in each feature's spec. Every surface keeps the rules of FEAT-088 (machine-readable `--json`, `NO_COLOR` and non-TTY handling, no information carried by color alone).

## `gedo radar` (FEAT-074, FEAT-130)

Shows the anatomy of a collision before it happens: the entity, what each side did to it and the impact, with a hint for the next command. Collisions that need action come first; concurrent edits that converge on their own are listed after them, so the developer sees that Gedo handled them.

```text
$ gedo radar
GEDO RADAR - Base: dev (fetched 2m ago) | Active stream: feat/retry-policy

[!] 1 CONTRACT COLLISION (action required before publish)
  fn crate::store::GitRefStore::apply
    Local (uncommitted):          BodyModified (calls compare_and_swap with 3 args)
    Remote (origin/fix/cas, 4m):  SignatureMutated (+ timeout: Duration)
    Impact: 2 call sites in crates/adapters/gedo-store/src/refs.rs no longer match
    Hint:   gedo peek crate::store::GitRefStore::apply --branch origin/fix/cas

[i] 2 CONCURRENT EDITS MERGED AUTOMATICALLY
  impl crate::store::GitRefStore
    Local:                        + fn retry_backoff()   NodeCreated
    Agent stream agent/metrics:   + fn record_latency()  NodeCreated
    Status: converged, 0 line conflicts
```

Sources include the upstream and remote branches (FEAT-074) and the agent streams of the same machine (FEAT-130).

## `gedo delta` and the pull request comment (FEAT-075, FEAT-096)

Groups changes by meaning instead of lines: what changed in a contract first, pure moves and formatting last and collapsed, with the amount of noise filtered stated in the summary. The same table is the body of the GitHub Action comment.

```markdown
### Gedo semantic delta (dev..feat/node-identity)
Summary: 18 entities touched | Noise filtered: ~420 lines of formatting and pure moves

| Change | Entity | Location (before -> after) | Contract impact |
|---|---|---|---|
| SignatureMutated | fn crate::ast::normalize_tokens | src/normalize.rs:42 -> :45 | all 4 callers updated |
| BodyModified | fn crate::walk::collect | src/walk.rs:110 -> :118 | internal only |
| NodeRenamed | struct TokenKind -> SyntaxClass | src/syntax.rs:12 | 11 references updated |
| NodeMoved (exact) | fn crate::rust::is_trivia and 8 more | src/lib.rs -> src/rust/tokens.rs | structural hash identical |
| Format only | 5 entities | src/rust/paths.rs | no semantic change |
```

## `gedo trace` (FEAT-084)

The biography of one entity, newest first, through renames, moves and squash merges, with the kind of each change and where the entity lived at that point.

```text
$ gedo trace crate::store::refs::is_busy_lock
Entity:  fn crate::store::refs::is_busy_lock
Current: crates/adapters/gedo-store/src/refs.rs:142..168

  * c54732f  2026-10-25  BodyModified  Treat Windows PermissionDenied in prepare as busy lock
  * 9a812bc  2026-10-24  NodeMoved     src/lib.rs -> src/refs.rs (structural hash unchanged)
  * 4f0012a  2026-10-24  NodeRenamed   check_lock_err -> is_busy_lock
  + 1b099e1  2026-10-24  NodeCreated   crates/adapters/gedo-store/src/lib.rs:88..104
```

The commits and paths in these examples are illustrative.
