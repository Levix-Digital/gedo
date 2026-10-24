//! File system helpers shared by the write-ahead log and the cache.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long an operation on a file another program holds open (an antivirus, an indexer) is retried.
pub const SHARING_RETRY_MS: u64 = 2_000;
const SHARING_RETRY_PAUSE_MS: u64 = 20;

/// Runs `operation`, retrying for up to [`SHARING_RETRY_MS`] while Windows reports a sharing
/// violation. On other platforms, and for other errors, the first result is returned.
pub fn retry_sharing_violation<T>(
    path: &Path,
    mut operation: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    let deadline = Instant::now() + Duration::from_millis(SHARING_RETRY_MS);
    loop {
        match operation() {
            Err(error) if is_sharing_violation(&error) && Instant::now() < deadline => {
                tracing::debug!(path = %path.display(), "file held by another program; retrying");
                std::thread::sleep(Duration::from_millis(SHARING_RETRY_PAUSE_MS));
            }
            result => return result,
        }
    }
}

#[cfg(windows)]
fn is_sharing_violation(error: &io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    matches!(
        error.raw_os_error(),
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
    )
}

#[cfg(not(windows))]
fn is_sharing_violation(_error: &io::Error) -> bool {
    false
}

/// Makes a rename or a file creation in `directory` durable (no-op where directories cannot be synced).
pub fn sync_directory(directory: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(directory)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(())
    }
}
