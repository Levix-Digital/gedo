//! The ports through which the core reaches storage and parsers. Adapters implement them.

pub mod cache;
pub mod object;
pub mod parser;
pub mod refs;
pub mod wal;

pub use cache::{CacheEntry, CacheRebuildSource, CacheStatus, RebuildReason, SymbolCache};
pub use object::{CommitInfo, EntryMode, NewCommit, ObjectStore, TreeEntry};
pub use parser::{ParseError, Parser};
pub use refs::{Expected, RefOutcome, RefStore, RefUpdate};
pub use wal::{LogSequence, OpenReport, WriteAheadLog};
