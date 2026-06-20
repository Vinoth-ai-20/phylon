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
                        egui::RichText::new(format!("↳ {}", constraint_name))
                            .small()
                            .color(egui::Color32::GRAY),
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

pub(crate) fn capsule_points(
    pa: common::Vec2,
    pb: common::Vec2,
    radius: f32,
    to_screen: impl Fn(common::Vec2) -> egui::Pos2,
) -> Vec<egui::Pos2> {
    let mut dir = pb - pa;
    let len = dir.length();
    let segments = 8;
    let mut points = Vec::new();

    if len < 0.001 {
        for i in 0..(segments * 2) {
            let t = std::f32::consts::TAU * (i as f32) / ((segments * 2) as f32);
            let p = common::Vec2::new(pa.x + t.cos() * radius, pa.y + t.sin() * radius);
            points.push(to_screen(p));
        }
        return points;
    }
    dir /= len;

    // Semicircle around pb
    let base_angle_pb = dir.y.atan2(dir.x);
    for i in 0..=segments {
        let a = base_angle_pb - std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32) / (segments as f32);
        let p = common::Vec2::new(pb.x + a.cos() * radius, pb.y + a.sin() * radius);
        points.push(to_screen(p));
    }

    // Semicircle around pa
    let base_angle_pa = (-dir.y).atan2(-dir.x);
    for i in 0..=segments {
        let a = base_angle_pa - std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32) / (segments as f32);
        let p = common::Vec2::new(pa.x + a.cos() * radius, pa.y + a.sin() * radius);
        points.push(to_screen(p));
    }

    points
}
