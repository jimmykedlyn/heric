//! The store port.

use heric_model::{Change, Doc, Id, Patch, Query};

use crate::error::StorageError;

/// The only path to documents.
///
/// Synchronous on purpose: SQLite is a blocking C API, and every call is made
/// from `tokio::task::spawn_blocking` by `heric-core`. A blocked call holds a
/// blocking-pool thread, never the UI thread.
pub trait Store: Send + Sync {
    /// Reads one document.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the store cannot be read.
    fn get(&self, id: &Id) -> Result<Option<Doc>, StorageError>;

    /// Reads the documents matching a query.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the store cannot be read.
    fn find(&self, query: &Query) -> Result<Vec<Doc>, StorageError>;

    /// Inserts a document and returns it as stored.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DuplicateId`] when the id already exists, and
    /// other variants when the write fails.
    fn insert(&self, doc: &Doc) -> Result<Doc, StorageError>;

    /// Inserts a document or replaces the one with the same id, returning it as
    /// stored.
    ///
    /// Wholesale replacement, which is what the original's `update` does: a
    /// field the stored document has and `doc` does not is removed, not kept.
    /// Partial updates go through [`Store::update`].
    ///
    /// Both paths return `doc`'s own `modified`: the insert stores it as given,
    /// and the default implementation's replacement patch sets it explicitly,
    /// which [`Store::update`] leaves alone instead of stamping the current
    /// time. The default implementation inserts, and on a duplicate turns the
    /// stored copy into `doc` with a full-replacement patch, so an
    /// implementation that can replace in one statement should override it.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the write fails.
    fn upsert(&self, doc: &Doc) -> Result<Doc, StorageError> {
        match self.insert(doc) {
            Ok(inserted) => Ok(inserted),
            Err(StorageError::DuplicateId(_)) => {
                let stored = self
                    .get(&doc.meta.id)?
                    .ok_or_else(|| StorageError::NotFound(doc.meta.id.as_str().to_owned()))?;
                self.update(&doc.meta.id, &replacement_patch(&stored, doc)?)
            }
            Err(error) => Err(error),
        }
    }

    /// Applies a patch and returns the updated document.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] when the id does not exist.
    fn update(&self, id: &Id, patch: &Patch) -> Result<Doc, StorageError>;

    /// Removes a document and its descendants, returning every id removed.
    ///
    /// Descendant walking mirrors the original's `getAllDescendantMap`: deleting
    /// a workspace deletes its requests, and deleting a request deletes its
    /// responses and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the write fails.
    fn remove(&self, id: &Id) -> Result<Vec<Id>, StorageError>;

    /// Subscribes to store changes.
    ///
    /// A `broadcast` receiver rather than a bespoke channel because the consumer
    /// is a GUI frame loop that drains at frame rate, and a lagged receiver
    /// should drop events rather than block the writer.
    fn changes(&self) -> tokio::sync::broadcast::Receiver<Change>;
}

/// A shared handle to the store.
///
/// Cloning is cheap and every clone sees the same database and the same change
/// broadcast.
pub type StoreHandle = std::sync::Arc<dyn Store>;

/// Opens the store at the given paths, creating the database if needed.
///
/// # Errors
///
/// Returns [`StorageError`] when the directory cannot be created, the database
/// cannot be opened, or the schema version is newer than this build supports.
pub fn open(paths: &crate::DataPaths) -> Result<StoreHandle, StorageError> {
    std::fs::create_dir_all(&paths.root).map_err(|error| {
        StorageError::Database(format!(
            "cannot create the data directory {}: {error}",
            paths.root.display()
        ))
    })?;
    let store = crate::sqlite::SqliteStore::open(&paths.database())?;
    Ok(std::sync::Arc::new(store))
}

/// The patch that turns `stored` into `doc`, field for field.
///
/// [`Store::update`] merges, so a wholesale replacement has to say which of the
/// stored fields are gone as well as which changed. Every key is taken from the
/// documents' JSON form, which is the same vocabulary a patch is written in.
fn replacement_patch(stored: &Doc, doc: &Doc) -> Result<Patch, StorageError> {
    let stored = object_of(stored)?;
    let replacement = object_of(doc)?;

    let mut patch = Patch::new();
    for key in stored.keys() {
        if !replacement.contains_key(key) {
            patch = patch.unset(key.clone());
        }
    }
    for (key, value) in &replacement {
        patch = patch.set(key.clone(), value.clone());
    }
    Ok(patch)
}

/// A document as the flat object a patch addresses.
fn object_of(doc: &Doc) -> Result<serde_json::Map<String, serde_json::Value>, StorageError> {
    match serde_json::to_value(doc) {
        Ok(serde_json::Value::Object(fields)) => Ok(fields),
        Ok(_) => Err(StorageError::Database(
            "a document must serialise to a JSON object".to_owned(),
        )),
        Err(error) => Err(StorageError::Database(error.to_string())),
    }
}
