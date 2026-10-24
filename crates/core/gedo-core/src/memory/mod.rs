//! In-memory adapters of every port, behaving like the real ones for the core's tests.
//!
//! They hold no files and spawn nothing; durability and crashes are simulated explicitly.

mod cache;
mod objects;
mod refs;
mod wal;

pub use cache::MemoryCache;
pub use objects::MemoryObjectStore;
pub use refs::MemoryRefStore;
pub use wal::MemoryWal;

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks a mutex, recovering the data if another thread panicked while holding it: every
/// critical section below leaves the data consistent before anything can panic.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
