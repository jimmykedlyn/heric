# 0009 — GUI concurrency

- Status: accepted
- Date: 2026-11-10

## Context

egui is an immediate-mode UI: it redraws only when asked to, and a blocking
call inside the update loop freezes the window. The application nevertheless
needs to do the opposite of that — HTTP transfers that last minutes, JS
scripts with time budgets, SQLite writes, Git operations — while remaining
responsive and cancellable.

The original solves this with Electron's process model: the renderer holds the
UI, a hidden window runs scripts, and the main process owns libcurl and the
database, all joined by IPC. Heric has threads, not processes.

## Options

1. **One egui thread plus a background tokio runtime**, with an unbounded
   channel of owned events into the UI and an unbounded channel of commands
   the other way.
2. **`async` egui**: run the runtime on the same thread and poll it between
   frames. Requires every future to be cooperatively polled by the UI thread,
   which makes a slow frame a stalled transfer.
3. **Worker threads per pane**, each owning its own state and communicating
   with the UI directly.

## Decision

Option 1.

- One UI thread runs `eframe::run_native`. It never awaits and never blocks.
- One multi-thread tokio runtime, created before the window and owned by
  `heric-core`. All I/O runs there.
- Two channels: `Command` into the core, `Event` out of it. Both carry owned
  data, because a consumer outliving a producer rules out borrowed payloads.
- Both channels are **unbounded**. A bounded channel would make the producer
  await, and a task blocked on a full queue while the UI sleeps deadlocks the
  repaint it is waiting for. Volume is controlled at the source: timeline
  chunks are coalesced before they are emitted.
- `CoreConfig` takes a `repaint: Arc<dyn Fn() + Send + Sync>`. The core calls
  it **after** pushing the event, never before; the opposite order loses
  frames when the UI is idle. The UI also requests a repaint while any
  operation is in flight so progress animation continues.
- Every operation that can fail reports through an `Event`, never through a
  `Result` the UI polls, because by the time it can fail it has already left
  the UI thread.
- Synchronous ports (`Store`, `BlobStore`, `HttpEngine`, `StreamingEngine`,
  `TagHost`, `ScriptHost`, `Connection`) are called from
  `tokio::task::spawn_blocking`. libcurl and SQLite are blocking C APIs, and an
  rquickjs `Runtime` must not cross an `.await`. A blocked port therefore holds
  a blocking-pool thread, never a runtime worker or the UI thread.
- Cancellation crosses threads through libcurl's progress callback reading an
  `Arc<AtomicBool>`, not through dropping a future. An aborted send is a
  distinct outcome, not an error.

## Consequences

- The GUI's state machine is explicit: it sends commands and consumes events,
  which also makes it testable with `egui_kittest` without a real network.
- Backpressure is sacrificed deliberately. A wedged consumer grows the queue;
  the mitigation is coalescing at the emitter and a `tracing` warning when the
  queue depth crosses a threshold.
- `TagHost::prompt` — the one port that waits on a person — runs on the
  blocking pool with a timeout. A closed window or a headless CLI run returns
  `Timeout` instead of hanging a send.
- `Store::changes()` returns a `tokio::sync::broadcast::Receiver`. That is the
  only place tokio leaks into a trait signature, and it is deliberate: the
  consumer is a GUI frame loop and a second channel type would earn nothing.
- Moving to a different UI toolkit later would not change this ADR; only the
  repaint callback and the frame loop are egui-specific.
