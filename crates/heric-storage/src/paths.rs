//! Platform data directories and Insomnia data directory discovery.

use std::path::{Path, PathBuf};

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

    /// The content-addressed blob store.
    #[must_use]
    pub fn blobs(&self) -> PathBuf {
        self.root.join("blobs")
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
/// Detection, never assumption. An explicit path wins: the argument when one is
/// given, otherwise `INSOMNIA_DATA_PATH`. Either is used verbatim, because the
/// user pointed at it and silently migrating a different directory would be
/// worse than failing; a path that is not a directory is reported as
/// [`StorageError::SourceNotFound`] naming it. Only when neither is supplied are
/// the platform defaults probed, in the order the original uses
/// (`%APPDATA%\Insomnia`, `~/.config/Insomnia`,
/// `~/Library/Application Support/Insomnia`), and those additionally have to
/// hold at least one `insomnia.*.db` file so an unrelated `Insomnia` directory
/// is never mistaken for a data directory.
///
/// # Errors
///
/// Returns [`StorageError::SourceNotFound`] when the explicit path does not
/// exist or no platform candidate holds an Insomnia database, listing every
/// path that was tried.
pub fn discover_insomnia_data_dir(
    explicit: Option<PathBuf>,
) -> Result<InsomniaDataDir, StorageError> {
    if let Some(path) = explicit {
        return explicit_source(path);
    }
    if let Some(path) = std::env::var_os(INSOMNIA_DATA_PATH).map(PathBuf::from) {
        return explicit_source(path);
    }
    let candidates = platform_candidates();
    for path in &candidates {
        if holds_insomnia_database(path) {
            return Ok(InsomniaDataDir {
                path: path.clone(),
                layout: SourceLayout::PlatformDefault,
            });
        }
    }
    Err(StorageError::SourceNotFound(describe(&candidates)))
}

/// Accepts a user-supplied directory verbatim, or reports it as missing.
fn explicit_source(path: PathBuf) -> Result<InsomniaDataDir, StorageError> {
    if path.is_dir() {
        Ok(InsomniaDataDir {
            layout: SourceLayout::Explicit(path.clone()),
            path,
        })
    } else {
        Err(StorageError::SourceNotFound(path.display().to_string()))
    }
}

/// Joins candidate paths into the message [`StorageError::SourceNotFound`] carries.
fn describe(candidates: &[PathBuf]) -> String {
    candidates
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The environment variable the original honors as a data-root override.
const INSOMNIA_DATA_PATH: &str = "INSOMNIA_DATA_PATH";

/// The platform default directories, as absolute paths.
///
/// A candidate whose base is missing (`APPDATA` unset, no home directory) is
/// skipped rather than guessed from environment strings.
fn platform_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("Insomnia"));
    }
    if let Some(base) = directories::BaseDirs::new() {
        let home = base.home_dir();
        candidates.push(home.join(".config").join("Insomnia"));
        candidates.push(
            home.join("Library")
                .join("Application Support")
                .join("Insomnia"),
        );
    }
    candidates
}

/// Whether `dir` is a directory holding at least one `insomnia.*.db` file.
fn holds_insomnia_database(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("insomnia.") && name.ends_with(".db"))
    })
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

    #[test]
    fn a_directory_with_a_database_holds_its_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join("heric-insomnia-probe-database");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("insomnia.Workspace.db"), b"")?;
        assert!(holds_insomnia_database(&dir));
        Ok(())
    }

    #[test]
    fn a_directory_without_a_database_is_not_a_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join("heric-insomnia-probe-empty");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("notes.db"), b"")?;
        std::fs::write(dir.join("insomnia.Workspace.json"), b"")?;
        assert!(!holds_insomnia_database(&dir));
        Ok(())
    }

    #[test]
    fn a_path_that_is_not_a_directory_is_not_a_source() -> Result<(), Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join("heric-insomnia-probe-file");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join("plain-file");
        std::fs::write(&file, b"")?;
        assert!(!holds_insomnia_database(&file));
        Ok(())
    }

    #[test]
    fn an_explicit_root_places_every_path_under_it() -> Result<(), StorageError> {
        let root = std::env::temp_dir().join("heric-paths-root");
        let paths = DataPaths::at(&root);
        assert_eq!(paths.database(), root.join("heric.db"));
        assert_eq!(paths.blobs(), root.join("blobs"));
        Ok(())
    }

    #[test]
    fn an_explicit_insomnia_directory_is_used_verbatim() -> Result<(), Box<dyn std::error::Error>> {
        let source = std::env::temp_dir().join("heric-insomnia-explicit");
        std::fs::create_dir_all(&source)?;
        let found = discover_insomnia_data_dir(Some(source.clone()))?;
        assert_eq!(found.path, source);
        assert_eq!(found.layout, SourceLayout::Explicit(source));
        Ok(())
    }

    #[test]
    fn a_missing_explicit_directory_is_reported_not_ignored() {
        let missing = std::env::temp_dir().join("heric-insomnia-absent");
        assert!(matches!(
            discover_insomnia_data_dir(Some(missing)),
            Err(StorageError::SourceNotFound(_))
        ));
    }
}
