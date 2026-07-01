#![allow(unused_variables)]
use crate::selection;
use bevy::prelude::{Time, Virtual};
use bevy_ecs::prelude::*;
use common;
use ecology;
use genetics;
use metabolism;
use organisms;
use physics;
use reproduction;

pub struct SpawnOrganismCommand {
    pub parent_id: Option<bevy_ecs::entity::Entity>,
    pub genome: genetics::Genome,
    pub position: common::Vec2,
    pub diet: ecology::Diet,
    pub category: ecology::EcologicalCategory,
}

impl bevy_ecs::system::Command for SpawnOrganismCommand {
    type Out = ();
    fn apply(self, world: &mut bevy_ecs::world::World) {
        let (lineage_id, species_id, generation) = {
            if let Some(parent_id) = self.parent_id {
                if let Some(tracker) = world.get_resource::<evolution::LineageTracker>() {
                    if let Some(parent_record) =
                        tracker.get_record(common::EntityId(parent_id.to_bits()))
                    {
                        (
                            parent_record.lineage,
                            parent_record.species,
                            parent_record.generation + 1,
                        )
                    } else {
                        (evolution::LineageId(0), evolution::SpeciesId(0), 1)
                    }
                } else {
                    (evolution::LineageId(0), evolution::SpeciesId(0), 1)
                }
            } else {
                let mut tracker = world.get_resource_mut::<evolution::LineageTracker>();
                if let Some(ref mut t) = tracker {
                    (t.new_lineage_id(), evolution::SpeciesId(0), 0)
                } else {
                    (evolution::LineageId(0), evolution::SpeciesId(0), 0)
                }
            }
        };

        let entity = organisms::spawn_organism(
            world,
            &self.genome,
            self.position,
            self.diet,
            self.category,
            generation as u32,
            0,
        );

        if let Some(mut tracker) = world.get_resource_mut::<evolution::LineageTracker>() {
            tracker.register_birth(
                common::EntityId(entity.to_bits()),
                self.parent_id.map(|p| common::EntityId(p.to_bits())),
                lineage_id,
                species_id,
                generation,
                0, // TODO: Get actual tick
            );
        }

        if generation > 0 && generation % 5 == 0 {
            if let Some(mut log) = world.get_resource_mut::<analytics::NarrationLog>() {
                log.push_event(
                    0, // TODO: tick
                    "Lineage",
                    format!(
                        "Lineage {} reached generation {}!",
                        lineage_id.0, generation
                    ),
                );
            }
        }
    }
}

pub fn process_births_system(
    mut commands: Commands,
    mut events: MessageReader<reproduction::BirthRequest>,
) {
    for event in events.read() {
        commands.queue(SpawnOrganismCommand {
            parent_id: event.parent_id,
            genome: event.genome.clone(),
            position: event.position,
            diet: event.diet.clone(),
            category: event.category.clone(),
        });
    }
}

pub fn process_narrative_events_system(
    mut hazard_events: MessageReader<ecology::catastrophe::HazardSpawned>,
    mut log: ResMut<analytics::NarrationLog>,
) {
    for event in hazard_events.read() {
        log.push_event(
            0, // TODO: tick
            "Hazard",
            format!(
                "Toxic cloud emerged at ({:.1}, {:.1})",
                event.0.x, event.0.y
            ),
        );
    }
}

/// Traverses the physics spring network to completely remove organisms marked as Dead.
pub fn process_deaths_system(
    mut commands: bevy_ecs::prelude::Commands,
    dead_q: bevy_ecs::prelude::Query<
        (
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            &metabolism::ChemicalEconomy,
            Option<&ecology::Eaten>,
        ),
        bevy_ecs::prelude::With<metabolism::Dead>,
    >,
    spring_q: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::Spring)>,
    mut tracker: Option<bevy_ecs::prelude::ResMut<evolution::LineageTracker>>,
) {
    if dead_q.is_empty() {
        return;
    }

    let mut adj: std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, bevy_ecs::entity::Entity)>,
    > = std::collections::HashMap::new();

    for (s_entity, spring) in spring_q.iter() {
        adj.entry(spring.node_a)
            .or_default()
            .push((spring.node_b, s_entity));
        adj.entry(spring.node_b)
            .or_default()
            .push((spring.node_a, s_entity));
    }

    let mut nodes_to_despawn = std::collections::HashSet::new();
    let mut springs_to_despawn = std::collections::HashSet::new();

    for (head, node, chem, eaten) in dead_q.iter() {
        if nodes_to_despawn.contains(&head) {
            continue;
        }

        if let Some(ref mut t) = tracker {
            t.register_death(common::EntityId(head.to_bits()), 0, "Died".to_string());
            // TODO: Get actual tick and cause
        }

        // Spawn a corpse entity at the position of the dead organism, unless it was eaten whole
        if eaten.is_none() {
            commands.spawn(ecology::Corpse {
                position: node.position,
                energy_value: chem.max_glucose + chem.max_atp, // Corpse yields the organism's max potential energy
                decay_timer: 1800,                             // About 30 seconds at 60 FPS
                max_decay: 1800,
            });
        }

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(head);
        nodes_to_despawn.insert(head);

        while let Some(curr) = queue.pop_front() {
            if let Some(neighbors) = adj.get(&curr) {
                for &(neighbor, s_entity) in neighbors {
                    springs_to_despawn.insert(s_entity);
                    if nodes_to_despawn.insert(neighbor) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    for n in nodes_to_despawn {
        if let Ok(mut e) = commands.get_entity(n) {
            e.despawn();
        }
    }
    for s in springs_to_despawn {
        if let Ok(mut e) = commands.get_entity(s) {
            e.despawn();
        }
    }
}

pub fn simulation_control_listener(
    mut events: bevy_ecs::prelude::MessageReader<workbench::events::SimulationControlEvent>,
    mut time: bevy_ecs::prelude::ResMut<bevy::prelude::Time<bevy::prelude::Virtual>>,
) {
    for ev in events.read() {
        match ev {
            workbench::events::SimulationControlEvent::Play => time.unpause(),
            workbench::events::SimulationControlEvent::Pause => time.pause(),
            workbench::events::SimulationControlEvent::Reset => {
                // To be implemented via a broader reset system/state
                tracing::info!("Reset requested (not fully implemented)");
            }
            workbench::events::SimulationControlEvent::SetSpeed(speed) => {
                time.set_relative_speed(*speed);
            }
            workbench::events::SimulationControlEvent::StepOneTick => {
                // If paused, advance exactly one tick (e.g., using a fixed timestep workaround,
                // but for now we just log it as it requires specialized scheduler logic)
                tracing::info!("Step One Tick requested (requires scheduler integration)");
            }
        }
    }
}

pub fn update_status_bar_system(
    mut text_query: Query<(
        &mut bevy::prelude::Text,
        &workbench::status_bar::StatusBarField,
    )>,
    time: Res<Time<Virtual>>,
    metrics: Option<Res<analytics::MetricsState>>,
    camera_query: Query<&crate::camera::MainCamera>,
    selected: Option<Res<selection::SelectedEntity>>,
    hovered: Option<Res<selection::HoveredEntity>>,
    overlay: Option<Res<crate::ActiveOverlay>>,
) {
    let main_camera = camera_query.iter().next();

    for (mut text, field) in text_query.iter_mut() {
        match field {
            workbench::status_bar::StatusBarField::State => {
                let state_str = if time.is_paused() {
                    "PAUSED"
                } else {
                    "RUNNING"
                };
                text.0 = format!("State: {}", state_str);
            }
            workbench::status_bar::StatusBarField::Tick => {
                let current_tick = time.elapsed_secs() as u64 * 60; // Approximate for now, or use a proper tick counter
                text.0 = format!("Tick: {}", current_tick);
            }
            workbench::status_bar::StatusBarField::FPS => {
                if let Some(m) = &metrics {
                    let fps = m.fps_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!("FPS: {}", fps);
                }
            }
            workbench::status_bar::StatusBarField::Camera => {
                if let Some(cam) = main_camera {
                    text.0 = format!("Camera: ({:.0}, {:.0})", cam.target_pos.x, cam.target_pos.y);
                }
            }
            workbench::status_bar::StatusBarField::Zoom => {
                if let Some(cam) = main_camera {
                    text.0 = format!("Zoom: {:.2}x", cam.target_zoom);
                }
            }
            workbench::status_bar::StatusBarField::Selected => {
                if let Some(sel) = &selected {
                    text.0 = format!(
                        "Selected: {}",
                        sel.0
                            .map(|e| e.to_bits().to_string())
                            .unwrap_or_else(|| "None".to_string())
                    );
                }
            }
            workbench::status_bar::StatusBarField::Hovered => {
                if let Some(hov) = &hovered {
                    text.0 = format!(
                        "Hovered: {}",
                        hov.0
                            .map(|e| e.to_bits().to_string())
                            .unwrap_or_else(|| "None".to_string())
                    );
                }
            }
            workbench::status_bar::StatusBarField::TimeScale => {
                text.0 = format!("Time Scale: {:.1}x", time.relative_speed());
            }
            workbench::status_bar::StatusBarField::Overlay => {
                if let Some(ov) = &overlay {
                    let ov_str = match ov.0 {
                        Some(diffusion::FieldLayer::Energy) => "Energy",
                        Some(diffusion::FieldLayer::O2) => "O2",
                        Some(diffusion::FieldLayer::CO2) => "CO2",
                        Some(diffusion::FieldLayer::Pheromones) => "Pheromones",
                        None => "None",
                    };
                    text.0 = format!("Overlay: {}", ov_str);
                }
            }
        }
    }
}

pub fn update_sidebar_system(
    mut text_query: Query<(&mut bevy::prelude::Text, &workbench::sidebar::SidebarPanel)>,
    metrics: Option<Res<analytics::MetricsState>>,
    tracker: Option<Res<evolution::LineageTracker>>,
    atmosphere: Option<Res<metabolism::GlobalAtmosphere>>,
    time: Res<Time<Virtual>>,
    selected: Option<Res<selection::SelectedEntity>>,
    organism_query: Query<(&organisms::components::BiologicalComponents, &ecology::Diet)>,
) {
    for (mut text, panel) in text_query.iter_mut() {
        match panel {
            workbench::sidebar::SidebarPanel::Ecology => {
                if let Some(m) = &metrics {
                    let prod = m.producers_history.latest().unwrap_or(0.0) as u32;
                    let herb = m.herbivores_history.latest().unwrap_or(0.0) as u32;
                    let carn = m.carnivores_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!(
                        "Producers: {}\nHerbivores: {}\nCarnivores: {}",
                        prod, herb, carn
                    );
                }
            }
            workbench::sidebar::SidebarPanel::Environment => {
                let temp = atmosphere.as_ref().map(|a| a.temp).unwrap_or(25.0);
                let t_secs = time.elapsed_secs();
                let mins = (t_secs / 60.0).floor() as u32;
                let secs = (t_secs % 60.0).floor() as u32;
                text.0 = format!("Time: {:02}:{:02}\nTemp: {:.1}C", mins, secs, temp);
            }
            workbench::sidebar::SidebarPanel::Population => {
                if let Some(m) = &metrics {
                    let prod = m.producers_history.latest().unwrap_or(0.0) as u32;
                    let herb = m.herbivores_history.latest().unwrap_or(0.0) as u32;
                    let carn = m.carnivores_history.latest().unwrap_or(0.0) as u32;
                    let omni = m.omnivores_history.latest().unwrap_or(0.0) as u32;
                    let decomp = m.decomposers_history.latest().unwrap_or(0.0) as u32;
                    let total = prod + herb + carn + omni + decomp;
                    text.0 = format!("Total: {}\nBirths: {}\nDeaths: {}", total, "N/A", "N/A");
                }
            }
            workbench::sidebar::SidebarPanel::Species => {
                if let Some(_t) = &tracker {
                    text.0 = format!("Active Lineages: {}", "N/A");
                }
            }
            workbench::sidebar::SidebarPanel::Resources => {
                if let Some(m) = &metrics {
                    let food = m.food_history.latest().unwrap_or(0.0) as u32;
                    let min = m.minerals_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!("Food Pellets: {}\nMinerals: {}", food, min);
                }
            }
            workbench::sidebar::SidebarPanel::Climate => {
                let hazards = if atmosphere.as_ref().map(|a| a.temp).unwrap_or(0.0) > 40.0 {
                    "Heatwave"
                } else {
                    "None"
                };
                text.0 = format!("Weather: Clear\nHazards: {}", hazards);
            }
            workbench::sidebar::SidebarPanel::Selection => {
                if let Some(sel) = &selected {
                    if let Some(e) = sel.0 {
                        if let Ok((bio, diet)) = organism_query.get(e) {
                            text.0 = format!(
                                "ID: {}\nDiet: {:?}\nAge: {}",
                                e.to_bits(),
                                diet,
                                bio.age_ticks
                            );
                        } else {
                            text.0 = format!("ID: {}\nType: Unknown", e.to_bits());
                        }
                    } else {
                        text.0 = "None Selected".to_string();
                    }
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn update_inspector_system(
    mut text_query: Query<(
        &mut bevy::prelude::Text,
        &workbench::inspector::InspectorField,
    )>,
    selected: Option<Res<selection::SelectedEntity>>,
    tracker: Option<Res<evolution::LineageTracker>>,
    organism_query: Query<(
        Option<&organisms::components::BiologicalComponents>,
        Option<&ecology::Diet>,
        Option<&bevy::transform::components::Transform>,
        Option<&physics::ParticleNode>,
        Option<&genetics::Genome>,
        Option<&brain::Brain>,
    )>,
) {
    if let Some(sel) = &selected {
        if let Some(e) = sel.0 {
            let record = tracker
                .as_ref()
                .and_then(|t| t.get_record(common::EntityId(e.to_bits())));
            let q_res = organism_query.get(e);

            for (mut text, field) in text_query.iter_mut() {
                match field {
                    workbench::inspector::InspectorField::EntityId => {
                        text.0 = format!("{}", e.to_bits())
                    }
                    workbench::inspector::InspectorField::Species => {
                        text.0 = record
                            .map(|r| format!("{}", r.species.0))
                            .unwrap_or_else(|| "-".to_string());
                    }
                    workbench::inspector::InspectorField::GenomeId => {
                        text.0 = "-".to_string(); // we don't have a genome ID readily available without a hash or something
                    }
                    workbench::inspector::InspectorField::Generation => {
                        text.0 = record
                            .map(|r| format!("{}", r.generation))
                            .unwrap_or_else(|| "-".to_string());
                    }
                    workbench::inspector::InspectorField::Age => {
                        if let Ok((Some(bio), _, _, _, _, _)) = q_res {
                            text.0 = format!("{}", bio.age_ticks);
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Health => {
                        if let Ok((Some(bio), _, _, _, _, _)) = q_res {
                            text.0 = format!("{:.1}", bio.energy.0 / 100.0); // Placeholder
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Energy => {
                        if let Ok((Some(bio), _, _, _, _, _)) = q_res {
                            text.0 = format!("{:.1}", bio.energy.0);
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Atp => {
                        if let Ok((_, _, _, _, _, _)) = q_res {
                            text.0 = format!("{:.1}", "N/A");
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Glucose => {
                        if let Ok((_, _, _, _, _, _)) = q_res {
                            text.0 = format!("{:.1}", "N/A");
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Position => {
                        if let Ok((_, _, Some(trans), _, _, _)) = q_res {
                            text.0 =
                                format!("({:.0}, {:.0})", trans.translation.x, trans.translation.y);
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Velocity => {
                        if let Ok((_, _, _, Some(vel), _, _)) = q_res {
                            text.0 = format!("({:.1}, {:.1})", vel.velocity.x, vel.velocity.y);
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Metabolism => {
                        if let Ok((_, Some(diet), _, _, _, _)) = q_res {
                            text.0 = format!("{:?}", diet);
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                    workbench::inspector::InspectorField::Behaviour => {
                        if let Ok((_, _, _, _, _, Some(brain))) = q_res {
                            text.0 = format!("Nodes: {}", brain.nodes.len());
                        } else {
                            text.0 = "-".to_string();
                        }
                    }
                }
            }
        } else {
            for (mut text, _) in text_query.iter_mut() {
                text.0 = "-".to_string();
            }
        }
    } else {
        for (mut text, _) in text_query.iter_mut() {
            text.0 = "-".to_string();
        }
    }
}

pub fn update_metrics_system(
    mut text_query: Query<(&mut bevy::prelude::Text, &workbench::metrics::MetricsField)>,
    metrics: Option<Res<analytics::MetricsState>>,
    tracker: Option<Res<evolution::LineageTracker>>,
    camera_query: Query<&crate::camera::MainCamera>,
    time: Res<Time<Virtual>>,
) {
    for (mut text, field) in text_query.iter_mut() {
        match field {
            workbench::metrics::MetricsField::Fps => {
                if let Some(m) = &metrics {
                    text.0 = format!("FPS: {}", m.fps_history.latest().unwrap_or(0.0) as u32);
                }
            }
            workbench::metrics::MetricsField::Tps => {
                if let Some(m) = &metrics {
                    text.0 = format!("TPS: {}", m.tps_history.latest().unwrap_or(0.0) as u32);
                }
            }
            workbench::metrics::MetricsField::FrameTime => {
                if let Some(m) = &metrics {
                    let fps = m.fps_history.latest().unwrap_or(60.0);
                    let ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };
                    text.0 = format!("Frame Time: {:.1}ms", ms);
                }
            }
            workbench::metrics::MetricsField::Entities => {
                // Approximate entities by adding known counts.
                if let Some(m) = &metrics {
                    let prod = m.producers_history.latest().unwrap_or(0.0) as u32;
                    let herb = m.herbivores_history.latest().unwrap_or(0.0) as u32;
                    let carn = m.carnivores_history.latest().unwrap_or(0.0) as u32;
                    let omni = m.omnivores_history.latest().unwrap_or(0.0) as u32;
                    let decomp = m.decomposers_history.latest().unwrap_or(0.0) as u32;
                    let food = m.food_history.latest().unwrap_or(0.0) as u32;
                    let min = m.minerals_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!(
                        "Entities: {}",
                        prod + herb + carn + omni + decomp + food + min
                    );
                }
            }
            workbench::metrics::MetricsField::CameraPos => {
                if let Some(cam) = camera_query.iter().next() {
                    text.0 = format!(
                        "Camera Pos: ({:.0}, {:.0})",
                        cam.target_pos.x, cam.target_pos.y
                    );
                }
            }
            workbench::metrics::MetricsField::Zoom => {
                if let Some(cam) = camera_query.iter().next() {
                    text.0 = format!("Zoom: {:.2}x", cam.target_zoom);
                }
            }
            workbench::metrics::MetricsField::Population => {
                if let Some(m) = &metrics {
                    let prod = m.producers_history.latest().unwrap_or(0.0) as u32;
                    let herb = m.herbivores_history.latest().unwrap_or(0.0) as u32;
                    let carn = m.carnivores_history.latest().unwrap_or(0.0) as u32;
                    let omni = m.omnivores_history.latest().unwrap_or(0.0) as u32;
                    let decomp = m.decomposers_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!("Population: {}", prod + herb + carn + omni + decomp);
                }
            }
            workbench::metrics::MetricsField::Births => {
                text.0 = "Births: N/A".to_string();
            }
            workbench::metrics::MetricsField::Deaths => {
                text.0 = "Deaths: N/A".to_string();
            }
            workbench::metrics::MetricsField::Species => {
                text.0 = "Species: N/A".to_string();
            }
            workbench::metrics::MetricsField::Biomass => {
                // N/A
                text.0 = "Biomass: N/A".to_string();
            }
            workbench::metrics::MetricsField::Resources => {
                if let Some(m) = &metrics {
                    let food = m.food_history.latest().unwrap_or(0.0) as u32;
                    let min = m.minerals_history.latest().unwrap_or(0.0) as u32;
                    text.0 = format!("Resources: {}", food + min);
                }
            }
            workbench::metrics::MetricsField::Energy => {
                text.0 = "Energy: N/A".to_string();
            }
        }
    }
}

pub fn sync_gpu_positions_to_cpu(
    pos_receiver: Res<crate::PositionReceiver>,
    entities_receiver: Res<crate::NodeEntitiesReceiver>,
    mut query: Query<&mut physics::ParticleNode>,
) {
    // Only process the latest frame's data if multiple frames backed up
    let mut latest_bytes = None;
    let mut bytes_count = 0;
    while let Ok(bytes) = pos_receiver.0.try_recv() {
        latest_bytes = Some(bytes);
        bytes_count += 1;
    }

    let mut latest_entities = None;
    let mut entities_count = 0;
    while let Ok(entities) = entities_receiver.0.try_recv() {
        latest_entities = Some(entities);
        entities_count += 1;
    }

    if bytes_count > 0 || entities_count > 0 {
        tracing::info!(
            "Sync received {} byte messages and {} entity messages",
            bytes_count,
            entities_count
        );
    }

    if let (Some(bytes), Some(entities)) = (latest_bytes, latest_entities) {
        let node_size = std::mem::size_of::<gpu::physics_pipeline::GpuParticleNode>();
        if bytes.len() % node_size != 0 {
            tracing::error!("Received malformed physics buffer, skipping update");
            return;
        }

        let gpu_nodes: &[gpu::physics_pipeline::GpuParticleNode] = bytemuck::cast_slice(&bytes);

        if gpu_nodes.len() < entities.len() {
            tracing::warn!(
                "Mismatch: entities count ({}) exceeds GPU buffer capacity ({}), skipping update",
                entities.len(),
                gpu_nodes.len()
            );
            return;
        }

        let mut updated = 0;
        for (i, &entity) in entities.iter().enumerate() {
            if let Ok(mut node) = query.get_mut(entity) {
                let gpu_node = &gpu_nodes[i];
                node.position.x = gpu_node.position[0];
                node.position.y = gpu_node.position[1];
                node.velocity.x = gpu_node.velocity[0];
                node.velocity.y = gpu_node.velocity[1];
                // Force gets cleared on the GPU, so we shouldn't necessarily read it back, but let's clear it
                node.force = common::Vec2::new(0.0, 0.0);
                updated += 1;
            }
        }
        tracing::info!("Synced {} CPU nodes from GPU", updated);
    }
}
