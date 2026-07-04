//! Neural Viewer plugin — renders both the phenotype (CTRNN runtime brain)
//! and the genotype (brain CPPN) of the currently selected/tracked organism
//! as node-link graphs.
//!
//! Both graphs used to be split across two panels (CTRNN here, CPPN under
//! the Genetics sidebar tab) — they're consolidated here since they're both
//! "the brain", just at different levels (what evolution encodes vs. what
//! actually runs).

use crate::types::*;

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
                    .color(egui::Color32::GRAY)
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
                    .color(egui::Color32::GRAY)
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
                    .color(egui::Color32::GRAY)
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
            ui.label(egui::RichText::new("Phenotype — CTRNN Runtime Network").strong());
            draw_brain_graph(ui, b);

            ui.add_space(crate::theme::SPACE_MD);

            if let Some(genome) = genome {
                ui.separator();
                ui.add_space(crate::theme::SPACE_MD);
                ui.label(egui::RichText::new("Genotype — Brain CPPN").strong());
                draw_cppn_graph(ui, &genome.brain_cppn);
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

/// Draws a small filled circle "swatch" followed by a label — used for
/// legends instead of a Unicode "●" glyph, which silently falls back to a
/// tofu/box glyph in fonts that don't carry that codepoint (as IBM Plex Sans
/// doesn't), regardless of which fallback font is configured after it.
fn legend_dot(ui: &mut egui::Ui, color: egui::Color32, label: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, color);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).small());
    });
}

/// Nearest node to `pointer` within `radius` + a small hit-test tolerance.
fn hit_test_node(pointer: egui::Pos2, positions: &[egui::Pos2], radius: f32) -> Option<usize> {
    let tolerance = radius + 3.0;
    positions
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.distance(pointer)))
        .filter(|(_, d)| *d <= tolerance)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Shortest distance from `p` to the line segment `a`–`b`.
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
/// hit-test tolerance of the segment.
fn hit_test_edge(
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

/// Draws a CTRNN brain graph: nodes as circles positioned by column (input /
/// hidden / output) and index-within-column, synapses as lines colored by
/// weight sign and shaded by magnitude. Hovering a node or synapse shows its
/// details in a tooltip.
fn draw_brain_graph(ui: &mut egui::Ui, b: &brain::Brain) {
    if b.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty network.")
                .italics()
                .color(egui::Color32::GRAY),
        );
        return;
    }

    let height = 240.0_f32.max(b.nodes.len() as f32 * 4.0).min(480.0);
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_rgb(16, 16, 20),
    );

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
        let strength = syn.weight.abs().min(3.0) / 3.0;
        let color = if syn.weight >= 0.0 {
            egui::Color32::from_rgba_unmultiplied(90, 200, 255, (80.0 + 140.0 * strength) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, (80.0 + 140.0 * strength) as u8)
        };
        painter.line_segment(
            [positions[source], positions[target]],
            egui::Stroke::new(0.5 + 2.0 * strength, color),
        );
    }

    // Nodes.
    let node_radius = 6.0;
    for (idx, node) in b.nodes.iter().enumerate() {
        let color = match column_of(idx) {
            0 => egui::Color32::from_rgb(120, 220, 120), // input
            2 => egui::Color32::from_rgb(255, 180, 90),  // output
            _ => egui::Color32::from_rgb(150, 170, 255), // hidden
        };
        painter.circle_filled(positions[idx], node_radius, color);
        painter.circle_stroke(
            positions[idx],
            node_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(20)),
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
        legend_dot(ui, egui::Color32::from_rgb(120, 220, 120), "input");
        legend_dot(ui, egui::Color32::from_rgb(150, 170, 255), "hidden");
        legend_dot(ui, egui::Color32::from_rgb(255, 180, 90), "output");
        ui.label(
            egui::RichText::new("— blue = excitatory, red = inhibitory")
                .small()
                .color(egui::Color32::GRAY),
        );
    });
}

/// Draws a CPPN (brain-genotype) graph: nodes as circles positioned by layer
/// (left → right) and index-within-layer (top → bottom), connections as
/// lines colored by weight sign and shaded by magnitude. Hovering a node or
/// connection shows its details in a tooltip.
fn draw_cppn_graph(ui: &mut egui::Ui, cppn: &genetics::cppn::Cppn) {
    if cppn.nodes.is_empty() {
        ui.label(
            egui::RichText::new("Empty network.")
                .italics()
                .color(egui::Color32::GRAY),
        );
        return;
    }

    let height = 200.0_f32.max(cppn.nodes.len() as f32 * 4.0).min(360.0);
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let rect = response.rect;
    painter.rect_filled(
        rect,
        egui::Rounding::same(4.0),
        egui::Color32::from_rgb(16, 16, 20),
    );

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
        let strength = conn.weight.abs().min(3.0) / 3.0;
        let color = if conn.weight >= 0.0 {
            egui::Color32::from_rgba_unmultiplied(90, 200, 255, (80.0 + 140.0 * strength) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(255, 100, 100, (80.0 + 140.0 * strength) as u8)
        };
        painter.line_segment(
            [positions[conn.source], positions[conn.target]],
            egui::Stroke::new(0.5 + 2.0 * strength, color),
        );
    }

    // Nodes.
    let node_radius = 5.0;
    for (idx, node) in cppn.nodes.iter().enumerate() {
        let color = match node.layer {
            0 => egui::Color32::from_rgb(120, 220, 120), // input
            l if l == max_layer => egui::Color32::from_rgb(255, 180, 90), // output
            _ => egui::Color32::from_rgb(150, 170, 255), // hidden
        };
        painter.circle_filled(positions[idx], node_radius, color);
        painter.circle_stroke(
            positions[idx],
            node_radius,
            egui::Stroke::new(1.0, egui::Color32::from_gray(20)),
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
        legend_dot(ui, egui::Color32::from_rgb(120, 220, 120), "input");
        legend_dot(ui, egui::Color32::from_rgb(150, 170, 255), "hidden");
        legend_dot(ui, egui::Color32::from_rgb(255, 180, 90), "output");
    });
}
