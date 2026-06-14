use common::Tick;
use egui::Ui;

pub fn render_timeline(ui: &mut Ui, tick: Tick, speed: &mut f32, is_paused: &mut bool) {
    ui.heading("Timeline & Replay");

    ui.horizontal(|ui| {
        if ui.button("◄◄").clicked() {
            // Seek to start
        }
        if ui.button("◄").clicked() {
            // Step back (not implemented in engine yet)
        }
        let play_pause_icon = if *is_paused { "▶" } else { "⏸" };
        if ui.button(play_pause_icon).clicked() {
            *is_paused = !*is_paused;
        }
        if ui.button("■").clicked() {
            // Stop / Reset
        }

        let mut t = tick.0 as f32;
        ui.add(egui::Slider::new(&mut t, 0.0..=10000.0).show_value(true));

        ui.add_space(20.0);

        ui.label("Speed");
        if ui.button("[-]").clicked() {
            *speed = (*speed - 0.1).max(0.1);
        }
        ui.add(egui::Slider::new(speed, 0.1..=10.0).show_value(true));
        if ui.button("[+]").clicked() {
            *speed = (*speed + 0.1).min(10.0);
        }
    });
}
