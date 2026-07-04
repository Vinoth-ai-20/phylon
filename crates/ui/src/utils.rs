/// Recursively draws the segment / spring tree of an organism in the inspector.
#[allow(dead_code)]
pub(crate) fn draw_segment_tree(
    ui: &mut egui::Ui,
    current_node: bevy_ecs::entity::Entity,
    adj: &std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, physics::Spring)>,
    >,
    world: &bevy_ecs::world::World,
    visited: &mut std::collections::HashSet<bevy_ecs::entity::Entity>,
    selected_entity: &mut Option<bevy_ecs::entity::Entity>,
) {
    if visited.contains(&current_node) {
        return;
    }
    visited.insert(current_node);

    let Some(node) = world.get::<physics::ParticleNode>(current_node) else {
        return;
    };

    let seg_name = match node.segment_type {
        0 => "Head",
        1 => "Torso",
        2 => "Muscle",
        3 => "Tail",
        4 => "Fin",
        _ => "Unknown",
    };

    // Find children
    let empty = Vec::new();
    let neighbors = adj.get(&current_node).unwrap_or(&empty);
    let mut children = Vec::new();
    for (neighbor, spring) in neighbors {
        if !visited.contains(neighbor) {
            children.push((*neighbor, spring.clone()));
        }
    }

    let label = format!("{:?} ({})", current_node, seg_name);
    let is_selected = *selected_entity == Some(current_node);

    if children.is_empty() {
        if ui.selectable_label(is_selected, label).clicked() {
            *selected_entity = Some(current_node);
        }
    } else {
        let header = egui::CollapsingHeader::new(label).default_open(true);

        let response = header.show(ui, |ui| {
            for (child, spring) in children {
                let constraint_name = match spring.constraint_type {
                    physics::ConstraintType::Elastic => "Elastic",
                    physics::ConstraintType::Rigid => "Rigid",
                    physics::ConstraintType::Passive => "Passive",
                    physics::ConstraintType::Rotational => "Rotational",
                };

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            egui_remixicon::icons::CORNER_DOWN_RIGHT_LINE,
                            constraint_name
                        ))
                        .small()
                        .color(crate::theme::DISABLED_FG),
                    );
                    if spring.actuation_amplitude > 0.0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "(amp: {:.1}, ph: {:.1})",
                                spring.actuation_amplitude, spring.actuation_phase
                            ))
                            .small()
                            .color(egui::Color32::from_rgb(200, 150, 100)),
                        );
                    }
                });

                draw_segment_tree(ui, child, adj, world, visited, selected_entity);
            }
        });

        if response.header_response.clicked() {
            *selected_entity = Some(current_node);
        }
    }
}
