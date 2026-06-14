use egui::{Color32, RichText, Ui};

pub fn render_system_logs(ui: &mut Ui, logs: &[String]) {
    ui.heading("System Logs");

    // Three column headers: Structured tracing | Output | Motion Value
    egui::Grid::new("system_logs_header")
        .num_columns(3)
        .show(ui, |ui| {
            ui.label(
                RichText::new("Structured tracing")
                    .strong()
                    .color(Color32::from_rgb(150, 150, 170)),
            );
            ui.label(
                RichText::new("Output")
                    .strong()
                    .color(Color32::from_rgb(150, 150, 170)),
            );
            ui.label(
                RichText::new("Motion Value")
                    .strong()
                    .color(Color32::from_rgb(150, 150, 170)),
            );
            ui.end_row();
        });

    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for log in logs {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("[Structured Tracing output]")
                            .color(Color32::from_rgb(100, 100, 120)),
                    );
                    ui.label(RichText::new(log).color(Color32::WHITE));
                });
            }
        });
}
