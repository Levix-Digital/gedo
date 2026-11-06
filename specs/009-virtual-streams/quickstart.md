# Quickstart: Virtual Streams

Developer checks for this feature (no end-user command yet).

1. Run the tests: `cargo test -p gedo-core -p gedo-crdt -p gedo-app -p gedo-store`.
2. Streams: `cargo test -p gedo-app --test streams` creates, lists, records into, switches, publishes, converges and discards streams on temporary repositories and checks the outcomes against the committed snapshot.
3. Garbage collection: `cargo test -p gedo-store --test streams_gc` (needs `git`) checks that saved work survives `git gc --prune=now`.
4. On a scratch repository: `cargo run -p gedo-app --example streams -- <repo> <name> [key=value,...]` creates the stream on `HEAD`, saves `HEAD`'s tree as its work, publishes it and prints the stream and the commit.
5. Benchmark: `cargo bench -p gedo-app --bench streams`.
