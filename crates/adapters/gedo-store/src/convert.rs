//! Conversions between `gedo-core` ids and gitoxide ids.

use gedo_core::{GitOid, StoreError};

pub(crate) fn to_gix_oid(oid: &GitOid) -> Result<gix::ObjectId, StoreError> {
    gix::ObjectId::try_from(oid.as_bytes()).map_err(|error| StoreError::backend("convert object id", error))
}

pub(crate) fn from_gix_oid(oid: gix::ObjectId) -> Result<GitOid, StoreError> {
    Ok(GitOid::from_bytes(oid.as_bytes())?)
}
