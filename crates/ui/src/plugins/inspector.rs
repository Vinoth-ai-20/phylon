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
            crate::widgets::empty_state(ui, "Select an organism to view its details");
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

    ui.add_space(crate::theme::SPACE_SM);

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
                egui::Grid::new("insp_identity")
                    .striped(true)
                    .show(ui, |ui| {
                        crate::widgets::kv_row(ui, "SpeciesId", "Not Available");
                        crate::widgets::kv_row(ui, "GenomeId", "Not Available");
                        crate::widgets::kv_row(ui, "EntityName", "Not Available");

                        let mut gen_q = world.ecs.query::<&organisms::Generation>();
                        if let Ok(gen) = gen_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(ui, "Generation", &gen.0.to_string());
                        } else {
                            crate::widgets::kv_row(ui, "Generation", "Not Available");
                        }

                        let mut age_q = world.ecs.query::<&metabolism::Age>();
                        if let Ok(age) = age_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "Age",
                                &format!("{} / {} ticks", age.ticks, age.max_lifespan),
                            );
                        } else {
                            let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                            if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row_mono(
                                    ui,
                                    "Age",
                                    &format!("{} ticks", bio.age_ticks),
                                );
                            } else {
                                crate::widgets::kv_row(ui, "Age", "Not Available");
                            }
                        }

                        let mut spawn_q = world.ecs.query::<&organisms::SpawnTick>();
                        if let Ok(spawn) = spawn_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(ui, "BirthTick", &spawn.0.to_string());
                        } else {
                            crate::widgets::kv_row(ui, "BirthTick", "Not Available");
                        }

                        let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                        if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row(
                                ui,
                                "ParentEntity",
                                &format!("{:?}", bio.parent),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "ParentEntity", "Not Available");
                        }
                    });
            });

            // --- PHYSIOLOGY ---
            egui::CollapsingHeader::new(format!(
                "{} Physiology",
                egui_remixicon::icons::HEART_PULSE_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("insp_physiology")
                    .striped(true)
                    .show(ui, |ui| {
                        let mut bio_q = world.ecs.query::<&organisms::BiologicalComponents>();
                        if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(ui, "Energy", &format!("{:?}", bio.energy));
                        } else {
                            crate::widgets::kv_row(ui, "Energy", "Not Available");
                        }

                        let mut chem_q = world.ecs.query::<&metabolism::ChemicalEconomy>();
                        if let Ok(chem) = chem_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "ATP",
                                &format!("{:.1} / {:.1}", chem.atp, chem.max_atp),
                            );
                            crate::widgets::kv_row_mono(
                                ui,
                                "Glucose",
                                &format!("{:.1} / {:.1}", chem.glucose, chem.max_glucose),
                            );
                            crate::widgets::kv_row_mono(
                                ui,
                                "Oxygen",
                                &format!("{:.1} / {:.1}", chem.o2, chem.max_o2),
                            );
                            crate::widgets::kv_row_mono(
                                ui,
                                "CarbonDioxide",
                                &format!("{:.1} / {:.1}", chem.co2, chem.max_co2),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "ATP", "Not Available");
                            crate::widgets::kv_row(ui, "Glucose", "Not Available");
                            crate::widgets::kv_row(ui, "Oxygen", "Not Available");
                            crate::widgets::kv_row(ui, "CarbonDioxide", "Not Available");
                        }

                        let mut health_q = world.ecs.query::<&metabolism::Health>();
                        if let Ok(health) = health_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "Health",
                                &format!("{:.1} / {:.1}", health.current, health.max),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "Health", "Not Available");
                        }

                        let mut hydro_q = world.ecs.query::<&metabolism::Hydration>();
                        if let Ok(hydro) = hydro_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "Hydration",
                                &format!("{:.1}%", hydro.level * 100.0),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "Hydration", "Not Available");
                        }

                        let mut temp_q = world.ecs.query::<&metabolism::BodyTemperature>();
                        if let Ok(temp) = temp_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "BodyTemperature",
                                &format!("{:.1}°C", temp.current),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "BodyTemperature", "Not Available");
                        }

                        let mut meta_q = world.ecs.query::<&metabolism::Metabolism>();
                        if let Ok(meta) = meta_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(ui, "Mass", &format!("{:.2}", meta.mass));
                        } else {
                            crate::widgets::kv_row(ui, "Mass", "Not Available");
                        }
                    });
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
                    egui::Grid::new("insp_genetics")
                        .striped(true)
                        .show(ui, |ui| {
                            crate::widgets::kv_row(ui, "GenomeId", &genome.id.0.to_string());
                            crate::widgets::kv_row(
                                ui,
                                "Schema",
                                &format!("v{}", genome.schema_version),
                            );
                            crate::widgets::kv_row(ui, "Ploidy", &format!("{:?}", genome.ploidy));
                            crate::widgets::kv_row(
                                ui,
                                "Brain CPPN",
                                &format!(
                                    "{} nodes, {} connections",
                                    genome.brain_cppn.nodes.len(),
                                    genome.brain_cppn.connections.len()
                                ),
                            );
                            crate::widgets::kv_row(
                                ui,
                                "Morph CPPN",
                                &format!(
                                    "{} nodes, {} connections",
                                    genome.morph_cppn.nodes.len(),
                                    genome.morph_cppn.connections.len()
                                ),
                            );
                            crate::widgets::kv_row(
                                ui,
                                "HoxSequence",
                                if genome.hox.is_some() {
                                    "Present"
                                } else {
                                    "None (CPPN-driven)"
                                },
                            );
                            if let Some(hox) = &genome.hox {
                                crate::widgets::kv_row(
                                    ui,
                                    "Hox Genes",
                                    &hox.genes.len().to_string(),
                                );
                            }
                            crate::widgets::kv_row(ui, "MutationHistory", "Not Available");
                            crate::widgets::kv_row(ui, "MutationCount", "Not Available");
                        });
                    if ui.button("Export Genome…").clicked() {
                        actions.push(MenuAction::ExportGenome);
                    }
                } else {
                    // Fallback: check if still growing
                    let mut growth_q = world.ecs.query::<&organisms::GrowthState>();
                    if let Ok(_growth) = growth_q.get(&world.ecs, entity) {
                        crate::widgets::empty_state(
                            ui,
                            "Genome: Active (Growing – head node not selected)",
                        );
                    } else {
                        crate::widgets::empty_state(
                            ui,
                            "Genome: Not Available (Select the head node)",
                        );
                    }
                }
            });

            // --- NEURAL ---
            egui::CollapsingHeader::new(format!("{} Neural", egui_remixicon::icons::BRAIN_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("insp_neural").striped(true).show(ui, |ui| {
                        let mut brain_q = world.ecs.query::<&brain::Brain>();
                        if let Ok(brain) = brain_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row(
                                ui,
                                "Brain",
                                &format!(
                                    "{} nodes, {} synapses",
                                    brain.nodes.len(),
                                    brain.synapses.len()
                                ),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "Brain", "Not Available");
                        }

                        crate::widgets::kv_row(
                            ui,
                            "CTRNNState",
                            "Not Available (In-place mutated)",
                        );
                        crate::widgets::kv_row(ui, "NeuronActivity", "Not Available");
                        crate::widgets::kv_row(ui, "SynapseActivity", "Not Available");
                        crate::widgets::kv_row(ui, "BrainInputs", "Not Available");
                        crate::widgets::kv_row(ui, "BrainOutputs", "Not Available");
                    });
                });

            // --- MORPHOLOGY ---
            egui::CollapsingHeader::new(format!(
                "{} Morphology",
                egui_remixicon::icons::SHAPE_LINE
            ))
            .default_open(true)
            .show(ui, |ui| {
                egui::Grid::new("insp_morphology")
                    .striped(true)
                    .show(ui, |ui| {
                        let mut spatial_q = world.ecs.query::<&organisms::SpatialComponents>();
                        if let Ok(spatial) = spatial_q.get(&world.ecs, entity) {
                            crate::widgets::kv_row_mono(
                                ui,
                                "Transform",
                                &format!("({:.1}, {:.1})", spatial.position.x, spatial.position.y),
                            );
                            crate::widgets::kv_row_mono(
                                ui,
                                "Velocity",
                                &format!("({:.2}, {:.2})", spatial.velocity.x, spatial.velocity.y),
                            );
                        } else {
                            let mut node_q = world.ecs.query::<&physics::ParticleNode>();
                            if let Ok(node) = node_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row_mono(
                                    ui,
                                    "Transform",
                                    &format!("({:.1}, {:.1})", node.position.x, node.position.y),
                                );
                                crate::widgets::kv_row_mono(
                                    ui,
                                    "Velocity",
                                    &format!("({:.2}, {:.2})", node.velocity.x, node.velocity.y),
                                );
                            } else {
                                crate::widgets::kv_row(ui, "Transform", "Not Available");
                                crate::widgets::kv_row(ui, "Velocity", "Not Available");
                            }
                        }

                        crate::widgets::kv_row(ui, "BodyPlan", "Not Available");
                        crate::widgets::kv_row(ui, "SegmentTree", "Not Available");
                        crate::widgets::kv_row(ui, "SensorArray", "Not Available");
                        crate::widgets::kv_row(ui, "MuscleSystem", "Not Available");
                    });
            });

            // --- BEHAVIOR ---
            egui::CollapsingHeader::new(format!("{} Behavior", egui_remixicon::icons::RUN_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("insp_behavior")
                        .striped(true)
                        .show(ui, |ui| {
                            let mut state_q = world.ecs.query::<&behavior::BehaviorState>();
                            if let Ok(bstate) = state_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row(
                                    ui,
                                    "BehaviorState",
                                    &format!("{:?}", bstate),
                                );
                            } else {
                                crate::widgets::kv_row(ui, "BehaviorState", "Not Available");
                            }

                            let mut goal_q = world.ecs.query::<&behavior::CurrentGoal>();
                            if let Ok(goal) = goal_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row(ui, "CurrentGoal", &goal.description);
                                if let Some(target) = goal.target_entity {
                                    crate::widgets::kv_row(
                                        ui,
                                        "CurrentTarget",
                                        &format!("{:?}", target),
                                    );
                                } else {
                                    crate::widgets::kv_row(ui, "CurrentTarget", "None");
                                }
                            } else {
                                crate::widgets::kv_row(ui, "CurrentGoal", "Not Available");
                                crate::widgets::kv_row(ui, "CurrentTarget", "Not Available");
                            }

                            crate::widgets::kv_row(ui, "ActionState", "Not Available");
                            crate::widgets::kv_row(ui, "MemoryState", "Not Available");
                        });
                });

            // --- ECOLOGY ---
            egui::CollapsingHeader::new(format!("{} Ecology", egui_remixicon::icons::EARTH_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("insp_ecology")
                        .striped(true)
                        .show(ui, |ui| {
                            let mut diet_q = world.ecs.query::<&ecology::Diet>();
                            if let Ok(diet) = diet_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row_colored(
                                    ui,
                                    "DietType",
                                    &format!("{:?}", diet),
                                    crate::theme::chart_color(diet),
                                );
                            } else {
                                let mut bio_q =
                                    world.ecs.query::<&organisms::BiologicalComponents>();
                                if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                                    crate::widgets::kv_row(
                                        ui,
                                        "DietType",
                                        &format!("{:?}", bio.diet),
                                    );
                                } else {
                                    crate::widgets::kv_row(ui, "DietType", "Not Available");
                                }
                            }

                            let mut cat_q = world.ecs.query::<&ecology::EcologicalCategory>();
                            if let Ok(cat) = cat_q.get(&world.ecs, entity) {
                                crate::widgets::kv_row(ui, "TrophicLevel", &format!("{:?}", cat));
                            } else {
                                let mut bio_q =
                                    world.ecs.query::<&organisms::BiologicalComponents>();
                                if let Ok(bio) = bio_q.get(&world.ecs, entity) {
                                    crate::widgets::kv_row(
                                        ui,
                                        "TrophicLevel",
                                        &format!("{:?}", bio.category),
                                    );
                                } else {
                                    crate::widgets::kv_row(ui, "TrophicLevel", "Not Available");
                                }
                            }

                            crate::widgets::kv_row(ui, "SpeciesMembership", "Not Available");
                        });
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

    ui.add_space(crate::theme::SPACE_SM);
    egui::Grid::new("insp_pellet").striped(true).show(ui, |ui| {
        crate::widgets::kv_row_mono(
            ui,
            "Position",
            &format!("({:.1}, {:.1})", position.x, position.y),
        );
        crate::widgets::kv_row_mono(ui, "EnergyValue", &format!("{:.1}", energy_value));
        if let Some((timer, max)) = decay {
            crate::widgets::kv_row_mono(ui, "DecayTimer", &format!("{} / {} ticks", timer, max));
        }
    });
}
