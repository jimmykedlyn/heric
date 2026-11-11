//! Reading Insomnia NeDB directories and migrating them into the store.

use std::path::PathBuf;

use heric_model::{DocKind, Id};

/// How a migration should behave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationOptions {
    /// Read and report without writing anything.
    pub dry_run: bool,
    /// Copy the source files to `backups/<timestamp>/` first. The original does
    /// this on upgrade and the design keeps the behaviour, narrowed to the
    /// files that are actually read.
    pub backup: bool,
    /// Where the pre-import backup goes.
    pub backup_dir: Option<PathBuf>,
}

/// Counts per entity type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCount {
    /// The type counted.
    pub kind: DocKind,
    /// Documents read from the source.
    pub read: usize,
    /// Documents written to the store.
    pub written: usize,
    /// Documents skipped, with the reasons in [`MigrationReport::notes`].
    pub skipped: usize,
}

/// What a dry run would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DryRunReport {
    /// Counts per entity type.
    pub counts: Vec<KindCount>,
    /// Corrupt lines, which NeDB tolerates up to its own threshold and so must
    /// Heric.
    pub corrupt_lines: usize,
}

/// A secret the source stored as Electron ciphertext.
///
/// Carried verbatim and reported so the UI can ask the user to re-enter it.
/// See ADR 0011 and section 9 of the design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSecret {
    /// The environment holding it.
    pub environment: Id,
    /// The key within that environment.
    pub key: String,
}

/// The outcome of a migration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Counts per entity type.
    pub counts: Vec<KindCount>,
    /// Response bodies and timelines copied into the blob store.
    pub blobs_written: usize,
    /// Secrets that arrived encrypted and need re-entry.
    pub pending_secrets: Vec<PendingSecret>,
    /// Human-readable notes: skipped documents, unknown types, missing files.
    pub notes: Vec<String>,
    /// Where the pre-import backup landed, when one was taken.
    pub backup_path: Option<PathBuf>,
}

impl MigrationReport {
    /// Total documents written.
    #[must_use]
    pub fn total_written(&self) -> usize {
        self.counts.iter().map(|c| c.written).sum()
    }

    /// Whether the migration completed without skipping anything.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.counts.iter().all(|c| c.skipped == 0) && self.notes.is_empty()
    }
}

/// A NeDB line reader.
///
/// NeDB files are newline-delimited JSON with a few metadata lines
/// (`$$indexCreated`, `$$deleted`). The reader applies appends and deletes in
/// file order, ignores index records, and tolerates corruption up to the
/// original's threshold rather than failing the whole file.
pub struct NeDbReader;

impl NeDbReader {
    /// Reads every document from one NeDB file.
    ///
    /// # Errors
    ///
    /// Returns [`crate::StorageError::SourceUnreadable`] when the file cannot be
    /// opened, and [`crate::StorageError::CorruptBeyondThreshold`] when more
    /// lines fail to parse than the original would tolerate.
    pub fn read_file(
        path: &std::path::Path,
    ) -> Result<Vec<serde_json::Value>, crate::StorageError> {
        let _ = path;
        todo!("M2: stream the file, apply appends and deletes, count corrupt lines")
    }
}

/// Migrates an Insomnia data directory into the store.
///
/// Runs as a single transaction: a failure rolls back and the user keeps an
/// untouched Insomnia installation.
///
/// # Errors
///
/// Returns [`crate::StorageError`] when the source cannot be read or the write
/// fails.
pub fn migrate(
    source: &crate::InsomniaDataDir,
    store: &dyn crate::Store,
    blobs: &dyn crate::BlobStore,
    options: &MigrationOptions,
) -> Result<MigrationReport, crate::StorageError> {
    let _ = (source, store, blobs, options);
    todo!("M2: read, normalise, write, report")
}

/// Reports what a migration would do, writing nothing.
///
/// # Errors
///
/// Returns [`crate::StorageError`] when the source cannot be read.
pub fn dry_run(source: &crate::InsomniaDataDir) -> Result<DryRunReport, crate::StorageError> {
    let _ = source;
    todo!("M2: read and count without writing")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_report_is_clean() {
        assert!(MigrationReport::default().is_clean());
        assert_eq!(MigrationReport::default().total_written(), 0);
    }
}
