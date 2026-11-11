//! The blob port: response bodies and timelines.

use std::io::Read;

use heric_model::{DocKind, Id};

use crate::error::StorageError;

/// Identifies a blob and says what it holds.
///
/// The original stores response bodies and timelines as side files named after
/// the response id. Keeping the response id and the medium's own name separate
/// is what lets the medium change without touching callers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobId {
    /// The response (or event log) document the blob belongs to.
    pub owner: Id,
    /// The owner's kind, so a blob is never confused between protocols.
    pub owner_kind: DocKind,
    /// Which of the owner's blobs this is.
    pub role: BlobRole,
}

/// The kinds of blob a response owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlobRole {
    /// The response body.
    Body,
    /// The Console timeline, NDJSON.
    Timeline,
    /// The protocol event log, NDJSON.
    EventLog,
}

/// Stores bytes the documents only point at.
///
/// Synchronous for the same reason as [`crate::Store`]. `stream` exists so a
/// multi-megabyte body is not materialised twice on its way to the viewer.
pub trait BlobStore: Send + Sync {
    /// Writes a blob, replacing any previous content.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the write fails.
    fn put(&self, id: &BlobId, bytes: &[u8]) -> Result<(), StorageError>;

    /// Reads a whole blob into memory.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the read fails. A missing blob is not an
    /// error: it returns `Ok(None)`, because the original tolerates a document
    /// whose body file was deleted.
    fn get(&self, id: &BlobId) -> Result<Option<Vec<u8>>, StorageError>;

    /// Opens a blob for streaming.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`] when the blob does not exist and
    /// other variants when it cannot be opened.
    fn stream(&self, id: &BlobId) -> Result<Box<dyn Read + Send>, StorageError>;

    /// Removes a blob. Removing an absent blob succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the removal fails.
    fn remove(&self, id: &BlobId) -> Result<(), StorageError>;
}

/// A shared handle to the blob store.
///
/// Mirrors [`crate::StoreHandle`]: `heric-core` holds trait objects, never the
/// concrete implementation.
pub type BlobStoreHandle = std::sync::Arc<dyn BlobStore>;
