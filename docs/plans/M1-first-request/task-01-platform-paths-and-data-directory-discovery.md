### Task 1: Platform paths and data-directory discovery

**Files:**
- Modify: `crates/heric-storage/src/paths.rs`
- Create: `crates/heric-core/src/platform.rs`
- Modify: `crates/heric-core/src/lib.rs`
- Test: `#[cfg(test)]` in both files

**Interfaces:**
- Consumes: `heric_storage::{DataPaths, InsomniaDataDir, SourceLayout, StorageError}` (declared).
- Produces: `DataPaths::resolve() -> Result<Self, StorageError>`, `DataPaths::at(root: impl Into<PathBuf>) -> Self`, `DataPaths::{database, blobs, ui_state, logs, backups}()`; `discover_insomnia_data_dir(explicit: Option<PathBuf>) -> Result<InsomniaDataDir, StorageError>`; `heric_core::platform::{Platform, current(), Platform::as_str()}` returning `"macos" | "windows" | "linux"`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_explicit_root_places_every_path_under_it() -> Result<(), StorageError> {
    let root = std::env::temp_dir().join("heric-paths-root");
    let paths = DataPaths::at(&root);
    assert_eq!(paths.database(), root.join("heric.db"));
    assert_eq!(paths.blobs(), root.join("blobs"));
    Ok(())
}

#[test]
fn an_explicit_insomnia_directory_is_used_verbatim() -> Result<(), StorageError> {
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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p heric-storage paths`
Expected: FAIL — `todo!("M2: probe platform paths…")`.

- [ ] **Step 3: Implement**

`DataPaths` gains `blobs()` returning `self.root.join("blobs")`. `discover_insomnia_data_dir` tries, in order, the explicit argument, `INSOMNIA_DATA_PATH` (read with `std::env::var_os`, never unwrapped), then the three platform directories `%APPDATA%\Insomnia`, `~/.config/Insomnia`, `~/Library/Application Support/Insomnia`; it returns the first that exists and holds at least one `insomnia.*.db`, and otherwise `StorageError::SourceNotFound` listing every candidate tried. A directory found by the environment variable is reported with `SourceLayout::Explicit`.

`platform.rs` mirrors the original's `common-src/constants.ts` detection as `Platform::{Mac, Windows, Linux}` from `cfg!(target_os = …)`, defaulting to Linux, matching `PLT-007`.

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p heric-storage -p heric-core paths platform`
Expected: PASS.

- [ ] **Step 5: Commit** — `feat(storage): resolve Heric and Insomnia data directories`

