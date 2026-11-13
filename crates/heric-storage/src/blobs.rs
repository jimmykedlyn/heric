//! The filesystem blob store.
//!
//! One file per blob at `<root>/blobs/<owner kind>/<role>/<owner id>`, so a
//! 40 MB response body never enters a SQLite `BLOB` column, where the WAL
//! would copy it. A file per blob also makes [`BlobStore::stream`] free: the
//! opened [`std::fs::File`] is the stream. The role directory keeps one
//! owner's body, timeline and event log from colliding.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::DataPaths;
use crate::blob::{BlobId, BlobRole, BlobStore, BlobStoreHandle};
use crate::error::StorageError;

/// The directory holding every blob, under the data `root`.
fn blob_root(root: &Path) -> PathBuf {
    DataPaths::at(root).blobs()
}

/// The file a blob is stored in, under the data `root`.
///
/// The path is derived, never listed: an owner id that names no file simply
/// has no blob. The function is pure and does not judge the id; a store
/// refuses an id that would escape the blob directory before calling this, so
/// a path returned here is only safe for introspection on its own.
#[must_use]
pub fn blob_path(root: &Path, id: &BlobId) -> PathBuf {
    blob_root(root)
        .join(id.owner_kind.as_str())
        .join(role_directory(id.role))
        .join(id.owner.as_str())
}

/// Whether `path` names an Insomnia datafile, `insomnia.<kind>.db`.
///
/// The check is lexical: it inspects the file name and never touches the
/// filesystem. The renderer's `{% file %}` tag and the plugin scope receive a
/// literal from a template, and what they need to know is whether the name
/// denotes a datafile, not whether a file currently exists at it.
///
/// Heric never reads or overwrites a datafile this way (`STO-020`).
#[must_use]
pub fn is_datafile(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_datafile_name)
}

/// Whether `name` is `insomnia.<kind>.db` with a non-empty `<kind>`.
fn is_datafile_name(name: &str) -> bool {
    name.strip_prefix("insomnia.")
        .and_then(|rest| rest.strip_suffix(".db"))
        .is_some_and(|kind| !kind.is_empty())
}

/// The directory segment a role is stored under.
fn role_directory(role: BlobRole) -> &'static str {
    match role {
        BlobRole::Body => "body",
        BlobRole::Timeline => "timeline",
        BlobRole::EventLog => "event-log",
    }
}

/// Opens the blob store under the given data paths, creating its directory if
/// needed.
///
/// # Errors
///
/// Returns [`StorageError`] when the blob directory cannot be created.
pub fn open_blobs(paths: &DataPaths) -> Result<BlobStoreHandle, StorageError> {
    Ok(Arc::new(FsBlobStore::open(&paths.root)?))
}

/// The blob store, over Heric's own data directory.
struct FsBlobStore {
    root: PathBuf,
    /// Keeps the temporary directory alive for as long as the store uses it.
    #[cfg(test)]
    _temp_dir: Option<tempfile::TempDir>,
}

impl FsBlobStore {
    /// Opens the store rooted at `root`, creating the blob directory if needed.
    fn open(root: &Path) -> Result<Self, StorageError> {
        let directory = blob_root(root);
        std::fs::create_dir_all(&directory).map_err(|error| blob_error(&directory, &error))?;
        Ok(Self {
            root: root.to_path_buf(),
            #[cfg(test)]
            _temp_dir: None,
        })
    }

    /// The file backing `id`, or the reason its id cannot name one.
    ///
    /// [`blob_path`] derives the path without judging the id, because it is a
    /// pure function. The store cannot afford that: an owner id arrives from a
    /// datafile Heric did not write, and [`crate::BlobStore`] must never read
    /// or write outside its own blob directory.
    fn checked_path(&self, id: &BlobId) -> Result<PathBuf, StorageError> {
        if !is_single_segment(id.owner.as_str()) {
            return Err(StorageError::Database(format!(
                "blob owner id {:?} is not a single path segment",
                id.owner.as_str()
            )));
        }
        Ok(blob_path(&self.root, id))
    }
}

/// Whether `raw` can be a path segment without escaping the directory it is
/// joined to.
///
/// The test is the one the standard library itself uses: the id must parse as
/// exactly one [`Component::Normal`]. That rejects `..` and `.`, a separator
/// on either platform (an id may have been written on the other one), and —
/// on Windows — a drive-relative spelling such as `C:evil`, where
/// `base.join("C:evil")` discards `base` entirely while containing no
/// separator at all.
fn is_single_segment(raw: &str) -> bool {
    // `Path::components` only splits on the host's separator, so on Unix a
    // backslash reads as an ordinary name character. Check both explicitly:
    // an id written on Windows must not name a different path on Linux, and
    // the reverse. On Unix this is what makes the test below fail rather than
    // quietly create a file literally called `..\..\..\insomnia.Request.db`.
    if raw.contains('/') || raw.contains('\\') {
        return false;
    }
    matches!(
        Path::new(raw).components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    )
}

impl BlobStore for FsBlobStore {
    fn put(&self, id: &BlobId, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.checked_path(id)?;
        let directory = path.parent().ok_or_else(|| {
            StorageError::Blob(format!("blob path {} has no directory", path.display()))
        })?;
        std::fs::create_dir_all(directory).map_err(|error| blob_error(directory, &error))?;
        // Written beside the target and renamed into place, so a crash leaves
        // either the previous blob or the new one, never a truncated file.
        // `fs::rename` replaces an existing destination on Windows as well as
        // on Unix, so no removal is needed first and the swap is atomic.
        let temporary = temporary_path(&path);
        std::fs::write(&temporary, bytes).map_err(|error| blob_error(&temporary, &error))?;
        if let Err(error) = std::fs::rename(&temporary, &path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(blob_error(&path, &error));
        }
        Ok(())
    }

    fn get(&self, id: &BlobId) -> Result<Option<Vec<u8>>, StorageError> {
        let path = self.checked_path(id)?;
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(blob_error(&path, &error)),
        }
    }

    fn stream(&self, id: &BlobId) -> Result<Box<dyn Read + Send>, StorageError> {
        let path = self.checked_path(id)?;
        match File::open(&path) {
            Ok(file) => {
                let reader: Box<dyn Read + Send> = Box::new(file);
                Ok(reader)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                Err(StorageError::NotFound(id.owner.as_str().to_owned()))
            }
            Err(error) => Err(blob_error(&path, &error)),
        }
    }

    fn remove(&self, id: &BlobId) -> Result<(), StorageError> {
        let path = self.checked_path(id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(blob_error(&path, &error)),
        }
    }
}

/// `path` with `.tmp` appended, the file a write lands in before the rename.
fn temporary_path(path: &Path) -> PathBuf {
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(".tmp");
    PathBuf::from(temporary)
}

/// A blob operation failure, naming the file it happened on.
fn blob_error(path: &Path, error: &std::io::Error) -> StorageError {
    StorageError::Blob(format!("{}: {error}", path.display()))
}

/// A store over a fresh temporary directory, for tests.
///
/// The handle owns the temporary directory, so the store outlives every
/// reference to it and disappears when the last one is dropped.
///
/// # Errors
///
/// Returns [`StorageError`] when the temporary directory cannot be created.
#[cfg(test)]
fn ephemeral_blobs() -> Result<BlobStoreHandle, StorageError> {
    let directory = tempfile::tempdir()
        .map_err(|error| StorageError::Blob(format!("temporary directory: {error}")))?;
    let mut store = FsBlobStore::open(directory.path())?;
    store._temp_dir = Some(directory);
    Ok(Arc::new(store))
}

#[cfg(test)]
mod tests {
    use heric_model::{DocKind, Id};

    use super::*;

    #[test]
    fn a_blob_streams_back_byte_for_byte() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral_blobs()?;
        let id = BlobId {
            owner: Id::new("res_1"),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        store.put(&id, b"hello")?;
        assert_eq!(store.get(&id)?, Some(b"hello".to_vec()));
        let mut reader = store.stream(&id)?;
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut buffer)?;
        assert_eq!(buffer, b"hello");
        store.remove(&id)?;
        assert_eq!(store.get(&id)?, None);
        Ok(())
    }

    #[test]
    fn a_missing_blob_reads_as_absent_rather_than_failing() -> Result<(), Box<dyn std::error::Error>>
    {
        let store = ephemeral_blobs()?;
        let id = BlobId {
            owner: Id::new("res_absent"),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        assert_eq!(store.get(&id)?, None);
        assert!(matches!(store.stream(&id), Err(StorageError::NotFound(_))));
        Ok(())
    }

    #[test]
    fn the_three_roles_do_not_collide() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral_blobs()?;
        let owner = Id::new("res_roles");
        let body = BlobId {
            owner: owner.clone(),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        let timeline = BlobId {
            owner: owner.clone(),
            owner_kind: DocKind::Response,
            role: BlobRole::Timeline,
        };
        let log = BlobId {
            owner,
            owner_kind: DocKind::Response,
            role: BlobRole::EventLog,
        };
        store.put(&body, b"body")?;
        store.put(&timeline, b"timeline")?;
        store.put(&log, b"log")?;
        assert_eq!(store.get(&body)?.as_deref(), Some(&b"body"[..]));
        assert_eq!(store.get(&timeline)?.as_deref(), Some(&b"timeline"[..]));
        assert_eq!(store.get(&log)?.as_deref(), Some(&b"log"[..]));
        assert_ne!(store.get(&body)?, store.get(&timeline)?);
        assert_ne!(store.get(&body)?, store.get(&log)?);
        assert_ne!(store.get(&timeline)?, store.get(&log)?);
        Ok(())
    }

    #[test]
    fn a_put_replaces_an_existing_blob() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral_blobs()?;
        let id = BlobId {
            owner: Id::new("res_replaced"),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        store.put(&id, b"first")?;
        store.put(&id, b"second")?;
        assert_eq!(store.get(&id)?, Some(b"second".to_vec()));
        Ok(())
    }

    #[test]
    fn the_on_disk_layout_is_the_documented_one() {
        let root = Path::new("/data/heric");
        let id = BlobId {
            owner: Id::new("res_1"),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        assert_eq!(blob_path(root, &id), root.join("blobs/Response/body/res_1"));
    }

    #[test]
    fn a_path_traversing_id_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let probe = tempfile::tempdir()?;
        // A file the unguarded `blob_path` would resolve to on Windows: three
        // `..` climb back from `blobs/Response/body/` to the data root itself.
        // On Unix a backslash is an ordinary name character, so the same string
        // names a file directly inside `blobs/Response/body/` instead.
        let outside = probe.path().join("insomnia.Request.db");
        std::fs::write(&outside, b"source")?;
        let store = FsBlobStore::open(probe.path())?;
        std::fs::create_dir_all(probe.path().join("blobs/Response/body"))?;

        // Only Windows reads `\` as a separator, so only there does the
        // unguarded path actually land on `outside`. On Unix the prologue is
        // still worth running — it proves the id is refused rather than
        // creating a strangely named blob — but there is no traversal to show.
        #[cfg(windows)]
        {
            let traversing = BlobId {
                owner: Id::new("..\\..\\..\\insomnia.Request.db"),
                owner_kind: DocKind::Response,
                role: BlobRole::Body,
            };
            assert_eq!(
                std::fs::canonicalize(blob_path(probe.path(), &traversing))?,
                std::fs::canonicalize(&outside)?,
                "the unguarded path is exactly the file the guard must not reach"
            );
        }

        for owner in [
            "..\\..\\..\\insomnia.Request.db",
            "a/b",
            "..",
            "",
            "a\\b",
            ".",
        ] {
            let id = BlobId {
                owner: Id::new(owner),
                owner_kind: DocKind::Response,
                role: BlobRole::Body,
            };
            assert!(
                matches!(store.put(&id, b"escaped"), Err(StorageError::Database(_))),
                "put must refuse the id {owner:?}"
            );
            assert!(
                matches!(store.get(&id), Err(StorageError::Database(_))),
                "get must refuse the id {owner:?}"
            );
            assert!(
                matches!(store.stream(&id), Err(StorageError::Database(_))),
                "stream must refuse the id {owner:?}"
            );
            assert!(
                matches!(store.remove(&id), Err(StorageError::Database(_))),
                "remove must refuse the id {owner:?}"
            );
        }

        // The file a naive read would have returned is untouched, so nothing
        // was read from it and nothing was written over it.
        assert_eq!(std::fs::read(&outside)?, b"source");
        Ok(())
    }

    /// A drive-relative spelling is a single component on Unix but makes
    /// `Path::join` discard the base on Windows, where it must be refused.
    #[cfg(windows)]
    #[test]
    fn a_drive_relative_id_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral_blobs()?;
        let id = BlobId {
            owner: Id::new("C:evil"),
            owner_kind: DocKind::Response,
            role: BlobRole::Body,
        };
        assert!(matches!(
            store.put(&id, b"escaped"),
            Err(StorageError::Database(_))
        ));
        Ok(())
    }

    #[test]
    fn datafiles_are_recognised() {
        assert!(is_datafile(Path::new("/x/insomnia.Request.db")));
        assert!(is_datafile(Path::new("/x/insomnia.Workspace.db")));
        assert!(is_datafile(Path::new("/x/insomnia.WorkspaceMeta.db")));
        assert!(!is_datafile(Path::new("/x/response.json")));
        assert!(!is_datafile(Path::new("/x/notes.db")));
        assert!(!is_datafile(Path::new("/x/insomnia.db")));
        assert!(!is_datafile(Path::new("/x/insomnia..db")));
        assert!(!is_datafile(Path::new("/x/insomnia.Workspace.json")));
        assert!(!is_datafile(Path::new("/x/insomnia.Workspace.db.bak")));
        assert!(!is_datafile(Path::new("/")));
    }
}
