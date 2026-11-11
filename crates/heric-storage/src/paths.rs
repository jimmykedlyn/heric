//! Platform data directories and Insomnia data directory discovery.

use std::path::PathBuf;

use crate::error::StorageError;

/// Where Heric keeps its own data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    /// Directory holding `heric.db`, `ui-state.json`, `logs/` and `backups/`.
    pub root: PathBuf,
}

impl DataPaths {
    /// Resolves the platform data directory and the paths under it.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NoDataDir`] when the platform reports no data
    /// directory, which happens in a container without `HOME`.
    pub fn resolve() -> Result<Self, StorageError> {
        let dirs =
            directories::ProjectDirs::from("", "", "Heric").ok_or(StorageError::NoDataDir)?;
        Ok(Self {
            root: dirs.data_dir().to_path_buf(),
        })
    }

    /// Builds paths rooted at an explicit directory, for tests and for a
    /// `--data-dir` override.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The SQLite database.
    #[must_use]
    pub fn database(&self) -> PathBuf {
        self.root.join("heric.db")
    }

    /// The UI state file kept out of sync (section 16.7 of the design).
    #[must_use]
    pub fn ui_state(&self) -> PathBuf {
        self.root.join("ui-state.json")
    }

    /// The log directory.
    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// The directory holding pre-import backups.
    #[must_use]
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
}

/// An Insomnia data directory that Heric may read.
///
/// Read-only by contract: Heric never writes into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsomniaDataDir {
    /// The directory itself.
    pub path: PathBuf,
    /// Which layout was found, because a directory may hold both.
    pub layout: SourceLayout,
}

/// How to find the source data directory for a migration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SourceLayout {
    /// The platform default: `%APPDATA%\Insomnia`, `~/.config/Insomnia` or
    /// `~/Library/Application Support/Insomnia`.
    #[default]
    PlatformDefault,
    /// An explicit directory, from `INSOMNIA_DATA_PATH` or a CLI flag.
    Explicit(PathBuf),
}

/// Finds the Insomnia data directory to migrate from.
///
/// Detection, never assumption: the platform paths are tried in the order the
/// original uses, then `INSOMNIA_DATA_PATH`, and an explicitly supplied path
/// wins over both.
///
/// # Errors
///
/// Returns [`StorageError::SourceNotFound`] when no candidate exists.
pub fn discover_insomnia_data_dir(
    explicit: Option<PathBuf>,
) -> Result<InsomniaDataDir, StorageError> {
    let _ = explicit;
    todo!("M2: probe platform paths and INSOMNIA_DATA_PATH")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_hang_off_the_root() {
        let paths = DataPaths::at("/tmp/heric");
        assert_eq!(paths.database(), PathBuf::from("/tmp/heric/heric.db"));
        assert_eq!(paths.backups(), PathBuf::from("/tmp/heric/backups"));
    }
}
