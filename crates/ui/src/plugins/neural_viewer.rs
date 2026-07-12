//! Neural Viewer plugin — renders both the phenotype (CTRNN runtime brain)
//! and the genotype (brain CPPN) of the currently selected/tracked organism
//! as node-link graphs.
//!
//! Both graphs are kept in one panel rather than split across two, since
//! they're both "the brain," just at different levels: the CPPN is what
//! evolution encodes (the genotype), the CTRNN is what actually runs each
//! tick (the phenotype it produces).

use crate::types::*;

// `apply_view`/`handle_pan_zoom`/`hit_test_node` live in `crate::graph_canvas`
// so the GRN Viewer panel can reuse the same pan/zoom/hit-test math instead
// of duplicating it. `begin_graph_canvas`/`draw_node`/`weighted_edge_stroke`/
// `hit_test_edge` are the layout-independent rendering pieces this file, GRN
// Viewer, and future graph viewers all share — see `graph_canvas`'s module
// doc comment.
use crate::graph_canvas::{
    apply_view, begin_graph_canvas, draw_node, hit_test_edge, hit_test_node, weighted_edge_stroke,
    NodeShape,
};

// Node/synapse colors are shared between the CTRNN (phenotype) and CPPN
// (genotype) canvases below — named here once rather than repeating the same
// literals at 4 call sites per color. Canvas backgrounds are deliberately
// NOT unified: `CTRNN_CANVAS_BG`/`CPPN_CANVAS_BG` differ on purpose (see
// `render_cppn_graph`'s doc comment) as the visual cue distinguishing the two
// graphs beyond their text headers.
const NODE_INPUT: egui::Color32 = egui::Color32::from_rgb(120, 220, 120);
const NODE_HIDDEN: egui::Color32 = egui::Color32::from_rgb(150, 170, 255);
const NODE_OUTPUT: egui::Color32 = egui::Color32::from_rgb(255, 180, 90);
const SYNAPSE_EXCITATORY_BASE: egui::Color32 = egui::Color32::from_rgb(90, 200, 255);
const SYNAPSE_INHIBITORY_BASE: egui::Color32 = egui::Color32::from_rgb(255, 100, 100);
const CTRNN_CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(16, 16, 20);
const CPPN_CANVAS_BG: egui::Color32 = egui::Color32::from_rgb(14, 18, 26);

/// Renders the Neural Viewer panel content.
#[allow(clippy::too_many_arguments)]
pub fn neural_viewer_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut crate::WorkbenchState,
    world: &mut world::World,
    _actions: &mut Vec<MenuAction>,
) {
    let Some(selected) = state.selected_entity.or(state.tracked_entity) else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Select an organism to view its brain")
                    .color(crate::theme::DISABLED_FG)
                    .italics(),
            );
        });
        return;
    };

    // The `Brain`/`Genome` components live on the organism's core/head
    // entity. If a non-head body segment is selected instead, fall back to
    // the head node of the same organism.
    let brain_entity = {
        let mut brain_q = world.ecs.query::<&brain::Brain>();
        if brain_q.get(&world.ecs, selected).is_ok() {
            Some(selected)
        } else {
            let organism_id = {
                let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                node_q.get(&world.ecs, selected).ok().map(|n| n.organism_id)
            };
            organism_id.and_then(|organism_id| {
                let mut node_q = world
                    .ecs
                    .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
                node_q
                    .iter(&world.ecs)
                    .find(|(_, n)| n.organism_id == organism_id && n.segment_type == 0)
                    .map(|(e, _)| e)
            })
        }
    };

    let Some(brain_entity) = brain_entity else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Selected organism has no Brain component")
                    .color(crate::theme::DISABLED_FG)
                    .italics(),
            );
        });
        return;
    };

    // Fetched together (rather than as two separate `world.ecs.query()`
    // calls) because building a second `QueryState` needs `&mut world.ecs`,
    // which would conflict with the immutable borrow of `b` that lives on
    // into the closure below.
    let mut q = world
        .ecs
        .query::<(&brain::Brain, Option<&genetics::Genome>)>();
    let Ok((b, genome)) = q.get(&world.ecs, brain_entity) else {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Selected organism has no Brain component")
                    .color(crate::theme::DISABLED_FG)
                    .italics(),
            );
        });
        return;
    };

    ui.label(format!(
        "BrainId {} — {} nodes, {} synapses ({} in / {} out / {} hidden)",
        b.id.0,
        b.nodes.len(),
        b.synapses.len(),
        b.input_count,
        b.output_count,
        b.nodes.len().saturating_sub(b.input_count + b.output_count),
    ));
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            graph_header(
                ui,
                "Phenotype — CTRNN Runtime Network",
                &mut state.neural_ctrnn_view,
            );
            draw_brain_graph(ui, b, &mut state.neural_ctrnn_view);

            ui.add_space(crate::theme::SPACE_MD);

            if let Some(genome) = genome {
                ui.separator();
                ui.add_space(crate::theme::SPACE_MD);
                graph_header(ui, "Genotype — Brain CPPN", &mut state.neural_cppn_view);
                draw_cppn_graph(ui, &genome.brain_cppn, &mut state.neural_cppn_view);
            }
        });
}

// ─── Shared graph-drawing helpers ───────────────────────────────────────────

/// Human-readable sense name for an input node, by its index within the
/// input column — which corresponds 1:1 with `SensoryState.inputs`, since
/// `brain.set_inputs(&sensory.inputs)` (`crates/app/src/simulation.rs`)
/// copies that array straight into the brain's first `input_count` node
/// states in order. Mirrors the exact assembly order in
/// `sensing::sensing_system` (`crates/sensing/src/lib.rs`): olfaction/signal/
/// hazard field samples, ATP/age proprioception, three vision bins, then the
/// internal pacemaker. Falls back to a generic "Input N" for any index this
/// organism's sensor set doesn't populate (e.g. no `HeadVision`, so no
/// vision/pacemaker slots) or that's beyond this known layout.
fn input_sense_name(index: usize) -> std::borrow::Cow<'static, str> {
    const NAMES: &[&str] = &[
        "Olfaction",
        "Signal",
        "Hazard",
        "Energy (ATP)",
        "Age",
        "Vision - Left",
        "Vision - Center",
        "Vision - Right",
        "Pacemaker",
    ];
    match NAMES.get(index) {
        Some(name) => std::borrow::Cow::Borrowed(name),
        None => std::borrow::Cow::Owned(format!("Input {index}")),
    }
}

/// Human-readable name for a CPPN input node, by its index within the input
/// layer. Unlike the CTRNN's inputs, these aren't senses — a CPPN is queried
/// with normalized *positions* (HyperNEAT-style substrate coordinates), not
/// live sensory readings. `crates/organisms/src/systems.rs`'s `growth_system`
/// always calls `brain_cppn.evaluate(&[i, j])` with exactly these two
/// normalized node-index positions: both set to the same node's position
/// when querying that node's own bias/time-constant, or to the
/// source/target pair when querying a connection weight between two nodes.
fn cppn_input_name(index: usize) -> &'static str {
    const NAMES: &[&str] = &["Source Position", "Target Position"];
    NAMES.get(index).copied().unwrap_or("Input")
}

/// Human-readable name for a CTRNN output node, by its index within the
/// output column. `growth_system` (`crates/organisms/src/systems.rs`) always
/// builds `output_count = effectors.len() + 1`: one node per muscle/fin
/// effector spring, followed by exactly one final node driving the
/// organism's `SignalEmitter`. The neural viewer doesn't have the
/// muscle-vs-fin split available (that mapping only exists transiently in
/// `GrowthState` during wiring), so non-final effectors are just numbered.
fn output_name(index: usize, output_count: usize) -> std::borrow::Cow<'static, str> {
    if output_count > 0 && index == output_count - 1 {
        std::borrow::Cow::Borrowed("Signal Emitter")
    } else {
        std::borrow::Cow::Owned(format!("Muscle {index}"))
    }
}

/// Human-readable name for a CPPN output node, by its index within the
/// output layer. `growth_system` always reads `brain_cppn.evaluate(...)`
/// results in this fixed order: `[0]` as a connection weight, `[1]` as a
/// node's bias, `[2]` as its time constant (see `cppn_input_name`'s doc for
/// the matching input-side call sites).
fn cppn_output_name(index: usize) -> std::borrow::Cow<'static, str> {
    const NAMES: &[&str] = &["Weight", "Bias", "Time Constant"];
    match NAMES.get(index) {
        Some(name) => std::borrow::Cow::Borrowed(*name),
        None => std::borrow::Cow::Owned(format!("Output {index}")),
    }
}

/// Human-readable name for a hidden node (CTRNN or CPPN), by its index
/// within the hidden layer/column. Hidden nodes have no fixed structural
/// meaning — their role emerges from evolved weights — so this just gives
/// each a distinct, numbered identity instead of a bare repeated "Hidden".
fn hidden_name(index: usize) -> String {
    format!("Hidden {index}")
}

/// Human-readable name for a [`brain::ActivationFn`] code, as used by
/// `CtrnnNode::activation` (see `brain::Brain::apply_activation`).
fn activation_name(act_id: u32) -> &'static str {
    match act_id {
        0 => "Sigmoid",
        1 => "Tanh",
        2 => "ReLU",
        3 => "LeakyReLU",
        4 => "Sine",
        5 => "Gaussian",
        6 => "Abs",
        7 => "Linear",
        8 => "Step",
        _ => "Unknown",
    }
}

/// Title row for a graph canvas: the section heading, a live zoom readout,
/// and a "Reset View" button (only shown once the view has actually moved
/// away from the default) so a user who's zoomed/panned into a large genome
/// always has a one-click way back.
fn graph_header(ui: &mut egui::Ui, title: &str, view: &mut crate::state::GraphViewState) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).strong());
        ui.label(
            egui::RichText::new(format!("{:.0}%", view.zoom * 100.0))
                .small()
                .color(crate::theme::DISABLED_FG),
        );
        if ((view.zoom - 1.0).abs() > 0.01 || view.pan != egui::Vec2::ZERO)
            && ui.small_button("Reset View").clicked()
        {
            *view = crate::state::GraphViewState::default();
        }
    });
    ui.label(
        egui::RichText::new("Scroll to zoom · drag to pan")
            .small()
            .color(crate::theme::DISABLED_FG),
    );
}

/// Draws a CTRNN brain graph: nodes as circles positioned by column (input /
/// hidden / output) and index-within-column, synapses as lines colored by
/// weight sign and shaded by magnitude. Hovering a node or synapse shows its
/// details in a tooltip.
fn draw_brain_graph(ui: &mut egui::Ui, b: &brain::Brain, view: &mut crate::state::GraphViewState) {
    if b.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty network.")
                .italics()
                .color(crate::theme::DISABLED_FG),
        );
        return;
    }

    let height = 240.0_f32.max(b.nodes.len() as f32 * 4.0).min(480.0);
    let (response, painter, rect) = begin_graph_canvas(ui, height, CTRNN_CANVAS_BG, view);

    // Classify each node index into a column: 0 = input, 1 = hidden, 2 = output.
    let input_count = b.input_count.min(b.nodes.len());
    let output_count = b
        .output_count
        .min(b.nodes.len().saturating_sub(input_count));
    let hidden_end = b.nodes.len().saturating_sub(output_count);

    let column_of = |idx: usize| -> usize {
        if idx < input_count {
            0
        } else if idx >= hidden_end {
            2
        } else {
            1
        }
    };
    let column_name = |col: usize| -> &'static str {
        match col {
            0 => "Input",
            2 => "Output",
            _ => "Hidden",
        }
    };

    let mut columns: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for idx in 0..b.nodes.len() {
        columns[column_of(idx)].push(idx);
    }
    // Index-within-column, keyed by node index, for the "Number" tooltip field.
    let mut number_in_column = vec![0usize; b.nodes.len()];
    for col in &columns {
        for (n, &idx) in col.iter().enumerate() {
            number_in_column[idx] = n;
        }
    }

    let margin = 24.0;
    let usable_w = (rect.width() - 2.0 * margin).max(1.0);
    let usable_h = (rect.height() - 2.0 * margin).max(1.0);

    let mut positions = vec![egui::Pos2::ZERO; b.nodes.len()];
    for (col_idx, indices) in columns.iter().enumerate() {
        let x = rect.left() + margin + usable_w * (col_idx as f32 / 2.0);
        let n = indices.len();
        for (slot, &node_idx) in indices.iter().enumerate() {
            let y = rect.top()
                + margin
                + if n > 1 {
                    usable_h * (slot as f32 / (n - 1) as f32)
                } else {
                    usable_h * 0.5
                };
            positions[node_idx] = egui::pos2(x, y);
        }
    }
    for pos in &mut positions {
        *pos = apply_view(*pos, rect, view);
    }
    let node_radius_scaled = 6.0 * view.zoom;

    // Synapses first, so nodes render on top.
    let edges: Vec<(usize, usize)> = b
        .synapses
        .iter()
        .map(|syn| (syn.source as usize, syn.target as usize))
        .collect();
    for syn in &b.synapses {
        let (source, target) = (syn.source as usize, syn.target as usize);
        if source >= positions.len() || target >= positions.len() {
            continue;
        }
        let (color, width) =
            weighted_edge_stroke(syn.weight, SYNAPSE_EXCITATORY_BASE, SYNAPSE_INHIBITORY_BASE);
        painter.line_segment(
            [positions[source], positions[target]],
            egui::Stroke::new(width, color),
        );
    }

    // Nodes.
    let node_radius = node_radius_scaled;
    for (idx, node) in b.nodes.iter().enumerate() {
        let color = match column_of(idx) {
            0 => NODE_INPUT,
            2 => NODE_OUTPUT,
            _ => NODE_HIDDEN,
        };
        draw_node(
            &painter,
            positions[idx],
            node_radius,
            color,
            egui::Stroke::new(1.0_f32, egui::Color32::from_gray(20)),
            NodeShape::Circle,
        );

        // Fill level indicator: a smaller inner dot brightness-scaled by the
        // node's current activation state, so the graph doubles as a live
        // activity monitor.
        let activity = brain::Brain::apply_activation(node.state + node.bias, node.activation);
        let inner_t = (activity.tanh().abs()).clamp(0.0, 1.0);
        painter.circle_filled(
            positions[idx],
            node_radius * 0.5,
            egui::Color32::from_white_alpha((inner_t * 255.0) as u8),
        );
    }

    // Hover tooltip: nodes take priority over edges since they're drawn on
    // top and are the more common target.
    if let Some(pointer) = response.hover_pos() {
        if let Some(idx) = hit_test_node(pointer, &positions, node_radius) {
            let node = &b.nodes[idx];
            let col = column_of(idx);
            // Every node gets a specific name, not just its generic column:
            // inputs by sense (Olfaction, Age, Vision, ...), outputs by
            // effector (Muscle N / Signal Emitter), hidden nodes numbered.
            let node_name: std::borrow::Cow<'static, str> = match col {
                0 => input_sense_name(number_in_column[idx]),
                2 => output_name(number_in_column[idx], output_count),
                _ => std::borrow::Cow::Owned(hidden_name(number_in_column[idx])),
            };
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("brain_node_tooltip"),
                |ui| {
                    ui.label(egui::RichText::new(node_name.as_ref()).strong());
                    egui::Grid::new("brain_node_tooltip_grid")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Name:");
                            ui.label(node_name.as_ref());
                            ui.end_row();
                            ui.label("Role:");
                            ui.label(column_name(col));
                            ui.end_row();
                            ui.label("Number:");
                            ui.label(number_in_column[idx].to_string());
                            ui.end_row();
                            ui.label("ID:");
                            ui.label(idx.to_string());
                            ui.end_row();
                            ui.label("Bias:");
                            ui.label(format!("{:.4}", node.bias));
                            ui.end_row();
                            ui.label("Activation:");
                            ui.label(activation_name(node.activation));
                            ui.end_row();
                            ui.label("State:");
                            ui.label(format!("{:.4}", node.state));
                            ui.end_row();
                        });
                },
            );
        } else if let Some(edge_idx) = hit_test_edge(pointer, &positions, &edges) {
            let syn = &b.synapses[edge_idx];
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("brain_edge_tooltip"),
                |ui| {
                    ui.label(format!("Synapse {} → {}", syn.source, syn.target));
                    ui.label(format!("Weight: {:.4}", syn.weight));
                },
            );
        }
    }

    ui.horizontal(|ui| {
        crate::widgets::chart_legend_dot(ui, NODE_INPUT, "input");
        crate::widgets::chart_legend_dot(ui, NODE_HIDDEN, "hidden");
        crate::widgets::chart_legend_dot(ui, NODE_OUTPUT, "output");
        ui.label(
            egui::RichText::new("— blue = excitatory, red = inhibitory")
                .small()
                .color(crate::theme::DISABLED_FG),
        );
    });
}

/// Draws a CPPN (brain-genotype) graph: nodes as circles positioned by layer
/// (left → right) and index-within-layer (top → bottom), connections as
/// lines colored by weight sign and shaded by magnitude. Hovering a node or
/// connection shows its details in a tooltip.
fn draw_cppn_graph(
    ui: &mut egui::Ui,
    cppn: &genetics::cppn::Cppn,
    view: &mut crate::state::GraphViewState,
) {
    if cppn.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty network.")
                .italics()
                .color(crate::theme::DISABLED_FG),
        );
        return;
    }

    let height = 200.0_f32.max(cppn.nodes.len() as f32 * 4.0).min(360.0);
    // A distinct blue-tinted background (vs. the CTRNN graph's neutral
    // near-black) plus square nodes below visually differentiate this from
    // the CTRNN canvas beyond the plain text header — this is the
    // *genotype* (evolved blueprint), not the running network.
    let (response, painter, rect) = begin_graph_canvas(ui, height, CPPN_CANVAS_BG, view);

    // Group node indices by layer, preserving genome order within a layer.
    let max_layer = cppn.nodes.iter().map(|n| n.layer).max().unwrap_or(0);
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for (idx, node) in cppn.nodes.iter().enumerate() {
        layers[node.layer].push(idx);
    }
    let layer_name = |layer: usize| -> &'static str {
        if layer == 0 {
            "Input"
        } else if layer == max_layer {
            "Output"
        } else {
            "Hidden"
        }
    };
    let mut number_in_layer = vec![0usize; cppn.nodes.len()];
    for layer in &layers {
        for (n, &idx) in layer.iter().enumerate() {
            number_in_layer[idx] = n;
        }
    }

    let margin = 24.0;
    let usable_w = (rect.width() - 2.0 * margin).max(1.0);
    let usable_h = (rect.height() - 2.0 * margin).max(1.0);

    let mut positions = vec![egui::Pos2::ZERO; cppn.nodes.len()];
    for (layer_idx, indices) in layers.iter().enumerate() {
        let x = rect.left()
            + margin
            + if max_layer > 0 {
                usable_w * (layer_idx as f32 / max_layer as f32)
            } else {
                usable_w * 0.5
            };
        let n = indices.len();
        for (slot, &node_idx) in indices.iter().enumerate() {
            let y = rect.top()
                + margin
                + if n > 1 {
                    usable_h * (slot as f32 / (n - 1) as f32)
                } else {
                    usable_h * 0.5
                };
            positions[node_idx] = egui::pos2(x, y);
        }
    }
    for pos in &mut positions {
        *pos = apply_view(*pos, rect, view);
    }
    let node_radius_scaled = 5.0 * view.zoom;

    // Edges first, so nodes render on top.
    let edges: Vec<(usize, usize)> = cppn
        .connections
        .iter()
        .filter(|c| c.enabled)
        .map(|c| (c.source, c.target))
        .collect();
    for conn in &cppn.connections {
        if !conn.enabled || conn.source >= positions.len() || conn.target >= positions.len() {
            continue;
        }
        let (color, width) = weighted_edge_stroke(
            conn.weight,
            SYNAPSE_EXCITATORY_BASE,
            SYNAPSE_INHIBITORY_BASE,
        );
        painter.line_segment(
            [positions[conn.source], positions[conn.target]],
            egui::Stroke::new(width, color),
        );
    }

    // Nodes — drawn as squares (vs. the CTRNN graph's circles) so the two
    // graphs are visually distinguishable even zoomed out or screenshotted
    // without their headers.
    let node_radius = node_radius_scaled;
    for (idx, node) in cppn.nodes.iter().enumerate() {
        let color = match node.layer {
            0 => NODE_INPUT,
            l if l == max_layer => NODE_OUTPUT,
            _ => NODE_HIDDEN,
        };
        draw_node(
            &painter,
            positions[idx],
            node_radius,
            color,
            egui::Stroke::new(1.0_f32, egui::Color32::from_gray(20)),
            NodeShape::Square,
        );
    }

    if let Some(pointer) = response.hover_pos() {
        if let Some(idx) = hit_test_node(pointer, &positions, node_radius) {
            let node = &cppn.nodes[idx];
            // Every node gets a specific name, not just its generic layer:
            // inputs are Source/Target Position, outputs are what they
            // configure (Weight/Bias/Time Constant), hidden nodes numbered.
            let name: std::borrow::Cow<'static, str> = if node.layer == 0 {
                std::borrow::Cow::Borrowed(cppn_input_name(number_in_layer[idx]))
            } else if node.layer == max_layer {
                cppn_output_name(number_in_layer[idx])
            } else {
                std::borrow::Cow::Owned(hidden_name(number_in_layer[idx]))
            };
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("cppn_node_tooltip"),
                |ui| {
                    egui::Grid::new("cppn_node_tooltip_grid")
                        .num_columns(2)
                        .show(ui, |ui| {
                            ui.label("Name:");
                            ui.label(name.as_ref());
                            ui.end_row();
                            ui.label("Role:");
                            ui.label(layer_name(node.layer));
                            ui.end_row();
                            ui.label("Number:");
                            ui.label(number_in_layer[idx].to_string());
                            ui.end_row();
                            ui.label("ID:");
                            ui.label(idx.to_string());
                            ui.end_row();
                            ui.label("Bias:");
                            ui.label(format!("{:.4}", node.bias));
                            ui.end_row();
                            ui.label("Activation:");
                            ui.label(format!("{:?}", node.activation));
                            ui.end_row();
                        });
                },
            );
        } else if let Some(edge_idx) = hit_test_edge(pointer, &positions, &edges) {
            let conn = &cppn.connections[edge_idx];
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("cppn_edge_tooltip"),
                |ui| {
                    ui.label(format!("Connection {} → {}", conn.source, conn.target));
                    ui.label(format!("Weight: {:.4}", conn.weight));
                },
            );
        }
    }

    ui.horizontal(|ui| {
        crate::widgets::chart_legend_dot(ui, NODE_INPUT, "input");
        crate::widgets::chart_legend_dot(ui, NODE_HIDDEN, "hidden");
        crate::widgets::chart_legend_dot(ui, NODE_OUTPUT, "output");
    });
}
