//! Heric desktop application: the egui front end.
//!
//! The UI thread never blocks. Request execution and storage run on a
//! background tokio runtime inside `heric-core`; results arrive as events
//! over channels and trigger `egui::Context::request_repaint`.

mod events;

use std::sync::Arc;

use egui::{CentralPanel, Panel};
use heric_core::{CoreEvent, EventSink, RepaintSignal};

use crate::events::EventBridge;

/// How many console lines the window keeps.
///
/// Bounded so a long session cannot grow the UI's memory without limit; the
/// core's event channel itself carries only what is in flight.
const CONSOLE_LINES: usize = 200;

/// Root application state for the UI.
struct HericApp {
    request_url: String,
    response_body: String,
    bridge: EventBridge,
    sink: EventSink,
    console: Vec<String>,
}

impl HericApp {
    fn new(bridge: EventBridge, sink: EventSink) -> Self {
        Self {
            request_url: String::new(),
            response_body: String::new(),
            bridge,
            sink,
            console: Vec::new(),
        }
    }

    /// Drains what the core emitted since the last frame.
    fn absorb_events(&mut self) {
        for event in self.bridge.drain() {
            let line = describe(event);
            if self.console.len() == CONSOLE_LINES {
                self.console.remove(0);
            }
            self.console.push(line);
        }
    }
}

/// One line for a console entry.
///
/// Exhaustive on purpose: a new `CoreEvent` variant must be given a rendering
/// rather than silently vanishing from the window.
fn describe(event: &CoreEvent) -> String {
    match event {
        CoreEvent::Changed { change } => format!("document changed: {change:?}"),
        CoreEvent::SendStarted { request, run } => format!("send started: {request} (run {run})"),
        CoreEvent::Timeline {
            request,
            run,
            entry,
        } => format!("{request} (run {run}): {}", entry.text),
        CoreEvent::SendFinished {
            request,
            run,
            outcome,
        } => match outcome.status() {
            Some(status) => format!("{request} (run {run}) finished with {status}"),
            None => format!("{request} (run {run}) finished without a response"),
        },
        CoreEvent::SendCancelled { request, run } => {
            format!("{request} (run {run}) cancelled")
        }
        CoreEvent::SessionChanged { session, detail } => {
            format!(
                "session {} ({}): {detail}",
                session.id.0,
                session.kind.as_str()
            )
        }
        CoreEvent::SessionMessage { session, text } => {
            format!("session {} message: {text}", session.id.0)
        }
        CoreEvent::ImportFinished {
            count,
            needs_action,
        } => {
            let suffix = if *needs_action {
                " — some items need your attention"
            } else {
                ""
            };
            format!("import finished: {count} documents{suffix}")
        }
        CoreEvent::Failed { message, fatal } => {
            if *fatal {
                format!("error: {message}")
            } else {
                format!("warning: {message}")
            }
        }
        CoreEvent::Shutdown => "core stopped".to_owned(),
    }
}

impl eframe::App for HericApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.absorb_events();

        Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Heric");
                ui.separator();
                ui.label("Native API client");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = if self.bridge.is_open() {
                        "core connected"
                    } else {
                        "core stopped"
                    };
                    ui.label(status);
                });
            });
        });

        Panel::left("requests").show(ui, |ui| {
            ui.heading("Collections");
            ui.label("No workspace loaded yet.");
        });

        Panel::bottom("console").show(ui, |ui| {
            ui.heading("Console");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &self.console {
                        ui.label(line);
                    }
                });
        });

        CentralPanel::default().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("URL");
                ui.add(
                    egui::TextEdit::singleline(&mut self.request_url)
                        .hint_text("https://example.com")
                        .desired_width(f32::INFINITY),
                );
            });
            ui.separator();
            ui.heading("Response");
            ui.add(
                egui::TextEdit::multiline(&mut self.response_body)
                    .desired_width(f32::INFINITY)
                    .desired_rows(12),
            );
        });
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 720.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Heric"),
        ..Default::default()
    };

    eframe::run_native(
        "Heric",
        options,
        Box::new(|cc| {
            // The one link from the core back to the toolkit: the sink calls
            // this after enqueueing, never before (ADR 0009).
            let ctx = cc.egui_ctx.clone();
            let repaint: RepaintSignal = Arc::new(move || ctx.request_repaint());
            let (sink, bridge) = events::channel(repaint);
            let mut app = HericApp::new(bridge, sink);
            // The store and the engine arrive in M1; until then the window
            // says so instead of showing an empty collection tree with no
            // explanation.
            app.sink.send(CoreEvent::Failed {
                message: "the store and the HTTP engine arrive in M1; no request can be sent yet"
                    .to_owned(),
                fatal: false,
            });
            app.absorb_events();
            Ok(Box::new(app))
        }),
    )?;
    Ok(())
}
