//! Platform data directories and Insomnia data directory discovery.

use std::ffi::OsString;
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

    /// The directory holding response bodies, timelines and event logs.
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
/// An `INSOMNIA_DATA_PATH` set to the empty string counts as unset, so a shell
/// profile that exports the variable empty does not block migration.
///
/// # Errors
///
/// Returns [`StorageError::SourceNotFound`] when the explicit path does not
/// exist or no platform candidate holds an Insomnia database, listing every
/// path that was tried.
pub fn discover_insomnia_data_dir(
    explicit: Option<PathBuf>,
) -> Result<InsomniaDataDir, StorageError> {
    discover_from(
        explicit,
        std::env::var_os(INSOMNIA_DATA_PATH),
        &platform_candidates(),
    )
}

/// The decision half of [`discover_insomnia_data_dir`], with every input
/// supplied by the caller so it can be tested without touching process state.
fn discover_from(
    explicit: Option<PathBuf>,
    env_value: Option<OsString>,
    candidates: &[PathBuf],
) -> Result<InsomniaDataDir, StorageError> {
    if let Some(path) = explicit {
        return explicit_source(path);
    }
    if let Some(path) = env_value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return explicit_source(path);
    }
    for path in candidates {
        if holds_insomnia_database(path) {
            return Ok(InsomniaDataDir {
                path: path.clone(),
                layout: SourceLayout::PlatformDefault,
            });
        }
    }
    Err(StorageError::SourceNotFound(describe(candidates)))
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

/// Whether `dir` is a directory holding at least one data file.
///
/// See [`crate::is_datafile`] for the name rule. The entry's own file type is
/// checked here because a directory can carry a datafile's name; the predicate
/// itself stays lexical.
fn holds_insomnia_database(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        is_file && crate::is_datafile(Path::new(&entry.file_name()))
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

    #[test]
    fn the_environment_variable_beats_the_platform_candidates()
    -> Result<(), Box<dyn std::error::Error>> {
        let chosen = temp_source("heric-env-wins")?;
        let candidate = temp_source("heric-env-loses")?;
        let found = discover_from(
            None,
            Some(chosen.as_os_str().to_owned()),
            std::slice::from_ref(&candidate),
        )?;
        assert_eq!(found.path, chosen);
        assert_eq!(found.layout, SourceLayout::Explicit(chosen));
        Ok(())
    }

    #[test]
    fn an_explicit_argument_beats_the_environment_variable()
    -> Result<(), Box<dyn std::error::Error>> {
        let chosen = temp_source("heric-argument-wins")?;
        let loser = temp_source("heric-argument-loses")?;
        let found = discover_from(
            Some(chosen.clone()),
            Some(loser.as_os_str().to_owned()),
            &[],
        )?;
        assert_eq!(found.path, chosen);
        assert_eq!(found.layout, SourceLayout::Explicit(chosen));
        Ok(())
    }

    #[test]
    fn an_empty_environment_variable_falls_through() -> Result<(), Box<dyn std::error::Error>> {
        let candidate = temp_source("heric-env-empty")?;
        let found = discover_from(
            None,
            Some(OsString::new()),
            std::slice::from_ref(&candidate),
        )?;
        assert_eq!(found.path, candidate);
        assert_eq!(found.layout, SourceLayout::PlatformDefault);
        Ok(())
    }

    #[test]
    fn the_first_candidate_holding_a_database_wins() -> Result<(), Box<dyn std::error::Error>> {
        let empty = std::env::temp_dir().join("heric-candidate-empty");
        std::fs::create_dir_all(&empty)?;
        let populated = temp_source("heric-candidate-populated")?;
        let later = temp_source("heric-candidate-later")?;
        let found = discover_from(None, None, &[empty, populated.clone(), later])?;
        assert_eq!(found.path, populated);
        assert_eq!(found.layout, SourceLayout::PlatformDefault);
        Ok(())
    }

    #[test]
    fn no_candidate_names_every_path_tried() -> Result<(), Box<dyn std::error::Error>> {
        let first = std::env::temp_dir().join("heric-candidate-none-first");
        std::fs::create_dir_all(&first)?;
        let second = std::env::temp_dir().join("heric-candidate-none-second");
        std::fs::create_dir_all(&second)?;
        let Err(error) = discover_from(None, None, &[first.clone(), second.clone()]) else {
            return Err("two empty candidates must not be found".into());
        };
        let StorageError::SourceNotFound(listing) = error else {
            return Err("the error should be SourceNotFound".into());
        };
        assert!(listing.contains(&first.display().to_string()), "{listing}");
        assert!(listing.contains(&second.display().to_string()), "{listing}");
        Ok(())
    }

    #[test]
    fn a_directory_named_like_a_database_is_not_a_source() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = std::env::temp_dir().join("heric-insomnia-probe-directory-named-db");
        std::fs::create_dir_all(dir.join("insomnia.Workspace.db"))?;
        assert!(!holds_insomnia_database(&dir));
        Ok(())
    }

    /// Creates a directory `name` under the temp dir holding one database file.
    fn temp_source(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("insomnia.Workspace.db"), b"")?;
        Ok(dir)
    }
}
