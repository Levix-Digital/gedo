# Quickstart: Event Model

Developer checks for this feature (no end-user command yet).

1. Run the tests: `cargo test -p gedo-core -p gedo-crdt -p gedo-app`.
2. Check determinism of reconstruction on this machine: `cargo test -p gedo-app --test event_history` compares the events reconstructed from selected commits of this repository with the committed snapshot (needs a full-depth clone).
3. Record and publish on a scratch repository: `cargo run -p gedo-app --example events -- <repo> <commit>` indexes the history, records a few events with an agent and labels, publishes them for `<commit>`, and prints the tape, the events and a label filter.
4. Benchmarks: `cargo bench -p gedo-app --bench events` (record, publish, reconstruct, filter).
5. Fuzzing (nightly): `cargo +nightly fuzz run decode_event_record -- -max_total_time=600`.
