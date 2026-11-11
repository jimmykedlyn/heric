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
    let _ = paths;
    todo!("M1: open SQLite at paths.database, run migrations, return a handle")
}
