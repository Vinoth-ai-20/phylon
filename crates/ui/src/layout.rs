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
pub const ALL_PANEL_NAMES: &[&str] = &[
    "Sidebar",
    "Viewport",
    "Metrics",
    "Event Log",
    "Neural Viewer",
];

/// Standard opaque panel background colour, used for every non-Viewport pane
/// (docked or floating) so the wgpu simulation world's colours never bleed
/// through transparent egui surfaces.
pub const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(24, 24, 28);

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
            .fill(PANEL_BG)
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

        // Close button
        if ui
            .small_button(
                egui::RichText::new(egui_remixicon::icons::CLOSE_LINE)
                    .color(egui::Color32::from_rgb(180, 80, 80))
                    .size(12.0),
            )
            .on_hover_text(format!("Close {} (reopen via Windows menu)", name))
            .clicked()
        {
            self.commands.push(MenuAction::ClosePanel(name.clone()));
        }

        // Detach button
        if ui
            .small_button(
                egui::RichText::new(egui_remixicon::icons::EXTERNAL_LINK_LINE)
                    .color(egui::Color32::from_rgb(150, 150, 220))
                    .size(12.0),
            )
            .on_hover_text(format!("Detach {} into a floating window", name))
            .clicked()
        {
            self.commands.push(MenuAction::DetachPanel(name));
        }
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

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
        |ui| {
            ui.add_space(crate::theme::SPACE_XS);
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .size(crate::theme::SIZE_SUBHEADING),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(crate::theme::SPACE_XS);

                // Close button
                if ui
                    .small_button(
                        egui::RichText::new(egui_remixicon::icons::CLOSE_LINE)
                            .color(egui::Color32::from_rgb(180, 80, 80))
                            .size(12.0),
                    )
                    .on_hover_text(format!("Close {} (reopen via Windows menu)", name))
                    .clicked()
                {
                    commands.push(MenuAction::ClosePanel(name.to_string()));
                }

                // Detach button
                if ui
                    .small_button(
                        egui::RichText::new(egui_remixicon::icons::EXTERNAL_LINK_LINE)
                            .color(egui::Color32::from_rgb(150, 150, 220))
                            .size(12.0),
                    )
                    .on_hover_text(format!("Detach {} into a floating window", name))
                    .clicked()
                {
                    commands.push(MenuAction::DetachPanel(name.to_string()));
                }
            });
        },
    );

    ui.separator();
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
                    .fill(PANEL_BG)
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
                    "Viewport" => {
                        crate::plugins::viewport::viewport_ui(
                            ctx,
                            ui,
                            state,
                            canvas_interaction,
                            commands,
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
    ui.horizontal(|ui| {
        // Drag handle icon
        ui.label(
            egui::RichText::new(egui_remixicon::icons::DRAG_MOVE_2_LINE)
                .color(egui::Color32::from_gray(120))
                .size(14.0),
        );

        // Panel title
        ui.label(
            egui::RichText::new(format!(
                "{} {}",
                egui_remixicon::icons::WINDOW_2_LINE,
                title
            ))
            .strong()
            .size(crate::theme::SIZE_SUBHEADING),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // ── Close button ──
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(egui_remixicon::icons::CLOSE_LINE)
                            .color(egui::Color32::from_rgb(220, 80, 80))
                            .size(13.0),
                    )
                    .min_size(egui::vec2(20.0, 20.0)),
                )
                .on_hover_text("Close (reopen via Windows › menu)")
                .clicked()
            {
                commands.push(MenuAction::ClosePanel(name.to_string()));
            }

            // ── Dock button ──
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(egui_remixicon::icons::PICTURE_IN_PICTURE_EXIT_LINE)
                            .size(13.0),
                    )
                    .min_size(egui::vec2(20.0, 20.0)),
                )
                .on_hover_text("Dock — attach back to the main layout")
                .clicked()
            {
                commands.push(MenuAction::DockPanel(name.to_string()));
            }

            // ── Minimize / Restore button ──
            let (min_icon, min_tip) = if *minimized {
                (egui_remixicon::icons::ARROW_UP_S_LINE, "Restore window")
            } else {
                (
                    egui_remixicon::icons::SUBTRACT_LINE,
                    "Minimize to title bar",
                )
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(min_icon)
                            .color(egui::Color32::from_rgb(180, 180, 60))
                            .size(13.0),
                    )
                    .min_size(egui::vec2(20.0, 20.0)),
                )
                .on_hover_text(min_tip)
                .clicked()
            {
                *minimized = !*minimized;
            }
        });
    });
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
) {
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

    // Bottom tab strip: Metrics | Event Log (whichever are docked).
    let bottom_panes: Vec<egui_tiles::TileId> =
        [metrics, event_log].into_iter().flatten().collect();
    let bottom_tabs = if bottom_panes.len() > 1 {
        Some(tiles.insert_tab_tile(bottom_panes))
    } else {
        bottom_panes.first().copied()
    };

    // Right column: Viewport (3/4) + bottom_tabs (1/4), or just Viewport.
    let right_col = match bottom_tabs {
        Some(bottom_tabs) => {
            let right_col = tiles.insert_vertical_tile(vec![viewport, bottom_tabs]);
            if let egui_tiles::Tile::Container(egui_tiles::Container::Linear(linear)) =
                tiles.get_mut(right_col).unwrap()
            {
                linear.shares.set_share(viewport, 3.0);
                linear.shares.set_share(bottom_tabs, 1.0);
            }
            right_col
        }
        None => viewport,
    };

    // Root row: Sidebar (1) | right_col (3) | Neural Viewer (1), omitting
    // whichever of Sidebar/Neural Viewer aren't currently docked.
    let mut root_children = Vec::new();
    let mut root_shares = Vec::new();
    if let Some(sidebar) = sidebar {
        root_children.push(sidebar);
        root_shares.push(1.0);
    }
    root_children.push(right_col);
    root_shares.push(3.0);
    if let Some(neural_viewer) = neural_viewer {
        root_children.push(neural_viewer);
        root_shares.push(1.0);
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

/// Reset the entire workspace to its default layout: every panel except
/// "Neural Viewer" docked in its canonical home slot. Used by both "Reset
/// Layout" menu entries so they can't drift out of sync with each other.
pub fn apply_default_layout(state: &mut WorkbenchState) {
    state.panel_modes = crate::state::default_panel_modes();
    rebuild_tree_from_modes(&mut state.dock_tree, &state.panel_modes);
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
