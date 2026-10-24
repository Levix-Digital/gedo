//! [`WriteAheadLog`] in `.git/gedo/wal.log`.
//!
//! File layout (little endian):
//!
//! ```text
//! header  magic "GEDOWAL\0" (8 bytes) | file format u16
//! record  length u32 (of version and payload) | crc32 u32 (of version and payload) | version u16 | payload
//! ```
//!
//! Appends go straight to the operating system; a flusher thread calls `sync_data` once the oldest
//! unsynced record is [`DURABILITY_WINDOW_MS`] old (group commit), and `sync` flushes at once.
//! One process writes at a time, holding an exclusive lock on `.git/gedo/wal.lock` that the
//! operating system releases if the process dies.

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gedo_core::StoreError;
use gedo_core::ports::wal::{DURABILITY_WINDOW_MS, WRITER_WAIT_MS};
use gedo_core::ports::{LogSequence, OpenReport, WriteAheadLog};
use gedo_core::record::MAX_RECORD_BYTES;

use crate::fs::{retry_sharing_violation, sync_directory};

/// File name of the log inside `.git/gedo/`.
pub const WAL_FILE_NAME: &str = "wal.log";
/// File name of the writer lock inside `.git/gedo/`.
pub const WAL_LOCK_FILE_NAME: &str = "wal.lock";
/// First bytes of every log file.
pub const WAL_MAGIC: &[u8; 8] = b"GEDOWAL\0";
/// Format of the log file this build writes.
pub const WAL_FILE_FORMAT: u16 = 1;
/// Format version stamped on every record this build appends.
pub const WAL_RECORD_FORMAT: u16 = 1;

const HEADER_LEN: usize = WAL_MAGIC.len() + 2;
const FRAME_HEADER_LEN: usize = 4 + 4 + 2;
const LOCK_POLL_MS: u64 = 10;

/// The write-ahead log of one repository, owned by this process while it is open.
pub struct FileWal {
	path: PathBuf,
	shared: Arc<Shared>,
	flusher: Option<JoinHandle<()>>,
	// Held for the lifetime of the log: the exclusive lock lives as long as this handle.
	_lock: File,
	records: u64,
	report: OpenReport,
	fail_after_bytes: Option<usize>,
}

/// State shared with the flusher thread.
struct Shared {
	state: Mutex<FlushState>,
	wake: Condvar,
}

struct FlushState {
	file: File,
	/// When the oldest record not yet synced was appended.
	unsynced_since: Option<Instant>,
	/// Set when the flusher failed; reported by the next operation.
	failure: Option<io::Error>,
	stop: bool,
}

impl std::fmt::Debug for FileWal {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FileWal")
			.field("path", &self.path)
			.field("records", &self.records)
			.finish()
	}
}

fn lock_state(shared: &Shared) -> MutexGuard<'_, FlushState> {
	shared.state.lock().unwrap_or_else(PoisonError::into_inner)
}

impl FileWal {
	/// Opens (or creates) the log in `gedo_dir` (normally `.git/gedo/`).
	///
	/// Waits up to [`WRITER_WAIT_MS`] for another writer, then fails with `WalBusy`. A torn last
	/// record is dropped, the file truncated to the last complete record, and reported in
	/// [`WriteAheadLog::open_report`]; any other damage is `Corrupted` with its offset.
	pub fn open(gedo_dir: &Path) -> Result<Self, StoreError> {
		let lock = acquire_lock(&gedo_dir.join(WAL_LOCK_FILE_NAME))?;
		let path = gedo_dir.join(WAL_FILE_NAME);
		let mut file = retry_sharing_violation(&path, || {
			OpenOptions::new()
				.read(true)
				.write(true)
				.create(true)
				.truncate(false)
				.open(&path)
		})
		.map_err(|error| StoreError::io("open write-ahead log", &path, error))?;

		let mut bytes = Vec::new();
		file.read_to_end(&mut bytes)
			.map_err(|error| StoreError::io("read write-ahead log", &path, error))?;
		let scan = scan(&bytes, &path)?;
		match scan.torn_at {
			Some(offset) if offset < HEADER_LEN as u64 => {
				if offset > 0 {
					tracing::warn!(path = %path.display(), "write-ahead log header was torn; starting an empty log");
				}
				file.set_len(0)
					.map_err(io_error("reset write-ahead log", &path))?;
				file.seek(SeekFrom::Start(0))
					.map_err(io_error("reset write-ahead log", &path))?;
				file.write_all(&header())
					.map_err(io_error("write write-ahead log header", &path))?;
				file.sync_all().map_err(io_error("sync write-ahead log", &path))?;
				sync_directory(gedo_dir).map_err(io_error("sync write-ahead log directory", &path))?;
			},
			Some(offset) => {
				tracing::warn!(path = %path.display(), offset, "dropping a torn record at the end of the write-ahead log");
				file.set_len(offset)
					.map_err(io_error("truncate torn write-ahead log record", &path))?;
				file.sync_all().map_err(io_error("sync write-ahead log", &path))?;
			},
			None => {},
		}
		file.seek(SeekFrom::End(0))
			.map_err(io_error("seek write-ahead log", &path))?;

		let records = scan.records.len() as u64;
		// A torn header means the file was being created: nothing was lost, nothing to report.
		let torn_tail = scan.torn_at.filter(|offset| *offset >= HEADER_LEN as u64);
		let report = OpenReport { records, torn_tail };
		let shared = Arc::new(Shared {
			state: Mutex::new(FlushState {
				file,
				unsynced_since: None,
				failure: None,
				stop: false,
			}),
			wake: Condvar::new(),
		});
		let flusher = spawn_flusher(Arc::clone(&shared), path.clone())?;
		Ok(FileWal {
			path,
			shared,
			flusher: Some(flusher),
			_lock: lock,
			records,
			report,
			fail_after_bytes: None,
		})
	}

	/// The log file.
	pub fn path(&self) -> &Path {
		&self.path
	}

	/// Whether some appended record is not durable yet.
	pub fn has_unsynced_records(&self) -> bool {
		lock_state(&self.shared).unsynced_since.is_some()
	}

	/// Makes the next append fail after writing `bytes` bytes of its frame, as a full disk would.
	#[doc(hidden)]
	pub fn inject_write_failure_after(&mut self, bytes: usize) {
		self.fail_after_bytes = Some(bytes);
	}

	fn take_flusher_failure(state: &mut FlushState, path: &Path) -> Result<(), StoreError> {
		match state.failure.take() {
			Some(error) => Err(StoreError::io("sync write-ahead log", path, error)),
			None => Ok(()),
		}
	}
}

fn io_error<'a>(operation: &'static str, path: &'a Path) -> impl FnOnce(io::Error) -> StoreError + 'a {
	move |error| StoreError::io(operation, path, error)
}

fn header() -> Vec<u8> {
	let mut header = WAL_MAGIC.to_vec();
	header.extend_from_slice(&WAL_FILE_FORMAT.to_le_bytes());
	header
}

fn frame(payload: &[u8]) -> Vec<u8> {
	let mut body = Vec::with_capacity(2 + payload.len());
	body.extend_from_slice(&WAL_RECORD_FORMAT.to_le_bytes());
	body.extend_from_slice(payload);
	let mut frame = Vec::with_capacity(8 + body.len());
	frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
	frame.extend_from_slice(&crc32fast::hash(&body).to_le_bytes());
	frame.extend_from_slice(&body);
	frame
}

fn acquire_lock(path: &Path) -> Result<File, StoreError> {
	let file = retry_sharing_violation(path, || {
		OpenOptions::new()
			.write(true)
			.create(true)
			.truncate(false)
			.open(path)
	})
	.map_err(|error| StoreError::io("open write-ahead log lock", path, error))?;
	let deadline = Instant::now() + Duration::from_millis(WRITER_WAIT_MS);
	let mut waited = false;
	loop {
		match file.try_lock() {
			Ok(()) => return Ok(file),
			Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
				if !waited {
					tracing::info!(path = %path.display(), "write-ahead log held by another process; waiting");
					waited = true;
				}
				std::thread::sleep(Duration::from_millis(LOCK_POLL_MS));
			},
			Err(TryLockError::WouldBlock) => {
				return Err(StoreError::WalBusy {
					path: path.to_path_buf(),
				});
			},
			Err(TryLockError::Error(error)) => {
				return Err(StoreError::io("lock write-ahead log", path, error));
			},
		}
	}
}

fn spawn_flusher(shared: Arc<Shared>, path: PathBuf) -> Result<JoinHandle<()>, StoreError> {
	let window = Duration::from_millis(DURABILITY_WINDOW_MS);
	std::thread::Builder::new()
		.name("gedo-wal-flusher".into())
		.spawn(move || {
			let mut state = lock_state(&shared);
			loop {
				if state.stop {
					return;
				}
				let Some(since) = state.unsynced_since else {
					state = shared.wake.wait(state).unwrap_or_else(PoisonError::into_inner);
					continue;
				};
				// Flush a little before the window ends so records are durable within it.
				let due = since + window / 2;
				let now = Instant::now();
				if now < due {
					state = shared
						.wake
						.wait_timeout(state, due - now)
						.unwrap_or_else(PoisonError::into_inner)
						.0;
					continue;
				}
				match state.file.sync_data() {
					Ok(()) => state.unsynced_since = None,
					Err(error) => {
						tracing::error!(path = %path.display(), %error, "group commit of the write-ahead log failed");
						state.failure = Some(error);
						state.unsynced_since = None;
					},
				}
			}
		})
		.map_err(|error| StoreError::io("start write-ahead log flusher", "", error))
}

/// Result of reading a log file.
struct Scan {
	/// Every complete record: (version, payload).
	records: Vec<(u16, Vec<u8>)>,
	/// Offset of an incomplete frame at the end (or of an incomplete header), if any.
	torn_at: Option<u64>,
}

fn scan(bytes: &[u8], path: &Path) -> Result<Scan, StoreError> {
	let corrupted = |offset: usize, reason: &str| StoreError::Corrupted {
		path: path.display().to_string(),
		offset: Some(offset as u64),
		reason: reason.to_owned(),
	};
	if bytes.len() < HEADER_LEN {
		let expected = header();
		if bytes != &expected[..bytes.len()] {
			return Err(corrupted(0, "not a Gedo write-ahead log"));
		}
		return Ok(Scan {
			records: Vec::new(),
			torn_at: Some(bytes.len() as u64),
		});
	}
	if &bytes[..WAL_MAGIC.len()] != WAL_MAGIC {
		return Err(corrupted(0, "not a Gedo write-ahead log"));
	}
	let file_format = u16::from_le_bytes([bytes[8], bytes[9]]);
	if file_format != WAL_FILE_FORMAT {
		return Err(StoreError::UnsupportedFormat {
			path: path.display().to_string(),
			found: file_format,
			supported: vec![WAL_FILE_FORMAT],
		});
	}

	let mut records = Vec::new();
	let mut offset = HEADER_LEN;
	while offset < bytes.len() {
		match read_frame(bytes, offset) {
			Frame::Complete {
				version,
				payload,
				next,
			} => {
				records.push((version, payload.to_vec()));
				offset = next;
			},
			Frame::Incomplete => {
				// Only the last append can be torn. If a complete record follows, this frame's
				// length was damaged instead, and dropping the rest would lose good records.
				if valid_frame_after(bytes, offset + 1) {
					return Err(corrupted(offset, "record length does not match the data"));
				}
				return Ok(Scan {
					records,
					torn_at: Some(offset as u64),
				});
			},
			Frame::Damaged(reason) => return Err(corrupted(offset, reason)),
		}
	}
	Ok(Scan {
		records,
		torn_at: None,
	})
}

enum Frame<'a> {
	Complete {
		version: u16,
		payload: &'a [u8],
		next: usize,
	},
	Incomplete,
	Damaged(&'static str),
}

fn read_frame(bytes: &[u8], offset: usize) -> Frame<'_> {
	let Some(frame_header) = bytes.get(offset..offset + FRAME_HEADER_LEN) else {
		return Frame::Incomplete;
	};
	let length =
		u32::from_le_bytes([frame_header[0], frame_header[1], frame_header[2], frame_header[3]]) as usize;
	let checksum = u32::from_le_bytes([frame_header[4], frame_header[5], frame_header[6], frame_header[7]]);
	if !(2..=MAX_RECORD_BYTES + 2).contains(&length) {
		return Frame::Damaged("record length out of range");
	}
	let Some(body) = bytes.get(offset + 8..offset + 8 + length) else {
		return Frame::Incomplete;
	};
	if crc32fast::hash(body) != checksum {
		return Frame::Damaged("record checksum mismatch");
	}
	Frame::Complete {
		version: u16::from_le_bytes([body[0], body[1]]),
		payload: &body[2..],
		next: offset + 8 + length,
	}
}

fn valid_frame_after(bytes: &[u8], from: usize) -> bool {
	(from..bytes.len()).any(|offset| matches!(read_frame(bytes, offset), Frame::Complete { .. }))
}

impl WriteAheadLog for FileWal {
	fn append(&mut self, payload: &[u8]) -> Result<LogSequence, StoreError> {
		if payload.len() > MAX_RECORD_BYTES {
			return Err(StoreError::TooLarge {
				what: "write-ahead log record".into(),
				size: payload.len() as u64,
				limit: MAX_RECORD_BYTES as u64,
			});
		}
		let frame = frame(payload);
		let mut state = lock_state(&self.shared);
		FileWal::take_flusher_failure(&mut state, &self.path)?;
		let end = state
			.file
			.stream_position()
			.map_err(|error| StoreError::io("append to write-ahead log", &self.path, error))?;
		let written = match self.fail_after_bytes.take() {
			Some(limit) => state
				.file
				.write_all(&frame[..limit.min(frame.len())])
				.and_then(|()| {
					Err(io::Error::new(
						io::ErrorKind::StorageFull,
						"injected write failure",
					))
				}),
			None => state.file.write_all(&frame),
		};
		if let Err(error) = written {
			// Never leave a partial record behind a live writer: cut the file back to the last
			// complete record. If even that fails, the next open drops the torn tail.
			if let Err(rollback) = state
				.file
				.set_len(end)
				.and_then(|()| state.file.seek(SeekFrom::Start(end)).map(drop))
			{
				tracing::error!(path = %self.path.display(), %rollback, "could not remove a partial write-ahead log record");
			}
			return Err(StoreError::io("append to write-ahead log", &self.path, error));
		}
		if state.unsynced_since.is_none() {
			state.unsynced_since = Some(Instant::now());
			self.shared.wake.notify_one();
		}
		let sequence = LogSequence(self.records);
		self.records += 1;
		Ok(sequence)
	}

	fn sync(&mut self) -> Result<(), StoreError> {
		let mut state = lock_state(&self.shared);
		FileWal::take_flusher_failure(&mut state, &self.path)?;
		state
			.file
			.sync_data()
			.map_err(|error| StoreError::io("sync write-ahead log", &self.path, error))?;
		state.unsynced_since = None;
		Ok(())
	}

	fn read_all(&self) -> Result<Vec<(LogSequence, Vec<u8>)>, StoreError> {
		let mut bytes = Vec::new();
		{
			let mut state = lock_state(&self.shared);
			let io_error = |error| StoreError::io("read write-ahead log", &self.path, error);
			let end = state.file.stream_position().map_err(io_error)?;
			state.file.seek(SeekFrom::Start(0)).map_err(io_error)?;
			let read = (&mut state.file).take(end).read_to_end(&mut bytes);
			state.file.seek(SeekFrom::Start(end)).map_err(io_error)?;
			read.map_err(io_error)?;
		}
		let scan = scan(&bytes, &self.path)?;
		if let Some(offset) = scan.torn_at {
			return Err(StoreError::Corrupted {
				path: self.path.display().to_string(),
				offset: Some(offset),
				reason: "incomplete record in an open log".into(),
			});
		}
		scan.records
			.into_iter()
			.enumerate()
			.map(|(index, (version, payload))| {
				if version != WAL_RECORD_FORMAT {
					return Err(StoreError::UnsupportedFormat {
						path: self.path.display().to_string(),
						found: version,
						supported: vec![WAL_RECORD_FORMAT],
					});
				}
				Ok((LogSequence(index as u64), payload))
			})
			.collect()
	}

	fn truncate_through(&mut self, sequence: LogSequence) -> Result<(), StoreError> {
		if sequence.0 >= self.records {
			return Err(StoreError::invalid_input(
				"truncate write-ahead log",
				format!("record {sequence} does not exist ({} records)", self.records),
			));
		}
		let kept: Vec<_> = self
			.read_all()?
			.into_iter()
			.skip(sequence.0 as usize + 1)
			.collect();
		let mut contents = header();
		for (_, payload) in &kept {
			contents.extend_from_slice(&frame(payload));
		}

		let directory = self.path.parent().unwrap_or(Path::new("."));
		let temporary = self.path.with_extension("log.tmp");
		let mut replacement =
			File::create(&temporary).map_err(io_error("create write-ahead log copy", &temporary))?;
		replacement
			.write_all(&contents)
			.map_err(io_error("write write-ahead log copy", &temporary))?;
		replacement
			.sync_all()
			.map_err(io_error("sync write-ahead log copy", &temporary))?;
		drop(replacement);

		let mut state = lock_state(&self.shared);
		retry_sharing_violation(&self.path, || std::fs::rename(&temporary, &self.path))
			.map_err(|error| StoreError::io("replace write-ahead log", &self.path, error))?;
		sync_directory(directory)
			.map_err(|error| StoreError::io("sync write-ahead log directory", directory, error))?;
		let mut file = retry_sharing_violation(&self.path, || {
			OpenOptions::new().read(true).write(true).open(&self.path)
		})
		.map_err(|error| StoreError::io("reopen write-ahead log", &self.path, error))?;
		file.seek(SeekFrom::End(0))
			.map_err(|error| StoreError::io("seek write-ahead log", &self.path, error))?;
		state.file = file;
		state.unsynced_since = None;
		self.records = kept.len() as u64;
		Ok(())
	}

	fn open_report(&self) -> &OpenReport {
		&self.report
	}
}

impl Drop for FileWal {
	fn drop(&mut self) {
		{
			let mut state = lock_state(&self.shared);
			if state.unsynced_since.is_some()
				&& let Err(error) = state.file.sync_data()
			{
				tracing::error!(path = %self.path.display(), %error, "final sync of the write-ahead log failed");
			}
			state.stop = true;
			self.shared.wake.notify_one();
		}
		if let Some(flusher) = self.flusher.take()
			&& flusher.join().is_err()
		{
			tracing::error!(path = %self.path.display(), "write-ahead log flusher panicked");
		}
	}
}
