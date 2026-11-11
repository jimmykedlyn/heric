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
/// has no blob.
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

    /// The file backing `id`.
    fn path(&self, id: &BlobId) -> PathBuf {
        blob_path(&self.root, id)
    }
}

impl BlobStore for FsBlobStore {
    fn put(&self, id: &BlobId, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.path(id);
        let directory = path.parent().ok_or_else(|| {
            StorageError::Blob(format!("blob path {} has no directory", path.display()))
        })?;
        std::fs::create_dir_all(directory).map_err(|error| blob_error(directory, &error))?;
        // Written beside the target and renamed into place, so a crash never
        // leaves a half-written blob where a whole one used to be.
        let temporary = temporary_path(&path);
        std::fs::write(&temporary, bytes).map_err(|error| blob_error(&temporary, &error))?;
        // `fs::rename` refuses to replace an existing file on Windows, and the
        // trait promises replacement. Windows has no portable atomic replace
        // in `std`, so the old file goes first; the window is between two
        // local metadata operations, and a crash there leaves an absent blob,
        // which reads as `None` rather than as a corrupt body.
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                let _ = std::fs::remove_file(&temporary);
                return Err(blob_error(&path, &error));
            }
        }
        if let Err(error) = std::fs::rename(&temporary, &path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(blob_error(&path, &error));
        }
        Ok(())
    }

    fn get(&self, id: &BlobId) -> Result<Option<Vec<u8>>, StorageError> {
        let path = self.path(id);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(blob_error(&path, &error)),
        }
    }

    fn stream(&self, id: &BlobId) -> Result<Box<dyn Read + Send>, StorageError> {
        let path = self.path(id);
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
        let path = self.path(id);
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
