# M1 — First Request Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user launches `heric`, creates a workspace with one request, presses Send, and sees the real response and libcurl timeline in the window, with every document persisted in Heric's own SQLite store.

**Architecture:** Phase 1 landed the contracts: twelve library crates expose their public types and every unimplemented body is a `todo!()` naming its milestone. M1 fills the M1 bodies only — the SQLite `Store`, the `BlobStore`, the Liquid renderer, the `curl`-backed `HttpEngine`, three auth providers, the send pipeline's non-script stages, and a GUI that edits one request and displays one response. `heric-core` keeps its Phase 1 shape; this milestone supplies the adapters it already expects.

**Tech Stack:** Rust 2024, `rusqlite` 0.41.1 (`bundled` SQLite), `curl` 0.4.51 (`curl-sys` 0.4.91, libcurl 8.22.0), `flate2` 1.1.11, `tempfile` 3.27.0 (tests), `serde_json`, `thiserror`, `tokio` 1.54, `eframe`/`egui` 0.37 with `egui_extras::TableBuilder`, `wiremock` 0.6.5.

**Spec:** `docs/superpowers/specs/2026-11-10-heric-architecture-design.md` (approved), read together with ADRs `0003-template-engine`, `0004-http-client`, `0006-storage-format`, `0009-gui-concurrency`, `0010-libcurl-build-and-tls`, `0011-secrets`, `0013-gui-layout`. The plan argues from those; executors read both.

## Global Constraints

- MSRV **1.95**, edition **2024** from `[workspace.package]`. An ADR records any new floor and the `msrv` CI job enforces it.
- `unsafe_code = "forbid"`. `clippy::unwrap_used` and `clippy::expect_used` are **denied workspace-wide, including `#[cfg(test)]`**. Test functions return `Result<(), Box<dyn std::error::Error>>` and use `?`, or `.ok_or("…")?` for an `Option`. Never add `#[allow(...)]`.
- Libraries use `thiserror` and return their crate's error type. `anyhow` only in `apps/gui` and `apps/cli`.
- No test touches the network. Every HTTP test is against `wiremock` or a loopback listener.
- `D:\Projects\Heric\Insomnia` is **read-only**. Fixtures are transcribed from it, never written into it, and Heric never writes into an Insomnia data directory.
- Licences: everything on `deny.toml`'s allow-list. New crates in this plan: `rusqlite` 0.41.1 (MIT), `flate2` 1.1.11 (MIT OR Apache-2.0), `tempfile` 3.27.0 (MIT OR Apache-2.0). `wiremock` 0.6.5 (MIT) is already in `[workspace.dependencies]` but unused, so it enters `Cargo.lock` for the first time in Task 15.
- A dependency reaches `[workspace.dependencies]` only when a task here names it, its version, and the ADR covering it.
- Conventional Commits in English with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Nothing is committed to `main` or `develop`; every task lands on `feature/<crate>-<task>` and merges by PR into `develop`.
- Code, comments, ADRs and plan documents in English. User docs ship `en`, `pt-BR` and `es` with identical heading shape, or `scripts/check-docs-structure.sh` fails.
- Gates before every task's final commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

## Review Focus

The spec says what Heric must do; it does not enumerate every input that will reach it, and its silence is not permission to break. These five are the inputs most likely to hurt a user, most likely first. Each gets a test in the task named beside it.

1. **A document carrying a field Heric has never heard of.** The original's 44 models drift between releases, and a user's workspace holds keys no table models. Expected: read, store and write back unchanged, and never fail a load over an unknown key. → Task 2.
2. **A request whose header references an undefined variable.** Expected: the send refuses and names the variable and its field path; it never sends a header with an empty value in its place. → Task 13 renders it and leaves the document untouched, and Task 17 proves the engine was never called.
3. **A response body containing `{{` or `{%`.** The second render pass is gated on the *original input* having no tags, so an echoed body must never be re-interpreted. Expected: a literal round trip. → Task 12.
4. **A secret awaiting re-entry after migration.** Expected: the send stops with `StopReason::PendingCredential` and the UI offers the environment editor; it never emits a blank credential. → Task 16 returns the error instead of an empty value, and Task 17 turns it into the stop reason.
5. **A body-less response (204, 304, HEAD) and a body over 1 MB.** Expected: status and headers render with an empty body and no panic, and the large body streams from the blob store instead of being held twice. → Task 4 streams, Task 15 pins the empty 204 at the engine, and Task 19 renders both cases.

---

## Test support the tasks add

These helpers do not exist yet and each task that uses one creates it inside its own `#[cfg(test)]` module. They are listed once here so two tasks do not invent two spellings of the same thing.

- `ephemeral() -> Result<StoreHandle, StorageError>` — Task 2, a store over a `tempfile::TempDir` database. `tempfile = "3.27.0"` (MIT OR Apache-2.0) joins `[workspace.dependencies]` in Task 2, and each crate that needs it takes it under `[dev-dependencies]`.
- `request_doc() -> Doc` — Task 2, one `Request` with a parent, a `url`, a `method` and one `headers` row. Task 3 adds `workspace_doc(id)`, `folder_doc(id, parent)`, `request_doc_at(id, parent)`, `response_doc(id, parent)`. Task 7 reuses `request_doc()?` from its own module, because each crate's tests are separate.
- `ephemeral_blobs() -> Result<BlobStoreHandle, StorageError>` — Task 4, blobs over a `TempDir`.
- `NoHost` — Task 10, a `TagHost` whose `read_file`, `prompt` and `send` all return `TagError::PathNotAllowed` or `TagError::NotAvailableInThisPurpose`. It lives in a `pub(crate) mod testing` inside `heric-template` so every module's tests share it rather than five modules each define one.
- `context(&Scope) -> RenderContext<'_>` and `context_with(&[(&str, Value)]) -> RenderContext<'_>` — Task 10, built from the `Scope`/`Scope` shape the Phase 1 `RenderContext` already declares.

---

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

---

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

---

### Task 3: Descendant traversal and the change broadcast

**Files:**
- Modify: `crates/heric-storage/src/sqlite.rs`
- Test: `#[cfg(test)]` in `sqlite.rs`

**Interfaces:**
- Produces: `Store::{insert, upsert, update, remove}` each emit one `Change` on a `broadcast::Sender<Change>`; `Store::changes()` returns a fresh receiver (already declared).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn removing_a_workspace_returns_every_descendant_id() -> Result<(), StorageError> {
    let store = ephemeral()?;
    store.insert(&workspace_doc("wrk_1"))?;
    store.insert(&folder_doc("fld_1", "wrk_1"))?;
    store.insert(&request_doc_at("req_1", "fld_1"))?;
    store.insert(&response_doc("res_1", "req_1"))?;
    let mut removed = store.remove(&Id::new("wrk_1"))?;
    removed.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        removed.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["fld_1", "req_1", "res_1", "wrk_1"]
    );
    assert!(store.get(&Id::new("req_1"))?.is_none());
    Ok(())
}

#[test]
fn a_removal_emits_one_change_listing_every_id() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let mut rx = store.changes();
    store.insert(&workspace_doc("wrk_1"))?;
    let _ = rx.try_recv()?;
    let removed = store.remove(&Id::new("wrk_1"))?;
    let Change::Removed(ids) = rx.try_recv()? else { return Err("expected Removed".into()) };
    assert_eq!(ids.len(), removed.len());
    Ok(())
}

#[test]
fn a_lagging_subscriber_never_blocks_a_writer() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let _rx = store.changes();          // never drained
    for index in 0..2000 {
        store.insert(&workspace_doc(&format!("wrk_{index}")))?;
    }
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

Descendant collection walks `SELECT id FROM documents WHERE parent = ?` breadth-first inside the delete transaction, then deletes by id, so `remove` returns the full set in one pass. The sender has capacity `1024` and `send` discards its result (`let _ = self.sender.send(change);`), because a lagging GUI must drop events rather than stall a writer.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(storage): cascade deletes and broadcast document changes`

---

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

---

### Task 5: Model defaults, `initModel` and the kind registry

**Files:**
- Create: `crates/heric-model/src/init.rs`, `crates/heric-model/src/registry.rs`
- Modify: `crates/heric-model/src/lib.rs`, `src/doc.rs`
- Test: `#[cfg(test)]` in both, plus `crates/heric-model/tests/v5_parser.rs`

**Interfaces:**
- Produces: `init::document(kind: DocKind, now: i64) -> Doc` (a fresh document with a generated id); `init::scratchpad() -> Vec<Doc>` (the three sentinel documents, fixed ids and names); `init::apply(doc: &mut Doc) -> Result<(), ModelError>`; `registry::{can_duplicate(DocKind) -> bool, rewrite_references(&mut Doc, &BTreeMap<Id, Id>), kinds_that_sync() -> &'static [DocKind]}`; constants `SCRATCHPAD_WORKSPACE`, `SCRATCHPAD_PROJECT`, `SCRATCHPAD_ORGANIZATION`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_new_request_gets_get_and_an_empty_body() -> Result<(), ModelError> {
    let doc = init::document(DocKind::Request, 1_000);
    assert_eq!(doc.body.get("method"), Some(&serde_json::json!("GET")));
    assert_eq!(doc.body.get("headers"), Some(&serde_json::json!([])));
    assert_eq!(doc.body.get("authentication"), Some(&serde_json::json!({})));
    assert_eq!(doc.body.get("isPrivate"), Some(&serde_json::json!(false)));
    assert_eq!(doc.body.get("metaSortKey"), Some(&serde_json::json!(-1_000.0)));
    assert!(doc.meta.id.as_str().starts_with("req_"));
    Ok(())
}

#[test]
fn pruning_drops_an_undeclared_key_and_keeps_a_declared_optional_one() -> Result<(), ModelError> {
    let mut doc = init::document(DocKind::Request, 0);
    doc.body.insert("notAModelField".into(), serde_json::json!(1));
    init::apply(&mut doc)?;
    assert!(!doc.body.contains_key("notAModelField"));
    Ok(())
}

#[test]
fn an_unmodelled_kind_keeps_every_key_it_was_given() -> Result<(), ModelError> {
    let mut doc = init::document(DocKind::MockRoute, 0);
    doc.body.insert("whatever".into(), serde_json::json!(1));
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("whatever"), Some(&serde_json::json!(1)));
    Ok(())
}

#[test]
fn the_scratchpad_uses_the_original_sentinel_ids() -> Result<(), ModelError> {
    let docs = init::scratchpad();
    let ids: Vec<&str> = docs.iter().map(|doc| doc.meta.id.as_str()).collect();
    assert_eq!(ids, [SCRATCHPAD_PROJECT, SCRATCHPAD_WORKSPACE, SCRATCHPAD_ORGANIZATION]);
    let workspace = docs.iter().find(|doc| doc.meta.kind == DocKind::Workspace).ok_or("no workspace")?;
    assert_eq!(workspace.meta.parent.as_ref().map(Id::as_str), Some(SCRATCHPAD_PROJECT));
    Ok(())
}

#[test]
fn a_fresh_workspace_gets_a_generated_id_not_the_scratchpad_one() {
    assert_ne!(init::document(DocKind::Workspace, 0).meta.id.as_str(), SCRATCHPAD_WORKSPACE);
}

#[test]
fn duplicating_a_request_rewrites_the_parent_reference() -> Result<(), ModelError> {
    // rewrite_references maps fld_old -> fld_new and the child's parent follows
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

A `ModelSpec { defaults: &'static [(&'static str, Value)], optional: &'static [&'static str] }` table keyed by `DocKind`, transcribed from `docs/inventory/models.md` in the original's `init()` order (so key order matches). M1 fills the kinds it writes: Workspace, WorkspaceMeta, Project, Request, RequestMeta, RequestGroup, RequestGroupMeta, Environment, Response, CookieJar, Settings. Every other kind gets the empty spec, which prunes nothing — an unmodelled kind keeps whatever it was handed, which is the same property the `Doc` round trip already guarantees. `init::apply` merges defaults for absent keys and removes body keys outside `defaults ∪ optional`.

`FIX-006`: transcribe the 62 expectations from `packages/insomnia/src/common/__tests__/import-v5-parser.test.ts` into `tests/v5_parser.rs` as model-validation tests — one per expectation, asserting a default, an accepted and a rejected shape of a discriminated union, or a mock/MCP variant, through `Doc` round trips rather than a schema library.

- [ ] **Step 4: Run to verify passing** — `cargo test -p heric-model`.

- [ ] **Step 5: Commit** — `feat(model): add initModel defaults, pruning and the kind registry`

---

### Task 6: Workspace, project and environment views

**Files:**
- Create: `crates/heric-model/src/views.rs`, `src/workspace.rs`, `src/environment.rs`
- Modify: `crates/heric-model/src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `Doc::as_group()`, `as_environment()`, `as_project()`, `as_response() -> Result<…, ModelError>`; `workspace::{Scope, CollectionScope, scope_of(&Doc) -> Result<Scope, ModelError>, Projects::sorted(&[Doc]) -> Vec<Doc>, storage_label(&Doc) -> &'static str, assert_parent_is_project(&Doc, &dyn Fn(&Id) -> Option<Doc>) -> Result<(), ModelError>}`; `environment::{kv_from_json, json_from_kv, base_for(parent: Id) -> Doc, BASE_NAME}`; `views::{midpoint(a: f64, b: f64) -> f64, copy_name(&str) -> String, inherited_headers(request: &Doc, ancestors: &[Doc]) -> Vec<Value>, inherited_authentication(request: &Doc, ancestors: &[Doc]) -> Option<Value>}`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_disabled_header_row_is_skipped_by_inheritance() -> Result<(), ModelError> {
    // folder headers: [{"name":"A","value":"1"},{"name":"B","value":"2","disabled":true}]
    // inherited_headers(request, &[folder]) keeps A and drops B
    Ok(())
}

#[test]
fn a_folder_auth_block_is_inherited_only_when_the_request_has_none() -> Result<(), ModelError> {
    // request authentication {} + folder {"type":"bearer"} -> inherited; request's own wins otherwise
    Ok(())
}

#[test]
fn a_workspace_parent_must_be_a_project() { /* a Request parent, a Project parent, an absent parent */ }

#[test]
fn projects_sort_by_organization_then_name() -> Result<(), ModelError> {
    // three projects: default org "zzz", another org "aaa", default org "bbb"
    Ok(())
}

#[test]
fn an_empty_url_produces_no_request() {
    // RequestEditor { url: String::new(), .. } -> to_patch() leaves `url` empty and
    // Send stays disabled; no PreparedRequest is built.
}

#[test]
fn the_scope_discriminator_reads_collection_design_and_mock_server() { /* MOD-005 */ }

#[test]
fn the_storage_label_is_human_readable() { /* Local, Remote, Git */ }

#[test]
fn the_middle_sort_key_lands_between_its_neighbours() {
    assert_eq!(views::midpoint(-10.0, 0.0), -5.0);
    assert_eq!(views::midpoint(0.0, 10.0), 5.0);
}

#[test]
fn a_copy_is_named_with_the_suffix_once() {
    assert_eq!(views::copy_name("Foo"), "Foo (Copy)");
    assert_eq!(views::copy_name("Foo (Copy)"), "Foo (Copy) (Copy)");
}

#[test]
fn the_base_environment_id_is_deterministic_for_a_parent() {
    assert_eq!(environment::base_for(Id::new("wrk_1")), environment::base_for(Id::new("wrk_1")));
    assert_ne!(environment::base_for(Id::new("wrk_1")), environment::base_for(Id::new("wrk_2")));
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the views as reads over `Doc::body` that never mutate. `scope_of` distinguishes `collection`, `design` and `mock-server` by the workspace's `scope` field, defaulting to `collection` when absent. `assert_parent_is_project` returns `ModelError::WrongKind { expected: DocKind::Project, found }`, the variant the crate already has, and is what the document service calls on create, update and upsert (`MOD-007`). `base_for` derives the id from a stable hash of `<parent>` prefixed `env_`, so a base environment is created lazily once per parent and never duplicated (`MOD-037`); `BASE_NAME` is `"Base Environment"`.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(model): add workspace, project and environment views`

---

### Task 7: Responses, snapshots and delete history

**Files:**
- Create: `crates/heric-model/src/response.rs`, `src/request.rs`
- Modify: `crates/heric-model/src/lib.rs`, `crates/heric-model/Cargo.toml`, `Cargo.toml`
- Test: `#[cfg(test)]` in both

**Interfaces:**
- Produces: `response::{create(request: &Doc, execution: &Execution, body_path: &str, timeline_path: &str, now: i64) -> Doc, read_body(bytes: &[u8], compressed: bool) -> Result<Vec<u8>, ModelError>, prune(docs: Vec<Doc>, max_responses: usize) -> Vec<Doc>, latest(docs: &[Doc], environment: Option<&Id>) -> Option<&Doc>}`; `request::{path_parameters(url: &str, parameters: &[Value]) -> Result<(String, Vec<(String, String)>), ModelError>, merge_parameters(saved: &[Value], unsaved: &[Value]) -> Vec<Value>, snapshot(&Doc, kind: DocKind, now: i64) -> Doc}`; one new `ModelError` variant, `CorruptBody { detail: String }`, with its `Display` text.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_response_snapshots_the_request_version_and_paths() -> Result<(), ModelError> {
    let response = response::create(&request_doc()?, &Execution::ok(204), "b/res_1", "t/res_1", 10);
    assert_eq!(response.body.get("statusCode"), Some(&serde_json::json!(204)));
    assert_eq!(response.body.get("bodyPath"), Some(&serde_json::json!("b/res_1")));
    assert_eq!(response.body.get("requestVersionId"), Some(&serde_json::json!("rvr_1")));
    Ok(())
}

#[test]
fn an_empty_body_is_read_as_empty_not_as_corrupt() {
    assert_eq!(response::read_body(&[], false), Ok(Vec::new()));
}

#[test]
fn a_gzip_body_is_decompressed_and_a_plain_one_passes_through() -> Result<(), ModelError> {
    // flate2 GzEncoder over b"hello" -> read_body(.., true) == b"hello"
    assert_eq!(response::read_body(b"plain", false)?, b"plain");
    Ok(())
}

#[test]
fn pruning_keeps_the_newest_twenty() { /* 25 docs, assert 20 kept and the oldest five dropped */ }

#[test]
fn the_latest_response_is_filtered_by_environment_and_sorted_desc() { /* MOD-023 */ }

#[test]
fn a_saved_path_parameter_is_merged_over_the_url_template() -> Result<(), ModelError> {
    let (url, _) = request::path_parameters("https://x.test/users/:id", &[serde_json::json!({"name": "id", "value": "7"})])?;
    assert_eq!(url, "https://x.test/users/7");
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** with `flate2 = "1.1.11"` in `[workspace.dependencies]` and the crate. `read_body` returns `Ok(Vec::new())` for empty input and gunzips when `compressed`. A decode failure returns a new variant added to `ModelError` in this task, `ModelError::CorruptBody { detail: String }`, because the existing three variants (`UnknownKind`, `WrongKind`, `MissingField`) all describe a document's shape and none describes bytes that would not decompress. `prune` sorts by `modified` descending and truncates at `max_responses`, which the caller supplies as 20 by default (`MOD-022`). `create` writes `statusCode`, `statusMessage`, `httpVersion`, `headers`, `bodyPath`, `timelinePath`, `requestVersionId`, `environmentId`, `bytesContent` and `elapsed`; the snapshot id `rvr_1` comes from `request::snapshot`, which copies the request and preserves `_id`, `created` and `parentId` (`MOD-026`). Deleting response history (`MOD-025`) is the store's cascade in Task 3 plus `BlobStore::remove` in Task 4; Task 17 carries the test that both the document and both blobs disappear.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(model): add responses, snapshots and delete history helpers`

---

### Task 8: The settings singleton

**Files:**
- Create: `crates/heric-model/src/settings.rs`
- Test: `#[cfg(test)]` in `settings.rs`

**Interfaces:**
- Produces: `settings::defaults() -> serde_json::Map<String, Value>`, `settings::singleton(now: i64) -> Doc`, `settings::repair(doc: &mut Doc) -> Vec<&'static str>` (names of the keys it had to fix), `settings::migrate_hotkeys(map: serde_json::Map<String, Value>) -> serde_json::Map<String, Value>`.

- [ ] **Step 1: Write the failing tests** — `every_default_key_is_present`, `a_missing_key_is_restored_and_reported`, `a_hotkey_bound_twice_is_reported_and_left_to_the_user` (repair reports, never silently rebinds — the original lets the user choose), `the_settings_id_is_the_singletons`. Transcribe the 60+ defaults from `docs/inventory/models.md` § Settings; do not invent a key. `MOD-054` is P2, so the accepted gap is: defaults and repair only, with hotkey *conflict* resolution deferred and named in the PR description.
- [ ] **Step 2–5:** verify failure, implement, verify `cargo test -p heric-model settings`, commit `feat(model): add the settings singleton and its defaults`.

---

### Task 9: Liquid lexer, parser and comment stripping

**Files:**
- Create: `crates/heric-template/src/lexer.rs`, `src/parser.rs`, `src/ast.rs`
- Modify: `crates/heric-template/src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `pub(crate) enum Node { Text(String), Output(Expr), Tag { name: String, args: Vec<Expr>, line: u32 }, If { … }, For { … }, Unless { … }, Case { … }, Assign { … }, Capture { … }, Raw(String) }`; `pub(crate) fn parse(input: &str) -> Result<Vec<Node>, RenderError>`; `pub(crate) fn strip_comments(input: &str) -> Cow<'_, str>`; `pub(crate) fn has_tags(input: &str) -> bool`.

**Parser choice, decided here (design §22 leaves it to the builder):** hand-written recursive descent, incrementally, driven by the conformance suite. No parser-combinator crate enters the tree until a case proves this approach cannot express a construct.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_multiline_comment_is_removed_before_parsing() {
    assert_eq!(strip_comments("a{# one\ntwo #}b"), "ab");
}

#[test]
fn a_hash_brace_inside_a_comment_ends_it() {
    // non-greedy, quote-blind, exactly as the original's regex behaves
    assert_eq!(strip_comments("{# 'a #}' tail"), " tail");
}

#[test]
fn input_without_delimiters_is_returned_untouched() {
    assert!(!has_tags("plain {%- not really -%}"));
    assert!(!has_tags("plain text"));
    assert!(has_tags("{{ name }}"));
    assert!(has_tags("{# #}"));
}

#[test]
fn text_without_delimiters_is_one_text_node() -> Result<(), RenderError> {
    assert!(matches!(parse("plain text")?.as_slice(), [Node::Text(t)] if t == "plain text"));
    Ok(())
}

#[test]
fn an_unclosed_output_tag_is_a_syntax_error() -> Result<(), RenderError> {
    match parse("{{ name") {
        Err(RenderError::Syntax { line, column, .. }) => {
            assert_eq!((line, column), (1, 1));
            Ok(())
        }
        other => Err(format!("expected a syntax error, got {other:?}").into()),
    }
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the lexer (`{{ }}`, `{% %}`, `{# #}`, `{% raw %}`), the recursive-descent parser with 1-based line and column threaded into every node and every error, and `strip_comments`, which applies the original's non-greedy, quote-blind `{#…#}` removal (`TPL-002`) before tokenising. `has_tags` implements `TPL-003`'s short-circuit test and is what `render` consults first.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): add the Liquid lexer, parser and comment stripping`

---

### Task 10: Evaluation, truthiness, limits and structured errors

**Files:**
- Create: `crates/heric-template/src/eval.rs`, `src/truthy.rs`
- Modify: `crates/heric-template/src/lib.rs`, `src/context.rs`
- Test: `#[cfg(test)]` in both

**Interfaces:**
- Produces: `heric_template::render(input, context, host) -> Result<String, RenderError>` (signature declared in Phase 1); `RenderContext::merged(&self) -> serde_json::Map<String, Value>`; `truthy::is_truthy(&Value) -> bool`; `pub(crate) mod testing { pub struct NoHost; pub fn context(&Scope) -> RenderContext<'_>; pub fn context_with(&[(&str, Value)]) -> RenderContext<'_>; }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_root_variable_and_its_underscore_form_resolve_alike() -> Result<(), RenderError> {
    let scope = Scope::from_map(serde_json::Map::from_iter([("name".into(), serde_json::json!("kyle"))]));
    assert_eq!(render("{{ name }}", &context(&scope), &NoHost)?, "kyle");
    assert_eq!(render("{{ _.name }}", &context(&scope), &NoHost)?, "kyle");
    Ok(())
}

#[test]
fn a_bracket_key_reaches_a_hyphenated_name() -> Result<(), RenderError> { /* {{ _['water-type'] }} */ }

#[test]
fn an_undefined_variable_names_itself_and_its_line() -> Result<(), RenderError> {
    match render("line one\n{{ nope }}", &context(&Scope::new()), &NoHost) {
        Err(RenderError::UndefinedVariable { name, line, .. }) => {
            assert_eq!((name.as_str(), line), ("nope", 2));
            Ok(())
        }
        other => Err(format!("expected UndefinedVariable, got {other:?}").into()),
    }
}

#[test]
fn an_empty_array_and_an_empty_object_are_truthy() {
    assert!(truthy::is_truthy(&serde_json::json!([])));
    assert!(truthy::is_truthy(&serde_json::json!({})));
    for falsy in [serde_json::json!(""), serde_json::json!(0), serde_json::Value::Null, serde_json::json!(false)] {
        assert!(!truthy::is_truthy(&falsy), "{falsy}");
    }
}

#[test]
fn a_render_over_ten_thousand_steps_is_a_limit_error() { /* RenderError::Limit { what: "render" } */ }
#[test]
fn a_value_outside_the_context_fails_closed() { /* own property only, never a prototype chain */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** evaluation over `Node`, with JavaScript truthiness (`""`, `0`, `null`, `false` falsy; `[]` and `{}` **truthy** — the original follows JS, not Liquid's Ruby rules, and the test above pins it), own-property-only lookups with no prototype traversal, `strictVariables` on by default and `strictFilters` likewise (`TPL-001`), a render limit of 10 000 steps and a memory limit of 10 000 000 bytes reported as `RenderError::Limit { what: "render" | "memory" }`. `TPL-005` is satisfied by `RenderError`'s existing shape: every variant carries `path`, `line` and `column`, and `UndefinedVariable` carries `name`.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): evaluate Liquid with strict variables and structured errors`

---

### Task 11: Filters, the tag argument tokenizer and the built-in tags

**Files:**
- Create: `crates/heric-template/src/filters.rs`, `src/tags/mod.rs`
- Modify: `crates/heric-template/src/tag.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `filters::apply(name: &str, value: &Value, args: &[Value]) -> Result<Value, RenderError>`; `tags::run(name: &str, args: &[Value], context: &mut RenderContext<'_>, host: &dyn TagHost) -> Result<String, TagError>`; `pub(crate) fn tokenize_args(input: &str) -> Result<Vec<Expr>, RenderError>`.

- [ ] **Step 1: Write the failing tests** — one per filter the original registers, transcribed from `docs/inventory/templating.md` § Custom filters (do not invent a name or a behaviour); `a_missing_comma_merges_the_remaining_arguments`, which pins the original's `tokenize-args` behaviour and is what `TPL-006` means by "quoted strings with escapes, booleans, numbers, variables and expressions"; one test per entry in `tag::tags()`: `base64`, `cookie`, `faker`, `file`, `hash`, `jsonpath`, `now`, `os`, `prompt`, `request`, `response`, `timestamp`, `uuid`. `{% prompt %}` returns `TagError::NotAvailableInThisPurpose` when `!context.purpose.allows_blocking_tags()`, and every tag stringifies with `null` becoming `""` (`TPL-014`).
- [ ] **Step 2–5:** verify failure, implement, verify `cargo test -p heric-template`, commit `feat(template): add filters, tag arguments and the built-in tags`.

---

### Task 12: Layered context, three-pass render and confidentiality

**Files:**
- Create: `crates/heric-template/src/render_context.rs`, `src/render_tree.rs`
- Modify: `crates/heric-template/src/context.rs`, `src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `Layer { scope: Scope, source: LayerSource }` with `LayerSource::{Global, GlobalSub, Collection, CollectionSub, Folder, Iteration, Transient}`; `RenderContext::build(layers: &[Layer]) -> OwnedContext`; `render_tree::render_value(value: &Value, context: &OwnedContext, host: &dyn TagHost, path: &str) -> Result<Value, RenderError>`; `render_tree::second_pass(original: &str, rendered: &str, …) -> Result<String, RenderError>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_later_layer_wins_over_an_earlier_one() { /* Global then Collection, same key */ }

#[test]
fn a_dangerous_key_is_skipped() {
    for key in ["__proto__", "constructor", "prototype"] {
        // merged map must not contain key even when a layer supplies it
    }
}

#[test]
fn a_self_recursive_variable_resolves_once() -> Result<(), RenderError> {
    // base_url = "{{ base_url }}/foo" over base_url = "google.com"
    // expected rendered base_url == "google.com/foo", with no further expansion
    Ok(())
}

#[test]
fn a_template_valued_key_sorts_after_plain_keys() { /* TPL-009 ordering */ }

#[test]
fn an_unchanged_value_is_not_re_rendered() { /* skipNextTime */ }

#[test]
fn a_second_pass_runs_only_when_the_input_had_no_tags() -> Result<(), RenderError> {
    // original "{{ body }}" with a body containing "{{ x }}" must NOT expand x
    let out = render("{{ body }}", &context_with(&[("body", serde_json::json!("{{ x }}")), ("x", serde_json::json!("LEAK"))]), &NoHost)?;
    assert_eq!(out, "{{ x }}");
    Ok(())
}

#[test]
fn a_disabled_row_is_skipped_entirely() { /* {"name":"A","value":"1","disabled":true} */ }
#[test]
fn a_blacklisted_path_is_left_verbatim() { /* TPL-010's blacklist regex */ }
#[test]
fn a_masked_value_is_never_re_materialised() { /* ConfidentialPolicy::Mask before substitution */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the seven-layer merge in increasing precedence with JSON property order preserved, the `__proto__`/`constructor`/`prototype` skip, template-last key ordering, and the three-pass loop with `skipNextTime` (`TPL-009`). `second_pass` runs only when the first output gained `{%` **and** the original input carried no tags (`TPL-011`) — this is Review Focus 3's guard, and the test above is its pin. Masking happens at merge time, before any substitution, using `ConfidentialPolicy::for_purpose` (already implemented in Phase 1 — use it, do not re-derive it), with the mask `••••••` (`TPL-015`).

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): layer environments and render in three passes`

---

### Task 13: Request-level render and error paths

**Files:**
- Create: `crates/heric-template/src/render_request.rs`
- Modify: `crates/heric-template/src/lib.rs`
- Test: `#[cfg(test)]` in `render_request.rs`, plus `crates/heric-template/tests/render_golden.rs`

**Interfaces:**
- Produces: `heric_template::render_request(request: &mut Doc, context: &RenderContext<'_>, host: &dyn TagHost) -> Result<(), RenderError>`; `render_request::FIELDS: &[&str]`, the rendered fields in order.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_url_headers_and_body_are_rendered() -> Result<(), RenderError> { /* … */ }

#[test]
fn a_header_with_an_undefined_variable_fails_the_whole_request() -> Result<(), RenderError> {
    let mut doc = request_doc()?;
    let before = doc.clone();
    let error = render_request(&mut doc, &context(&Scope::new()), &NoHost);
    assert!(matches!(error, Err(RenderError::UndefinedVariable { .. })));
    assert_eq!(doc, before, "a failed render must leave the document unmodified");
    Ok(())
}

#[test]
fn an_error_path_names_the_field() -> Result<(), RenderError> {
    // TPL-030: the path reads `_request.headers[0].value`
    Ok(())
}

#[test]
fn only_a_manual_cookie_is_rendered() -> Result<(), RenderError> {
    // a {"source":"server"} cookie keeps its literal "{{"
    Ok(())
}

#[test]
fn a_description_is_rendered_at_lower_priority() -> Result<(), RenderError> { /* TPL-012 */ }
```

- [ ] **Step 2: Golden tests.** `tests/render_golden.rs` transcribes `FIX-011`'s 61 cases from `packages/insomnia/src/common/__tests__/render.test.ts`, including the gRPC variants, as a table of `(template, context, expected)`, keeping the original's `describe` names as module names. Transcribe the cases; never copy files at build time.

- [ ] **Step 3: Implement** `render_request` as all-or-nothing: render into a scratch copy, and write it back only when every field succeeded, so a partial render can never reach the wire. Render in `FIELDS` order — URL, path parameters, query parameters, headers, body text, authentication — and set `RenderError`'s `path` to the field path, so `TPL-030`'s `_request.headers[0].value` form comes out of the error the renderer already produces.

- [ ] **Step 4: Log a breadcrumb after five seconds.** `TPL-040` is a `tracing::warn!` emitted from a timer started around the render when it is still running at 5 s; it never aborts. The test asserts the event through a `tracing_subscriber` capture, not a wall-clock wait.

- [ ] **Step 5: Verify and commit** — `cargo test -p heric-template`, then `feat(template): render a request's fields atomically`

---

### Task 14: The Liquid conformance suite

**Files:**
- Create: `crates/heric-template/tests/liquid_compat.rs`
- Modify: `crates/heric-template/src/*` only where a case fails.

**Interfaces:** none new. This task is `FIX-010`'s definition of done and the acceptance for `TPL-001`.

- [ ] **Step 1:** Transcribe all **99** cases from `packages/insomnia/src/templating/__tests__/liquid-compat.test.ts` (26 `describe` blocks) into `tests/liquid_compat.rs`, one `#[test]` per original case, each carrying a comment naming its `describe`/`it` pair. Keep the Nunjucks-breaking-change group and the security group as their own modules, because the row names them separately.
- [ ] **Step 2:** Run `cargo test -p heric-template --test liquid_compat`. Record every failure in the PR description before fixing anything.
- [ ] **Step 3:** Fix in `heric-template` until green. A case that cannot pass exactly is reported to the owner as a scope question before the branch finishes; an expectation is never weakened to make a test green.
- [ ] **Step 4:** Verify `cargo test -p heric-template` is green across all three test binaries.
- [ ] **Step 5: Commit** — `test(template): port the 99-case Liquid conformance suite`

---

### Task 15: ADR 0010 spike and the libcurl engine

**Files:**
- Modify: `Cargo.toml`, `crates/heric-http/Cargo.toml`
- Create: `crates/heric-http/src/engine.rs`
- Modify: `crates/heric-http/src/lib.rs`
- Test: `#[cfg(test)]` in `engine.rs`, plus `crates/heric-http/tests/pipeline.rs`

**Interfaces:**
- Produces: `heric_http::engine() -> Result<Box<dyn HttpEngine>, HttpError>`; `pub(crate) struct CurlEngine`; `engine::map_curl_error(error: curl::Error) -> HttpError`.

**Manifest (ADR 0010 option A):**

```toml
# [workspace.dependencies]
curl = "0.4.51"
# crates/heric-http/Cargo.toml
curl = { workspace = true }
[target.'cfg(target_os = "macos")'.dependencies]
curl = { workspace = true, features = ["apple-sectrust"] }
```

- [ ] **Step 1: Run the spike before writing engine code.** Add the manifest lines with a body that only constructs `curl::easy::Easy::new()`, push the branch, and confirm all four `test` jobs build and pass. If any target fails, apply ADR 0010's fallback order (`static-ssl` on Linux, then revisit ADR 0004) and record the observed result in the ADR. No engine code before this is green.
- [ ] **Step 2: Write the failing tests** against `wiremock`

```rust
#[test]
fn a_get_returns_status_headers_and_body() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(201).header("x-test", "yes").body("hi");
    });
    let execution = execute(&server.url("/hello"))?;
    assert_eq!(execution.status, 201);
    assert!(execution.headers.iter().any(|h| h.name.eq_ignore_ascii_case("x-test") && h.value == "yes"));
    assert_eq!(execution.body, b"hi");
    Ok(())
}

#[test]
fn no_content_has_a_status_and_no_body() { /* 204: status 204, body empty */ }
#[test]
fn a_cancelled_token_stops_the_transfer() { /* cancel from another thread -> HttpError::Cancelled */ }
#[test]
fn the_timeline_records_request_then_response_headers() { /* HeaderOut before HeaderIn */ }
#[test]
fn an_unreachable_port_is_a_connect_error() { /* bind and drop a listener */ }
#[test]
fn every_phase_timing_is_monotonic() { /* name_lookup <= connect <= pre_transfer <= total */ }
```

- [ ] **Step 3: Implement** `CurlEngine::execute`: set URL, method, headers, body from `Body::{Empty, Bytes, File}` (`File` via `read_function`, so a large upload never enters memory), `ACCEPT_ENCODING` so libcurl decompresses as the original does, timeouts from `Timeout`, `RedirectPolicy` with `max_redirects`, `CAINFO_BLOB` from `TlsConfig::workspace_ca` while never disabling the native store (`PLT-024`), the proxy from `ProxyConfig`, and a progress callback reading `CancelToken`. Map `curl::Error` by category into `HttpError::{Cancelled, Timeout, InvalidUrl, Dns, Connect, Tls, Protocol, TooManyRedirects, Engine}`. Build `Timings` from `curl::easy::Info` and push `TimelineEntry` values to `options.timeline` when present.
- [ ] **Step 4: `FIX-012`.** Port the 52 cases from `packages/insomnia/src/network/__tests__/network.test.ts` into `tests/pipeline.rs`, covering the parts of the pipeline Heric owns now: headers, HTTP version, cookie parsing, inheritance, and "a response tag is never re-rendered". Cases that need multipart, a unix socket or netrc get a named `#[ignore = "needs multipart (M3)"]`, and every skip is listed in the PR description.
- [ ] **Step 5: Verify and commit** — `cargo test -p heric-http`, then `feat(http): execute requests through libcurl`

---

### Task 16: The auth providers M1 needs

**Files:**
- Create: `crates/heric-auth/src/providers/{mod.rs,basic.rs,bearer.rs,apikey.rs}`
- Modify: `crates/heric-auth/src/lib.rs`
- Test: `#[cfg(test)]` in `providers/mod.rs`

**Interfaces:**
- Produces: `heric_auth::providers() -> Result<ProviderRegistry, AuthError>` registering exactly `BasicProvider`, `BearerProvider`, `ApiKeyProvider`.

**Scope, decided here:** three providers. `ProviderRegistry::apply` already answers `AuthError::Unsupported` for an unregistered method, which is the honest result for Digest, NTLM, OAuth and IAM until M3, and `heric_auth::detect` already feeds the migration report so a user learns at import time rather than at send time.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn basic_adds_an_authorization_header_with_base64_credentials() -> Result<(), AuthError> {
    // user "kyle" / pass "secret" -> "Basic a3lsZTpzZWNyZXQ="
    Ok(())
}

#[test]
fn a_secret_pending_reentry_is_an_error_not_an_empty_value() {
    // SecretState::PendingReEntry -> Err(AuthError::PendingReEntry(_)), never Ok((""))
}

#[test]
fn bearer_uses_the_configured_prefix() { /* default "Bearer", or "Token" */ }
#[test]
fn an_api_key_lands_in_a_header_a_query_parameter_or_a_cookie() { /* ApiKeyLocation x3 */ }
#[test]
fn an_unregistered_method_is_unsupported() { /* registry.get("oauth2") is None */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** `AuthProvider` for each: read the method's fields from `AuthMethod`, resolve credentials through `AuthContext::secrets` so `SecretState::PendingReEntry` becomes `AuthError::PendingReEntry` (Review Focus 4's pin) and `Absent` becomes `AuthError::Missing`, and return an `AuthOutcome` carrying headers, query pairs or cookies. Base64 is a hand-written eleven-line encoder rather than a new dependency.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(auth): add the basic, bearer and API-key providers`

---

### Task 17: The send pipeline

**Files:**
- Modify: `crates/heric-core/src/service.rs`, `src/host.rs`, `src/runtime.rs`
- Test: `#[cfg(test)]` in `service.rs`, plus `crates/heric-core/tests/send.rs`

**Interfaces:**
- Produces: `RequestService::{prepare, send_and_wait}` implemented for the stages M1 runs; `SendingHost::read_file` consulting `is_datafile` before reading.

**Stages M1 runs:** `Inherit`, `Render`, `BuildBody`, `Authenticate`, `Execute`, `StoreResponse`, `RecordHistory`. `PreRequestScript`, `Plugins`, `AfterResponseScript` and `PatchResponse` arrive with M5, so `ExecutionPlan::stages` omits them — which is exactly what `ExecutionPlan::minimal` already encodes.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_prepared_request_carries_the_rendered_url_and_headers() -> Result<(), CoreError> { /* … */ }

#[test]
fn an_undefined_variable_stops_the_send_before_the_engine_runs() { /* fake engine records zero calls */ }

#[test]
fn a_pending_credential_stops_the_send_with_a_stop_reason() { /* StopReason::PendingCredential */ }

#[test]
fn a_response_is_stored_with_its_body_and_timeline_blobs() -> Result<(), CoreError> {
    // after the send: one Response doc, BlobRole::Body and BlobRole::Timeline both non-empty
    Ok(())
}

#[test]
fn a_finished_send_reports_send_finished_and_releases_the_flight() { /* is_sending() back to false */ }
#[test]
fn a_cancelled_send_reports_send_cancelled() { /* … */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** `prepare` as `Inherit` (through `views::inherited_headers` and `inherited_authentication` from Task 6) → `Render` (`render_request` with a `SendingHost` at `RenderPurpose::Send`) → `BuildBody` (`Body::Empty` for GET and HEAD, `Body::Bytes` for text, `Body::File` for a `{% file %}` body) → `Authenticate` (through the existing `apply_auth` helper). Implement `send_and_wait` to call `ports().engine.execute` inside `run_blocking`, then `response::create`, `blobs.put` for body and timeline, `store.insert` for the response document, and a `Patch` setting `RequestMeta.activeResponseId`. Every failure path still calls `end_flight`, which Phase 1 already guarantees.

In `host.rs`, `read_file` returns `TagError::PathNotAllowed` for a path where `is_datafile` is true, closing `STO-020`'s template and plugin route.

- [ ] **Step 4: Run to verify passing** — `cargo test -p heric-core`.

- [ ] **Step 5: Commit** — `feat(core): run the M1 send pipeline end to end`

---

### Task 18: A working `heric-cli run`

**Files:**
- Modify: `apps/cli/src/main.rs`, `apps/cli/Cargo.toml`
- Test: `#[cfg(test)]` in `main.rs`, plus `apps/cli/tests/run.rs`

**Interfaces:**
- Produces: `Command::Run` printing a status line, the headers and the body; exit code `0` for any HTTP status, `1` for a transport or configuration failure; a top-level `--data-dir` flag.

- [ ] **Step 1: Write the failing tests** — `run_prints_the_status_and_body`, `run_exits_one_when_the_host_is_unreachable`, `run_accepts_a_data_dir_override`.
- [ ] **Step 2–5:** add `tokio` to the CLI crate; start the core with `Core::start(CoreBuilder::new(paths).build())`, take `core.handle()`, load the request from the store through the document service, `handle.block_on(handle.requests().send_and_wait(id))`, print. Replace the `pending("run", …)` bail; leave the other subcommands reporting their milestone. The `Core` stays alive for the whole command, because dropping it stops the runtime. Verify against a loopback listener; commit `feat(cli): send a request and print the response`.

---

### Task 19: The request editor and response pane

**Files:**
- Modify: `apps/gui/src/main.rs`, `src/events.rs`
- Create: `apps/gui/src/editor.rs`, `src/response.rs`, `src/tree.rs`
- Test: `#[cfg(test)]` in `editor.rs` and `response.rs`

**Interfaces:**
- Produces: `editor::RequestEditor { method: String, url: String, headers: Vec<Row>, body: String }` with `to_patch(&self) -> Patch`; `response::ResponseView { status: u16, status_message: String, elapsed: Duration, size: u64, tab: ResponseTab, body: String }` with `ResponseView::from_execution(&Execution) -> Self`; `tree::collections(&[Doc]) -> Vec<TreeNode>`.

**Layout (ADR 0013, design §16.2):** `Panel::top` toolbar, `Panel::left` collection tree, `Panel::right` response, `CentralPanel` request editor, per-region tab strips drawn with `ui.selectable_label`. No docking crate.

- [ ] **Step 1: Write the failing tests**, all on pure functions so they run headless: `a_disabled_header_row_is_dropped_from_the_patch`, `an_empty_url_produces_no_request`, `a_tree_nests_a_request_under_its_folder`, `a_2xx_response_selects_the_body_tab`, `a_204_shows_an_empty_body_without_panicking` (Review Focus 5), `a_megabyte_body_is_not_pretty_printed` (the 1 MiB guard).
- [ ] **Step 2–5:** replace the Phase 1 placeholder frames, wire Send to `CoreHandle::spawn(requests.send_and_wait(…))`, absorb `SendStarted`, `Timeline`, `SendFinished` and `SendCancelled` into the response view and console, and keep `describe` exhaustive so a new variant is a compile error. Verify `cargo test -p heric` and run the binary against a loopback server; commit `feat(gui): edit a request and show the response`.

---

### Task 20: macOS window lifecycle and the Intel runner

**Files:**
- Create: `apps/gui/src/lifecycle.rs`
- Modify: `apps/gui/src/main.rs`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- Test: `#[cfg(test)]` in `lifecycle.rs`

**Interfaces:**
- Produces: `lifecycle::{CloseAction, ActivateAction, on_close(Platform) -> CloseAction, on_activate(Platform) -> ActivateAction}` with `CloseAction::{Hide, Exit}` and `ActivateAction::{Reveal, Ignore}`.

- [ ] **Step 1: Write the failing tests** — `a_close_on_macos_hides_the_window`, `a_close_elsewhere_exits`, `an_activate_on_macos_reveals_and_elsewhere_ignores`.
- [ ] **Step 2:** on macOS the app sends `ViewportCommand::CancelClose` then `Visible(false)`, and shows the viewport again when the window is activated (`PLT-015`). Elsewhere the close proceeds.
- [ ] **Step 3: CI.** Design §7 and §21 require the Intel Mac to run the **full** suite from M1. Replace the cross-build entry with a dedicated runner and remove `build_only` from both workflows:

```yaml
          - os: macos-15-intel
            target: x86_64-apple-darwin
```

Remove `if: ${{ !matrix.build_only }}` from the `cargo test` step, and make the same change in `release.yml` so the Intel archive is built on Intel hardware. If `macos-15-intel` is unavailable to this repository — it is a paid larger runner on some plans — stop and report the observed billing or availability message to the owner before substituting anything, because a silent fallback to a cross-build would reverse the owner's "dedicated Intel runner" decision.
- [ ] **Step 4: Run to verify passing** — `cargo test -p heric lifecycle`, and the workflow must be green on the PR.
- [ ] **Step 5: Commit** — `ci: run the full suite on an Intel macOS runner`

---

### Task 21: The end-to-end smoke harness

**Files:**
- Create: `apps/gui/tests/e2e.rs`
- Test: `apps/gui/tests/e2e.rs`

**Interfaces:**
- Produces: `E2e { data_dir: TempDir, core: Core, handle: CoreHandle }` with `E2e::start()`, `create_workspace(&self, name) -> Result<Id, CoreError>`, `create_request(&self, workspace, method, url) -> Result<Id, CoreError>`, `send(&self, id) -> Result<SendOutcome, CoreError>`. `Core` is held, not the handle alone: dropping `Core` stops the runtime, so a harness that kept only the handle would hang on the first await.

- [ ] **Step 1: Write the failing test** — `a_request_created_stored_and_sent_round_trips`: start a core on a temporary data directory, create a workspace and a request, send it to a loopback listener, assert a `Response` document exists with a body blob, then reopen the store from the same directory and assert both survive.
- [ ] **Step 2–5:** implement. A fake engine is injected through `CoreBuilder::with_http_engine` for any suite that must not touch the network. `STR-002`'s critical and migration suites arrive with M2; this task delivers the smoke suite and the harness they will use. Verify `cargo test -p heric --test e2e`; commit `test(gui): add the M1 end-to-end smoke harness`.

---

### Task 22: Documentation, status and the parity matrix

**Files:**
- Modify: `docs/book/{en,pt-BR,es}/src/getting-started.md`, `docs/book/{en,pt-BR,es}/src/requests-and-environments.md`
- Modify: `STATUS.md`, `PLAN.md`, `docs/inventory/FEATURES.md`
- Test: `bash scripts/check-docs-structure.sh`

- [ ] **Step 1:** Rewrite the getting-started page to match what M1 delivers: create a request, send it, read the response and the Console timeline. Do not describe environments, scripts, plugins or other protocols as available — they are M3 and later. Keep every heading identical in shape across the three languages.
- [ ] **Step 2:** In `docs/inventory/FEATURES.md`, move an M1 row to `Done` only where its task merged and its tests pass on all four targets. A row whose task was cut or deferred stays `In progress` with the deferral recorded in its Issue column. Never mark a row done for a task that did not land.
- [ ] **Step 3:** Update `STATUS.md` and `PLAN.md`'s M1 row.
- [ ] **Step 4:** Verify the five gates and the docs script.
- [ ] **Step 5: Commit** — `docs: describe M1 in three languages and close its parity rows`

---

## Row traceability

All 57 M1 rows in `docs/inventory/FEATURES.md`, each mapped to exactly one task.

| Rows | Task | What the task must make true |
|---|---|---|
| PLT-001, PLT-002, PLT-007 | 1 | Platform data paths, `INSOMNIA_DATA_PATH`, platform detection |
| STO-001, STO-002, STO-003 | 2 | The store over all 44 kinds, injected per binary, full CRUD with upsert |
| STO-004, STO-005 | 3 | Descendant traversal and the change broadcast |
| STO-015, STO-020 | 4 | Body and timeline blobs; datafiles refused to templates and plugins |
| MOD-001, MOD-002, MOD-003, MOD-006, MOD-017, MOD-030, MOD-035, FIX-006 | 5 | Base fields, `initModel`, registry, scratchpad sentinels, request defaults, groups, environments |
| MOD-005, MOD-007, MOD-009, MOD-010, MOD-012, MOD-032, MOD-037 | 6 | Workspace scopes, parent rule, meta lazily, project variants and order, base environment |
| MOD-015, MOD-020, MOD-021, MOD-022, MOD-023, MOD-024, MOD-025, MOD-026, MOD-027, MOD-031 | 7 | Request and response models, creation, pruning, latest lookup, gunzip, history delete, snapshots, path parameters, sort keys |
| MOD-054 | 8 | Settings defaults and repair |
| TPL-002, TPL-003 | 9 | Comment stripping and the no-delimiter short-circuit |
| TPL-001, TPL-005 | 10 | Strict variables and filters, limits, structured errors with line and column |
| TPL-006, TPL-014 | 11 | Argument tokenizer; the tag contract and its thirteen tags |
| TPL-004, TPL-008, TPL-009, TPL-010, TPL-011, TPL-015 | 12 | Dual context, layering, three passes, traversal, injection guard, masking |
| TPL-012, TPL-030, TPL-040, FIX-011 | 13 | Request-level render, error paths, breadcrumb, golden expectations |
| FIX-010 | 14 | The 99-case Liquid conformance suite |
| FIX-012 | 15 | The engine and the pipeline table test |
| STR-001 | 16, 17 | The contracts realised end to end: one wiring per binary, the send path |
| PLT-015 | 20 | macOS window lifecycle |
| STR-002 | 21 | The smoke suite and its harness |

Rows with no dedicated task are deliberate: `MOD-024`'s file-ownership check is the blob store's `remove` in Task 4, and `MOD-025` is Task 3's cascade plus that removal, both proved in Task 17. Every row above ends the milestone in `Done` or, if deferred, `In progress` with the reason recorded in Task 22.
