//! Shared helpers for displaying a `genetics::RegulatoryNetwork` — used by
//! `plugins::grn_viewer` and `plugins::evolution_debugger` (its
//! mutation-diff view reuses the exact same network-building and
//! gene-labeling logic rather than duplicating it).

/// Builds and develops a `RegulatoryNetwork` for one (position, step) pair —
/// a thin wrapper around `genetics`' public pieces, since
/// `genetics::develop_at_position` only returns the final decoded outputs,
/// not the intermediate network structure a graph view needs.
pub(crate) fn developed_network(
    regulatory_cppn: &genetics::Cppn,
    position: usize,
    step: usize,
) -> genetics::RegulatoryNetwork {
    let gene_count = genetics::REGULATORY_GENE_ROLES.len();
    let mut network = genetics::RegulatoryNetwork::generate(regulatory_cppn, gene_count);
    let inputs =
        genetics::external_inputs_for_position(position, organisms::MAX_SEGMENTS, gene_count);
    network.develop(step, &inputs);
    network
}

/// A short, human-readable label for gene `index` — its role plus an
/// ordinal among genes sharing that role (e.g. the second Hox gene is
/// "Hox1"), matching `REGULATORY_GENE_ROLES`'s fixed ordering.
pub(crate) fn node_label(index: usize) -> String {
    let role = genetics::REGULATORY_GENE_ROLES[index];
    let same_role_ordinal = genetics::REGULATORY_GENE_ROLES[..index]
        .iter()
        .filter(|&&r| r == role)
        .count();
    let prefix = match role {
        genetics::RegulatoryGeneRole::Hox => "Hox",
        genetics::RegulatoryGeneRole::Differentiation => "Diff",
        genetics::RegulatoryGeneRole::Effector => "Eff",
        genetics::RegulatoryGeneRole::Pigment => "Pig",
    };
    format!("{prefix}{same_role_ordinal}")
}

/// Per-gene bias comparison row: `(label, self_bias, other_bias, delta)`.
pub(crate) struct BiasDiffRow {
    pub label: String,
    pub self_bias: f32,
    pub other_bias: f32,
    pub delta: f32,
}

/// Compares two developed networks' per-gene biases — the core of every
/// "mutation diff" view (GRN Viewer's parent comparison, Evolution
/// Debugger's arbitrary-pair comparison).
pub(crate) fn bias_diff_rows(
    self_network: &genetics::RegulatoryNetwork,
    other_network: &genetics::RegulatoryNetwork,
) -> Vec<BiasDiffRow> {
    let count = self_network.nodes.len().min(other_network.nodes.len());
    (0..count)
        .map(|i| {
            let self_bias = self_network.nodes[i].bias;
            let other_bias = other_network.nodes[i].bias;
            BiasDiffRow {
                label: node_label(i),
                self_bias,
                other_bias,
                delta: self_bias - other_bias,
            }
        })
        .collect()
}

/// Renders `rows` (see [`bias_diff_rows`]) as a striped grid, flagging any
/// `|delta| > 0.1` in `theme::WARN` — shared table styling for every
/// mutation-diff view.
pub(crate) fn render_bias_diff_grid(ui: &mut egui::Ui, id: &str, rows: &[BiasDiffRow]) {
    egui::Grid::new(id).striped(true).show(ui, |ui| {
        ui.label(egui::RichText::new("Gene").strong());
        ui.label(egui::RichText::new("Self bias").strong());
        ui.label(egui::RichText::new("Other bias").strong());
        ui.label(egui::RichText::new("Δ").strong());
        ui.end_row();

        for row in rows {
            ui.label(&row.label);
            ui.label(format!("{:.3}", row.self_bias));
            ui.label(format!("{:.3}", row.other_bias));
            let delta_color = if row.delta.abs() > 0.1 {
                crate::theme::WARN
            } else {
                crate::theme::DISABLED_FG
            };
            ui.label(egui::RichText::new(format!("{:+.3}", row.delta)).color(delta_color));
            ui.end_row();
        }
    });
}
