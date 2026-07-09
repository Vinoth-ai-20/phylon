//! Shared node-link graph canvas helpers (pan/zoom/hit-testing) — extracted
//! from `plugins::neural_viewer` (Phase 3, M11) so the GRN Viewer panel can
//! navigate a `genetics::RegulatoryNetwork` graph the same way Neural Viewer
//! navigates a `Brain`/`Cppn` graph, instead of duplicating this math.
//!
//! Extended (Phase 7, W2c) with the *layout-independent* rendering pieces
//! that were duplicated identically across Neural Viewer's CTRNN/CPPN
//! canvases and the GRN Viewer's canvas: canvas setup boilerplate, the
//! node fill+stroke paint primitive, and the edge color/width formula. This
//! module owns HOW to render a generic node-link graph; it never owns WHAT
//! a node/edge means — layout algorithms, node classification, liveness
//! indicators, and tooltip content all stay in each viewer, by design (see
//! ADR-W2-01).

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

/// Shortest distance from `p` to the line segment `a`–`b` — the geometric
/// core of [`hit_test_edge`]. Pure math, zero domain content (Phase 7,
/// W2c — moved here from `plugins::neural_viewer`, its only prior user,
/// purely for cohesion alongside `hit_test_node`; behavior is unchanged).
fn dist_to_segment(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    p.distance(closest)
}

/// Nearest edge (by index into `edges`) to `pointer`, within a small
/// hit-test tolerance of the segment. `edges` holds endpoint indices into
/// `positions`.
pub(crate) fn hit_test_edge(
    pointer: egui::Pos2,
    positions: &[egui::Pos2],
    edges: &[(usize, usize)],
) -> Option<usize> {
    const TOLERANCE: f32 = 4.0;
    edges
        .iter()
        .enumerate()
        .filter_map(|(i, &(src, dst))| {
            if src >= positions.len() || dst >= positions.len() {
                return None;
            }
            let d = dist_to_segment(pointer, positions[src], positions[dst]);
            (d <= TOLERANCE).then_some((i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Allocates a graph canvas's painter, applies pan/zoom input, and fills its
/// background — the setup sequence every node-link graph canvas in this UI
/// repeated identically before W2c, differing only in `height`/`background`.
/// Returns the interaction `Response` (for later hover/hit-testing), the
/// `Painter` to draw into, and its `Rect` (for layout math and
/// `apply_view`).
pub(crate) fn begin_graph_canvas(
    ui: &mut egui::Ui,
    height: f32,
    background: egui::Color32,
    view: &mut crate::state::GraphViewState,
) -> (egui::Response, egui::Painter, egui::Rect) {
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click_and_drag(),
    );
    handle_pan_zoom(ui, &response, view);
    let rect = response.rect;
    painter.rect_filled(rect, egui::Rounding::same(4.0), background);
    (response, painter, rect)
}

/// Which primitive shape a node paints as — the CTRNN/GRN graphs use
/// circles, the CPPN graph deliberately uses squares as a visual cue
/// distinguishing "genotype" from "phenotype" beyond the text header (see
/// `plugins::neural_viewer::draw_cppn_graph`'s doc comment). The choice of
/// shape is a per-viewer decision; only the act of painting one is shared.
pub(crate) enum NodeShape {
    Circle,
    Square,
}

/// Paints one node: a filled shape plus its outline stroke, at `pos` with
/// `radius`. Fill color, stroke, and shape are all caller-decided — this
/// function only knows HOW to paint a circle or square, never WHAT a node
/// represents.
pub(crate) fn draw_node(
    painter: &egui::Painter,
    pos: egui::Pos2,
    radius: f32,
    fill: egui::Color32,
    stroke: egui::Stroke,
    shape: NodeShape,
) {
    match shape {
        NodeShape::Circle => {
            painter.circle_filled(pos, radius, fill);
            painter.circle_stroke(pos, radius, stroke);
        }
        NodeShape::Square => {
            let square = egui::Rect::from_center_size(pos, egui::vec2(radius * 2.0, radius * 2.0));
            painter.rect_filled(square, egui::Rounding::same(1.0), fill);
            painter.rect_stroke(square, egui::Rounding::same(1.0), stroke);
        }
    }
}

/// The edge color/width formula every node-link graph canvas in this UI
/// applied identically: a weight's magnitude (clamped to a max of 3.0)
/// linearly drives both alpha (80–220) and stroke width (0.5–2.5), while its
/// sign picks between the caller's two base colors. Callers supply their own
/// `positive_base`/`negative_base` (e.g. synapse excitatory/inhibitory vs.
/// gene activator/repressor) — this function never picks a color itself,
/// only shapes one given the caller's choice, so distinct domain vocabularies
/// stay distinct (Phase 7, W2c re-audit deliberately did not merge
/// `neural_viewer`'s and `grn_viewer`'s same-valued color constants — see
/// ADR-W2-01).
pub(crate) fn weighted_edge_stroke(
    weight: f32,
    positive_base: egui::Color32,
    negative_base: egui::Color32,
) -> (egui::Color32, f32) {
    let strength = (weight.abs() / 3.0).min(1.0);
    let alpha = (80.0 + 140.0 * strength) as u8;
    let base = if weight >= 0.0 {
        positive_base
    } else {
        negative_base
    };
    let color = egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha);
    let width = 0.5 + 2.0 * strength;
    (color, width)
}
