//! Opening a repository: object format check, user identity and the local runtime directory.

use std::path::{Path, PathBuf};

use gedo_core::StoreError;

/// Directory, inside the repository's common Git directory, holding Gedo's local runtime files.
pub const GEDO_DIR_NAME: &str = "gedo";

/// Smallest number of pack index slots of the object database. gitoxide fixes the number when a
/// repository is opened (32 by default for a repository with few packs) and cannot read a pack
/// beyond it, so a long-running process that writes packs ([`crate::pack`]) needs room for them.
const MIN_OBJECT_STORE_SLOTS: usize = 1024;

/// The identity chain commits are written with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// `user.name` (or the committer name from the environment).
    pub name: String,
    /// `user.email` (or the committer email from the environment).
    pub email: String,
}

/// An open Git repository (with a working tree, a linked worktree or bare) in the SHA-1 format.
#[derive(Clone)]
pub struct GitRepository {
    repo: gix::ThreadSafeRepository,
}

impl std::fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRepository")
            .field("git_dir", &self.git_dir())
            .finish()
    }
}

impl GitRepository {
    /// Opens the repository at `path` (a working tree, a `.git` directory or a bare repository)
    /// with the user's whole Git configuration, like `git` itself.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        GitRepository::open_with(path.as_ref(), gix::open::Options::default())
    }

    /// Opens the repository reading only its own configuration: no system or global files and no
    /// `GIT_*` environment overrides. Used by tests and tools that need reproducible behavior.
    pub fn open_isolated(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        GitRepository::open_with(path.as_ref(), gix::open::Options::isolated())
    }

    fn open_with(path: &Path, options: gix::open::Options) -> Result<Self, StoreError> {
        let options = options.object_store_slots(gix::odb::store::init::Slots::AsNeededByDiskState {
            multiplier: 1.1,
            minimum: MIN_OBJECT_STORE_SLOTS,
        });
        let repo = gix::ThreadSafeRepository::open_opts(path, options).map_err(|error| {
            StoreError::backend("open repository", format!("{}: {error}", path.display()))
        })?;
        let object_hash = repo.to_thread_local().object_hash();
        if object_hash != gix::hash::Kind::Sha1 {
            return Err(StoreError::UnsupportedObjectFormat {
                repository: path.to_path_buf(),
                found: object_hash.to_string(),
            });
        }
        Ok(GitRepository { repo })
    }

    /// The repository's Git directory (`.git`, or the worktree's own directory for a linked worktree).
    pub fn git_dir(&self) -> &Path {
        self.repo.git_dir()
    }

    /// The Git directory shared by every worktree of the repository.
    pub fn common_dir(&self) -> PathBuf {
        self.local().common_dir().to_path_buf()
    }

    /// `.git/gedo/`, created if missing. Shared by every worktree; never pushed.
    pub fn gedo_dir(&self) -> Result<PathBuf, StoreError> {
        let directory = self.common_dir().join(GEDO_DIR_NAME);
        // `create_dir_all` succeeds when another process creates the directory concurrently.
        std::fs::create_dir_all(&directory)
            .map_err(|error| StoreError::io("create directory", &directory, error))?;
        Ok(directory)
    }

    /// The committer identity, or `MissingIdentity` when none is configured.
    pub fn identity(&self) -> Result<Identity, StoreError> {
        let repo = self.local();
        let missing = || StoreError::MissingIdentity {
            repository: self.git_dir().to_path_buf(),
        };
        let signature = repo
            .committer()
            .ok_or_else(missing)?
            .map_err(|error| StoreError::backend("read identity", error))?;
        let (name, email) = (signature.name.to_string(), signature.email.to_string());
        if name.trim().is_empty() || email.trim().is_empty() {
            return Err(missing());
        }
        Ok(Identity { name, email })
    }

    /// The identity as a signature dated now, for commits and reference log entries.
    pub(crate) fn signature(&self) -> Result<gix::actor::Signature, StoreError> {
        let identity = self.identity()?;
        Ok(gix::actor::Signature {
            name: identity.name.into(),
            email: identity.email.into(),
            time: gix::date::Time::now_local_or_utc(),
        })
    }

    /// A handle for the current thread.
    pub(crate) fn local(&self) -> gix::Repository {
        self.repo.to_thread_local()
    }
}
