//! Which workspace is open, and the document tree under it.

use std::sync::Arc;

use heric_model::{Change, Doc, DocKind, Id, Patch, Query};

use crate::CoreHandle;
use crate::error::CoreError;

/// The workspace a session is working in.
///
/// Kept by the core rather than by each tab, because a send resolves its
/// environment from this and two tabs must not resolve different ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveContext {
    /// The open workspace.
    pub workspace: Id,
    /// The environment in use, when one is selected.
    pub environment: Option<Id>,
}

impl ActiveContext {
    /// A workspace with no environment selected.
    #[must_use]
    pub fn new(workspace: Id) -> Self {
        Self {
            workspace,
            environment: None,
        }
    }
}

/// A row in the workspace list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSummary {
    /// The workspace.
    pub id: Id,
    /// Its display name, empty when absent.
    pub name: String,
    /// How many requests it holds, counting descendants.
    pub requests: usize,
    /// Whether it is the scratchpad, which the original pins to the top and
    /// never syncs.
    pub scratchpad: bool,
}

/// Document reads and writes.
///
/// Every method hops to the blocking pool, because the store is SQLite. The
/// GUI calls them through `CoreHandle::spawn` and receives results as events or
/// through a tab's own future; the CLI awaits them directly.
#[derive(Debug, Clone)]
pub struct DocumentService {
    core: CoreHandle,
}

impl DocumentService {
    /// Builds the service. Front ends get one from [`CoreHandle::documents`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Reads one document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the store cannot be read.
    pub async fn get(&self, id: Id) -> Result<Option<Doc>, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        self.core.run_blocking(move || Ok(store.get(&id)?)).await
    }

    /// Reads the documents matching a query.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the store cannot be read.
    pub async fn find(&self, query: Query) -> Result<Vec<Doc>, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        self.core
            .run_blocking(move || Ok(store.find(&query)?))
            .await
    }

    /// The direct children of a parent.
    ///
    /// # Errors
    ///
    /// As [`DocumentService::find`].
    pub async fn children(&self, parent: Id) -> Result<Vec<Doc>, CoreError> {
        self.find(Query::children_of(parent)).await
    }

    /// The documents of one kind under a parent.
    ///
    /// # Errors
    ///
    /// As [`DocumentService::find`].
    pub async fn of_kind(&self, parent: Id, kind: DocKind) -> Result<Vec<Doc>, CoreError> {
        self.find(Query {
            parent: Some(parent),
            kind: Some(kind),
            ..Query::default()
        })
        .await
    }

    /// Creates a document, stamping a fresh id and the current time.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the insert fails, including when the
    /// id already exists.
    pub async fn create(
        &self,
        kind: DocKind,
        parent: Option<Id>,
        name: Option<String>,
        body: serde_json::Map<String, serde_json::Value>,
    ) -> Result<Doc, CoreError> {
        let doc = Doc {
            meta: heric_model::DocMeta {
                id: Id::generate(kind),
                kind,
                parent,
                modified: now_millis(),
                created: now_millis(),
                private: None,
                name,
            },
            body,
        };
        let store = Arc::clone(&self.core.ports().store);
        let created = doc.clone();
        self.core
            .run_blocking(move || Ok(store.insert(&created)?))
            .await
    }

    /// Inserts a document as given.
    ///
    /// The difference from [`DocumentService::create`] matters: this keeps the
    /// document's own id and parent, which is what an import and the migration
    /// need so their references still resolve. `create` mints a fresh id and
    /// would leave every `parentId` dangling.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DuplicateId`](heric_storage::StorageError::DuplicateId) when the id already exists.
    pub async fn insert(&self, doc: Doc) -> Result<Doc, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        self.core
            .run_blocking(move || Ok(store.insert(&doc)?))
            .await
    }

    /// Applies a patch and returns the updated document.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`](heric_storage::StorageError::NotFound) when the id does not exist.
    pub async fn update(&self, id: Id, patch: Patch) -> Result<Doc, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        self.core
            .run_blocking(move || Ok(store.update(&id, &patch)?))
            .await
    }

    /// Removes a document and its descendants.
    ///
    /// Returns the ids removed so the UI can close their tabs, which is how the
    /// original's tab bar reacts to `db.changes`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the write fails.
    pub async fn remove(&self, id: Id) -> Result<Vec<Id>, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        self.core.run_blocking(move || Ok(store.remove(&id)?)).await
    }

    /// Subscribes to store changes.
    ///
    /// The GUI drains this once per frame. A lagged receiver drops the oldest
    /// changes, which is the right trade: a frame that missed an intermediate
    /// state reloads the current one anyway.
    #[must_use]
    pub fn changes(&self) -> tokio::sync::broadcast::Receiver<Change> {
        self.core.ports().store.changes()
    }
}

/// Workspaces and the active selection.
#[derive(Debug, Clone)]
pub struct WorkspaceService {
    core: CoreHandle,
}

impl WorkspaceService {
    /// Builds the service. Front ends get one from [`CoreHandle::workspace`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Every workspace, in the original's default order.
    ///
    /// The scratchpad is listed first because the original pins it, regardless
    /// of creation time.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the store cannot be read.
    pub async fn list(&self) -> Result<Vec<WorkspaceSummary>, CoreError> {
        let documents = self.core.documents();
        let mut workspaces: Vec<WorkspaceSummary> = documents
            .find(Query {
                parent: None,
                kind: Some(DocKind::Workspace),
                ..Query::default()
            })
            .await?
            .into_iter()
            .map(|doc| WorkspaceSummary {
                scratchpad: doc.meta.id.as_str() == "wrk_scratchpad",
                name: doc.meta.name.clone().unwrap_or_default(),
                requests: 0,
                id: doc.meta.id,
            })
            .collect();
        workspaces.sort_by(|left, right| {
            right
                .scratchpad
                .cmp(&left.scratchpad)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(workspaces)
    }

    /// The active workspace and environment, or `None` when nothing is open.
    #[must_use]
    pub fn active(&self) -> Option<ActiveContext> {
        self.core
            .ports()
            .active
            .read()
            .ok()
            .and_then(|active| active.clone())
    }

    /// Opens a workspace and makes it active.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NotFound`](heric_storage::StorageError::NotFound) when the id is not a workspace,
    /// and [`CoreError::Task`] when the lock is poisoned.
    pub async fn open(&self, workspace: Id) -> Result<Doc, CoreError> {
        let doc = self
            .core
            .documents()
            .get(workspace.clone())
            .await?
            .ok_or_else(|| {
                CoreError::Storage(heric_storage::StorageError::NotFound(
                    workspace.as_str().to_owned(),
                ))
            })?;
        if doc.meta.kind != DocKind::Workspace {
            return Err(CoreError::Model(heric_model::ModelError::WrongKind {
                expected: DocKind::Workspace,
                found: doc.meta.kind,
            }));
        }
        let context = ActiveContext::new(doc.meta.id.clone());
        self.set_active(Some(context))?;
        Ok(doc)
    }

    /// Replaces the active context.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Task`] when the lock is poisoned.
    pub fn set_active(&self, context: Option<ActiveContext>) -> Result<(), CoreError> {
        let mut active = self
            .core
            .ports()
            .active
            .write()
            .map_err(|error| CoreError::Task(error.to_string()))?;
        *active = context;
        Ok(())
    }

    /// Selects the environment a send will resolve.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NoWorkspace`] when nothing is open and
    /// [`CoreError::Task`] when the lock is poisoned.
    pub fn set_environment(&self, environment: Option<Id>) -> Result<ActiveContext, CoreError> {
        let mut active = self
            .core
            .ports()
            .active
            .write()
            .map_err(|error| CoreError::Task(error.to_string()))?;
        let Some(context) = active.as_mut() else {
            return Err(CoreError::NoWorkspace);
        };
        context.environment = environment;
        Ok(context.clone())
    }

    /// Closes the active workspace, if any.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Task`] when the lock is poisoned.
    pub fn close(&self) -> Result<(), CoreError> {
        self.set_active(None)
    }

    /// Creates a workspace and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the insert fails.
    pub async fn create(&self, name: String) -> Result<Doc, CoreError> {
        self.core
            .documents()
            .create(DocKind::Workspace, None, Some(name), serde_json::Map::new())
            .await
    }

    /// Renames a workspace or any other named document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the update fails.
    pub async fn rename(&self, id: Id, name: String) -> Result<Doc, CoreError> {
        self.core
            .documents()
            .update(
                id,
                Patch::new().set("name", serde_json::Value::String(name)),
            )
            .await
    }

    /// Deletes a workspace and everything under it.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the write fails.
    pub async fn delete(&self, id: Id) -> Result<Vec<Id>, CoreError> {
        let removed = self.core.documents().remove(id.clone()).await?;
        if self.active().is_some_and(|context| context.workspace == id) {
            self.close()?;
        }
        Ok(removed)
    }
}

/// Milliseconds since the Unix epoch, saturating rather than panicking.
///
/// The original stores `modified` and `created` in milliseconds, so a document
/// written by Heric sorts the same as one written by the original.
pub(crate) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_context_has_no_environment() {
        let context = ActiveContext::new(Id::new("wrk_1"));
        assert!(context.environment.is_none());
    }

    #[test]
    fn now_is_after_the_epoch() {
        assert!(now_millis() > 0);
    }
}
