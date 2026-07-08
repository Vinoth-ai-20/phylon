//! Layout manager — egui_tiles tree structure, pane rendering, and panel chrome.
//!
//! ## Architecture
//!
//! The tile tree is the sole layout manager for the CentralPanel. Each pane can be in one
//! of three modes (stored in `WorkbenchState::panel_modes`):
//!
//! - `Docked`   — rendered inside the tiles tree (default)
//! - `Floating` — rendered as a free `egui::Window` in `render.rs`
//! - `Closed`   — hidden; reopen via the **Windows** menu
//!
//! When a panel is Floating or Closed, `retain_pane` returns `false` and it is removed
//! from the tile tree. When re-docked, it is re-inserted into the root container.
//!
//! ## Tree Layout (default)
//!
//! ```text
//! ┌───────────┬─────────────────────┐
//! │  Sidebar  │      Viewport       │
//! │           ├──────────┬──────────┤
//! │           │ Metrics  │ EventLog │
//! └───────────┴──────────┴──────────┘
//! ```

use crate::state::PanelMode;
use crate::types::MenuAction;
use crate::WorkbenchState;
use egui_tiles::{Behavior, TileId, UiResponse};
use world::World;

/// Every named panel that can be docked, floated, or closed.
///
/// "Placeholder Panel" carries no real content — it exists solely to prove
/// the forward-compatibility claim in `docs/design/layout.md`: a future
/// module (Experiment Manager, Replay Timeline, Genome Editor, ...) needs
/// only a name here and one dispatch arm in `WorkbenchBehavior::pane_ui` /
/// `render_floating_panels` to be fully dockable/floatable/closable, exactly
/// like every panel above it.
pub const ALL_PANEL_NAMES: &[&str] = &[
    "Sidebar",
    "Viewport",
    "Metrics",
    "Event Log",
    "Neural Viewer",
    "Research Dashboard",
    "Replay Browser",
    "Evolution Debugger",
    "Physiology Viewer",
    "Circulation Viewer",
    "Hormone Viewer",
    "Immune Viewer",
    "Cell Lineage Viewer",
    "Placeholder Panel",
];

/// egui_tiles `Behavior` implementation that dispatches pane rendering.
pub struct WorkbenchBehavior<'a> {
    /// Mutable reference to the global workbench UI state.
    pub state: &'a mut WorkbenchState,
    /// Mutable reference to the ECS world for read-only queries.
    pub world: &'a mut World,
    /// Action queue accumulated during this frame.
    pub commands: &'a mut Vec<MenuAction>,
    /// Canvas interaction result produced by the Viewport pane (if any).
    pub canvas_interaction: Option<crate::types::CanvasInteraction>,
}

impl<'a> Behavior<String> for WorkbenchBehavior<'a> {
    // ── Pane content ─────────────────────────────────────────────────────────

    fn pane_ui(&mut self, ui: &mut egui::Ui, _tile_id: TileId, pane: &mut String) -> UiResponse {
        let ctx = ui.ctx().clone();
        let name = pane.clone();

        // Opaque panel background — prevents the wgpu simulation world colours
        // from bleeding through transparent egui surfaces.
        egui::Frame::none()
            .fill(crate::theme::CHROME_BG)
            .inner_margin(egui::Margin::ZERO)
            .show(ui, |ui| {
                // Panel chrome: thin bar with title + Detach/Close buttons.
                // For the Sidebar pane, show the active tab's icon/label
                // (Inspector, Genetics, ...) instead of the literal tile name
                // "Sidebar" — this is the one merged bar for every sidebar
                // tab, replacing the heading each of them used to draw again
                // just below it.
                let chrome_title = if name == "Sidebar" {
                    format!(
                        "{} {}",
                        crate::plugins::sidebar::tab_icon(self.state.active_tab),
                        crate::plugins::sidebar::tab_label(self.state.active_tab)
                    )
                } else if name == "Neural Viewer" {
                    format!("{} Neural Viewer", egui_remixicon::icons::BRAIN_LINE)
                } else if name == "Research Dashboard" {
                    format!("{} Research Dashboard", egui_remixicon::icons::FLASK_LINE)
                } else if name == "Replay Browser" {
                    format!("{} Replay Browser", egui_remixicon::icons::FOLDER_OPEN_LINE)
                } else if name == "Evolution Debugger" {
                    format!("{} Evolution Debugger", egui_remixicon::icons::BUG_LINE)
                } else if name == "Physiology Viewer" {
                    format!(
                        "{} Physiology Viewer",
                        egui_remixicon::icons::HEART_PULSE_LINE
                    )
                } else if name == "Circulation Viewer" {
                    format!("{} Circulation Viewer", egui_remixicon::icons::DROP_LINE)
                } else if name == "Hormone Viewer" {
                    format!("{} Hormone Viewer", egui_remixicon::icons::FLASK_LINE)
                } else if name == "Immune Viewer" {
                    format!("{} Immune Viewer", egui_remixicon::icons::SHIELD_LINE)
                } else if name == "Cell Lineage Viewer" {
                    format!(
                        "{} Cell Lineage Viewer",
                        egui_remixicon::icons::GIT_BRANCH_LINE
                    )
                } else {
                    name.clone()
                };
                panel_chrome(ui, &name, &chrome_title, self.commands);

                // Content padding: every panel except the Viewport (a
                // full-bleed canvas) gets consistent breathing room from
                // `theme::PANEL_PADDING` here, instead of each plugin adding
                // its own ad hoc `ui.add_space(...)` at the top.
                let content_margin = if name == "Viewport" {
                    egui::Margin::ZERO
                } else {
                    egui::Margin::symmetric(
                        crate::theme::PANEL_PADDING,
                        crate::theme::PANEL_PADDING,
                    )
                };

                egui::Frame::none()
                    .inner_margin(content_margin)
                    .show(ui, |ui| match name.as_str() {
                        "Viewport" => {
                            crate::plugins::viewport::viewport_ui(
                                &ctx,
                                ui,
                                self.state,
                                &mut self.canvas_interaction,
                                self.commands,
                            );
                        }
                        "Sidebar" | "Inspector" => {
                            crate::plugins::sidebar::sidebar_content_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Metrics" => {
                            crate::plugins::metrics::metrics_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Event Log" => {
                            crate::plugins::event_log::event_log_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Neural Viewer" => {
                            crate::plugins::neural_viewer::neural_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Research Dashboard" => {
                            crate::plugins::research_dashboard::research_dashboard_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Replay Browser" => {
                            crate::plugins::replay_browser::replay_browser_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Evolution Debugger" => {
                            crate::plugins::evolution_debugger::evolution_debugger_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Physiology Viewer" => {
                            crate::plugins::physiology_viewer::physiology_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Circulation Viewer" => {
                            crate::plugins::circulation_viewer::circulation_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Hormone Viewer" => {
                            crate::plugins::hormone_viewer::hormone_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Immune Viewer" => {
                            crate::plugins::immune_viewer::immune_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Cell Lineage Viewer" => {
                            crate::plugins::lineage_viewer::lineage_viewer_ui(
                                &ctx,
                                ui,
                                self.state,
                                self.world,
                                self.commands,
                            );
                        }
                        "Placeholder Panel" => {
                            crate::widgets::empty_state(
                                ui,
                                "Forward-compatibility placeholder — proves a new panel type can \
                                 dock, float, and close like any other without redesign. \
                                 See docs/design/layout.md.",
                            );
                        }
                        _ => {
                            ui.label(format!("Unknown pane: {}", name));
                        }
                    });
            });

        UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &String) -> egui::WidgetText {
        pane.clone().into()
    }

    /// Draws the Close/Detach buttons into egui_tiles' own tab-strip bar for
    /// tabbed panes (Metrics/Event Log/Neural Viewer), instead of `pane_ui`
    /// drawing a second bar via `panel_chrome` underneath it — previously
    /// every tabbed pane showed two stacked chrome bars (the tab strip, then
    /// our buttons row).
    fn top_bar_right_ui(
        &mut self,
        tiles: &egui_tiles::Tiles<String>,
        ui: &mut egui::Ui,
        _tile_id: TileId,
        tabs: &egui_tiles::Tabs,
        _scroll_offset: &mut f32,
    ) {
        let Some(active_id) = tabs.active else {
            return;
        };
        let Some(egui_tiles::Tile::Pane(name)) = tiles.get(active_id) else {
            return;
        };
        let name = name.clone();

        chrome_bar(ui, &name, None, None, self.commands, false, None);
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// No child panel should shrink below this width/height when a split is
    /// dragged. `egui_tiles` only supports one global floor (not a distinct
    /// minimum per panel), so this is the smallest value that still keeps
    /// every panel's content usable — see the per-panel *target* minimums
    /// documented in `docs/design/layout.md`, which this single floor backs.
    fn min_size(&self) -> f32 {
        160.0
    }

    /// Remove panes from the tree if they are Floating or Closed.
    /// egui_tiles will then simplify empty containers automatically.
    fn retain_pane(&mut self, pane: &String) -> bool {
        matches!(
            self.state
                .panel_modes
                .get(pane.as_str())
                .copied()
                .unwrap_or(PanelMode::Docked),
            PanelMode::Docked
        )
    }
}

// ─── Panel chrome ─────────────────────────────────────────────────────────────

/// Thin chrome bar rendered at the top of every docked pane that ISN'T
/// inside an egui_tiles `Tabs` container. Shows `title` on the left (e.g. the
/// active sidebar tab's icon + label) and Detach/Close buttons on the right,
/// in a single row — mirrors the merged tab-strip bar used for tabbed panes
/// (see `WorkbenchBehavior::top_bar_right_ui`) instead of stacking a second,
/// separate title row underneath.
fn panel_chrome(ui: &mut egui::Ui, name: &str, title: &str, commands: &mut Vec<MenuAction>) {
    // Viewport has no chrome (transparent pass-through). Metrics/Event Log
    // live in a `Tabs` container, which has its own tab-strip bar — their
    // Close/Detach buttons are drawn into that bar instead (see
    // `WorkbenchBehavior::top_bar_right_ui`), so they don't get a second one
    // here. Neural Viewer is NOT in that Tabs container (it's a standalone
    // side column — see `rebuild_tree_from_modes`), so it still needs this
    // bar like Sidebar does.
    if matches!(name, "Viewport" | "Metrics" | "Event Log") {
        return;
    }

    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), crate::theme::CHROME_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| chrome_bar(ui, name, Some(title), None, commands, false, None),
    );

    ui.separator();
}

/// Draws one Close/Detach/Dock/Minimize icon button with the shared chrome
/// styling — single source for the button size/color every chrome variant
/// below uses, so `CLOSE_RED`/`DETACH_BLUE` can never drift out of sync
/// between the docked, tabbed, and floating chrome bars again.
fn chrome_button(
    ui: &mut egui::Ui,
    icon: &str,
    color: egui::Color32,
    tooltip: &str,
) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(icon)
                .color(color)
                .size(crate::theme::ICON_SM),
        )
        .min_size(egui::vec2(20.0, 20.0)),
    )
    .on_hover_text(tooltip)
}

/// The two panel tiers ADR-P5-05 defines (Phase 5, SX-8a/8b) — Viewport is
/// the third, Primary tier, but it never reaches `chrome_bar` at all (see
/// `panel_chrome`'s early return for it), so there's no variant for it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelTier {
    /// Content changes with the current selection: Sidebar (hosts
    /// Inspector), Neural Viewer, and the P4-R-tier Physiology/Circulation/
    /// Hormone/Immune/Cell Lineage viewers.
    Contextual,
    /// Aggregate/session-wide content, not tied to a single selection:
    /// Metrics, Event Log, Research Dashboard, Replay Browser, Evolution
    /// Debugger, and the content-free Placeholder Panel.
    Secondary,
}

/// Classifies a panel by `ALL_PANEL_NAMES` name into its ADR-P5-05 tier.
fn panel_tier(name: &str) -> PanelTier {
    match name {
        "Sidebar"
        | "Neural Viewer"
        | "Physiology Viewer"
        | "Circulation Viewer"
        | "Hormone Viewer"
        | "Immune Viewer"
        | "Cell Lineage Viewer" => PanelTier::Contextual,
        _ => PanelTier::Secondary,
    }
}

/// Single chrome-bar renderer, consolidating what were three independent
/// implementations (`panel_chrome` for docked/untabbed panes,
/// `WorkbenchBehavior::top_bar_right_ui` for tabbed panes, and
/// `floating_chrome` for floating windows) that had drifted to mismatched
/// close-button reds (`rgb(180,80,80)` vs `rgb(220,80,80)`).
///
/// `title`/`leading_icon` are `None` for the tabbed variant, since
/// egui_tiles' own tab strip already draws the title there. `minimized` is
/// `Some` only for floating windows, which get a third Minimize/Restore
/// button; `dock_instead_of_detach` swaps the second button from "Detach"
/// (docked/tabbed → floating) to "Dock" (floating → docked).
fn chrome_bar(
    ui: &mut egui::Ui,
    name: &str,
    title: Option<&str>,
    leading_icon: Option<&str>,
    commands: &mut Vec<MenuAction>,
    dock_instead_of_detach: bool,
    minimized: Option<&mut bool>,
) {
    let tier = panel_tier(name);

    ui.horizontal(|ui| {
        ui.add_space(crate::theme::SPACE_XS);

        // SX-8b: the accent bar is the Contextual tier's structural tell —
        // a colored rect, not text, so it reads even before the title glyph
        // does. `hover()` sense since it's decorative, not interactive.
        if tier == PanelTier::Contextual {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(3.0, crate::theme::CHROME_HEIGHT - 6.0),
                egui::Sense::hover(),
            );
            ui.painter()
                .rect_filled(rect, 0.0, crate::theme::CHROME_ACCENT_BAR);
            ui.add_space(crate::theme::SPACE_XS);
        }

        if let Some(icon) = leading_icon {
            ui.label(
                egui::RichText::new(icon)
                    .color(egui::Color32::from_gray(120))
                    .size(crate::theme::ICON_SM),
            );
        }
        if let Some(title) = title {
            let title_color = match tier {
                PanelTier::Contextual => crate::theme::CHROME_TITLE_CONTEXTUAL,
                PanelTier::Secondary => crate::theme::CHROME_TITLE_SECONDARY,
            };
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .color(title_color)
                    .size(crate::theme::SIZE_SUBHEADING),
            );
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(crate::theme::SPACE_XS);

            if chrome_button(
                ui,
                egui_remixicon::icons::CLOSE_LINE,
                crate::theme::CLOSE_RED,
                &format!("Close {} (reopen via Windows menu)", name),
            )
            .clicked()
            {
                commands.push(MenuAction::ClosePanel(name.to_string()));
            }

            if dock_instead_of_detach {
                if chrome_button(
                    ui,
                    egui_remixicon::icons::PICTURE_IN_PICTURE_EXIT_LINE,
                    crate::theme::DETACH_BLUE,
                    "Dock — attach back to the main layout",
                )
                .clicked()
                {
                    commands.push(MenuAction::DockPanel(name.to_string()));
                }
            } else if chrome_button(
                ui,
                egui_remixicon::icons::EXTERNAL_LINK_LINE,
                crate::theme::DETACH_BLUE,
                &format!("Detach {} into a floating window", name),
            )
            .clicked()
            {
                commands.push(MenuAction::DetachPanel(name.to_string()));
            }

            if let Some(minimized) = minimized {
                let (min_icon, min_tip) = if *minimized {
                    (egui_remixicon::icons::ARROW_UP_S_LINE, "Restore window")
                } else {
                    (
                        egui_remixicon::icons::SUBTRACT_LINE,
                        "Minimize to title bar",
                    )
                };
                if chrome_button(ui, min_icon, crate::theme::MINIMIZE_YELLOW, min_tip).clicked() {
                    *minimized = !*minimized;
                }
            }
        });
    });
}

// ─── Floating window rendering ────────────────────────────────────────────────

/// Render all panels that are in `Floating` mode as egui::Windows.
/// Call this AFTER the tiles tree in render.rs.
#[allow(clippy::too_many_arguments)]
pub fn render_floating_panels(
    ctx: &egui::Context,
    state: &mut WorkbenchState,
    world: &mut world::World,
    commands: &mut Vec<MenuAction>,
    canvas_interaction: &mut Option<crate::types::CanvasInteraction>,
) {
    // Collect floating panels (avoid borrow conflict with state)
    let floating: Vec<String> = state
        .panel_modes
        .iter()
        .filter(|(_, mode)| **mode == PanelMode::Floating)
        .map(|(name, _)| name.clone())
        .collect();

    for name in floating {
        let mut open = true;

        // Read minimized state for this panel
        let mut minimized = state.panel_minimized.get(&name).copied().unwrap_or(false);

        // Collapse to title-bar height when minimized
        let min_h = if minimized { 0.0 } else { 200.0 };

        let mut window = egui::Window::new(&name)
            .open(&mut open)
            .resizable(!minimized) // no resize handle when collapsed
            .min_width(280.0)
            .min_height(min_h)
            .default_width(360.0)
            .default_height(320.0) // without this, egui sizes to content's
            // natural (unbounded) height on first show — this is what made
            // detaching a panel look like it "snapped" to a huge window.
            .title_bar(false) // custom chrome
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(crate::theme::CHROME_BG)
                    .inner_margin(egui::Margin::same(crate::theme::PANEL_PADDING)),
            );

        // One-shot position correction from a snap computed on the previous
        // frame's drag release (see below) — applied once, then forgotten,
        // so the user can freely drag again afterward.
        if let Some(pos) = state.floating_snap_pos.remove(&name) {
            window = window.current_pos(pos);
        }

        let window_response = window.show(ctx, |ui| {
            // Custom chrome — floating_chrome can toggle minimized. For the
            // Sidebar panel, show the active tab's icon/label (matching the
            // docked chrome bar) instead of the literal tile name "Sidebar".
            let chrome_title = if name == "Sidebar" {
                format!(
                    "{} {}",
                    crate::plugins::sidebar::tab_icon(state.active_tab),
                    crate::plugins::sidebar::tab_label(state.active_tab)
                )
            } else if name == "Neural Viewer" {
                format!("{} Neural Viewer", egui_remixicon::icons::BRAIN_LINE)
            } else if name == "Research Dashboard" {
                format!("{} Research Dashboard", egui_remixicon::icons::FLASK_LINE)
            } else if name == "Replay Browser" {
                format!("{} Replay Browser", egui_remixicon::icons::FOLDER_OPEN_LINE)
            } else if name == "Evolution Debugger" {
                format!("{} Evolution Debugger", egui_remixicon::icons::BUG_LINE)
            } else if name == "Physiology Viewer" {
                format!(
                    "{} Physiology Viewer",
                    egui_remixicon::icons::HEART_PULSE_LINE
                )
            } else if name == "Circulation Viewer" {
                format!("{} Circulation Viewer", egui_remixicon::icons::DROP_LINE)
            } else if name == "Hormone Viewer" {
                format!("{} Hormone Viewer", egui_remixicon::icons::FLASK_LINE)
            } else if name == "Immune Viewer" {
                format!("{} Immune Viewer", egui_remixicon::icons::SHIELD_LINE)
            } else if name == "Cell Lineage Viewer" {
                format!(
                    "{} Cell Lineage Viewer",
                    egui_remixicon::icons::GIT_BRANCH_LINE
                )
            } else {
                name.clone()
            };
            floating_chrome(ui, &name, &chrome_title, &mut minimized, commands);

            // Only render content when NOT minimized
            if !minimized {
                ui.separator();
                ui.add_space(crate::theme::SPACE_XS);

                match name.as_str() {
                    "Sidebar" | "Inspector" => {
                        crate::plugins::sidebar::sidebar_content_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Metrics" => {
                        crate::plugins::metrics::metrics_ui(ctx, ui, state, world, commands);
                    }
                    "Event Log" => {
                        crate::plugins::event_log::event_log_ui(ctx, ui, state, world, commands);
                    }
                    "Neural Viewer" => {
                        crate::plugins::neural_viewer::neural_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Research Dashboard" => {
                        crate::plugins::research_dashboard::research_dashboard_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Replay Browser" => {
                        crate::plugins::replay_browser::replay_browser_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Evolution Debugger" => {
                        crate::plugins::evolution_debugger::evolution_debugger_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Physiology Viewer" => {
                        crate::plugins::physiology_viewer::physiology_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Circulation Viewer" => {
                        crate::plugins::circulation_viewer::circulation_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Hormone Viewer" => {
                        crate::plugins::hormone_viewer::hormone_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Immune Viewer" => {
                        crate::plugins::immune_viewer::immune_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Cell Lineage Viewer" => {
                        crate::plugins::lineage_viewer::lineage_viewer_ui(
                            ctx, ui, state, world, commands,
                        );
                    }
                    "Viewport" => {
                        crate::plugins::viewport::viewport_ui(
                            ctx,
                            ui,
                            state,
                            canvas_interaction,
                            commands,
                        );
                    }
                    "Placeholder Panel" => {
                        crate::widgets::empty_state(
                            ui,
                            "Forward-compatibility placeholder — see docs/design/layout.md.",
                        );
                    }
                    _ => {
                        ui.label(format!("Unknown panel: {}", name));
                    }
                }
            }
        });

        // Edge/corner snapping: on the frame the drag is released, if the
        // window landed within a small threshold of a screen edge, snap it
        // flush against that edge (applied next frame via `current_pos`
        // above).
        if let Some(resp) = &window_response {
            let is_dragging = resp.response.dragged();
            let was_dragging = state
                .floating_was_dragging
                .get(&name)
                .copied()
                .unwrap_or(false);

            if was_dragging && !is_dragging {
                const SNAP_THRESHOLD: f32 = 24.0;
                let screen = ctx.screen_rect();
                let rect = resp.response.rect;
                let mut snapped_pos = rect.min;
                let mut snapped = false;

                if (rect.min.x - screen.min.x).abs() < SNAP_THRESHOLD {
                    snapped_pos.x = screen.min.x;
                    snapped = true;
                } else if (screen.max.x - rect.max.x).abs() < SNAP_THRESHOLD {
                    snapped_pos.x = screen.max.x - rect.width();
                    snapped = true;
                }
                if (rect.min.y - screen.min.y).abs() < SNAP_THRESHOLD {
                    snapped_pos.y = screen.min.y;
                    snapped = true;
                } else if (screen.max.y - rect.max.y).abs() < SNAP_THRESHOLD {
                    snapped_pos.y = screen.max.y - rect.height();
                    snapped = true;
                }

                if snapped {
                    state.floating_snap_pos.insert(name.clone(), snapped_pos);
                }
            }

            state
                .floating_was_dragging
                .insert(name.clone(), is_dragging);
        }

        // Write back possibly-toggled minimized state
        state.panel_minimized.insert(name.clone(), minimized);

        // Window X button → ClosePanel
        if !open {
            commands.push(MenuAction::ClosePanel(name.clone()));
        }
    }
}

/// Chrome bar rendered inside floating windows.
/// Buttons (right-to-left): Close, Dock, Minimize/Restore.
fn floating_chrome(
    ui: &mut egui::Ui,
    name: &str,
    title: &str,
    minimized: &mut bool,
    commands: &mut Vec<MenuAction>,
) {
    let full_title = format!("{} {}", egui_remixicon::icons::WINDOW_2_LINE, title);
    chrome_bar(
        ui,
        name,
        Some(&full_title),
        Some(egui_remixicon::icons::DRAG_MOVE_2_LINE),
        commands,
        true,
        Some(minimized),
    );
}

// ─── Layout initialisation ────────────────────────────────────────────────────

/// Rebuild the tile tree from scratch, including a tile for each panel whose
/// current mode (in `panel_modes`) is `Docked` — panels that are `Floating`
/// or `Closed` are simply omitted, rather than inserted-then-removed.
///
/// This is the single authoritative layout builder: it always reproduces the
/// canonical shape (Sidebar on the left, Viewport + Metrics/Event Log tabs on
/// the right) for whichever panels are docked, so a panel restored via
/// `DockPanel` — or the whole layout via "Reset Layout" — always lands back
/// in its correct home slot instead of wherever the tree's current root
/// happens to be.
pub fn rebuild_tree_from_modes(
    tree: &mut egui_tiles::Tree<String>,
    panel_modes: &std::collections::HashMap<String, PanelMode>,
    shares: &std::collections::HashMap<String, f32>,
) {
    let share_of = |key: &str, default: f32| shares.get(key).copied().unwrap_or(default);

    let mut tiles = egui_tiles::Tiles::default();
    let is_docked = |name: &str| {
        panel_modes.get(name).copied().unwrap_or(PanelMode::Docked) == PanelMode::Docked
    };

    let sidebar = is_docked("Sidebar").then(|| tiles.insert_pane("Sidebar".to_string()));
    let viewport = tiles.insert_pane("Viewport".to_string());
    let metrics = is_docked("Metrics").then(|| tiles.insert_pane("Metrics".to_string()));
    let event_log = is_docked("Event Log").then(|| tiles.insert_pane("Event Log".to_string()));
    // Neural Viewer gets its own side column (see `root` construction below)
    // rather than sharing the bottom tab strip with Metrics/Event Log — it
    // wants more vertical space for the node-link graph, and its own tab
    // switch would fight with theirs. It previously had no home at all in
    // this tree (toggling it "Docked" from the Windows menu set its
    // `PanelMode` but it was never actually inserted anywhere, so it
    // silently never appeared).
    let neural_viewer =
        is_docked("Neural Viewer").then(|| tiles.insert_pane("Neural Viewer".to_string()));
    // Research Dashboard — shares the root row with Sidebar/Neural Viewer,
    // same treatment as Placeholder Panel below: defaults to Closed (see
    // `apply_layout_preset`) so it never takes space unless explicitly
    // opened from the Windows menu.
    let research_dashboard = is_docked("Research Dashboard")
        .then(|| tiles.insert_pane("Research Dashboard".to_string()));
    // Replay Browser — same treatment as Research Dashboard above.
    let replay_browser =
        is_docked("Replay Browser").then(|| tiles.insert_pane("Replay Browser".to_string()));
    // Evolution Debugger — same treatment as Research Dashboard/Replay
    // Browser above: defaults to Closed, shares the root row when docked.
    let evolution_debugger = is_docked("Evolution Debugger")
        .then(|| tiles.insert_pane("Evolution Debugger".to_string()));
    // P4-R1-R5 physiology/lineage panels — same treatment as Research
    // Dashboard/Replay Browser/Evolution Debugger above: default to Closed
    // (see `apply_layout_preset`), sharing the root row when docked.
    let physiology_viewer =
        is_docked("Physiology Viewer").then(|| tiles.insert_pane("Physiology Viewer".to_string()));
    let circulation_viewer = is_docked("Circulation Viewer")
        .then(|| tiles.insert_pane("Circulation Viewer".to_string()));
    let hormone_viewer =
        is_docked("Hormone Viewer").then(|| tiles.insert_pane("Hormone Viewer".to_string()));
    let immune_viewer =
        is_docked("Immune Viewer").then(|| tiles.insert_pane("Immune Viewer".to_string()));
    let lineage_viewer = is_docked("Cell Lineage Viewer")
        .then(|| tiles.insert_pane("Cell Lineage Viewer".to_string()));
    // Placeholder Panel — see `ALL_PANEL_NAMES` doc comment; shares the root
    // row with Sidebar/Neural Viewer, defaulting to Closed so it never takes
    // space unless explicitly docked.
    let placeholder =
        is_docked("Placeholder Panel").then(|| tiles.insert_pane("Placeholder Panel".to_string()));

    // Bottom tab strip: Metrics | Event Log (whichever are docked).
    let bottom_panes: Vec<egui_tiles::TileId> =
        [metrics, event_log].into_iter().flatten().collect();
    let bottom_tabs = if bottom_panes.len() > 1 {
        Some(tiles.insert_tab_tile(bottom_panes))
    } else {
        bottom_panes.first().copied()
    };

    // Right column: Viewport (3/4) + bottom_tabs (1/4), or just Viewport —
    // ratio persisted from the last frame's live tree (see
    // `extract_shares`), falling back to the 3:1 default the first time.
    let right_col = match bottom_tabs {
        Some(bottom_tabs) => {
            let right_col = tiles.insert_vertical_tile(vec![viewport, bottom_tabs]);
            if let egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear)) =
                tiles.get_mut(right_col).unwrap()
            {
                linear.shares.set_share(viewport, share_of("Viewport", 3.0));
                linear
                    .shares
                    .set_share(bottom_tabs, share_of("BottomTabs", 1.0));
            }
            right_col
        }
        None => viewport,
    };

    // Root row: Sidebar | right_col | Neural Viewer | Placeholder Panel,
    // omitting whichever aren't currently docked, ratios persisted the same
    // way as the nested split above.
    let mut root_children = Vec::new();
    let mut root_shares = Vec::new();
    if let Some(sidebar) = sidebar {
        root_children.push(sidebar);
        root_shares.push(share_of("Sidebar", 1.0));
    }
    root_children.push(right_col);
    root_shares.push(share_of("MainColumn", 3.0));
    if let Some(neural_viewer) = neural_viewer {
        root_children.push(neural_viewer);
        root_shares.push(share_of("Neural Viewer", 1.0));
    }
    if let Some(research_dashboard) = research_dashboard {
        root_children.push(research_dashboard);
        root_shares.push(share_of("Research Dashboard", 1.0));
    }
    if let Some(replay_browser) = replay_browser {
        root_children.push(replay_browser);
        root_shares.push(share_of("Replay Browser", 1.0));
    }
    if let Some(evolution_debugger) = evolution_debugger {
        root_children.push(evolution_debugger);
        root_shares.push(share_of("Evolution Debugger", 1.0));
    }
    if let Some(physiology_viewer) = physiology_viewer {
        root_children.push(physiology_viewer);
        root_shares.push(share_of("Physiology Viewer", 1.0));
    }
    if let Some(circulation_viewer) = circulation_viewer {
        root_children.push(circulation_viewer);
        root_shares.push(share_of("Circulation Viewer", 1.0));
    }
    if let Some(hormone_viewer) = hormone_viewer {
        root_children.push(hormone_viewer);
        root_shares.push(share_of("Hormone Viewer", 1.0));
    }
    if let Some(immune_viewer) = immune_viewer {
        root_children.push(immune_viewer);
        root_shares.push(share_of("Immune Viewer", 1.0));
    }
    if let Some(lineage_viewer) = lineage_viewer {
        root_children.push(lineage_viewer);
        root_shares.push(share_of("Cell Lineage Viewer", 1.0));
    }
    if let Some(placeholder) = placeholder {
        root_children.push(placeholder);
        root_shares.push(share_of("Placeholder Panel", 1.0));
    }

    let root = if root_children.len() > 1 {
        let root = tiles.insert_horizontal_tile(root_children.clone());
        if let egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear)) =
            tiles.get_mut(root).unwrap()
        {
            for (&child, &share) in root_children.iter().zip(root_shares.iter()) {
                linear.shares.set_share(child, share);
            }
        }
        root
    } else {
        root_children[0]
    };

    *tree = egui_tiles::Tree::new("workbench_tree", root, tiles);
}

/// Reads the current split ratio for every named docking split out of the
/// live tree, keyed by each child's label (`tile_label`) — the vertical
/// Viewport/bottom-tabs split is labeled by its two children's own names
/// (`"Viewport"`/`"BottomTabs"`); the root row's non-leaf child (whichever
/// of Viewport-alone or the vertical split occupies that slot) is labeled
/// `"MainColumn"` so the root row always has a stable key for it regardless
/// of whether Metrics/Event Log are currently docked. Called once per frame
/// (see `render.rs`) right after the tree renders, so a user's drag this
/// frame is captured before the next rebuild (dock/undock/reset) would
/// otherwise discard it.
pub fn extract_shares(tree: &egui_tiles::Tree<String>) -> std::collections::HashMap<String, f32> {
    let mut out = std::collections::HashMap::new();
    if let Some(root) = tree.root {
        collect_shares(&tree.tiles, root, &mut out);
    }
    out
}

fn tile_label(tiles: &egui_tiles::Tiles<String>, id: egui_tiles::TileId) -> Option<String> {
    match tiles.get(id) {
        Some(egui_tiles::Tile::Pane(name)) => Some(name.clone()),
        Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(_))) => {
            Some("BottomTabs".to_string())
        }
        Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(_))) => {
            Some("MainColumn".to_string())
        }
        _ => None,
    }
}

fn collect_shares(
    tiles: &egui_tiles::Tiles<String>,
    id: egui_tiles::TileId,
    out: &mut std::collections::HashMap<String, f32>,
) {
    if let Some(egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear))) = tiles.get(id)
    {
        for &child in &linear.children {
            if let Some(label) = tile_label(tiles, child) {
                out.insert(label, linear.shares[child]);
            }
            collect_shares(tiles, child, out);
        }
    }
}

/// A named, fixed `PanelMode` configuration selectable from the Windows menu
/// — see `docs/design/layout.md`'s Layout Presets section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutPreset {
    /// Sidebar + Viewport + Neural Viewer docked, Metrics/Event Log tabbed
    /// at the bottom. The default layout.
    Research,
    /// Sidebar and Neural Viewer closed, Viewport maximized, Metrics
    /// floating — for screen-sharing a clean simulation view.
    Presentation,
    /// Everything docked and visible, including panels a researcher might
    /// normally leave closed.
    Debug,
}

/// Apply a named layout preset: set every panel's `PanelMode`, clear any
/// stale persisted split ratios (a preset is a deliberate reset, not a drag),
/// and rebuild the tree.
pub fn apply_layout_preset(state: &mut WorkbenchState, preset: LayoutPreset) {
    let mut modes = std::collections::HashMap::new();
    for &name in ALL_PANEL_NAMES {
        modes.insert(name.to_string(), PanelMode::Docked);
    }
    match preset {
        LayoutPreset::Research => {
            modes.insert("Neural Viewer".to_string(), PanelMode::Closed);
            modes.insert("Research Dashboard".to_string(), PanelMode::Closed);
            modes.insert("Replay Browser".to_string(), PanelMode::Closed);
            // Phase 5, SX-8b: this preset's own doc comment (below, on
            // `LayoutPreset::Research`) and §2.4 of the roadmap both say
            // Evolution Debugger should be closed by default like the other
            // debug/analysis panels here — it just wasn't, until now. Found
            // by re-auditing this exact function while implementing the
            // panel-tier system, not a change made speculatively.
            modes.insert("Evolution Debugger".to_string(), PanelMode::Closed);
            modes.insert("Physiology Viewer".to_string(), PanelMode::Closed);
            modes.insert("Circulation Viewer".to_string(), PanelMode::Closed);
            modes.insert("Hormone Viewer".to_string(), PanelMode::Closed);
            modes.insert("Immune Viewer".to_string(), PanelMode::Closed);
            modes.insert("Cell Lineage Viewer".to_string(), PanelMode::Closed);
            modes.insert("Placeholder Panel".to_string(), PanelMode::Closed);
        }
        LayoutPreset::Presentation => {
            modes.insert("Sidebar".to_string(), PanelMode::Closed);
            modes.insert("Neural Viewer".to_string(), PanelMode::Closed);
            modes.insert("Research Dashboard".to_string(), PanelMode::Closed);
            modes.insert("Replay Browser".to_string(), PanelMode::Closed);
            // Phase 5, SX-8b: same fix as `LayoutPreset::Research` above —
            // Presentation is meant to be the most minimal preset, so it
            // can't be missing this while Research has it.
            modes.insert("Evolution Debugger".to_string(), PanelMode::Closed);
            modes.insert("Physiology Viewer".to_string(), PanelMode::Closed);
            modes.insert("Circulation Viewer".to_string(), PanelMode::Closed);
            modes.insert("Hormone Viewer".to_string(), PanelMode::Closed);
            modes.insert("Immune Viewer".to_string(), PanelMode::Closed);
            modes.insert("Cell Lineage Viewer".to_string(), PanelMode::Closed);
            modes.insert("Placeholder Panel".to_string(), PanelMode::Closed);
            modes.insert("Metrics".to_string(), PanelMode::Floating);
        }
        LayoutPreset::Debug => {
            // Everything docked, including Neural Viewer, Research
            // Dashboard, Replay Browser, and the P4-R-tier physiology/lineage
            // panels — Placeholder Panel stays closed even here since it
            // carries no real content a researcher would want visible by
            // default.
            modes.insert("Placeholder Panel".to_string(), PanelMode::Closed);
        }
    }

    state.panel_modes = modes;
    state.layout_shares.clear();
    rebuild_tree_from_modes(
        &mut state.dock_tree,
        &state.panel_modes,
        &state.layout_shares,
    );
}

/// Reset the entire workspace to its default (Research) layout. Used by both
/// "Reset Layout" menu entries so they can't drift out of sync with each
/// other.
pub fn apply_default_layout(state: &mut WorkbenchState) {
    apply_layout_preset(state, LayoutPreset::Research);
}

/// Toggle Focus Mode (Phase 2, M16): closes every panel except the
/// Viewport, remembering the prior arrangement in
/// `WorkbenchState::focus_mode_previous` so a second toggle restores it
/// exactly — a fullscreen-viewport toggle, not a fourth named preset.
/// Entirely UI-side, following the same direct-call pattern
/// `apply_layout_preset` already uses from `menu.rs` (no `MenuAction`
/// round-trip needed for panel-arrangement changes).
pub fn toggle_focus_mode(state: &mut WorkbenchState) {
    if let Some(previous) = state.focus_mode_previous.take() {
        state.panel_modes = previous;
    } else {
        state.focus_mode_previous = Some(state.panel_modes.clone());
        let mut modes = std::collections::HashMap::new();
        for &name in ALL_PANEL_NAMES {
            let mode = if name == "Viewport" {
                PanelMode::Docked
            } else {
                PanelMode::Closed
            };
            modes.insert(name.to_string(), mode);
        }
        state.panel_modes = modes;
    }
    state.layout_shares.clear();
    rebuild_tree_from_modes(
        &mut state.dock_tree,
        &state.panel_modes,
        &state.layout_shares,
    );
}

/// Immediately remove a named panel's tile from the tree, if present.
///
/// `retain_pane` already causes egui_tiles to drop Floating/Closed panes on
/// the next `simplify` pass, but that happens lazily (after the next `ui()`
/// call) which can leave a stale empty container visible for one frame, or
/// leave the tile present if `ui()` is never called again for that container
/// (e.g. a docked panel closed while another is being dragged). Calling this
/// eagerly from the `DetachPanel`/`ClosePanel` handlers keeps the tree
/// consistent immediately.
pub fn remove_panel_from_tree(tree: &mut egui_tiles::Tree<String>, name: &str) {
    if let Some(tile_id) = tree.tiles.find_pane(&name.to_string()) {
        tree.remove_recursively(tile_id);
        tree.simplify(&egui_tiles::SimplificationOptions::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 5, SX-8b: §2.4 of the roadmap found that `LayoutPreset::Research`
    /// left Evolution Debugger docked by default despite this same function's
    /// own doc comment claiming it was treated like Research Dashboard/Replay
    /// Browser (both correctly closed). Proves the fix, not just the comment.
    #[test]
    fn research_preset_closes_evolution_debugger() {
        let mut state = WorkbenchState::default();
        apply_layout_preset(&mut state, LayoutPreset::Research);
        assert_eq!(
            state.panel_modes.get("Evolution Debugger"),
            Some(&PanelMode::Closed)
        );
    }

    /// Presentation is meant to be the most minimal preset (screen-sharing a
    /// clean simulation view) — it can't leave a debug panel open that the
    /// default Research preset closes.
    #[test]
    fn presentation_preset_closes_evolution_debugger() {
        let mut state = WorkbenchState::default();
        apply_layout_preset(&mut state, LayoutPreset::Presentation);
        assert_eq!(
            state.panel_modes.get("Evolution Debugger"),
            Some(&PanelMode::Closed)
        );
    }

    /// Debug preset is the one place Evolution Debugger should still default
    /// to visible — "everything docked" is its whole purpose.
    #[test]
    fn debug_preset_leaves_evolution_debugger_docked() {
        let mut state = WorkbenchState::default();
        apply_layout_preset(&mut state, LayoutPreset::Debug);
        assert_eq!(
            state.panel_modes.get("Evolution Debugger"),
            Some(&PanelMode::Docked)
        );
    }

    /// Phase 5, SX-8a/8b: a spot-check of the tier classification driving
    /// `chrome_bar`'s accent bar/title color — Sidebar (hosts Inspector) is
    /// Contextual, Metrics (an aggregate dashboard) is Secondary. Not
    /// exhaustive over all 14 panels (the classification is a static match,
    /// not derived logic that could silently drift), just proof the two
    /// tiers are reachable and distinct.
    #[test]
    fn panel_tier_distinguishes_contextual_from_secondary() {
        assert_eq!(panel_tier("Sidebar"), PanelTier::Contextual);
        assert_eq!(panel_tier("Neural Viewer"), PanelTier::Contextual);
        assert_eq!(panel_tier("Metrics"), PanelTier::Secondary);
        assert_eq!(panel_tier("Evolution Debugger"), PanelTier::Secondary);
    }
}
