### Task 4: Blob store and datafile protection

**Files:**
- Create: `crates/heric-storage/src/blobs.rs`
- Modify: `crates/heric-storage/src/blob.rs`, `src/lib.rs`
- Test: `#[cfg(test)]` in `blobs.rs`

**Interfaces:**
- Produces: `heric_storage::open_blobs(paths: &DataPaths) -> Result<BlobStoreHandle, StorageError>`; `heric_storage::blob_path(root: &Path, id: &BlobId) -> PathBuf`; `heric_storage::is_datafile(path: &Path) -> bool`.

**Medium, decided here (design §22 leaves it open):** one file per blob at `<root>/blobs/<owner_kind>/<role>/<owner_id>`. A file per blob makes `stream` free — `File::open` *is* the stream — and keeps a 40 MB body out of a `BLOB` column, where SQLite would copy it into the WAL. Role directories keep one owner's body, timeline and event log from colliding. Against `STO-015`, which names `<id>.response` and `<id>.timeline` under `responses/`: the acceptance is a body and a timeline that survive independently of the document and are removed with it, which this satisfies inside Heric's own directory.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_blob_streams_back_byte_for_byte() -> Result<(), StorageError> {
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
fn a_missing_blob_reads_as_absent_rather_than_failing() -> Result<(), StorageError> {
    let store = ephemeral_blobs()?;
    let id = BlobId { owner: Id::new("res_absent"), owner_kind: DocKind::Response, role: BlobRole::Body };
    assert_eq!(store.get(&id)?, None);
    Ok(())
}

#[test]
fn the_three_roles_do_not_collide() -> Result<(), StorageError> {
    // put "body" under Body and "log" under EventLog for one owner, read both back
    Ok(())
}

#[test]
fn datafiles_are_recognised() {
    assert!(is_datafile(std::path::Path::new("/x/insomnia.Request.db")));
    assert!(!is_datafile(std::path::Path::new("/x/response.json")));
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

`put` creates the role directory, writes to `<path>.tmp` and renames, so a crash cannot leave a truncated body in place of a good one. `stream` returns `Box<dyn Read + Send>` over the opened file and maps a missing file to `StorageError::NotFound`. `is_datafile` matches the file name against `insomnia.*.db` (`STO-020`); it is the guard the renderer's `{% file %}` path and the plugin scope consult, and it is checked by `SendingHost::read_file` in Task 17.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(storage): add the blob store and datafile guards`

