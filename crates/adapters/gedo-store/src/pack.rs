//! Writing a batch of new objects as one pack with its index, the way `git` stores a batch it
//! fetches, instead of one loose object file each.
//!
//! The pack and its index are written to temporary files and renamed into `objects/pack/` (the
//! index last), so a pack is either complete or not seen by the object database.

use std::io::Write;
use std::sync::atomic::AtomicBool;

use gedo_core::StoreError;
use gix::odb::pack::{Bundle, bundle};

/// Smallest batch of new objects written as a pack; smaller batches are written as loose
/// objects, like `git`'s `fetch.unpackLimit` default.
pub(crate) const PACK_THRESHOLD: usize = 100;

/// Writes `objects`, none of which is in the object database yet, as one pack.
pub(crate) fn write_pack(
    repo: &gix::Repository,
    objects: &[(gix::object::Kind, &[u8])],
) -> Result<(), StoreError> {
    let stream = pack_stream(objects)?;
    let directory = repo.objects.store_ref().path().join("pack");
    std::fs::create_dir_all(&directory)
        .map_err(|error| StoreError::io("create directory", &directory, error))?;
    let outcome = Bundle::write_to_directory(
        &mut stream.as_slice(),
        Some(&directory),
        &mut gix::progress::Discard,
        &AtomicBool::new(false),
        None::<gix::objs::find::Never>,
        gix::hash::Kind::Sha1,
        bundle::write::Options {
            thread_limit: Some(1),
            ..bundle::write::Options::default()
        },
    )
    .map_err(|error| StoreError::backend("write pack", error))?;
    // The `.keep` file protects a fetched pack until references point to it; the collection's
    // reference is updated right after this write, and a kept pack would never be repacked.
    if let Some(keep) = outcome.keep_path
        && let Err(error) = std::fs::remove_file(&keep)
    {
        tracing::warn!(path = %keep.display(), %error, "could not remove the pack keep file");
    }
    Ok(())
}

/// The bytes of a version 2 pack holding `objects` undeltified, with its trailing checksum.
fn pack_stream(objects: &[(gix::object::Kind, &[u8])]) -> Result<Vec<u8>, StoreError> {
    let count = u32::try_from(objects.len())
        .map_err(|_| StoreError::invalid_input("write pack", "too many objects in one pack"))?;
    let mut pack = Vec::with_capacity(12 + objects.iter().map(|(_, data)| data.len() + 16).sum::<usize>());
    pack.extend_from_slice(b"PACK");
    pack.extend_from_slice(&2u32.to_be_bytes());
    pack.extend_from_slice(&count.to_be_bytes());
    for (kind, data) in objects {
        entry_header(&mut pack, entry_type(*kind), data.len() as u64);
        let mut deflate =
            gix::zlib::stream::deflate::Write::new(&mut pack, gix::zlib::Compression::BEST_SPEED);
        deflate
            .write_all(data)
            .and_then(|()| deflate.flush())
            .map_err(|error| StoreError::backend("compress pack entry", error))?;
    }
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(&pack);
    let checksum = hasher
        .try_finalize()
        .map_err(|error| StoreError::backend("hash pack", error))?;
    pack.extend_from_slice(checksum.as_bytes());
    Ok(pack)
}

/// The pack entry type of an object kind.
fn entry_type(kind: gix::object::Kind) -> u8 {
    match kind {
        gix::object::Kind::Commit => 1,
        gix::object::Kind::Tree => 2,
        gix::object::Kind::Blob => 3,
        gix::object::Kind::Tag => 4,
    }
}

/// An entry header: the type and the size of the uncompressed object, 4 bits then 7 per byte.
fn entry_header(pack: &mut Vec<u8>, kind: u8, size: u64) {
    let mut byte = (kind << 4) | (size & 0x0f) as u8;
    let mut rest = size >> 4;
    while rest != 0 {
        pack.push(byte | 0x80);
        byte = (rest & 0x7f) as u8;
        rest >>= 7;
    }
    pack.push(byte);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_headers_follow_the_pack_format() {
        let header = |size| {
            let mut bytes = Vec::new();
            entry_header(&mut bytes, entry_type(gix::object::Kind::Blob), size);
            bytes
        };
        assert_eq!(header(0), [0x30]);
        assert_eq!(header(15), [0x3f]);
        assert_eq!(header(16), [0xb0, 0x01]);
        assert_eq!(header(300), [0xbc, 0x12]);
        assert_eq!(header(1 << 20), [0xb0, 0x80, 0x80, 0x04]);
    }
}
