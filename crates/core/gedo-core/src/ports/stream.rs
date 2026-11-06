//! Port to the local store of virtual streams (`specs/009-virtual-streams/research.md` R2).
//!
//! Streams are kept on the developer's machine only, shared by the repository's working trees and
//! never pushed. Records are opaque bytes encoded and checked by the core
//! ([`crate::stream`]); each stream has its own write-ahead log; the active stream and the outside
//! work belong to one working tree. Pins keep the objects a stream needs reachable, so that
//! `git gc` does not prune them.

use crate::error::StoreError;
use crate::ids::GitOid;
use crate::ports::WriteAheadLog;
use crate::stream::StreamName;

/// The local streams of a repository, seen from one working tree.
pub trait StreamStore {
    /// The log of one stream.
    type Log: WriteAheadLog;

    /// The names of every stored stream, in name order.
    fn names(&self) -> Result<Vec<StreamName>, StoreError>;
    /// The record of `name`, or `None` when no such stream exists.
    fn read(&self, name: &StreamName) -> Result<Option<Vec<u8>>, StoreError>;
    /// Stores the record of a new stream; `false` (and nothing written) when `name` already
    /// exists. Of two concurrent creations of one name, exactly one returns `true`. What an
    /// earlier stream of that name left behind (a removal or a creation that did not finish) is
    /// cleared first, so a new stream starts with an empty log.
    fn create(&self, name: &StreamName, record: &[u8]) -> Result<bool, StoreError>;
    /// Replaces the record of an existing stream atomically.
    fn replace(&self, name: &StreamName, record: &[u8]) -> Result<(), StoreError>;
    /// Removes the stream whose log the caller holds: its record first, while `log` is still
    /// held, so no other operation can open the stream after it; then its log and its pin.
    /// Removing an absent stream succeeds.
    fn remove(&self, name: &StreamName, log: Self::Log) -> Result<(), StoreError>;
    /// Opens the stream's log; one writer at a time (`WalBusy` after the wait).
    fn open_log(&self, name: &StreamName) -> Result<Self::Log, StoreError>;
    /// Keeps `commit` (and everything it reaches) alive for the stream; `None` drops the pin.
    fn pin(&self, name: &StreamName, commit: Option<GitOid>) -> Result<(), StoreError>;
    /// The state of this working tree, or `None` when it never had one.
    fn read_state(&self) -> Result<Option<Vec<u8>>, StoreError>;
    /// Replaces the state of this working tree atomically.
    fn write_state(&self, state: &[u8]) -> Result<(), StoreError>;
    /// Keeps `commit`, this working tree's outside work, alive; `None` drops the pin.
    fn pin_outside(&self, commit: Option<GitOid>) -> Result<(), StoreError>;
}
