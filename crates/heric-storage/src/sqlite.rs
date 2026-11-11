//! The SQLite-backed document store.
//!
//! One table with a `kind` discriminator rather than 44 tables: the original
//! filters every query by `parentId` and scans otherwise, so a discriminator
//! index costs nothing, and 44 near-identical tables would mean 44 migrations
//! to keep in step.
//!
//! `body` holds the document serialised without its meta fields, so a key this
//! build has never heard of is stored exactly as it arrived. The meta columns
//! are separate because they are what queries filter and sort on.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use heric_model::{Change, Doc, DocKind, DocMeta, Id, Order, Patch, PatchOp, Query};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use serde_json::Value as Json;

use crate::error::StorageError;
use crate::store::Store;

#[cfg(test)]
use crate::store::StoreHandle;

/// The schema version this build writes and understands.
const SCHEMA_VERSION: u32 = 1;

/// How many changes a lagging subscriber may fall behind before it starts
/// losing the oldest ones.
const CHANGE_CAPACITY: usize = 1024;

/// The columns every read selects, in the order [`doc_from_row`] expects.
const SELECT_COLUMNS: &str =
    "SELECT id, kind, parent, created, modified, is_private, name, body FROM documents";

/// The schema. Every statement is idempotent, so opening an existing database
/// only re-asserts what is already there.
const SCHEMA: &str = "
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
";

/// The document store, over one SQLite database.
///
/// Every call arrives from `tokio::task::spawn_blocking`, so one connection
/// behind a mutex is enough: the contention a pool would avoid does not exist
/// on a single-user desktop, and a second connection would only complicate
/// transaction ordering.
pub(crate) struct SqliteStore {
    connection: Mutex<Connection>,
    sender: tokio::sync::broadcast::Sender<Change>,
    /// Keeps the temporary directory alive for as long as the store uses it.
    #[cfg(test)]
    _temp_dir: Option<tempfile::TempDir>,
}

impl SqliteStore {
    /// Opens the database at `path`, creating it and its schema if needed.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::SchemaTooNew`] when the database was written by
    /// a newer build, and [`StorageError::Database`] when SQLite refuses.
    pub(crate) fn open(path: &Path) -> Result<Self, StorageError> {
        let connection = Connection::open(path).map_err(database)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(database)?;
        connection.execute_batch(SCHEMA).map_err(database)?;
        check_schema_version(&connection)?;
        let (sender, _receiver) = tokio::sync::broadcast::channel(CHANGE_CAPACITY);
        Ok(Self {
            connection: Mutex::new(connection),
            sender,
            #[cfg(test)]
            _temp_dir: None,
        })
    }

    /// Takes the connection, mapping a poisoned lock rather than panicking on
    /// it.
    fn lock(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection.lock().map_err(|_| {
            StorageError::Database("the store lock was poisoned by a panic".to_owned())
        })
    }
}

impl Store for SqliteStore {
    fn get(&self, id: &Id) -> Result<Option<Doc>, StorageError> {
        let connection = self.lock()?;
        read_document(&connection, id)
    }

    fn find(&self, query: &Query) -> Result<Vec<Doc>, StorageError> {
        let connection = self.lock()?;
        let (sql, values) = find_statement(query);
        let mut statement = connection.prepare(&sql).map_err(database)?;
        let rows = statement
            .query_map(params_from_iter(values), doc_from_row)
            .map_err(database)?;
        let mut documents = Vec::new();
        for row in rows {
            documents.push(row.map_err(database)?);
        }
        Ok(documents)
    }

    fn insert(&self, doc: &Doc) -> Result<Doc, StorageError> {
        let connection = self.lock()?;
        let body = body_json(doc)?;
        let result = connection.execute(
            "INSERT INTO documents (id, kind, parent, created, modified, is_private, name, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                doc.meta.id.as_str(),
                doc.meta.kind.as_str(),
                doc.meta.parent.as_ref().map(Id::as_str),
                doc.meta.created,
                doc.meta.modified,
                doc.meta.private,
                doc.meta.name.as_deref(),
                body,
            ],
        );
        match result {
            Ok(_) => Ok(doc.clone()),
            Err(error) if is_duplicate_id(&error) => {
                Err(StorageError::DuplicateId(doc.meta.id.as_str().to_owned()))
            }
            Err(error) => Err(database(error)),
        }
    }

    fn update(&self, id: &Id, patch: &Patch) -> Result<Doc, StorageError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(database)?;
        let stored = read_document(&transaction, id)?
            .ok_or_else(|| StorageError::NotFound(id.as_str().to_owned()))?;

        let updated = apply_patch(&stored, id, patch)?;
        transaction
            .execute(
                "UPDATE documents SET kind = ?1, parent = ?2, created = ?3, modified = ?4,
                     is_private = ?5, name = ?6, body = ?7
                 WHERE id = ?8",
                params![
                    updated.meta.kind.as_str(),
                    updated.meta.parent.as_ref().map(Id::as_str),
                    updated.meta.created,
                    updated.meta.modified,
                    updated.meta.private,
                    updated.meta.name.as_deref(),
                    body_json(&updated)?,
                    id.as_str(),
                ],
            )
            .map_err(database)?;
        transaction.commit().map_err(database)?;
        Ok(updated)
    }

    /// Removes the document itself, returning its id.
    ///
    /// Descendants are not walked yet: that traversal, and the change
    /// broadcast, belong to the task that completes removal.
    fn remove(&self, id: &Id) -> Result<Vec<Id>, StorageError> {
        let connection = self.lock()?;
        let deleted = connection
            .execute("DELETE FROM documents WHERE id = ?1", [id.as_str()])
            .map_err(database)?;
        Ok(if deleted == 0 {
            Vec::new()
        } else {
            vec![id.clone()]
        })
    }

    fn changes(&self) -> tokio::sync::broadcast::Receiver<Change> {
        self.sender.subscribe()
    }
}

/// A store over a fresh temporary database, for tests.
///
/// The handle owns the temporary directory, so the database outlives every
/// reference to it and disappears when the last one is dropped.
///
/// # Errors
///
/// Returns [`StorageError`] when the temporary directory or the database
/// cannot be created.
#[cfg(test)]
pub(crate) fn ephemeral() -> Result<StoreHandle, StorageError> {
    let directory = tempfile::tempdir().map_err(database)?;
    let mut store = SqliteStore::open(&directory.path().join("heric.db"))?;
    store._temp_dir = Some(directory);
    Ok(std::sync::Arc::new(store))
}

/// Applies a patch to a document and stamps it with the current time.
///
/// The patch is applied to the document's JSON form, so a field that names a
/// meta column and a field that names a body key are handled by the same
/// code.
fn apply_patch(stored: &Doc, id: &Id, patch: &Patch) -> Result<Doc, StorageError> {
    let value = serde_json::to_value(stored).map_err(database)?;
    let Json::Object(mut fields) = value else {
        return Err(StorageError::Database(
            "a document must serialise to a JSON object".to_owned(),
        ));
    };
    for (field, op) in &patch.fields {
        match op {
            PatchOp::Set(value) => {
                fields.insert(field.clone(), value.clone());
            }
            PatchOp::Unset => {
                fields.remove(field);
            }
        }
    }
    // The id addresses the row, so a patch cannot move it; every other meta
    // field is patchable like a body key. `modified` is stamped last and wins
    // over a patch that sets it, which is the store's contract for an edit.
    fields.insert("_id".to_owned(), Json::String(id.as_str().to_owned()));
    fields.insert("modified".to_owned(), Json::from(now_millis()));
    serde_json::from_value(Json::Object(fields)).map_err(database)
}

/// Builds the `SELECT` and its bound values for a query.
fn find_statement(query: &Query) -> (String, Vec<Value>) {
    let mut sql = String::from(SELECT_COLUMNS);
    let mut conditions: Vec<&str> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    if let Some(parent) = &query.parent {
        conditions.push("parent = ?");
        values.push(Value::Text(parent.as_str().to_owned()));
    }
    if let Some(kind) = query.kind {
        conditions.push("kind = ?");
        values.push(Value::Text(kind.as_str().to_owned()));
    }
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }

    // The id breaks ties so that a listing cannot reorder between two
    // identical queries.
    sql.push_str(match query.order {
        Order::Created => " ORDER BY created ASC, id ASC",
        Order::ModifiedDesc => " ORDER BY modified DESC, id ASC",
    });

    if let Some(limit) = query.limit {
        sql.push_str(" LIMIT ?");
        values.push(Value::Integer(i64::try_from(limit).unwrap_or(i64::MAX)));
    }
    (sql, values)
}

/// Reads one document, or `None` when the id is not stored.
fn read_document(connection: &Connection, id: &Id) -> Result<Option<Doc>, StorageError> {
    connection
        .query_row(
            &format!("{SELECT_COLUMNS} WHERE id = ?1"),
            [id.as_str()],
            doc_from_row,
        )
        .optional()
        .map_err(database)
}

/// Rebuilds a document from the columns a row selected with
/// [`SELECT_COLUMNS`] carries.
fn doc_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Doc> {
    let id: String = row.get(0)?;
    let kind: String = row.get(1)?;
    let parent: Option<String> = row.get(2)?;
    let created: i64 = row.get(3)?;
    let modified: i64 = row.get(4)?;
    let private: Option<bool> = row.get(5)?;
    let name: Option<String> = row.get(6)?;
    let body: String = row.get(7)?;

    let kind = DocKind::parse(&kind).map_err(|error| conversion_error(1, error))?;
    let body: serde_json::Map<String, Json> =
        serde_json::from_str(&body).map_err(|error| conversion_error(7, error))?;
    Ok(Doc {
        meta: DocMeta {
            id: Id::new(id),
            kind,
            parent: parent.map(Id::new),
            modified,
            created,
            private,
            name,
        },
        body,
    })
}

/// Turns a value that will not convert into a row error the caller can report.
fn conversion_error(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

/// Serialises the part of a document that belongs in the `body` column.
///
/// The document's own serialisation already merges the meta fields with the
/// body, so subtracting the meta fields it emitted leaves exactly the body —
/// no second copy of the meta key list, and a body key that shadows a meta
/// field stays excluded the way the model's own serialiser excludes it.
fn body_json(doc: &Doc) -> Result<String, StorageError> {
    let value = serde_json::to_value(doc).map_err(database)?;
    let Json::Object(mut fields) = value else {
        return Err(StorageError::Database(
            "a document must serialise to a JSON object".to_owned(),
        ));
    };
    let meta = serde_json::to_value(&doc.meta).map_err(database)?;
    let Json::Object(meta) = meta else {
        return Err(StorageError::Database(
            "document meta must serialise to a JSON object".to_owned(),
        ));
    };
    for key in meta.keys() {
        fields.remove(key);
    }
    serde_json::to_string(&fields).map_err(database)
}

/// Reads the stored schema version, writing the current one when it is absent.
fn check_schema_version(connection: &Connection) -> Result<(), StorageError> {
    let stored: Option<String> = connection
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(database)?;

    let Some(stored) = stored else {
        connection
            .execute(
                "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
                [SCHEMA_VERSION.to_string()],
            )
            .map_err(database)?;
        return Ok(());
    };

    let found = stored.parse::<u32>().map_err(|error| {
        StorageError::Database(format!(
            "schema version {stored:?} is not a number: {error}"
        ))
    })?;
    if found > SCHEMA_VERSION {
        return Err(StorageError::SchemaTooNew {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}

/// Whether a failed insert failed because the id is already taken.
fn is_duplicate_id(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.code == rusqlite::ErrorCode::ConstraintViolation
                && inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_PRIMARYKEY
    )
}

/// The current time in Unix milliseconds, or `0` when the clock is before the
/// epoch.
fn now_millis() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

/// Wraps any failure that is really SQLite refusing into a storage error.
fn database(error: impl std::fmt::Display) -> StorageError {
    StorageError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use heric_model::{DocKind, DocMeta, Order};
    use serde_json::json;

    use super::*;

    fn request_doc() -> Doc {
        let mut body = serde_json::Map::new();
        body.insert("url".into(), json!("https://example.com/users"));
        body.insert("method".into(), json!("GET"));
        body.insert(
            "headers".into(),
            json!([{"name": "Accept", "value": "application/json"}]),
        );
        Doc {
            meta: DocMeta {
                id: Id::new("req_1"),
                kind: DocKind::Request,
                parent: Some(Id::new("wrk_1")),
                modified: 1,
                created: 0,
                private: None,
                name: Some("Get users".into()),
            },
            body,
        }
    }

    fn document(id: &str, kind: DocKind, parent: Option<&str>, created: i64, modified: i64) -> Doc {
        Doc {
            meta: DocMeta {
                id: Id::new(id),
                kind,
                parent: parent.map(Id::new),
                modified,
                created,
                private: None,
                name: None,
            },
            body: serde_json::Map::new(),
        }
    }

    #[test]
    fn an_inserted_document_reads_back_identically() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        let doc = request_doc();
        assert_eq!(store.insert(&doc)?, doc);
        assert_eq!(store.get(&doc.meta.id)?, Some(doc));
        Ok(())
    }

    #[test]
    fn unknown_body_keys_survive_the_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        let mut doc = request_doc();
        doc.body
            .insert("futureField".into(), json!({"nested": [1, 2, 3]}));
        store.insert(&doc)?;
        let back = store.get(&doc.meta.id)?.ok_or("missing")?;
        assert_eq!(
            back.body.get("futureField"),
            Some(&json!({"nested": [1, 2, 3]}))
        );
        Ok(())
    }

    #[test]
    fn every_kind_round_trips_with_its_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        for kind in DocKind::ALL {
            let id = Id::generate(*kind);
            assert!(id.as_str().starts_with(kind.prefix()), "{kind:?}");
            let doc = Doc {
                meta: DocMeta {
                    id: id.clone(),
                    kind: *kind,
                    parent: None,
                    modified: 0,
                    created: 0,
                    private: None,
                    name: None,
                },
                body: serde_json::Map::new(),
            };
            store.insert(&doc)?;
            assert_eq!(store.get(&id)?, Some(doc), "{kind:?}");
        }
        Ok(())
    }

    #[test]
    fn a_duplicate_insert_is_refused_and_an_upsert_replaces()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        let mut doc = request_doc();
        store.insert(&doc)?;
        assert!(matches!(
            store.insert(&doc),
            Err(StorageError::DuplicateId(_))
        ));
        doc.body.insert("method".into(), json!("POST"));
        doc.body.remove("headers");
        assert_eq!(store.upsert(&doc)?.body.get("method"), Some(&json!("POST")));
        // Replacement, not merge: the removed field does not survive.
        assert_eq!(
            store.get(&doc.meta.id)?.map(|back| back.body),
            Some(doc.body)
        );
        Ok(())
    }

    #[test]
    fn an_absent_private_field_stays_absent() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        store.insert(&request_doc())?;
        let mut public = request_doc();
        public.meta.id = Id::new("req_2");
        public.meta.private = Some(false);
        store.insert(&public)?;

        assert_eq!(
            store
                .get(&request_doc().meta.id)?
                .map(|doc| doc.meta.private),
            Some(None)
        );
        assert_eq!(
            store.get(&public.meta.id)?.map(|doc| doc.meta.private),
            Some(Some(false))
        );
        Ok(())
    }

    #[test]
    fn an_update_applies_the_patch_and_restamps_modified() -> Result<(), Box<dyn std::error::Error>>
    {
        let store = ephemeral()?;
        let mut doc = request_doc();
        doc.meta.modified = 1;
        store.insert(&doc)?;
        let patch = Patch::new().set("url", json!("https://changed.test"));
        let updated = store.update(&doc.meta.id, &patch)?;
        assert!(updated.meta.modified > 1);
        assert_eq!(
            updated.body.get("url"),
            Some(&json!("https://changed.test"))
        );
        Ok(())
    }

    #[test]
    fn an_update_reaches_the_body_and_the_meta_columns() -> Result<(), Box<dyn std::error::Error>> {
        let store = ephemeral()?;
        let doc = request_doc();
        store.insert(&doc)?;
        let patch = Patch::new().unset("headers").set("name", json!("Renamed"));
        let updated = store.update(&doc.meta.id, &patch)?;
        assert_eq!(updated.body.get("headers"), None);
        assert_eq!(updated.meta.name.as_deref(), Some("Renamed"));
        assert_eq!(store.get(&doc.meta.id)?, Some(updated));
        Ok(())
    }

    #[test]
    fn open_creates_the_data_directory_and_the_database() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir().map_err(database)?;
        let paths = crate::DataPaths::at(directory.path().join("Heric"));

        let store = crate::open(&paths)?;
        let doc = request_doc();
        store.insert(&doc)?;
        assert!(paths.database().is_file());
        // A second open over the same file sees what the first wrote, which is
        // what a restart looks like.
        assert_eq!(crate::open(&paths)?.get(&doc.meta.id)?, Some(doc));
        Ok(())
    }

    #[test]
    fn a_query_filters_by_parent_and_kind_in_both_orders() -> Result<(), Box<dyn std::error::Error>>
    {
        let store = ephemeral()?;
        store.insert(&document("req_a", DocKind::Request, Some("wrk_1"), 3, 30))?;
        store.insert(&document("req_b", DocKind::Request, Some("wrk_1"), 1, 10))?;
        store.insert(&document(
            "reqm_a",
            DocKind::RequestMeta,
            Some("wrk_1"),
            2,
            20,
        ))?;
        store.insert(&document("req_c", DocKind::Request, Some("wrk_2"), 0, 40))?;
        store.insert(&document("wrk_1", DocKind::Workspace, None, 4, 50))?;

        let of_wrk_1 = store.find(&Query {
            parent: Some(Id::new("wrk_1")),
            kind: Some(DocKind::Request),
            order: Order::Created,
            limit: None,
        })?;
        assert_eq!(
            of_wrk_1
                .iter()
                .map(|doc| doc.meta.id.as_str())
                .collect::<Vec<_>>(),
            ["req_b", "req_a"]
        );

        let newest_requests = store.find(&Query {
            parent: None,
            kind: Some(DocKind::Request),
            order: Order::ModifiedDesc,
            limit: Some(2),
        })?;
        assert_eq!(
            newest_requests
                .iter()
                .map(|doc| doc.meta.id.as_str())
                .collect::<Vec<_>>(),
            ["req_c", "req_a"]
        );
        Ok(())
    }

    #[test]
    fn a_newer_schema_version_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir().map_err(database)?;
        let path = directory.path().join("heric.db");

        drop(SqliteStore::open(&path)?);
        // A version bumped behind the store's back is what a database written
        // by a newer build looks like on disk.
        let connection = Connection::open(&path).map_err(database)?;
        connection
            .execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '2')",
                [],
            )
            .map_err(database)?;
        drop(connection);

        assert!(matches!(
            SqliteStore::open(&path),
            Err(StorageError::SchemaTooNew {
                found: 2,
                supported: 1
            })
        ));
        Ok(())
    }
}
