//! The core object: one tokio runtime, one set of ports, one event sink.
//!
//! The GUI holds a [`CoreHandle`], never the [`Core`]: the handle is cheap to
//! clone, is `Send + Sync`, and cannot shut the runtime down by being dropped.
//! Only the application's `main` owns the `Core`, and dropping it is the
//! deliberate act that stops every background task.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use heric_auth::{MemorySecrets, ProviderRegistry, SecretResolverHandle};
use heric_http::{CancelToken, HttpEngine};
use heric_model::Id;
use heric_script::SecurityPolicy;
use heric_storage::{BlobStoreHandle, DataPaths, StoreHandle};

use crate::error::CoreError;
use crate::events::{CoreEvent, EventReceiver, EventSink, RepaintSignal, no_repaint};
use crate::workspace::ActiveContext;

/// A send in flight, so the UI can cancel it.
#[derive(Debug, Clone)]
pub(crate) struct Flight {
    /// Which run this flight belongs to, so a cancel cannot hit a later run of
    /// the same request.
    pub(crate) run: u64,
    /// The token the engine polls.
    pub(crate) cancel: CancelToken,
}

/// Everything the services share.
///
/// One struct rather than a field per service so a new port is added in one
/// place and no service silently holds a stale copy.
pub(crate) struct Ports {
    /// The document store.
    pub(crate) store: StoreHandle,
    /// Response bodies and timelines.
    pub(crate) blobs: BlobStoreHandle,
    /// The HTTP engine.
    ///
    /// Held for the send pipeline, which is still a Phase 1 stub.
    pub(crate) engine: Arc<dyn HttpEngine>,
    /// The registered auth providers.
    pub(crate) auth: ProviderRegistry,
    /// Where credentials come from.
    pub(crate) secrets: SecretResolverHandle,
    /// The sandbox rules scripts run under.
    pub(crate) policy: SecurityPolicy,
    /// Which workspace and environment are active.
    ///
    /// Lives here rather than in the UI because a send started from the CLI and
    /// a send started from a tab must resolve the same environment.
    pub(crate) active: RwLock<Option<ActiveContext>>,
    /// Sends that can still be cancelled, keyed by request.
    ///
    /// One entry per request: starting a second send of the same request
    /// replaces the entry, and the older run notices its token is stale because
    /// its run number no longer matches.
    pub(crate) flights: Mutex<HashMap<Id, Flight>>,
}

impl Ports {
    /// Registers a send as cancellable.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Task`] when the mutex is poisoned, which means a
    /// thread panicked while holding it.
    pub(crate) fn begin_flight(
        &self,
        request: Id,
        run: u64,
        cancel: CancelToken,
    ) -> Result<(), CoreError> {
        let mut flights = self
            .flights
            .lock()
            .map_err(|error| CoreError::Task(error.to_string()))?;
        flights.insert(request, Flight { run, cancel });
        Ok(())
    }

    /// Removes a send's entry, but only when it is still the same run.
    ///
    /// The run check matters: without it, a slow run finishing would erase the
    /// entry of a newer run of the same request and make it uncancellable.
    ///
    /// The send pipeline that calls this is still a Phase 1 stub.
    pub(crate) fn end_flight(&self, request: &Id, run: u64) {
        if let Ok(mut flights) = self.flights.lock()
            && flights.get(request).is_some_and(|flight| flight.run == run)
        {
            flights.remove(request);
        }
    }

    /// The run currently in flight for a request.
    #[must_use]
    pub(crate) fn flight(&self, request: &Id) -> Option<Flight> {
        self.flights
            .lock()
            .ok()
            .and_then(|flights| flights.get(request).cloned())
    }
}

/// Counts runs so a stale event can be dropped.
///
/// A response that arrives after the user pressed send again belongs to the old
/// run, and showing it would replace a newer response with an older one.
#[derive(Debug, Default)]
struct RunCounter(AtomicU64);

impl RunCounter {
    /// The next run number, starting at 1 so `0` can mean "no run".
    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// Configures a [`Core`].
///
/// Every port has a default so the CLI needs one line, and every port can be
/// replaced so a test can run the whole pipeline against a fake engine.
#[derive(Clone)]
pub struct CoreBuilder {
    paths: DataPaths,
    repaint: RepaintSignal,
    engine: Option<Arc<dyn HttpEngine>>,
    secrets: Option<SecretResolverHandle>,
    store: Option<StoreHandle>,
    blobs: Option<BlobStoreHandle>,
    worker_threads: Option<usize>,
}

impl CoreBuilder {
    /// A builder rooted at Heric's data directory.
    #[must_use]
    pub fn new(paths: DataPaths) -> Self {
        Self {
            paths,
            repaint: no_repaint(),
            engine: None,
            secrets: None,
            store: None,
            blobs: None,
            worker_threads: None,
        }
    }

    /// Sets the callback that asks the UI to redraw.
    ///
    /// The GUI passes a closure calling `egui::Context::request_repaint`. The
    /// CLI and the tests keep the no-op default.
    #[must_use]
    pub fn with_repaint(mut self, repaint: RepaintSignal) -> Self {
        self.repaint = repaint;
        self
    }

    /// Replaces the HTTP engine.
    ///
    /// Used by tests to run the pipeline without a socket, and by the CLI to
    /// share one engine across a collection run.
    #[must_use]
    pub fn with_http_engine(mut self, engine: Arc<dyn HttpEngine>) -> Self {
        self.engine = Some(engine);
        self
    }

    /// Sets where credentials come from.
    ///
    /// **Risk:** when this is not called the core uses an in-memory resolver.
    /// The session works, and every secret is gone at exit. That default exists
    /// so a test or a one-shot CLI call needs no keychain; the GUI must always
    /// pass the OS keychain resolver at startup (ADR 0011).
    #[must_use]
    pub fn with_secrets(mut self, secrets: SecretResolverHandle) -> Self {
        self.secrets = Some(secrets);
        self
    }

    /// Uses an already-open store, for a second window or a test.
    #[must_use]
    pub fn with_store(mut self, store: StoreHandle) -> Self {
        self.store = Some(store);
        self
    }

    /// Uses an already-open blob store.
    #[must_use]
    pub fn with_blobs(mut self, blobs: BlobStoreHandle) -> Self {
        self.blobs = Some(blobs);
        self
    }

    /// Overrides the runtime's worker thread count.
    #[must_use]
    pub fn with_worker_threads(mut self, threads: usize) -> Self {
        self.worker_threads = Some(threads);
        self
    }

    /// The data directory this core is rooted at.
    #[must_use]
    pub fn paths(&self) -> &DataPaths {
        &self.paths
    }

    /// Starts the runtime and every port.
    ///
    /// Returns the receiver alongside the core because there is exactly one UI:
    /// the channel is created here and handed over once, rather than being
    /// reachable from the handle where a second caller could drain the UI's
    /// events.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] when the database or blob directory cannot
    /// be opened, [`CoreError::Http`] when the engine cannot be built, and
    /// [`CoreError::Task`] when the runtime cannot start.
    pub fn build(self) -> Result<(Core, EventReceiver), CoreError> {
        let store = match self.store {
            Some(store) => store,
            None => heric_storage::open(&self.paths)?,
        };
        let blobs = match self.blobs {
            Some(blobs) => blobs,
            None => heric_storage::open_blobs(&self.paths)?,
        };
        let engine: Arc<dyn HttpEngine> = match self.engine {
            Some(engine) => engine,
            None => Arc::from(heric_http::engine()?),
        };
        let secrets = self.secrets.unwrap_or_else(ephemeral_secrets);
        let auth = heric_auth::providers()?;
        let threads = self
            .worker_threads
            .unwrap_or_else(crate::default_worker_threads);

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(threads)
            .enable_all()
            .thread_name("heric-worker")
            .build()
            .map_err(|error| CoreError::Task(error.to_string()))?;

        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let sink = EventSink::new(sender, self.repaint);
        let ports = Arc::new(Ports {
            store,
            blobs,
            engine,
            auth,
            secrets,
            policy: SecurityPolicy::strict(),
            active: RwLock::new(None),
            flights: Mutex::new(HashMap::new()),
        });
        let handle = CoreHandle {
            runtime: runtime.handle().clone(),
            ports,
            sink,
            runs: Arc::new(RunCounter::default()),
        };

        Ok((Core { runtime, handle }, receiver))
    }
}

/// The application core.
///
/// Owns the runtime. Dropping it stops every task, so it lives as long as the
/// process does.
pub struct Core {
    /// Owned so dropping `Core` stops the runtime.
    runtime: tokio::runtime::Runtime,
    handle: CoreHandle,
}

impl Core {
    /// Starts the core from a builder.
    ///
    /// # Errors
    ///
    /// As [`CoreBuilder::build`].
    pub fn start(builder: CoreBuilder) -> Result<(Self, EventReceiver), CoreError> {
        builder.build()
    }

    /// A handle for the services and the UI.
    #[must_use]
    pub fn handle(&self) -> CoreHandle {
        self.handle.clone()
    }

    /// Tells the UI the core is going away, then stops the runtime.
    ///
    /// Dropping a multi-thread runtime blocks until every blocking task
    /// finishes, so this can take as long as the slowest in-flight send. It is
    /// therefore called from `main`, after the window has closed, never from
    /// the UI's teardown path.
    pub fn shutdown(self) {
        self.handle.events().send(CoreEvent::Shutdown);
        // Dropping the runtime waits for in-flight blocking work (libcurl,
        // SQLite, Git) to finish, so exiting never cuts a write in half. The
        // sink must go first, or a task finishing now enqueues into a channel
        // whose reader is already gone.
        drop(self.handle);
        drop(self.runtime);
    }
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

/// A cheap, clonable handle to the core.
///
/// The only way a front end reaches a port. Cloning is an `Arc` bump, and
/// dropping a clone never stops the runtime.
#[derive(Clone)]
pub struct CoreHandle {
    runtime: tokio::runtime::Handle,
    ports: Arc<Ports>,
    sink: EventSink,
    runs: Arc<RunCounter>,
}

impl CoreHandle {
    /// The shared ports. Crate-internal: front ends go through a service.
    #[must_use]
    pub(crate) fn ports(&self) -> &Ports {
        &self.ports
    }

    /// Where events go.
    #[must_use]
    pub fn events(&self) -> &EventSink {
        &self.sink
    }

    /// The engine every send goes through.
    ///
    /// Exposed because two callers outside the send pipeline need it: the GUI's
    /// stream views open the same engine the send path uses, and `heric-cli`
    /// reports it in `--version --verbose`.
    #[must_use]
    pub fn http_engine(&self) -> Arc<dyn heric_http::HttpEngine> {
        Arc::clone(&self.ports.engine)
    }

    /// The runtime, for a caller that needs to spawn its own task.
    #[must_use]
    pub fn runtime(&self) -> tokio::runtime::Handle {
        self.runtime.clone()
    }

    /// The next run number.
    #[must_use]
    pub(crate) fn next_run(&self) -> u64 {
        self.runs.next()
    }

    /// Spawns a task on the runtime.
    ///
    /// The task is detached. Anything the UI must learn about has to go through
    /// [`CoreHandle::events`], because the UI never awaits it.
    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(future)
    }

    /// Runs a blocking port on the blocking pool.
    ///
    /// Every synchronous port (libcurl, SQLite, Git, QuickJS) goes through here.
    /// A join failure means the task panicked, which is reported as
    /// [`CoreError::Task`] rather than propagated, so one panicking request does
    /// not take the window down with it.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Task`] when the blocking task panics, and whatever
    /// the work itself returns otherwise.
    pub async fn run_blocking<T, F>(&self, work: F) -> Result<T, CoreError>
    where
        F: FnOnce() -> Result<T, CoreError> + Send + 'static,
        T: Send + 'static,
    {
        self.runtime
            .spawn_blocking(work)
            .await
            .map_err(|error| CoreError::Task(error.to_string()))?
    }

    /// Blocks the calling thread on a future.
    ///
    /// **Only the CLI may call this.** Calling it on the GUI thread freezes the
    /// window for as long as the future takes, which is the one thing the
    /// concurrency model exists to prevent. It also panics when called from
    /// inside a runtime, which is how that mistake surfaces immediately in a
    /// test instead of as a frozen window in production.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    /// Whether the UI has stopped listening.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.sink.is_closed()
    }

    /// Documents: read, create, patch, remove.
    #[must_use]
    pub fn documents(&self) -> crate::DocumentService {
        crate::DocumentService::new(self.clone())
    }

    /// Workspaces: list, open, close, create.
    #[must_use]
    pub fn workspace(&self) -> crate::WorkspaceService {
        crate::WorkspaceService::new(self.clone())
    }

    /// Requests: prepare, send, cancel.
    #[must_use]
    pub fn requests(&self) -> crate::RequestService {
        crate::RequestService::new(self.clone())
    }

    /// Environments and their variables.
    #[must_use]
    pub fn environments(&self) -> crate::EnvironmentService {
        crate::EnvironmentService::new(self.clone())
    }

    /// Cookies per workspace.
    #[must_use]
    pub fn cookies(&self) -> crate::CookieService {
        crate::CookieService::new(self.clone())
    }

    /// Import, export and the Insomnia migration.
    #[must_use]
    pub fn imports(&self) -> crate::ImportService {
        crate::ImportService::new(self.clone())
    }

    /// Export in the supported formats.
    #[must_use]
    pub fn exports(&self) -> crate::ExportService {
        crate::ExportService::new(self.clone())
    }

    /// Script execution outside a send, for the editor's run button.
    #[must_use]
    pub fn scripts(&self) -> crate::ScriptService {
        crate::ScriptService::new(self.clone())
    }

    /// The collection runner.
    #[must_use]
    pub fn runner(&self) -> crate::RunnerService {
        crate::RunnerService::new(self.clone())
    }

    /// The settings singleton.
    #[must_use]
    pub fn settings(&self) -> crate::SettingsService {
        crate::SettingsService::new(self.clone())
    }

    /// Live sessions: WebSocket, Socket.IO, gRPC streams, event streams.
    #[must_use]
    pub fn sessions(&self) -> crate::SessionService {
        crate::SessionService::new(self.clone())
    }
}

impl std::fmt::Debug for CoreHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreHandle")
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

/// The default secret resolver, used when none is injected.
pub(crate) fn ephemeral_secrets() -> SecretResolverHandle {
    Arc::new(MemorySecrets::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_numbers_start_at_one() {
        let counter = RunCounter::default();
        assert_eq!(counter.next(), 1);
    }

    #[test]
    fn run_numbers_are_monotonic() {
        let counter = RunCounter::default();
        let first = counter.next();
        let second = counter.next();
        assert!(second > first);
        assert_eq!(second, first + 1);
    }

    #[test]
    fn the_ephemeral_resolver_can_write() {
        assert!(ephemeral_secrets().can_write());
    }

    #[test]
    fn a_flight_can_be_cancelled_by_request() -> Result<(), Box<dyn std::error::Error>> {
        let ports = test_ports();
        let token = CancelToken::new();
        ports.begin_flight(Id::new("req_1"), 7, token.clone())?;
        let flight = ports
            .flight(&Id::new("req_1"))
            .ok_or("flight must be registered")?;
        assert_eq!(flight.run, 7);
        flight.cancel.cancel();
        assert!(token.is_cancelled());
        ports.end_flight(&Id::new("req_1"), 7);
        assert!(ports.flight(&Id::new("req_1")).is_none());
        Ok(())
    }

    #[test]
    fn a_stale_run_does_not_erase_a_newer_flight() -> Result<(), Box<dyn std::error::Error>> {
        let ports = test_ports();
        ports.begin_flight(Id::new("req_2"), 2, CancelToken::new())?;
        ports.end_flight(&Id::new("req_2"), 1);
        assert!(
            ports.flight(&Id::new("req_2")).is_some(),
            "an older run finishing must not cancel the newer one"
        );
        Ok(())
    }

    #[test]
    fn ending_an_unknown_flight_is_harmless() {
        let ports = test_ports();
        ports.end_flight(&Id::new("req_missing"), 1);
        assert!(ports.flight(&Id::new("req_missing")).is_none());
    }

    /// A `Ports` with the real types but no I/O.
    fn test_ports() -> Ports {
        struct NoEngine;

        impl HttpEngine for NoEngine {
            fn execute(
                &self,
                _request: &heric_model::PreparedRequest,
                _options: &heric_http::ExecuteOptions,
            ) -> Result<heric_model::Execution, heric_http::HttpError> {
                Err(heric_http::HttpError::Unsupported)
            }
        }

        struct NoStore;

        impl heric_storage::Store for NoStore {
            fn get(
                &self,
                _id: &Id,
            ) -> Result<Option<heric_model::Doc>, heric_storage::StorageError> {
                Ok(None)
            }

            fn find(
                &self,
                _query: &heric_model::Query,
            ) -> Result<Vec<heric_model::Doc>, heric_storage::StorageError> {
                Ok(Vec::new())
            }

            fn insert(
                &self,
                doc: &heric_model::Doc,
            ) -> Result<heric_model::Doc, heric_storage::StorageError> {
                Ok(doc.clone())
            }

            fn update(
                &self,
                id: &Id,
                _patch: &heric_model::Patch,
            ) -> Result<heric_model::Doc, heric_storage::StorageError> {
                Err(heric_storage::StorageError::NotFound(
                    id.as_str().to_owned(),
                ))
            }

            fn remove(&self, _id: &Id) -> Result<Vec<Id>, heric_storage::StorageError> {
                Ok(Vec::new())
            }

            fn changes(&self) -> tokio::sync::broadcast::Receiver<heric_model::Change> {
                let (sender, receiver) = tokio::sync::broadcast::channel(1);
                drop(sender);
                receiver
            }
        }

        struct NoBlobs;

        impl heric_storage::BlobStore for NoBlobs {
            fn put(
                &self,
                _id: &heric_storage::BlobId,
                _bytes: &[u8],
            ) -> Result<(), heric_storage::StorageError> {
                Ok(())
            }

            fn get(
                &self,
                _id: &heric_storage::BlobId,
            ) -> Result<Option<Vec<u8>>, heric_storage::StorageError> {
                Ok(None)
            }

            fn stream(
                &self,
                id: &heric_storage::BlobId,
            ) -> Result<Box<dyn std::io::Read + Send>, heric_storage::StorageError> {
                Err(heric_storage::StorageError::NotFound(
                    id.owner.as_str().to_owned(),
                ))
            }

            fn remove(
                &self,
                _id: &heric_storage::BlobId,
            ) -> Result<(), heric_storage::StorageError> {
                Ok(())
            }
        }

        Ports {
            store: Arc::new(NoStore),
            blobs: Arc::new(NoBlobs),
            engine: Arc::new(NoEngine),
            auth: ProviderRegistry::new(),
            secrets: ephemeral_secrets(),
            policy: SecurityPolicy::strict(),
            active: RwLock::new(None),
            flights: Mutex::new(HashMap::new()),
        }
    }
}
