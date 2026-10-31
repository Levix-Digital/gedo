# Quickstart: Bounded Detached-Tape Walk

Developer checks (no end-user command):

1. `cargo test -p gedo-app --test reattach` and `cargo test -p gedo-app --test generations`: the bound, the cache and the properties.
2. `GEDO_BENCH_DEPTH=100000 cargo bench -p gedo-app --bench detached`: the first call with an empty cache, then the calls with generations cached (SC-001, SC-002).
