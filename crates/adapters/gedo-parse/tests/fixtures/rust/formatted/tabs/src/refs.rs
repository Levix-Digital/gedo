//! [`RefStore`] over Git reference transactions, through gitoxide.
//!
//! Each update takes the reference's `.lock` file, checks the expected value while holding it and
//! records the change in the reference log, so concurrent processes cannot both win.

use std::time::Duration;

use gedo_core::ids::{GEDO_REF_PREFIX, GEDO_REMOTE_REF_PREFIX, validate_remote_name};
use gedo_core::ports::{Expected, RefOutcome, RefStore, RefUpdate};
use gedo_core::{GedoRefName, GitOid, StoreError};
use gix::bstr::ByteSlice;
use gix::refs::Target;
use gix::refs::file::transaction::prepare::{MustNotExist, ReferenceOutOfDate};
use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};

use crate::convert::{from_gix_oid, to_gix_oid};
use crate::repository::GitRepository;

/// How long an update waits for another writer's lock on the same reference.
pub const REF_LOCK_WAIT_MS: u64 = 2_000;

/// The `refs/gedo/*` references of a Git repository.
#[derive(Clone, Debug)]
pub struct GitRefStore {
	repository: GitRepository,
}

impl GitRefStore {
	/// The reference store of `repository`.
	pub fn new(repository: GitRepository) -> Self {
		GitRefStore { repository }
	}

	/// The values a reference took according to its reference log, oldest first.
	pub fn reflog(&self, name: &GedoRefName) -> Result<Vec<GitOid>, StoreError> {
		let repo = self.repository.local();
		let Some(reference) = find(&repo, name.as_str())? else {
			return Ok(Vec::new());
		};
		let mut platform = reference.log_iter();
		let Some(lines) = platform
			.all()
			.map_err(|error| StoreError::io("read reference log", self.repository.git_dir(), error))?
		else {
			return Ok(Vec::new());
		};
		lines
			.map(|line| {
				let line = line.map_err(|error| StoreError::backend("read reference log", error))?;
				from_gix_oid(line.new_oid())
			})
			.collect()
	}

	/// Applies one edit, waiting for a concurrent writer's lock.
	fn apply(&self, edit: RefEdit) -> Result<(), ApplyError> {
		let repo = self.repository.local();
		let signature = self.repository.signature().map_err(ApplyError::Store)?;
		let mut time = gix::date::parse::TimeBuf::default();
		let wait =
			gix::lock::acquire::Fail::AfterDurationWithBackoff(Duration::from_millis(REF_LOCK_WAIT_MS));
		let transaction = repo
			.refs
			.transaction()
			.prepare(Some(edit), wait, wait)
			.map_err(ApplyError::Git)?;
		transaction
			.commit(signature.to_ref(&mut time))
			.map_err(ApplyError::Git)?;
		Ok(())
	}

	/// Turns a refused edit into `Moved` when the expected value no longer matched.
	fn outcome(&self, name: &GedoRefName, result: Result<(), ApplyError>) -> Result<RefOutcome, StoreError> {
		let error = match result {
			Ok(()) => return Ok(RefOutcome::Updated),
			Err(ApplyError::Store(error)) => return Err(error),
			Err(ApplyError::Git(error)) => error,
		};
		let actual = error
			.downcast_any_ref::<ReferenceOutOfDate>()
			.map(|conflict| &conflict.actual)
			.or_else(|| {
				error
					.downcast_any_ref::<MustNotExist>()
					.map(|conflict| &conflict.actual)
			});
		if let Some(actual) = actual {
			return Ok(RefOutcome::Moved {
				current: Some(target_oid(name.as_str(), actual)?),
			});
		}
		if error.is_not_found() {
			return Ok(RefOutcome::Moved { current: None });
		}
		if error.is_retryable() {
			// The lock stayed taken by other writers for the whole wait; nothing was written.
			tracing::debug!(reference = %name, wait_ms = REF_LOCK_WAIT_MS, "reference lock still busy");
			return Err(StoreError::Contended {
				name: name.to_string(),
				attempts: 1,
			});
		}
		Err(StoreError::backend(
			"update reference",
			format!("{name}: {error:?}"),
		))
	}
}

enum ApplyError {
	Store(StoreError),
	Git(gix::Error),
}

fn full_name(name: &GedoRefName) -> Result<gix::refs::FullName, StoreError> {
	gix::refs::FullName::try_from(name.as_str()).map_err(|error| StoreError::backend("reference name", error))
}

fn find<'repo>(
	repo: &'repo gix::Repository,
	name: &str,
) -> Result<Option<gix::Reference<'repo>>, StoreError> {
	repo.try_find_reference(name)
		.map_err(|error| StoreError::backend("read reference", format!("{name}: {error}")))
}

fn target_oid(name: &str, target: &Target) -> Result<GitOid, StoreError> {
	match target {
		Target::Object(id) => from_gix_oid(*id),
		Target::Symbolic(other) => Err(StoreError::Corrupted {
			path: name.to_owned(),
			offset: None,
			reason: format!("symbolic reference to {other}; Gedo references point to commits"),
		}),
	}
}

/// Every reference whose full name starts with `prefix`, as `(full name, value)`.
fn list_prefixed(repo: &gix::Repository, prefix: &str) -> Result<Vec<(String, GitOid)>, StoreError> {
	let backend =
		|error: &dyn std::fmt::Display| StoreError::backend("list references", format!("{prefix}: {error}"));
	let platform = repo.references().map_err(|error| backend(&error))?;
	let mut listed = Vec::new();
	for reference in platform.prefixed(prefix).map_err(|error| backend(&error))? {
		let reference = reference.map_err(|error| backend(&error))?;
		let name = reference
			.name()
			.as_bstr()
			.to_str()
			.map_err(|error| backend(&error))?
			.to_owned();
		let oid = target_oid(&name, &reference.target().into_owned())?;
		listed.push((name, oid));
	}
	listed.sort();
	Ok(listed)
}

impl RefStore for GitRefStore {
	fn read(&self, name: &GedoRefName) -> Result<Option<GitOid>, StoreError> {
		let repo = self.repository.local();
		let Some(reference) = find(&repo, name.as_str())? else {
			return Ok(None);
		};
		let target = reference.target().into_owned();
		Ok(Some(target_oid(name.as_str(), &target)?))
	}

	fn list(&self, prefix: &GedoRefName) -> Result<Vec<(GedoRefName, GitOid)>, StoreError> {
		let repo = self.repository.local();
		let mut listed = Vec::new();
		for (name, oid) in list_prefixed(&repo, prefix.as_str())? {
			match GedoRefName::new(&name) {
				Ok(name) if name.starts_with(prefix) => listed.push((name, oid)),
				Ok(_) => {},
				Err(error) => {
					tracing::warn!(reference = %name, %error, "ignoring a reference Gedo cannot have written")
				},
			}
		}
		Ok(listed)
	}

	fn list_remote(&self, remote: &str) -> Result<Vec<(String, GitOid)>, StoreError> {
		validate_remote_name(remote)?;
		let prefix = format!("{GEDO_REMOTE_REF_PREFIX}{remote}/");
		let repo = self.repository.local();
		Ok(list_prefixed(&repo, &prefix)?
			.into_iter()
			.map(|(name, oid)| (name[prefix.len()..].to_owned(), oid))
			.collect())
	}

	fn compare_and_swap(&self, update: &RefUpdate) -> Result<RefOutcome, StoreError> {
		debug_assert!(update.name.as_str().starts_with(GEDO_REF_PREFIX));
		let expected = match update.expected {
			Expected::Absent => PreviousValue::MustNotExist,
			Expected::Value(oid) => PreviousValue::MustExistAndMatch(Target::Object(to_gix_oid(&oid)?)),
		};
		let edit = RefEdit {
			change: Change::Update {
				log: LogChange {
					mode: RefLog::AndReference,
					force_create_reflog: true,
					message: update.log_message.as_str().into(),
				},
				expected,
				new: Target::Object(to_gix_oid(&update.new)?),
			},
			name: full_name(&update.name)?,
			deref: false,
		};
		let result = self.apply(edit);
		self.outcome(&update.name, result)
	}

	fn delete(&self, name: &GedoRefName, expected: GitOid) -> Result<RefOutcome, StoreError> {
		let edit = RefEdit {
			change: Change::Delete {
				expected: PreviousValue::MustExistAndMatch(Target::Object(to_gix_oid(&expected)?)),
				log: RefLog::AndReference,
			},
			name: full_name(name)?,
			deref: false,
		};
		let result = self.apply(edit);
		self.outcome(name, result)
	}
}
