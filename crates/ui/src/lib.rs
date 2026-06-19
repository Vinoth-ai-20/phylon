//! # Phylon UI
//!
//! `egui`-based research interface: entity inspector, analytics dashboard,
//! experiment controls, replay timeline, and debug overlay toggles.
//!
//! The UI crate renders on top of the simulation frame using egui's wgpu
//! backend. It reads from the simulation state (via shared snapshots) and
//! publishes intervention events to the event bus.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the UI subsystem.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// An egui widget encountered an invalid state.
    #[error("UI state error: {message}")]
    StateError {
        /// Description of the invalid state.
        message: String,
    },
}

impl common::PhylonError for UiError {}

/// The active tab in the primary sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SidebarTab {
    /// Inspect single organisms and the physical environment
    #[default]
    Inspector,
    /// View neural networks and genotypes
    Genetics,
    /// Global metrics and population charts
    Analytics,
}

/// Renders the main immediate-mode user interface.
///
/// Returns the screen-space `Rect` of the transparent `CentralPanel` so the
/// caller can set the simulation's GPU viewport and scissor rect to match it.
///
/// `debug_structural` is mutated by a checkbox in the Inspector sidebar.
/// When `true`, the caller should render raw physics quads instead of the SDF
/// organic skin.
#[allow(clippy::too_many_arguments)]
pub fn render_ui(
    ctx: &egui::Context,
    world: &mut world::World,
    camera_pos: common::Vec2,
    camera_zoom: f32,
    selected_entity: Option<bevy_ecs::entity::Entity>,
    debug_structural: &mut bool,
    bone_line_thickness: &mut f32,
    active_tab: &mut SidebarTab,
) -> egui::Rect {
    // ── Top menu bar ───────────────────────────────────────────────────────
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                let _ = ui.button("Save State");
                let _ = ui.button("Load State");
                ui.separator();
                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("Edit", |ui| {
                let _ = ui.button("Undo");
                let _ = ui.button("Redo");
            });
            ui.menu_button("Simulation", |ui| {
                let _ = ui.button("Play/Pause");
                let _ = ui.button("Step Forward");
                let _ = ui.button("Reset");
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(debug_structural, "Debug Structural View");
            });
            ui.menu_button("Selection", |ui| {
                let _ = ui.button("Select All");
                let _ = ui.button("Deselect");
            });
            ui.menu_button("Tools", |ui| {
                let _ = ui.button("Spawn Proto-Fish");
            });
            ui.menu_button("Help", |ui| {
                let _ = ui.button("Documentation");
                let _ = ui.button("About Phylon");
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!(
                    "Cam: ({:.0}, {:.0})  ×{:.1}",
                    camera_pos.x, camera_pos.y, camera_zoom
                ));
            });
        });
    });

    // ── Activity bar (narrow icon strip, far left) ─────────────────────────
    egui::SidePanel::left("activity_bar")
        .exact_width(40.0)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                if ui
                    .selectable_label(*active_tab == SidebarTab::Inspector, "🔍")
                    .on_hover_text("Inspector")
                    .clicked()
                {
                    *active_tab = SidebarTab::Inspector;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(*active_tab == SidebarTab::Genetics, "🧬")
                    .on_hover_text("Genetics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Genetics;
                }
                ui.add_space(4.0);
                if ui
                    .selectable_label(*active_tab == SidebarTab::Analytics, "📈")
                    .on_hover_text("Analytics")
                    .clicked()
                {
                    *active_tab = SidebarTab::Analytics;
                }
            });
        });

    // ── Primary sidebar ────────────────────────────────────────────────────
    egui::SidePanel::left("primary_sidebar")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            match active_tab {
                SidebarTab::Inspector => {
                    ui.heading("Inspector");
                    ui.separator();
                    ui.checkbox(debug_structural, "🔲 Debug Structural View");
                    if *debug_structural {
                        ui.add(
                            egui::Slider::new(bone_line_thickness, 0.5..=5.0)
                                .text("Bone Line Thickness"),
                        );
                    }
                    ui.separator();
                    if let Some(entity) = selected_entity {
                        ui.label(
                            egui::RichText::new(format!("Entity {:?}", entity))
                                .small()
                                .color(egui::Color32::GRAY),
                        );

                        // Physics node
                        let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                        if let Ok(node) = node_q.get(&world.ecs, entity) {
                            egui::CollapsingHeader::new("⚙ Physics Node")
                                .default_open(true)
                                .show(ui, |ui| {
                                    let seg_name = match node.segment_type {
                                        0 => "Head",
                                        1 => "Torso",
                                        2 => "Muscle",
                                        3 => "Tail",
                                        4 => "Fin",
                                        _ => "Unknown",
                                    };
                                    ui.label(format!("Segment  : {seg_name}"));
                                    ui.label(format!(
                                        "Position : ({:.1}, {:.1})",
                                        node.position.x, node.position.y
                                    ));
                                    ui.label(format!(
                                        "Velocity : ({:.2}, {:.2})",
                                        node.velocity.x, node.velocity.y
                                    ));
                                    ui.label(format!("Mass     : {:.2}", node.mass));
                                });
                        }

                        // Metabolism — Energy
                        let mut energy_q = world.ecs.query::<&metabolism::Energy>();
                        let mut age_q = world.ecs.query::<&metabolism::Age>();
                        let mut meta_q = world.ecs.query::<&metabolism::Metabolism>();
                        let has_meta = energy_q.get(&world.ecs, entity).is_ok();

                        if has_meta {
                            egui::CollapsingHeader::new("🧬 Biology")
                                .default_open(true)
                                .show(ui, |ui| {
                                    if let Ok(en) = energy_q.get(&world.ecs, entity) {
                                        let pct = en.current / en.max;
                                        ui.label(format!(
                                            "Energy : {:.1} / {:.1}",
                                            en.current, en.max
                                        ));
                                        ui.add(
                                            egui::ProgressBar::new(pct)
                                                .text(format!("{:.0}%", pct * 100.0)),
                                        );
                                    }
                                    if let Ok(age) = age_q.get(&world.ecs, entity) {
                                        ui.label(format!(
                                            "Age    : {} / {} ticks",
                                            age.ticks, age.max_lifespan
                                        ));
                                    }
                                    if let Ok(meta) = meta_q.get(&world.ecs, entity) {
                                        ui.label(format!("Mass   : {:.2}", meta.mass));
                                        ui.label(format!("Rate   : {:.3} /tick", meta.base_rate));
                                    }
                                });
                        }

                        // Biological components (diet)
                        let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                        if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                            ui.label(format!("Diet   : {:?}", bio.diet));
                        }
                    } else {
                        ui.label(
                            egui::RichText::new("Click a node to inspect")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    }
                }
                SidebarTab::Genetics => {
                    ui.heading("Genetics");
                    ui.separator();
                    ui.label("Genome view not implemented yet.");
                }
                SidebarTab::Analytics => {
                    ui.heading("Analytics");
                    ui.separator();
                    ui.label("Analytics dashboard not implemented yet.");
                }
            }
        });

    // ── Status bar (bottom strip) ──────────────────────────────────────────
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let entity_count = world.ecs.entities().len();
                let fps = world
                    .ecs
                    .get_resource::<analytics::MetricsState>()
                    .map(|m| m.smoothed_fps)
                    .unwrap_or(0.0);

                let sim_time = world
                    .ecs
                    .get_resource::<analytics::MetricsState>()
                    .map(|m| m.sim_time)
                    .unwrap_or(0.0);
                let tick_count = (sim_time / 0.016).round() as u64;

                ui.label(format!("⏱ Tick: {}", tick_count));
                ui.separator();
                ui.label(format!("⚡ FPS: {:.0}", fps));
                ui.separator();
                ui.label(format!("🦠 Entities: {}", entity_count));
                ui.separator();
                ui.label(if *debug_structural {
                    "👁 Mode: Structural"
                } else {
                    "👁 Mode: SDF Skin"
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Mem: <100MB"); // Placeholder, proper system memory integration later
                    ui.separator();
                    ui.label("🟢 Engine Online");
                });
            });
        });

    // ── Bottom panel — Metrics plots ───────────────────────────────────────
    egui::TopBottomPanel::bottom("bottom_panel")
        .resizable(true)
        .default_height(180.0)
        .show(ctx, |ui| {
            ui.heading("Output / Metrics");
            ui.separator();

            if let Some(metrics) = world.ecs.get_resource::<analytics::MetricsState>() {
                let pop_pts: egui_plot::PlotPoints =
                    metrics.population_history.iter().copied().collect();
                let fps_pts: egui_plot::PlotPoints = metrics.fps_history.iter().copied().collect();

                ui.columns(2, |cols| {
                    cols[0].label("Population");
                    egui_plot::Plot::new("pop_plot")
                        .height(120.0)
                        .show(&mut cols[0], |plot_ui| {
                            plot_ui.line(egui_plot::Line::new(pop_pts).name("entities"));
                        });

                    cols[1].label("FPS");
                    egui_plot::Plot::new("fps_plot")
                        .height(120.0)
                        .show(&mut cols[1], |plot_ui| {
                            plot_ui.line(egui_plot::Line::new(fps_pts).name("fps"));
                        });
                });
            } else {
                ui.label("Metrics not yet available.");
            }
        });

    // ── Central panel (transparent — simulation renders underneath) ────────
    let central = egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(120)))
                .rounding(4.0),
        )
        .show(ctx, |_ui| {});

    central.response.rect
}
