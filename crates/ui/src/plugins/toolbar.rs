//! Toolbar plugin — playback controls, speed slider, overlay selector, camera controls.
//!
//! This module implements the simulation toolbar row that appears below the main menu bar.
//! Overlay changes are dispatched as `MenuAction::SetOverlay(ActiveHeatmap)` so the
//! `HeatmapState` ECS resource remains the single source of truth.

use crate::types::*;

/// Render the simulation toolbar strip.
///
/// Should be called inside a `TopBottomPanel` row after the main menu bar.
#[allow(clippy::too_many_arguments)]
pub fn toolbar_ui(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let _ = ctx; // may be used for context menus later
    ui.horizontal(|ui| {
        // ── Playback Controls ──────────────────────────────────────────────
        ui.separator();

        // Play / Pause
        let play_icon = if state.is_paused {
            egui_remixicon::icons::PLAY_FILL
        } else {
            egui_remixicon::icons::PAUSE_FILL
        };
        let play_color = if state.is_paused {
            crate::theme::PLAYBACK_PAUSED
        } else {
            crate::theme::PLAYBACK_LIVE
        };
        let play_label = if state.is_paused { "PAUSED" } else { "LIVE" };
        if ui
            .add(egui::Button::new(
                egui::RichText::new(format!("{} {}", play_icon, play_label))
                    .size(crate::theme::SIZE_BODY)
                    .color(play_color),
            ))
            .on_hover_text("Play / Pause (Space)")
            .clicked()
        {
            // MenuAction::TogglePlayPause's handler flips `is_paused` — don't
            // also do it here, or the two toggles cancel out.
            actions.push(MenuAction::TogglePlayPause);
        }

        // Step forward
        if ui
            .add(egui::Button::new(
                egui::RichText::new(egui_remixicon::icons::SKIP_FORWARD_FILL)
                    .size(crate::theme::ICON_MD),
            ))
            .on_hover_text("Step Forward (→)")
            .clicked()
        {
            actions.push(MenuAction::StepForward);
        }

        // Restart
        if ui
            .add(egui::Button::new(
                egui::RichText::new(egui_remixicon::icons::RESTART_FILL)
                    .size(crate::theme::ICON_MD),
            ))
            .on_hover_text("Reset Simulation")
            .clicked()
        {
            actions.push(MenuAction::ReseedEcosystem);
        }

        ui.separator();

        // ── Speed Controls ────────────────────────────────────────────────
        ui.label("Speed:");
        ui.add(
            egui::Slider::new(&mut state.simulation_speed, 0.1..=10.0)
                .logarithmic(true)
                .text("×")
                .max_decimals(1),
        );
        // Speed presets
        for (label, speed) in [("1×", 1.0f32), ("2×", 2.0), ("5×", 5.0), ("10×", 10.0)] {
            if ui
                .selectable_label((state.simulation_speed - speed).abs() < 0.05, label)
                .clicked()
            {
                state.simulation_speed = speed;
            }
        }

        ui.separator();

        // ── Overlay Selector ─────────────────────────────────────────────
        let current_heatmap = world
            .ecs
            .get_resource::<HeatmapState>()
            .map(|h| h.active)
            .unwrap_or(ActiveHeatmap::None);

        let overlay_label = heatmap_label(current_heatmap);
        egui::ComboBox::from_id_salt("overlay_selector")
            .selected_text(format!(
                "{} {}",
                egui_remixicon::icons::MAP_LINE,
                overlay_label
            ))
            .show_ui(ui, |ui| {
                for (variant, label) in HEATMAP_VARIANTS {
                    let selected = current_heatmap == variant;
                    if ui.selectable_label(selected, label).clicked() {
                        actions.push(MenuAction::SetOverlay(variant));
                    }
                }
            });

        // Colormap selector and World Boundary toggle moved to the View menu
        // (Milestone 13) — they're configuration, not per-frame controls,
        // and the audit flagged the toolbar as overcrowded with always-on
        // controls that don't need constant visibility.

        ui.separator();

        // ── Screenshot / Recording ──────────────────────────────────────────
        if ui
            .button(
                egui::RichText::new(egui_remixicon::icons::SCREENSHOT_LINE)
                    .size(crate::theme::ICON_SM),
            )
            .on_hover_text("Take Screenshot (Ctrl+Shift+S)")
            .clicked()
        {
            actions.push(MenuAction::TakeScreenshot);
        }

        let (rec_icon, rec_color) = if state.recording_active {
            (
                egui_remixicon::icons::RECORD_CIRCLE_FILL,
                crate::theme::DANGER,
            )
        } else {
            (
                egui_remixicon::icons::RECORD_CIRCLE_LINE,
                crate::theme::DISABLED_FG,
            )
        };
        let rec_label =
            if let (true, Some(started)) = (state.recording_active, state.recording_started_at) {
                let elapsed = (state.time - started).max(0.0);
                format!(
                    "{} REC {:02}:{:02}",
                    rec_icon,
                    (elapsed as u64) / 60,
                    (elapsed as u64) % 60
                )
            } else {
                format!("{} Record", rec_icon)
            };
        if ui
            .button(egui::RichText::new(rec_label).color(rec_color))
            .on_hover_text("Start/Stop Recording (Ctrl+Shift+R)")
            .clicked()
        {
            actions.push(MenuAction::ToggleRecording);
        }

        ui.separator();

        // Measure tool (Phase 2, M11) — toggling clears any in-progress
        // marquee-select drag interaction by construction, since the two
        // share one click-drag gesture in `viewport.rs`, branching on this
        // flag.
        if ui
            .selectable_label(
                state.measure_mode,
                egui_remixicon::icons::RULER_LINE.to_string(),
            )
            .on_hover_text("Measure distance (drag in the viewport)")
            .clicked()
        {
            state.measure_mode = !state.measure_mode;
        }

        ui.separator();

        // Focus Mode (Phase 2, M16) — fullscreen viewport toggle, entirely
        // UI-side (see `layout::toggle_focus_mode`'s doc comment).
        if ui
            .selectable_label(
                state.focus_mode_previous.is_some(),
                egui_remixicon::icons::FULLSCREEN_LINE.to_string(),
            )
            .on_hover_text("Focus Mode — hide all panels except the Viewport")
            .clicked()
        {
            crate::layout::toggle_focus_mode(state);
        }

        ui.separator();

        // Bookmarks (Phase 2, M12) — save/jump-to camera views. Entirely
        // UI-side: camera position/zoom already live in `WorkbenchState`,
        // so no `MenuAction`/ECS round-trip is needed to apply one.
        ui.menu_button(
            format!("{} Bookmarks", egui_remixicon::icons::BOOKMARK_LINE),
            |ui| {
                if ui.button("+ Save Current View").clicked() {
                    state.bookmarks.push(crate::CameraBookmark {
                        label: format!("View {}", state.bookmarks.len() + 1),
                        position: state.camera_pos,
                        zoom: state.camera_zoom,
                    });
                    ui.close_menu();
                }
                if !state.bookmarks.is_empty() {
                    ui.separator();
                    let mut to_remove = None;
                    for (i, bookmark) in state.bookmarks.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.button(&bookmark.label).clicked() {
                                state.camera_pos = bookmark.position;
                                state.camera_zoom = bookmark.zoom;
                                ui.close_menu();
                            }
                            if ui
                                .small_button(egui_remixicon::icons::CLOSE_LINE)
                                .on_hover_text("Remove bookmark")
                                .clicked()
                            {
                                to_remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = to_remove {
                        state.bookmarks.remove(i);
                    }
                }
            },
        );

        ui.separator();

        // ── Camera Controls (right-aligned) ───────────────────────────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Zoom in/out
            if ui.button("-").on_hover_text("Zoom Out (−)").clicked() {
                actions.push(MenuAction::CameraZoomOut);
            }
            ui.label(format!("{:.0}%", state.camera_zoom * 100.0));
            if ui.button("+").on_hover_text("Zoom In (+)").clicked() {
                actions.push(MenuAction::CameraZoomIn);
            }
            // Home
            if ui
                .button(egui_remixicon::icons::HOME_LINE)
                .on_hover_text("Reset Camera (Home)")
                .clicked()
            {
                actions.push(MenuAction::CameraHome);
            }
            ui.separator();
            // Spectator mode
            let spec_text = if state.spectator_mode {
                egui::RichText::new(format!("{} Spectator", egui_remixicon::icons::FILM_LINE))
                    .color(egui::Color32::LIGHT_GREEN)
            } else {
                egui::RichText::new(format!("{} Spectator", egui_remixicon::icons::FILM_LINE))
                    .color(crate::theme::DISABLED_FG)
            };
            if ui
                .selectable_label(state.spectator_mode, spec_text)
                .on_hover_text("Automatically follow the most interesting organism")
                .clicked()
            {
                state.spectator_mode = !state.spectator_mode;
                if !state.spectator_mode {
                    state.tracked_entity = None;
                }
            }

            // Follow selected
            if state.selected_entity.is_some() {
                ui.separator();
                if ui
                    .button(format!("{} Follow", egui_remixicon::icons::FOCUS_LINE))
                    .on_hover_text("Follow selected organism (F)")
                    .clicked()
                {
                    state.tracked_entity = state.selected_entity;
                    state.spectator_mode = false;
                }
            }

            // Camera position readout
            ui.separator();
            let track_str = if let Some(e) = state.tracked_entity {
                format!(" [Tracking {:?}]", e)
            } else {
                String::new()
            };
            ui.label(
                egui::RichText::new(format!(
                    "Cam ({:.0}, {:.0}){}",
                    state.camera_pos.x, state.camera_pos.y, track_str
                ))
                .color(crate::theme::DISABLED_FG)
                .size(crate::theme::SIZE_MICRO),
            );

            // Sunlight indicator (from atmosphere)
            if let Some(atmosphere) = world.ecs.get_resource::<metabolism::GlobalAtmosphere>() {
                ui.separator();
                ui.label(format!(
                    "{} {:.0}%",
                    egui_remixicon::icons::SUN_LINE,
                    atmosphere.sunlight * 100.0
                ));
            }
        });
    });
}

/// Human-readable label for each heatmap variant.
pub fn heatmap_label(h: ActiveHeatmap) -> &'static str {
    match h {
        ActiveHeatmap::None => "None",
        ActiveHeatmap::Glucose => "Glucose",
        ActiveHeatmap::ATP => "ATP",
        ActiveHeatmap::Pheromones => "Pheromones",
        ActiveHeatmap::EnergyDensity => "Energy Density",
        ActiveHeatmap::O2 => "Oxygen",
        ActiveHeatmap::CO2 => "CO₂",
    }
}

/// All selectable heatmap variants with their display labels.
const HEATMAP_VARIANTS: [(ActiveHeatmap, &str); 7] = [
    (ActiveHeatmap::None, "None"),
    (ActiveHeatmap::Glucose, "Glucose"),
    (ActiveHeatmap::ATP, "ATP"),
    (ActiveHeatmap::Pheromones, "Pheromones"),
    (ActiveHeatmap::EnergyDensity, "Energy Density"),
    (ActiveHeatmap::O2, "Oxygen"),
    (ActiveHeatmap::CO2, "CO₂"),
];

/// Human-readable label for each colormap variant (must match the shader's
/// `config.colormap` index switch in `field_overlay.wgsl`).
pub fn colormap_label(index: u32) -> &'static str {
    match index {
        0 => "Viridis",
        1 => "Magma",
        2 => "Plasma",
        3 => "Inferno",
        _ => "Turbo",
    }
}

/// All selectable colormap variants with their display labels.
pub const COLORMAP_VARIANTS: [(u32, &str); 5] = [
    (0, "Viridis"),
    (1, "Magma"),
    (2, "Plasma"),
    (3, "Inferno"),
    (4, "Turbo"),
];
