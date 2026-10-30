# Quickstart: Tree CRDT

Developer checks for this feature (no end-user command yet).

1. Run the tests: `cargo test -p gedo-crdt -p gedo-app`.
2. Properties: `cargo test -p gedo-crdt --test tree` runs the property-based tests of the convergence (arrival order, duplicates, shared events, permuted sides, convergence with itself, no lost change).
3. Scenarios: `cargo test -p gedo-app --test converge` builds scripted branches on a temporary repository (additions to the same `impl`, move against edit, rename against move, removal against edit, additions on both sides, a graft, live events on top) and checks the convergence against the committed snapshot.
4. Converge on a scratch repository: `cargo run -p gedo-app --example converge -- <repo> <base> <tip>...` indexes the commits, converges the tips over the base and prints the entities changed, the links and the conflicts.
5. Benchmarks: `cargo bench -p gedo-crdt --bench tree` (two sides of 10,000 events over 10,000 entities) and `cargo bench -p gedo-app --bench converge` (two branches of 100 commits).
