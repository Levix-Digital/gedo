# Quickstart: Semantic Lanes and Split Publish

Developer checks for this feature (no end-user command yet).

1. Run the tests: `cargo test -p gedo-crdt -p gedo-app`.
2. Lanes and builder: `cargo test -p gedo-crdt --test lanes` runs the lane rules, the follow-up cases and the property tests of the refactor builder.
3. Split publish: `cargo test -p gedo-app --test split` builds scripted work on temporary repositories (rename with callers, move across files, module split, mixed refactor and feature work, cases that fall back) and checks the outcome against the committed snapshot.
4. Split on a scratch repository: `cargo run -p gedo-app --example split -- <repo> <base> <final-commit>` indexes the history, split-publishes the final commit's tree on the base and prints the commits written and their changes.
5. Benchmark: `cargo bench -p gedo-app --bench split` (100 files, 1,000 events).
