### Task 2: SQLite document store

**Files:**
- Create: `crates/heric-storage/src/sqlite.rs`
- Modify: `crates/heric-storage/src/store.rs`, `src/lib.rs`, `crates/heric-storage/Cargo.toml`, `Cargo.toml`
- Test: `#[cfg(test)]` in `sqlite.rs`

**Interfaces:**
- Consumes: `heric_model::{Doc, DocMeta, DocKind, Id, Patch, PatchOp, Query, Order, Change}`.
- Produces: `heric_storage::open(paths: &DataPaths) -> Result<StoreHandle, StorageError>` returning `Arc<SqliteStore>`; `pub(crate) fn ephemeral() -> Result<StoreHandle, StorageError>` for tests; a new defaulted trait method `Store::upsert(&self, doc: &Doc) -> Result<Doc, StorageError>`.

**Schema, decided here (design §22 leaves the shape to the builder):** one table with a `kind` discriminator, not 44 tables. The original scans and filters by `parentId`, so a discriminator index costs nothing, and 44 near-identical tables would mean 44 migrations to keep in step.

```sql
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS documents (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    parent     TEXT,
    created    INTEGER NOT NULL DEFAULT 0,
    modified   INTEGER NOT NULL DEFAULT 0,
    is_private INTEGER,
    name       TEXT,
    body       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS documents_parent ON documents(parent, kind);
CREATE INDEX IF NOT EXISTS documents_kind ON documents(kind);
```

`body` is the document serialised with the seven `META_KEYS` removed, so an unknown key is stored as it arrived. `is_private` is `NULL` when the field is absent, which stays distinct from `0`.

**STO-001 deviation, recorded deliberately:** the row describes 44 NeDB files per data directory. ADR 0006 replaces the medium with SQLite; what M1 must preserve is that every one of the 44 kinds round-trips with its own prefix, which the `every_kind_round_trips` test below pins.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_inserted_document_reads_back_identically() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let doc = request_doc();
    assert_eq!(store.insert(&doc)?, doc);
    assert_eq!(store.get(&doc.meta.id)?, Some(doc));
    Ok(())
}

#[test]
fn unknown_body_keys_survive_the_round_trip() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let mut doc = request_doc();
    doc.body.insert("futureField".into(), serde_json::json!({"nested": [1, 2, 3]}));
    store.insert(&doc)?;
    let back = store.get(&doc.meta.id)?.ok_or("missing")?;
    assert_eq!(back.body.get("futureField"), Some(&serde_json::json!({"nested": [1, 2, 3]})));
    Ok(())
}

#[test]
fn every_kind_round_trips_with_its_prefix() -> Result<(), StorageError> {
    let store = ephemeral()?;
    for kind in DocKind::ALL {
        let id = Id::generate(kind);
        assert!(id.as_str().starts_with(kind.prefix()), "{kind:?}");
        let doc = Doc {
            meta: DocMeta { id: id.clone(), kind, parent: None, modified: 0, created: 0, private: None, name: None },
            body: serde_json::Map::new(),
        };
        store.insert(&doc)?;
        assert_eq!(store.get(&id)?, Some(doc), "{kind:?}");
    }
    Ok(())
}

#[test]
fn a_duplicate_insert_is_refused_and_an_upsert_replaces() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let mut doc = request_doc();
    store.insert(&doc)?;
    assert!(matches!(store.insert(&doc), Err(StorageError::DuplicateId(_))));
    doc.body.insert("method".into(), serde_json::json!("POST"));
    assert_eq!(store.upsert(&doc)?.body.get("method"), Some(&serde_json::json!("POST")));
    Ok(())
}

#[test]
fn an_update_applies_the_patch_and_restamps_modified() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let mut doc = request_doc();
    doc.meta.modified = 1;
    store.insert(&doc)?;
    let patch = Patch::new().set("url", serde_json::json!("https://changed.test"));
    let updated = store.update(&doc.meta.id, &patch)?;
    assert!(updated.meta.modified > 1);
    assert_eq!(updated.body.get("url"), Some(&serde_json::json!("https://changed.test")));
    Ok(())
}

#[test]
fn a_query_filters_by_parent_and_kind_in_both_orders() -> Result<(), StorageError> { /* five docs, two queries, assert ids */ }

#[test]
fn a_newer_schema_version_is_refused() -> Result<(), StorageError> { /* write meta version 2, expect SchemaTooNew */ }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p heric-storage sqlite` panics on `todo!()`.

- [ ] **Step 3: Implement**

Add `rusqlite = { version = "0.41.1", features = ["bundled"] }` to `[workspace.dependencies]` and `rusqlite = { workspace = true }` to the crate. `bundled` compiles the SQLite amalgamation, so there is no system `libsqlite3` dependency — the same posture ADR 0010 takes for libcurl.

`open` runs `PRAGMA journal_mode=WAL` and the DDL above, then reads `schema_version` from `meta`: absent writes `"1"`, greater than `1` returns `StorageError::SchemaTooNew { found, supported: 1 }`. The connection lives behind a `std::sync::Mutex`, because every call arrives from `spawn_blocking` and contention is not worth a pool. `update` reads the row, applies each `PatchOp` to the body or the matching meta column, sets `modified` to `now_millis()`, writes in one transaction and returns the result. `find` builds one `SELECT` with the optional parent and kind filters and the `ORDER BY` the `Order` names. `now_millis()` reads `SystemTime::now` against `UNIX_EPOCH` and yields `0` rather than panicking when the clock is before the epoch.

- [ ] **Step 4: Run to verify passing** — `cargo test -p heric-storage` and `cargo clippy -p heric-storage --all-targets -- -D warnings`.

- [ ] **Step 5: Commit** — `feat(storage): add the SQLite document store`

