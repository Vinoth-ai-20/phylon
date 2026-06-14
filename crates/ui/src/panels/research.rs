use egui::{Color32, RichText, Ui};

pub fn render_research(ui: &mut Ui, script_path: &mut String, load_script: &mut bool) {
    ui.heading("Research & Plugins");
    ui.label(".rhai script scripts:");

    ui.text_edit_singleline(script_path);

    if ui
        .button(RichText::new("✦ Load & Run").size(16.0))
        .clicked()
    {
        *load_script = true;
    }

    ui.separator();
    ui.label(RichText::new("Script Output Log").color(Color32::from_rgb(150, 150, 170)));
    // In a real implementation this would show the output log
}
