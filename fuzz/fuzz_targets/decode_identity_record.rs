//! Any bytes, decoded as each record of the identity index: no panic, hang or out-of-memory, and
//! every accepted map satisfies its invariants.
#![no_main]

use gedo_app::index::records::{Checkpoint, CommitRecord, MergeList, decode, decode_map};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = decode::<CommitRecord>(bytes, "fuzz");
    let _ = decode::<Checkpoint>(bytes, "fuzz");
    let _ = decode::<MergeList>(bytes, "fuzz");
    if let Ok(map) = decode_map(bytes, "fuzz") {
        map.check().expect("a decoded map is valid");
    }
});
