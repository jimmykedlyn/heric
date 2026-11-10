//! Heric desktop application: the egui front end.
//!
//! The UI thread never blocks. Request execution and storage run on a
//! background tokio runtime inside `heric-core`; results arrive as events
//! over channels and trigger `egui::Context::request_repaint`.

use egui::{CentralPanel, Panel};

/// Root application state for the UI.
#[derive(Debug, Default)]
struct HericApp {
    request_url: String,
    response_body: String,
}

impl eframe::App for HericApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Heric");
                ui.separator();
                ui.label("Native API client");
            });
        });

        Panel::left("requests").show(ui, |ui| {
            ui.heading("Collections");
            ui.label("No workspace loaded yet.");
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
        Box::new(|_cc| Ok(Box::<HericApp>::default())),
    )?;
    Ok(())
}
