# Quickstart: History Re-attachment

Developer checks for this feature (no end-user command yet).

1. Run the tests: `cargo test -p gedo-crdt -p gedo-app`.
2. Rewrite scenarios: `cargo test -p gedo-app --test reattach` builds scripted histories on a temporary repository (rebase, amend, interactive squash, split, server squash and rebase merge, cherry-pick, deleted branch), publishes events, rewrites them and checks the index entries against the committed snapshot.
3. Re-attach on a scratch repository: `cargo run -p gedo-app --example reattach -- <repo>` publishes events on a new branch, squashes it onto the current `HEAD` as a server would, re-attaches the squash commit and prints the attached tapes, the remainder, the links and the detached report.
4. Benchmarks: `cargo bench -p gedo-app --bench reattach` (one fetched squash with 10,000 stored events of which 1,000 are detached, the pair path, `fill_cache` of 10,000 events and the first filter after it).
