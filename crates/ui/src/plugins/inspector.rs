use crate::types::*;
use crate::WorkbenchState;

/// Inspector panel — shows the selected/tracked organism's live component data.
pub fn inspector_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    let entity = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            ui.label(
                egui::RichText::new("No organism selected.")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
            return;
        }
    };

    // Prefer a "<Diet> {Idx, Gen}" label over the raw Entity Debug format
    // whenever the selected entity has a Diet (i.e. is an organism node).
    let mut diet_q = world.ecs.query::<&ecology::Diet>();
    let diet = diet_q.get(&world.ecs, entity).ok().cloned();

    // Pellets (Food/Mineral/Corpse) have no Diet/Genome/Brain/etc — show a
    // dedicated summary instead of falling through to the organism-only
    // sections below, which would otherwise render nothing but "Not
    // Available" for every single field.
    if diet.is_none() {
        let mut food_q = world.ecs.query::<&ecology::FoodPellet>();
        if let Ok(food) = food_q.get(&world.ecs, entity) {
            render_pellet_summary(
                ui,
                state,
                entity,
                "Food Pellet",
                food.position,
                food.energy_value,
                None,
            );
            return;
        }
        let mut mineral_q = world.ecs.query::<&ecology::MineralPellet>();
        if let Ok(mineral) = mineral_q.get(&world.ecs, entity) {
            render_pellet_summary(
                ui,
                state,
                entity,
                "Mineral Pellet",
                mineral.position,
                mineral.energy_value,
                None,
            );
            return;
        }
        let mut corpse_q = world.ecs.query::<&ecology::Corpse>();
        if let Ok(corpse) = corpse_q.get(&world.ecs, entity) {
            render_pellet_summary(
                ui,
                state,
                entity,
                "Corpse",
                corpse.position,
                corpse.energy_value,
                Some((corpse.decay_timer, corpse.max_decay)),
            );
            return;
        }
    }

    let label_text = match &diet {
        Some(diet) => format!(
            "{:?} {{Idx: {}, Gen: {}}}",
            diet,
            entity.index(),
            entity.generation()
        ),
        None => format!("Selected: {:?}", entity),
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label_text)
                .heading()
                .color(egui::Color32::LIGHT_GREEN),
        );
        let mut is_tracked = state.tracked_entity == Some(entity);
        if ui.checkbox(&mut is_tracked, "Track").changed() {
            if is_tracked {
                state.tracked_entity = Some(entity);
            } else if state.tracked_entity == Some(entity) {
                state.tracked_entity = None;
            }
        }
    });

    {
        let mut node_q = world.ecs.query::<&physics::ParticleNode>();
        if let Ok(node) = node_q.get(&world.ecs, entity) {
            if ui
                .button(format!(
                    "{} Go to Head",
                    egui_remixicon::icons::ARROW_UP_LINE
                ))
                .clicked()
            {
                actions.push(MenuAction::SelectHeadOf(node.organism_id));
            }
        }
    }

    ui.add_space(8.0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // --- IDENTITY ---
            egui::CollapsingHeader::new(format!(
                "{} Identity",
                egui_remixicon::icons::FINGERPRINT_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                ui.label("SpeciesId: Not Available");
                ui.label("GenomeId: Not Available");
                ui.label("EntityName: Not Available");

                let mut gen_q = world.ecs.query::<&organisms::Generation>();
                if let Ok(gen) = gen_q.get(&world.ecs, entity) {
                    ui.label(format!("Generation: {}", gen.0));
                } else {
                    ui.label("Generation: Not Available");
                }

                let mut age_q = world.ecs.query::<&metabolism::Age>();
                if let Ok(age) = age_q.get(&world.ecs, entity) {
                    ui.label(format!("Age: {} / {} ticks", age.ticks, age.max_lifespan));
                } else {
                    let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                    if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                        ui.label(format!("Age: {} ticks", bio.age_ticks));
                    } else {
                        ui.label("Age: Not Available");
                    }
                }

                let mut spawn_q = world.ecs.query::<&organisms::SpawnTick>();
                if let Ok(spawn) = spawn_q.get(&world.ecs, entity) {
                    ui.label(format!("BirthTick: {}", spawn.0));
                } else {
                    ui.label("BirthTick: Not Available");
                }

                let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                    ui.label(format!("ParentEntity: {:?}", bio.parent));
                } else {
                    ui.label("ParentEntity: Not Available");
                }
            });

            // --- PHYSIOLOGY ---
            egui::CollapsingHeader::new(format!(
                "{} Physiology",
                egui_remixicon::icons::HEART_PULSE_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                    ui.label(format!("Energy: {:?}", bio.energy));
                } else {
                    ui.label("Energy: Not Available");
                }

                let mut chem_q = world.ecs.query::<&metabolism::ChemicalEconomy>();
                if let Ok(chem) = chem_q.get(&world.ecs, entity) {
                    ui.label(format!("ATP: {:.1} / {:.1}", chem.atp, chem.max_atp));
                    ui.label(format!(
                        "Glucose: {:.1} / {:.1}",
                        chem.glucose, chem.max_glucose
                    ));
                    ui.label(format!("Oxygen: {:.1} / {:.1}", chem.o2, chem.max_o2));
                    ui.label(format!(
                        "CarbonDioxide: {:.1} / {:.1}",
                        chem.co2, chem.max_co2
                    ));
                } else {
                    ui.label("ATP: Not Available");
                    ui.label("Glucose: Not Available");
                    ui.label("Oxygen: Not Available");
                    ui.label("CarbonDioxide: Not Available");
                }

                let mut health_q = world.ecs.query::<&metabolism::Health>();
                if let Ok(health) = health_q.get(&world.ecs, entity) {
                    ui.label(format!("Health: {:.1} / {:.1}", health.current, health.max));
                } else {
                    ui.label("Health: Not Available");
                }

                let mut hydro_q = world.ecs.query::<&metabolism::Hydration>();
                if let Ok(hydro) = hydro_q.get(&world.ecs, entity) {
                    ui.label(format!("Hydration: {:.1}%", hydro.level * 100.0));
                } else {
                    ui.label("Hydration: Not Available");
                }

                let mut temp_q = world.ecs.query::<&metabolism::BodyTemperature>();
                if let Ok(temp) = temp_q.get(&world.ecs, entity) {
                    ui.label(format!("BodyTemperature: {:.1}°C", temp.current));
                } else {
                    ui.label("BodyTemperature: Not Available");
                }

                let mut meta_q = world.ecs.query::<&metabolism::Metabolism>();
                if let Ok(meta) = meta_q.get(&world.ecs, entity) {
                    ui.label(format!("Mass: {:.2}", meta.mass));
                } else {
                    ui.label("Mass: Not Available");
                }
            });

            // --- GENETICS ---
            egui::CollapsingHeader::new(format!(
                "{} Genetics",
                egui_remixicon::icons::TEST_TUBE_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                let mut genome_q = world.ecs.query::<&genetics::Genome>();
                if let Ok(genome) = genome_q.get(&world.ecs, entity) {
                    ui.label(format!("GenomeId: {}", genome.id.0));
                    ui.label(format!("Schema: v{}", genome.schema_version));
                    ui.label(format!("Ploidy: {:?}", genome.ploidy));
                    ui.label(format!(
                        "Brain CPPN: {} nodes, {} connections",
                        genome.brain_cppn.nodes.len(),
                        genome.brain_cppn.connections.len()
                    ));
                    ui.label(format!(
                        "Morph CPPN: {} nodes, {} connections",
                        genome.morph_cppn.nodes.len(),
                        genome.morph_cppn.connections.len()
                    ));
                    ui.label(format!(
                        "HoxSequence: {}",
                        if genome.hox.is_some() {
                            "Present"
                        } else {
                            "None (CPPN-driven)"
                        }
                    ));
                    if let Some(hox) = &genome.hox {
                        ui.label(format!("  Hox Genes: {}", hox.genes.len()));
                    }
                    if ui.button("Export Genome…").clicked() {
                        actions.push(MenuAction::ExportGenome);
                    }
                } else {
                    // Fallback: check if still growing
                    let mut growth_q = world.ecs.query::<&organisms::GrowthState>();
                    if let Ok(_growth) = growth_q.get(&world.ecs, entity) {
                        ui.label("Genome: Active (Growing – head node not selected)");
                    } else {
                        ui.label("Genome: Not Available (Select the head node)");
                    }
                }

                ui.label("MutationHistory: Not Available");
                ui.label("MutationCount: Not Available");
            });

            // --- NEURAL ---
            egui::CollapsingHeader::new(format!("{} Neural", egui_remixicon::icons::BRAIN_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    let mut brain_q = world.ecs.query::<&brain::Brain>();
                    if let Ok(brain) = brain_q.get(&world.ecs, entity) {
                        ui.label(format!(
                            "Brain: {} nodes, {} synapses",
                            brain.nodes.len(),
                            brain.synapses.len()
                        ));
                    } else {
                        ui.label("Brain: Not Available");
                    }

                    ui.label("CTRNNState: Not Available (In-place mutated)");
                    ui.label("NeuronActivity: Not Available");
                    ui.label("SynapseActivity: Not Available");
                    ui.label("BrainInputs: Not Available");
                    ui.label("BrainOutputs: Not Available");
                });

            // --- MORPHOLOGY ---
            egui::CollapsingHeader::new(format!(
                "{} Morphology",
                egui_remixicon::icons::SHAPE_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                let mut spatial_q = world.ecs.query::<&organisms::SpatialComponents>();
                if let Ok(spatial) = spatial_q.get(&world.ecs, entity) {
                    ui.label(format!(
                        "Transform: ({:.1}, {:.1})",
                        spatial.position.x, spatial.position.y
                    ));
                    ui.label(format!(
                        "Velocity: ({:.2}, {:.2})",
                        spatial.velocity.x, spatial.velocity.y
                    ));
                } else {
                    let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                    if let Ok(node) = node_q.get(&world.ecs, entity) {
                        ui.label(format!(
                            "Transform: ({:.1}, {:.1})",
                            node.position.x, node.position.y
                        ));
                        ui.label(format!(
                            "Velocity: ({:.2}, {:.2})",
                            node.velocity.x, node.velocity.y
                        ));
                    } else {
                        ui.label("Transform: Not Available");
                        ui.label("Velocity: Not Available");
                    }
                }

                ui.label("BodyPlan: Not Available");
                ui.label("SegmentTree: Not Available");
                ui.label("SensorArray: Not Available");
                ui.label("MuscleSystem: Not Available");
            });

            // --- BEHAVIOR ---
            egui::CollapsingHeader::new(format!("{} Behavior", egui_remixicon::icons::RUN_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    let mut state_q = world.ecs.query::<&behavior::BehaviorState>();
                    if let Ok(bstate) = state_q.get(&world.ecs, entity) {
                        ui.label(format!("BehaviorState: {:?}", bstate));
                    } else {
                        ui.label("BehaviorState: Not Available");
                    }

                    let mut goal_q = world.ecs.query::<&behavior::CurrentGoal>();
                    if let Ok(goal) = goal_q.get(&world.ecs, entity) {
                        ui.label(format!("CurrentGoal: {}", goal.description));
                        if let Some(target) = goal.target_entity {
                            ui.label(format!("CurrentTarget: {:?}", target));
                        } else {
                            ui.label("CurrentTarget: None");
                        }
                    } else {
                        ui.label("CurrentGoal: Not Available");
                        ui.label("CurrentTarget: Not Available");
                    }

                    ui.label("ActionState: Not Available");
                    ui.label("MemoryState: Not Available");
                });

            // --- ECOLOGY ---
            egui::CollapsingHeader::new(format!("{} Ecology", egui_remixicon::icons::EARTH_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    let mut diet_q = world.ecs.query::<&ecology::Diet>();
                    if let Ok(diet) = diet_q.get(&world.ecs, entity) {
                        ui.label(format!("DietType: {:?}", diet));
                    } else {
                        let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                        if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                            ui.label(format!("DietType: {:?}", bio.diet));
                        } else {
                            ui.label("DietType: Not Available");
                        }
                    }

                    let mut cat_q = world.ecs.query::<&ecology::EcologicalCategory>();
                    if let Ok(cat) = cat_q.get(&world.ecs, entity) {
                        ui.label(format!("TrophicLevel: {:?}", cat));
                    } else {
                        let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                        if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                            ui.label(format!("TrophicLevel: {:?}", bio.category));
                        } else {
                            ui.label("TrophicLevel: Not Available");
                        }
                    }

                    ui.label("SpeciesMembership: Not Available");
                });
        });
}

/// Renders the Inspector summary for a non-organism resource entity (a food,
/// mineral, or corpse pellet) — these carry only `position`/`energy_value`
/// (plus decay progress for corpses), so they get a much smaller view than
/// the full organism sections above.
fn render_pellet_summary(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    entity: bevy_ecs::entity::Entity,
    kind: &str,
    position: common::Vec2,
    energy_value: f32,
    decay: Option<(u32, u32)>,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} {{Idx: {}, Gen: {}}}",
                kind,
                entity.index(),
                entity.generation()
            ))
            .heading()
            .color(egui::Color32::LIGHT_GREEN),
        );
        let mut is_tracked = state.tracked_entity == Some(entity);
        if ui.checkbox(&mut is_tracked, "Track").changed() {
            if is_tracked {
                state.tracked_entity = Some(entity);
            } else if state.tracked_entity == Some(entity) {
                state.tracked_entity = None;
            }
        }
    });

    ui.add_space(8.0);
    ui.label(format!("Position: ({:.1}, {:.1})", position.x, position.y));
    ui.label(format!("EnergyValue: {:.1}", energy_value));
    if let Some((timer, max)) = decay {
        ui.label(format!("DecayTimer: {} / {} ticks", timer, max));
    }
}
