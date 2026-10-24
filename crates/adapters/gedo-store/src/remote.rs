//! Remote configuration and push arguments for Gedo data (FR-013, ADR-0011).
//!
//! A plain `git fetch` mirrors each remote's `refs/gedo/*` into `refs/gedo-remote/<remote>/*`,
//! where Gedo merges it into the local chains; local chains are never overwritten. No push
//! refspec is ever added: it would replace Git's `push.default` behavior for branches. The push
//! of `refs/gedo/*` runs from the pre-push hook with [`push_arguments`].

use std::io::Write;
use std::time::Duration;

use gedo_core::StoreError;
use gedo_core::ids::{GEDO_REF_PREFIX, GEDO_REMOTE_REF_PREFIX, validate_remote_name};

use crate::repository::GitRepository;

/// Environment variable set on the inner `git push` of Gedo data, so the pre-push hook it
/// triggers does not push again.
pub const INNER_PUSH_GUARD: &str = "GEDO_INNER_PUSH";
/// How long the setup waits for another program editing `.git/config`.
pub const CONFIG_LOCK_WAIT_MS: u64 = 2_000;

/// What the remote setup did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteSetup {
    /// The fetch refspec was added.
    Added,
    /// The remote already had it; nothing changed.
    AlreadyConfigured,
}

/// The fetch refspec that mirrors `remote`'s Gedo data: `+refs/gedo/*:refs/gedo-remote/<remote>/*`.
pub fn fetch_refspec(remote: &str) -> String {
    format!("+{GEDO_REF_PREFIX}*:{GEDO_REMOTE_REF_PREFIX}{remote}/*")
}

/// Arguments of the `git` invocation that pushes Gedo data to `remote`, fast-forward only.
pub fn push_arguments(remote: &str) -> [String; 3] {
    [
        "push".to_owned(),
        remote.to_owned(),
        format!("{GEDO_REF_PREFIX}*:{GEDO_REF_PREFIX}*"),
    ]
}

/// Adds the Gedo fetch refspec to `remote` in the repository's own configuration, once.
///
/// Every other setting, including the remote's other fetch refspecs, is kept as it is. The
/// remote must already exist in the repository configuration.
pub fn configure_remote(repository: &GitRepository, remote: &str) -> Result<RemoteSetup, StoreError> {
    validate_remote_name(remote)?;
    let path = repository.common_dir().join("config");
    let wait = gix::lock::acquire::Fail::AfterDurationWithBackoff(Duration::from_millis(CONFIG_LOCK_WAIT_MS));
    let mut lock = gix::lock::File::acquire_to_update_resource(&path, wait, None, 0).map_err(|error| {
        StoreError::backend(
            "lock repository configuration",
            format!("{}: {error}", path.display()),
        )
    })?;

    let mut config = gix::config::File::from_path_no_includes(path.clone(), gix::config::Source::Local)
        .map_err(|error| {
            StoreError::backend(
                "read repository configuration",
                format!("{}: {error}", path.display()),
            )
        })?;
    if config.string_by("remote", Some(remote.into()), "url").is_none() {
        return Err(StoreError::invalid_input(
            "configure remote",
            format!("remote {remote:?} has no url in {}", path.display()),
        ));
    }
    let refspec = fetch_refspec(remote);
    let existing = config
        .strings_by("remote", Some(remote.into()), "fetch")
        .unwrap_or_default();
    if existing.iter().any(|value| value == refspec.as_str()) {
        return Ok(RemoteSetup::AlreadyConfigured);
    }
    config
        .section_mut_or_create_new("remote", Some(remote.into()))
        .and_then(|mut section| section.push("fetch", Some(refspec.as_str().into())).map(drop))
        .map_err(|error| StoreError::backend("edit repository configuration", error))?;

    let mut contents = Vec::new();
    config
        .write_to(&mut contents)
        .map_err(|error| StoreError::io("write repository configuration", &path, error))?;
    lock.write_all(&contents)
        .map_err(|error| StoreError::io("write repository configuration", &path, error))?;
    lock.commit()
        .map_err(|error| StoreError::io("replace repository configuration", &path, error.error))?;
    tracing::info!(remote, refspec, "configured the remote to fetch Gedo data");
    Ok(RemoteSetup::Added)
}
