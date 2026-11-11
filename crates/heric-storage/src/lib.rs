//! Persistence for Heric: the document store, the blob store, and the NeDB
//! migration layer.
//!
//! Nothing outside this crate knows that SQLite is involved. Callers hold
//! [`Store`] and [`BlobStore`] trait objects; the concrete implementations are
//! constructed here by [`open`] and [`open_blobs`] and injected by `heric-core`.

#![deny(unsafe_code)]

mod blob;
mod blobs;
mod error;
mod migratedb;
mod paths;
mod sqlite;
mod store;

pub use blob::{BlobId, BlobRole, BlobStore, BlobStoreHandle};
pub use blobs::{blob_path, is_datafile, open_blobs};
pub use error::{StorageDiagnostic, StorageError};
pub use heric_model::Change;
pub use migratedb::{
    DryRunReport, KindCount, MigrationOptions, MigrationReport, NeDbReader, PendingSecret, dry_run,
    migrate,
};
pub use paths::{DataPaths, InsomniaDataDir, SourceLayout, discover_insomnia_data_dir};
pub use store::{Store, StoreHandle, open};

/// Returns the version of `heric-storage`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
