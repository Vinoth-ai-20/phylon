//! Shared node-link graph canvas helpers (pan/zoom/hit-testing) — extracted
//! from `plugins::neural_viewer` (Phase 3, M11) so the GRN Viewer panel can
//! navigate a `genetics::RegulatoryNetwork` graph the same way Neural Viewer
//! navigates a `Brain`/`Cppn` graph, instead of duplicating this math.

/// Transforms a layout-space position into screen space via `view`'s
/// current pan/zoom, anchored at `rect`'s center.
pub(crate) fn apply_view(
    pos: egui::Pos2,
    rect: egui::Rect,
    view: &crate::state::GraphViewState,
) -> egui::Pos2 {
    rect.center() + (pos - rect.center()) * view.zoom + view.pan
}

/// Reads scroll (zoom, only while hovering the canvas) and drag (pan) input
/// from a graph canvas's response into its `GraphViewState` — shared by
/// every node-link graph canvas in the UI, so a large graph is as navigable
/// in one view as another.
pub(crate) fn handle_pan_zoom(
    ui: &egui::Ui,
    response: &egui::Response,
    view: &mut crate::state::GraphViewState,
) {
    if response.dragged() {
        view.pan += response.drag_delta();
    }
    if response.hovered() {
        let scroll_y = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll_y != 0.0 {
            view.zoom = (view.zoom * (1.0 + scroll_y * 0.001)).clamp(0.2, 4.0);
        }
    }
}

/// Nearest node to `pointer` within `radius` + a small hit-test tolerance.
pub(crate) fn hit_test_node(
    pointer: egui::Pos2,
    positions: &[egui::Pos2],
    radius: f32,
) -> Option<usize> {
    let tolerance = radius + 3.0;
    positions
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.distance(pointer)))
        .filter(|(_, d)| *d <= tolerance)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}
