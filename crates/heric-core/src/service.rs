//! The services a front end calls.
//!
//! Each service is a thin, clonable view over [`CoreHandle`]: it holds no state
//! of its own, so a second tab, the CLI and a test all see the same ports. The
//! methods are `async` where they touch a blocking port, because the caller
//! decides whether to await them (the CLI) or spawn them and listen for events
//! (the GUI).

use std::sync::Arc;

use heric_auth::{AuthContext, AuthMethod, SecretRef};
use heric_import::{
    ExportFormat, ExportOptions, ExportedFile, Format, ImportOptions, ImportReport, ImportResult,
    IterationData, RedactionMode,
};
use heric_model::{Doc, DocKind, Id, Patch, PreparedRequest, TimelineEntry, TimelineKind};
use heric_script::{ScriptContext, ScriptOutcome, ScriptPhase, SecurityPolicy, TestResult};

use crate::CoreHandle;
use crate::error::CoreError;
use crate::events::CoreEvent;
use crate::plan::{ExecutionPlan, PlanStage, SendOutcome, StopReason};

/// An environment's variables, or an empty map when the field is absent.
///
/// A non-object `data` field is treated as empty rather than as an error: the
/// field is written by whichever version of the client last saved the
/// environment, and refusing to edit one would strand the user.
fn data_object(doc: &Doc) -> serde_json::Map<String, serde_json::Value> {
    doc.body
        .get("data")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// The singleton settings document's id.
///
/// Fixed rather than generated: the original keeps one settings row and looks it
/// up by a known id, and two settings rows would make every read ambiguous.
pub const SETTINGS_ID: &str = "set_heric";

/// Requests: prepare, send, cancel.
#[derive(Debug, Clone)]
pub struct RequestService {
    core: CoreHandle,
}

impl RequestService {
    /// Builds the service. Front ends get one from [`CoreHandle::requests`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Renders a request into what the engine takes.
    ///
    /// The stages are explicit because each one can fail for a different reason
    /// and the UI shows which: a template error is the user's request, a
    /// credential error is their settings, a body error is their file.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Template`] when a template does not render,
    /// [`CoreError::Auth`] when a credential is unavailable, and
    /// [`CoreError::Model`] when the document is not a request.
    pub fn prepare(&self, doc: &Doc) -> Result<PreparedRequest, CoreError> {
        let _ = (doc, &self.core);
        todo!("M3: inherit, render, build the body and apply auth")
    }

    /// Sends a request, reporting progress on the event channel.
    ///
    /// Returns the plan immediately so the UI can show the stages and offer a
    /// cancel control; the outcome arrives as [`CoreEvent::SendFinished`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Model`] when the document is not a request, and
    /// [`CoreError::Task`] when the flight cannot be registered.
    pub fn send(&self, request: Id) -> Result<ExecutionPlan, CoreError> {
        let run = self.core.next_run();
        let plan = ExecutionPlan::minimal(request.clone(), run);
        self.core
            .ports()
            .begin_flight(request.clone(), run, plan.cancel.clone())?;
        self.core
            .events()
            .send(CoreEvent::SendStarted { request, run });
        Ok(plan)
    }

    /// Sends a request and waits for the outcome.
    ///
    /// For the CLI and the runner. The GUI uses [`RequestService::send`] and
    /// listens for the event, so this method must never be called from the UI
    /// thread.
    ///
    /// # Errors
    ///
    /// As [`RequestService::prepare`] and [`CoreError::Http`].
    pub async fn send_and_wait(&self, request: Id) -> Result<SendOutcome, CoreError> {
        let plan = self.send(request.clone())?;
        let run = plan.run;
        let outcome = self
            .core
            .run_blocking(move || {
                let _ = &plan;
                todo!("M3: run the send pipeline")
            })
            .await;
        // Released on the way out whichever way the pipeline ended: a flight
        // left registered makes the request look in flight forever, and the
        // run guard keeps a slow old run from clearing a newer one.
        self.core.ports().end_flight(&request, run);
        outcome
    }

    /// Cancels an in-flight send.
    ///
    /// Returns whether there was anything to cancel. A send that already
    /// finished is not an error: a cancel button whose request completed while
    /// the user was clicking it should do nothing.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Task`] when the flight map is poisoned.
    pub fn cancel(&self, request: &Id) -> Result<bool, CoreError> {
        let Some(flight) = self.core.ports().flight(request) else {
            return Ok(false);
        };
        flight.cancel.cancel();
        self.core.events().send(CoreEvent::SendCancelled {
            request: request.clone(),
            run: flight.run,
        });
        Ok(true)
    }

    /// Whether a request is currently in flight.
    #[must_use]
    pub fn is_sending(&self, request: &Id) -> bool {
        self.core.ports().flight(request).is_some()
    }

    /// Generates a code snippet for a request.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Codegen`] when the target and client pair is not
    /// supported, and [`CoreError::Model`] when the document is not a request.
    pub fn snippet(
        &self,
        doc: &Doc,
        target: heric_codegen::Target,
        client: heric_codegen::Client,
    ) -> Result<String, CoreError> {
        let prepared = self.prepare(doc)?;
        let request = heric_codegen::SnippetRequest::new(&prepared, target, client);
        Ok(heric_codegen::generate(&request, target, client)?)
    }
}

/// Environments and their variables.
#[derive(Debug, Clone)]
pub struct EnvironmentService {
    core: CoreHandle,
}

impl EnvironmentService {
    /// Builds the service. Front ends get one from [`CoreHandle::environments`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Every environment belonging to a workspace.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the store cannot be read.
    pub async fn list(&self, workspace: Id) -> Result<Vec<Doc>, CoreError> {
        self.core
            .documents()
            .of_kind(workspace, DocKind::Environment)
            .await
    }

    /// Creates an environment under a workspace.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the insert fails.
    pub async fn create(&self, workspace: Id, name: String) -> Result<Doc, CoreError> {
        self.core
            .documents()
            .create(
                DocKind::Environment,
                Some(workspace),
                Some(name),
                serde_json::Map::new(),
            )
            .await
    }

    /// Sets one variable in an environment.
    ///
    /// The variables live under the document's `data` object, which is why this
    /// is a read-modify-write rather than a single-field patch: patching `data`
    /// wholesale would let two concurrent edits drop each other's variables.
    ///
    /// # Errors
    ///
    /// Returns C[`StorageError::NotFound`](heric_storage::StorageError::NotFound) when the environment is gone.
    pub async fn set_variable(
        &self,
        environment: Id,
        key: String,
        value: serde_json::Value,
    ) -> Result<Doc, CoreError> {
        let doc = self.get_doc(&environment).await?;
        let mut data = data_object(&doc);
        data.insert(key, value);
        self.core
            .documents()
            .update(
                environment,
                Patch::new().set("data", serde_json::Value::Object(data)),
            )
            .await
    }

    /// Removes one variable from an environment.
    ///
    /// # Errors
    ///
    /// Returns C[`StorageError::NotFound`](heric_storage::StorageError::NotFound) when the environment is gone.
    pub async fn remove_variable(&self, environment: Id, key: String) -> Result<Doc, CoreError> {
        let doc = self.get_doc(&environment).await?;
        let mut data = data_object(&doc);
        data.remove(&key);
        self.core
            .documents()
            .update(
                environment,
                Patch::new().set("data", serde_json::Value::Object(data)),
            )
            .await
    }

    /// The environment's variables, flattened to strings.
    ///
    /// A non-string value is stringified the way the original does it, because
    /// a template can only interpolate text.
    #[must_use]
    pub fn variables(doc: &Doc) -> std::collections::BTreeMap<String, String> {
        data_object(doc)
            .into_iter()
            .map(|(key, value)| {
                let text = match value {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                };
                (key, text)
            })
            .collect()
    }

    /// Reads an environment, failing when it is gone.
    async fn get_doc(&self, id: &Id) -> Result<Doc, CoreError> {
        self.core.documents().get(id.clone()).await?.ok_or_else(|| {
            CoreError::Storage(heric_storage::StorageError::NotFound(
                id.as_str().to_owned(),
            ))
        })
    }

    /// Selects the environment a send resolves against.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::NoWorkspace`] when nothing is open.
    pub fn select(&self, environment: Option<Id>) -> Result<(), CoreError> {
        self.core
            .workspace()
            .set_environment(environment)
            .map(|_| ())
    }
}

/// The cookie jar of a workspace.
///
/// Cookies live in a [`DocKind::CookieJar`] document rather than in engine
/// memory, because the original persists them per workspace and a restart must
/// not silently log the user out (`HTTP-023`).
#[derive(Debug, Clone)]
pub struct CookieService {
    core: CoreHandle,
}

impl CookieService {
    /// Builds the service. Front ends get one from [`CoreHandle::cookies`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// The jar document for a workspace, creating it when absent.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the read or insert fails.
    pub async fn jar(&self, workspace: Id) -> Result<Doc, CoreError> {
        let mut jars = self
            .core
            .documents()
            .of_kind(workspace.clone(), DocKind::CookieJar)
            .await?;
        if let Some(jar) = jars.pop() {
            return Ok(jar);
        }
        self.core
            .documents()
            .create(
                DocKind::CookieJar,
                Some(workspace),
                Some("Cookies".to_owned()),
                serde_json::Map::new(),
            )
            .await
    }

    /// Removes every cookie of a workspace, which is what a privacy action does.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the update fails.
    pub async fn clear(&self, workspace: Id) -> Result<Doc, CoreError> {
        let jar = self.jar(workspace).await?;
        self.core
            .documents()
            .update(
                jar.meta.id,
                Patch::new().set("cookies", serde_json::Value::Array(Vec::new())),
            )
            .await
    }
}

/// Import and the one-time Insomnia migration.
#[derive(Debug, Clone)]
pub struct ImportService {
    core: CoreHandle,
}

impl ImportService {
    /// Builds the service. Front ends get one from [`CoreHandle::imports`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// What the migration would move, without writing anything.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the source cannot be found or read.
    pub async fn dry_run(
        &self,
        source: heric_storage::InsomniaDataDir,
    ) -> Result<heric_storage::DryRunReport, CoreError> {
        self.core
            .run_blocking(move || Ok(heric_storage::dry_run(&source)?))
            .await
    }

    /// Copies an Insomnia data directory into Heric's own store.
    ///
    /// The source is only ever read. Secrets that arrived as Electron
    /// ciphertext are copied verbatim and reported as pending re-entry rather
    /// than decrypted (ADR 0011).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the source cannot be read or the
    /// write fails.
    pub async fn migrate(
        &self,
        source: heric_storage::InsomniaDataDir,
        options: heric_storage::MigrationOptions,
    ) -> Result<heric_storage::MigrationReport, CoreError> {
        let store = Arc::clone(&self.core.ports().store);
        let blobs = Arc::clone(&self.core.ports().blobs);
        self.core
            .run_blocking(move || Ok(heric_storage::migrate(&source, &*store, &*blobs, &options)?))
            .await
    }

    /// Imports a source of an unknown format, detecting it first.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Import`] when the format is not recognised or the
    /// source does not parse.
    pub async fn import(
        &self,
        source: String,
        options: ImportOptions,
    ) -> Result<ImportResult, CoreError> {
        let format = detect_or(&source)?;
        self.core
            .run_blocking(move || Ok(heric_import::import(&source, format, &options)?))
            .await
    }

    /// Writes imported documents to the store.
    ///
    /// Separate from the import so a GUI can preview the count and the warnings
    /// before committing, and a CLI can dry-run.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when an insert fails.
    pub async fn commit(&self, result: ImportResult) -> Result<ImportReport, CoreError> {
        // Inserted as produced, ids and parents included: `create` would mint new
        // ids and leave every `parentId` pointing at a document that was never
        // written.
        for doc in result.documents {
            self.core.documents().insert(doc).await?;
        }
        self.core.events().send(CoreEvent::ImportFinished {
            count: result.report.total(),
            needs_action: result.report.needs_action(),
        });
        Ok(result.report)
    }
}

/// A detected format, or an error naming the source's first characters.
fn detect_or(source: &str) -> Result<Format, CoreError> {
    heric_import::detect(source).ok_or_else(|| {
        CoreError::Import(heric_import::ImportError::UnrecognisedFormat(
            source.chars().take(40).collect(),
        ))
    })
}

/// Export in the supported formats.
#[derive(Debug, Clone)]
pub struct ExportService {
    core: CoreHandle,
}

impl ExportService {
    /// Builds the service. Front ends get one from [`CoreHandle::exports`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Exports documents, returning the files to write.
    ///
    /// The caller writes the files, so a GUI can show a save dialog and a CLI
    /// can stream to stdout. Secrets are redacted unless the caller explicitly
    /// asks for plaintext.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Import`] when there is nothing to export or a
    /// document cannot be represented in the target format.
    pub async fn export(
        &self,
        documents: Vec<Doc>,
        format: ExportFormat,
    ) -> Result<Vec<ExportedFile>, CoreError> {
        self.core
            .run_blocking(move || {
                Ok(heric_import::export(
                    &documents,
                    &ExportOptions::new(format),
                )?)
            })
            .await
    }

    /// Exports with an explicit redaction mode.
    ///
    /// # Errors
    ///
    /// As [`ExportService::export`].
    pub async fn export_with(
        &self,
        documents: Vec<Doc>,
        options: ExportOptions,
        redaction: RedactionMode,
    ) -> Result<Vec<ExportedFile>, CoreError> {
        let options = ExportOptions {
            redaction,
            ..options
        };
        self.core
            .run_blocking(move || Ok(heric_import::export(&documents, &options)?))
            .await
    }
}

/// Script execution outside a send, for the editor's run button.
#[derive(Debug, Clone)]
pub struct ScriptService {
    core: CoreHandle,
}

impl ScriptService {
    /// Builds the service. Front ends get one from [`CoreHandle::scripts`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// The sandbox rules this build applies.
    #[must_use]
    pub fn policy(&self) -> &SecurityPolicy {
        &self.core.ports().policy
    }

    /// Runs a script against a request, for the editor's console.
    ///
    /// QuickJS is a blocking engine, so the run hops to the blocking pool and is
    /// interrupted by the engine's own deadline rather than by the runtime.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Script`] when the sandbox refuses the source, the
    /// script throws, or it overruns [`heric_script::DEFAULT_TIMEOUT`].
    pub async fn run(
        &self,
        source: String,
        request: Doc,
        phase: ScriptPhase,
        mut environment: std::collections::BTreeMap<String, String>,
        mut globals: std::collections::BTreeMap<String, String>,
    ) -> Result<ScriptOutcome, CoreError> {
        let policy = self.core.ports().policy.clone();
        let info = heric_script::EventInfo {
            event_name: phase.as_str().to_owned(),
            iteration: 1,
            iteration_count: 1,
            request_name: request.meta.name.clone().unwrap_or_default(),
            request_id: request.meta.id.clone(),
        };
        self.core
            .run_blocking(move || {
                let mut base_environment = environment.clone();
                let mut collection_variables = std::collections::BTreeMap::new();
                let mut base_globals = globals.clone();
                let mut iteration = std::collections::BTreeMap::new();
                let mut context = ScriptContext {
                    request: &request,
                    response: None,
                    environment: &mut environment,
                    base_environment: &mut base_environment,
                    collection_variables: &mut collection_variables,
                    globals: &mut globals,
                    base_globals: &mut base_globals,
                    iteration_data: &mut iteration,
                    info: &info,
                    phase,
                };
                Ok(heric_script::run(&source, &mut context, &policy)?)
            })
            .await
    }

    /// Applies a script's patches to a document.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Script`] when a patch cannot be applied.
    pub fn apply(&self, outcome: &ScriptOutcome, doc: &mut Doc) -> Result<Vec<Patch>, CoreError> {
        Ok(heric_script::apply(outcome, doc)?)
    }
}

/// The collection runner.
#[derive(Debug, Clone)]
pub struct RunnerService {
    core: CoreHandle,
}

impl RunnerService {
    /// Builds the service. Front ends get one from [`CoreHandle::runner`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Runs a folder's requests, one iteration per row of iteration data.
    ///
    /// Returns the assertions the run produced, which is what the runner's
    /// results pane lists.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the requests cannot be read and
    /// [`CoreError::Script`] when a script fails hard.
    pub async fn run(
        &self,
        folder: Id,
        data: Option<IterationData>,
    ) -> Result<Vec<TestResult>, CoreError> {
        let _ = (&self.core, folder, data);
        todo!("M7: run a folder's requests over the iteration data")
    }

    /// Every request a run would execute, in order.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the tree cannot be read.
    pub async fn plan(&self, folder: Id) -> Result<Vec<Id>, CoreError> {
        let _ = (&self.core, folder);
        todo!("M7: flatten a folder into the request order")
    }
}

/// The settings singleton.
#[derive(Debug, Clone)]
pub struct SettingsService {
    core: CoreHandle,
}

impl SettingsService {
    /// Builds the service. Front ends get one from [`CoreHandle::settings`].
    #[must_use]
    pub fn new(core: CoreHandle) -> Self {
        Self { core }
    }

    /// The handle this service works through.
    #[must_use]
    pub fn core(&self) -> &CoreHandle {
        &self.core
    }

    /// Reads the settings document, creating it when absent.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the read or insert fails.
    pub async fn get(&self) -> Result<Doc, CoreError> {
        if let Some(doc) = self.core.documents().get(Id::new(SETTINGS_ID)).await? {
            return Ok(doc);
        }
        let doc = Doc {
            meta: heric_model::DocMeta {
                id: Id::new(SETTINGS_ID),
                kind: DocKind::Settings,
                parent: None,
                modified: crate::workspace::now_millis(),
                created: crate::workspace::now_millis(),
                private: None,
                name: None,
            },
            body: serde_json::Map::new(),
        };
        self.core.documents().insert(doc).await
    }

    /// Changes one setting.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the update fails.
    pub async fn set(&self, key: String, value: serde_json::Value) -> Result<Doc, CoreError> {
        self.core
            .documents()
            .update(Id::new(SETTINGS_ID), Patch::new().set(key, value))
            .await
    }
}

/// A script's console output as timeline entries.
///
/// The Console pane shows logs and script output in one stream, so the script's
/// `console.log` lines have to become timeline entries rather than a separate
/// list.
#[must_use]
pub fn console_entries(entries: &[heric_script::ConsoleEntry]) -> Vec<TimelineEntry> {
    entries
        .iter()
        .map(|entry| TimelineEntry {
            at: entry.at,
            kind: TimelineKind::Text,
            text: format!("[{}] {}", entry.level.as_str(), entry.text),
        })
        .collect()
}

/// A stop reason as the user should read it.
#[must_use]
pub fn describe_stop(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::SkippedByScript => "skipped by a pre-request script",
        StopReason::PendingCredential { .. } => "a credential must be entered again",
        StopReason::Cancelled => "cancelled",
        StopReason::ScriptFailed => "a pre-request script failed",
    }
}

/// The auth method a request uses, if any.
///
/// # Errors
///
/// Returns [`CoreError::Auth`] when the auth block is not an object.
pub fn auth_method(doc: &Doc) -> Result<Option<AuthMethod>, CoreError> {
    Ok(heric_auth::detect(doc)?)
}

/// The secret reference for a credential in a document.
#[must_use]
pub fn secret_ref(doc: &Doc, field: &str) -> SecretRef {
    SecretRef {
        doc: doc.meta.id.clone(),
        field: field.to_owned(),
        service: "heric".to_owned(),
    }
}

/// Applies a method to a request using the core's providers.
///
/// # Errors
///
/// Returns [`CoreError::Auth`] when the provider needs a credential the resolver
/// cannot supply.
pub fn apply_auth(
    core: &CoreHandle,
    method: &AuthMethod,
    request: &PreparedRequest,
    doc: &Doc,
) -> Result<heric_auth::AuthOutcome, CoreError> {
    let secrets = Arc::clone(&core.ports().secrets);
    let reference = secret_ref(doc, "authentication");
    let context = AuthContext {
        request,
        doc,
        secrets: &*secrets,
        secret_ref: &reference,
    };
    Ok(core.ports().auth.apply(method, &context)?)
}

/// Whether a plan will run scripts, so the UI can hide a column when it will
/// not.
#[must_use]
pub fn plan_runs_scripts(plan: &ExecutionPlan) -> bool {
    plan.includes(PlanStage::PreRequestScript) || plan.includes(PlanStage::AfterResponseScript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_id_is_stable() {
        assert_eq!(SETTINGS_ID, "set_heric");
    }

    #[test]
    fn a_stop_reason_has_a_readable_description() {
        let reason = StopReason::PendingCredential {
            field: "token".to_owned(),
        };
        assert!(!describe_stop(&reason).is_empty());
    }

    #[test]
    fn console_entries_become_timeline_entries() {
        let entries = [heric_script::ConsoleEntry {
            level: heric_script::ConsoleLevel::Log,
            text: "hello".to_owned(),
            at: std::time::Duration::from_millis(3),
        }];
        let timeline = console_entries(&entries);
        assert_eq!(timeline.len(), 1);
        assert!(timeline[0].text.contains("hello"));
        assert_eq!(timeline[0].kind, TimelineKind::Text);
    }

    #[test]
    fn a_secret_reference_names_the_document_and_field() -> Result<(), Box<dyn std::error::Error>> {
        let doc: Doc = serde_json::from_value(serde_json::json!({
            "_id": "req_1",
            "type": "Request"
        }))?;
        let reference = secret_ref(&doc, "token");
        assert_eq!(reference.field, "token");
        assert_eq!(reference.doc, Id::new("req_1"));
        Ok(())
    }
}
