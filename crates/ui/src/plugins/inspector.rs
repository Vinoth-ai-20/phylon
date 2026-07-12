//! The Inspector panel — the selected/tracked organism's live component
//! data, presented as a sequence of logical sections rather than one
//! monolithic view. Each section below is already a self-contained
//! `(ctx, ui, state, world, actions) -> ()` render step operating on its
//! own slice of the entity's components — the same shape
//! `physiology_viewer_ui`/`circulation_viewer_ui`/`hormone_viewer_ui`/
//! `immune_viewer_ui`/`lineage_viewer_ui` use as real, separately-defined
//! functions reused verbatim here. The sections that are still inline in
//! `inspector_ui` (Identity, Physiology summary, Genetics, Neural,
//! Morphology, Behavior, Ecology, Relationships/History, Body Plan) are
//! conceptually the same kind of independent widget, just not yet
//! extracted to their own functions/files — a future pass could give each
//! its own `fn foo_section_ui(...)` in its own module, mirroring the
//! pattern the already-extracted viewers demonstrate.
//!
//! ## Section inventory (render order)
//! 1. Recent Selections (`render_recent_selections`) — already its own function.
//! 2. Identity — head-level facts (species, generation, age, birth tick, parent).
//! 3. Physiology — energy/health/hydration/temperature summary, plus 4 nested,
//!    already-independent sections (Per-Segment Detail, Circulation, Hormones,
//!    Immune Response — each a real, separate `*_viewer_ui` function).
//! 4. Genetics — genome identity, CPPN sizes, mutation count, Export Genome action.
//! 5. Evolution / History — already its own function (`lineage_viewer_ui`).
//! 6. Neural — brain topology size and a live activation/weight preview.
//! 7. Morphology — transform/velocity.
//! 8. Behavior — current behavior state/goal/target.
//! 9. Ecology — diet, trophic level, species population.
//! 10. Relationships / History — nearby organisms, trajectory summary.
//! 11. Body Plan (`render_body_plan`) — already its own function; the real
//!     segment tree (see Morphology's note on why a duplicate summary of
//!     this was removed, not kept).

use crate::types::*;
use crate::WorkbenchState;

/// Renders a compact "Recent:" row of the last few distinct entities
/// `selected_entity` has pointed at ("Recent Selections"), each a
/// clickable, Diet-colored chip. Shown above the Inspector's normal
/// content — including when nothing is currently selected — so a user can
/// click back into a recent organism. Entities that have since despawned
/// (killed, died) are skipped rather than shown as dead links, but are left
/// in `recent_selections` itself (no mutation here) since a future
/// re-selection of the same `Entity` value would be a bevy_ecs generation
/// mismatch anyway, not a meaningful "undo despawn."
fn render_recent_selections(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    if state.recent_selections.is_empty() {
        return;
    }
    let mut diet_q = world.ecs.query::<&ecology::Diet>();
    let live: Vec<(bevy_ecs::entity::Entity, Option<ecology::Diet>)> = state
        .recent_selections
        .iter()
        .filter(|&&e| world.ecs.get_entity(e).is_some())
        .map(|&e| (e, diet_q.get(&world.ecs, e).ok().cloned()))
        .collect();
    if live.is_empty() {
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Recent:")
                .small()
                .color(crate::theme::DISABLED_FG),
        );
        for (entity, diet) in live {
            let label = match &diet {
                Some(diet) => format!("{diet:?}"),
                None => "Entity".to_string(),
            };
            let color = diet
                .as_ref()
                .map(crate::theme::chart_color)
                .unwrap_or(crate::theme::DISABLED_FG);
            if ui
                .small_button(egui::RichText::new(label).color(color))
                .clicked()
            {
                actions.push(MenuAction::SelectEntity(entity));
            }
        }
    });
    ui.add_space(crate::theme::SPACE_SM);
    ui.separator();
    ui.add_space(crate::theme::SPACE_SM);
}

/// Inspector panel — shows the selected/tracked organism's live component data.
pub fn inspector_ui(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    world: &mut world::World,
    actions: &mut Vec<MenuAction>,
) {
    render_recent_selections(ui, state, world, actions);

    let entity = match state.selected_entity.or(state.tracked_entity) {
        Some(e) => e,
        None => {
            crate::widgets::empty_state(ui, "Select an organism to view its details");
            return;
        }
    };

    // An explicit "this entity no longer exists" state, checked *before*
    // any component query — otherwise a despawned entity (almost always
    // because it died) would fall through every query below as a plain
    // `Err` indistinguishable from "exists but happens to lack this one
    // optional component," rendering dozens of generic "Not Available"
    // rows with no indication the organism was gone at all. Deliberately
    // does *not* clear `selected_entity`/`tracked_entity` — showing "you
    // were looking at this, and it died" is more informative than
    // silently reverting to the generic empty-selection prompt.
    if world.ecs.get_entity(entity).is_none() {
        crate::widgets::empty_state(
            ui,
            "This entity no longer exists (it may have died or been despawned).",
        );
        return;
    }

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
                food.position.truncate(),
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
                mineral.position.truncate(),
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
                corpse.position.truncate(),
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
                .color(crate::theme::GOOD),
        );
        let mut is_tracked = state.tracked_entity == Some(entity);
        if ui.checkbox(&mut is_tracked, "Track").changed() {
            // The explicit per-entity Follow toggle, routed through the
            // single `set_follow` pathway.
            state.set_follow(is_tracked.then_some(entity));
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
                        let species_label = world
                            .ecs
                            .get_resource::<evolution::LineageTracker>()
                            .and_then(|tracker| {
                                tracker.get_record(common::EntityId(entity.to_bits()))
                            })
                            .map(|record| record.species.0.to_string());
                        match &species_label {
                            Some(label) => crate::widgets::kv_row_mono(ui, "SpeciesId", label),
                            None => crate::widgets::kv_row(ui, "SpeciesId", "Not Available"),
                        }
                        // No "GenomeId" row here — the Genetics section a
                        // few sections down is the correct owner of genome
                        // facts. No "EntityName" row either: no such
                        // concept exists anywhere in this codebase;
                        // organisms aren't named. Neither is shown as a
                        // placeholder — add them here only alongside real
                        // data.
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

                // Folds the Physiology/Circulation/Hormone/Immune Viewer
                // panels' full per-segment detail directly into Inspector,
                // rather than leaving them only as separate, default-closed
                // dock panels a researcher would need to discover
                // independently. Each nested section reuses that panel's
                // own render function verbatim (not a reimplementation) —
                // `physiology_viewer_ui`/`circulation_viewer_ui`/
                // `hormone_viewer_ui`/`immune_viewer_ui` all take the same
                // `(ctx, ui, state, world, actions)` shape Inspector itself
                // uses, so they compose directly inside a nested
                // `CollapsingHeader`. Collapsed by default — this is a lot
                // of additional detail, and progressive disclosure keeps
                // Inspector from becoming a wall of everything competing
                // for attention at once.
                ui.add_space(crate::theme::SPACE_SM);
                egui::CollapsingHeader::new("Per-Segment Detail")
                    .default_open(false)
                    .show(ui, |ui| {
                        crate::plugins::physiology_viewer::physiology_viewer_ui(
                            _ctx, ui, state, world, actions,
                        );
                    });
                egui::CollapsingHeader::new("Circulation")
                    .default_open(false)
                    .show(ui, |ui| {
                        crate::plugins::circulation_viewer::circulation_viewer_ui(
                            _ctx, ui, state, world, actions,
                        );
                    });
                egui::CollapsingHeader::new("Hormones")
                    .default_open(false)
                    .show(ui, |ui| {
                        crate::plugins::hormone_viewer::hormone_viewer_ui(
                            _ctx, ui, state, world, actions,
                        );
                    });
                egui::CollapsingHeader::new("Immune Response")
                    .default_open(false)
                    .show(ui, |ui| {
                        crate::plugins::immune_viewer::immune_viewer_ui(
                            _ctx, ui, state, world, actions,
                        );
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
                                "Regulatory CPPN",
                                &format!(
                                    "{} nodes, {} connections",
                                    genome.regulatory_cppn.nodes.len(),
                                    genome.regulatory_cppn.connections.len()
                                ),
                            );
                            crate::widgets::kv_row(
                                ui,
                                "Regulatory Genes",
                                &genetics::REGULATORY_GENE_ROLES.len().to_string(),
                            );
                            // `Genome::mutation_count` is a real running
                            // count, incremented once per `Genome::mutate`
                            // call. A full per-event history (what
                            // changed, when) is a larger, separate feature
                            // — not implemented, so no "MutationHistory"
                            // row is shown rather than a fabricated or
                            // placeholder one.
                            crate::widgets::kv_row_mono(
                                ui,
                                "MutationCount",
                                &genome.mutation_count.to_string(),
                            );
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

            // --- EVOLUTION / HISTORY ---
            // Folds the Cell Lineage Viewer in the same way the
            // Physiology/Circulation/Hormone/Immune sections fold their own
            // panels above — `lineage_viewer_ui` takes the same
            // `(ctx, ui, state, world, actions)` shape, reused verbatim.
            // Collapsed by default, same progressive-disclosure reasoning.
            egui::CollapsingHeader::new(format!(
                "{} Evolution / History",
                egui_remixicon::icons::GIT_BRANCH_LINE
            ))
            .default_open(false)
            .show(ui, |ui| {
                crate::plugins::lineage_viewer::lineage_viewer_ui(_ctx, ui, state, world, actions);
            });

            // --- NEURAL ---
            egui::CollapsingHeader::new(format!("{} Neural", egui_remixicon::icons::BRAIN_LINE))
                .default_open(true)
                .show(ui, |ui| {
                    egui::Grid::new("insp_neural").striped(true).show(ui, |ui| {
                        // `Brain.nodes[i].state` is real, per-tick CTRNN
                        // (continuous-time recurrent neural network — the
                        // organism's brain model) activation:
                        // `brain::Brain::set_inputs`/`get_outputs` write
                        // inputs into the first `input_count` node states
                        // and read outputs from the last `output_count`. A
                        // compact preview (first 6 values), not a full
                        // per-neuron dump — the dedicated Neural Viewer
                        // panel owns full topology detail; this Inspector
                        // section is a quick-glance summary.
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

                            let preview = |values: &[f32]| -> String {
                                let shown: Vec<String> = values
                                    .iter()
                                    .take(6)
                                    .map(|v| format!("{v:.2}"))
                                    .collect();
                                if values.len() > 6 {
                                    format!("[{}, …] ({} total)", shown.join(", "), values.len())
                                } else {
                                    format!("[{}]", shown.join(", "))
                                }
                            };

                            let input_states: Vec<f32> = brain
                                .nodes
                                .iter()
                                .take(brain.input_count)
                                .map(|n| n.state)
                                .collect();
                            crate::widgets::kv_row_mono(
                                ui,
                                "BrainInputs",
                                &preview(&input_states),
                            );

                            let outputs = brain.get_outputs();
                            crate::widgets::kv_row_mono(ui, "BrainOutputs", &preview(&outputs));

                            let states: Vec<f32> = brain.nodes.iter().map(|n| n.state).collect();
                            let mean_activity = if states.is_empty() {
                                0.0
                            } else {
                                states.iter().map(|s| s.abs()).sum::<f32>() / states.len() as f32
                            };
                            let max_activity =
                                states.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                            crate::widgets::kv_row_mono(
                                ui,
                                "NeuronActivity",
                                &format!(
                                    "mean|state|={mean_activity:.2}, max={max_activity:.2} ({} neurons)",
                                    states.len()
                                ),
                            );

                            let mean_weight = if brain.synapses.is_empty() {
                                0.0
                            } else {
                                brain.synapses.iter().map(|s| s.weight.abs()).sum::<f32>()
                                    / brain.synapses.len() as f32
                            };
                            let max_weight = brain
                                .synapses
                                .iter()
                                .fold(0.0f32, |m, s| m.max(s.weight.abs()));
                            crate::widgets::kv_row_mono(
                                ui,
                                "SynapseActivity",
                                &format!(
                                    "mean|weight|={mean_weight:.2}, max={max_weight:.2} ({} synapses)",
                                    brain.synapses.len()
                                ),
                            );
                        } else {
                            crate::widgets::kv_row(ui, "Brain", "Not Available");
                        }
                    });
                });

            // --- MORPHOLOGY ---
            // Deliberately just Transform/Velocity here — a real, working
            // segment tree is already rendered a few sections below in
            // "Body Plan", so a second "BodyPlan"/"SegmentTree" summary
            // here would be redundant. "SensorArray"/"MuscleSystem" have no
            // backing data source in this codebase and are omitted rather
            // than shown as a placeholder.
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
                    });
            });

            // --- BEHAVIOR ---
            // No "ActionState"/"MemoryState" rows here — no such concepts
            // exist in `behavior`'s component set today. Add them only
            // alongside real data if those concepts are built.
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

                            // Rather than duplicate the same `SpeciesId`
                            // value the Identity section already shows,
                            // this row answers a genuinely different,
                            // Ecology-relevant question: how large is that
                            // species' *current living population* —
                            // reusing `MetricsState::species_distribution`'s
                            // periodic snapshot, not a second lookup
                            // mechanism.
                            let species_id = world
                                .ecs
                                .get_resource::<evolution::LineageTracker>()
                                .and_then(|tracker| {
                                    tracker.get_record(common::EntityId(entity.to_bits()))
                                })
                                .map(|record| record.species.0);
                            match species_id {
                                Some(id) => {
                                    let population = world
                                        .ecs
                                        .get_resource::<analytics::MetricsState>()
                                        .and_then(|m| {
                                            m.species_distribution
                                                .iter()
                                                .find(|&&(sid, _)| sid == id)
                                                .map(|&(_, count)| count)
                                        });
                                    match population {
                                        Some(count) => crate::widgets::kv_row_mono(
                                            ui,
                                            "SpeciesMembership",
                                            &format!("{count} organisms currently alive"),
                                        ),
                                        None => crate::widgets::kv_row(
                                            ui,
                                            "SpeciesMembership",
                                            "Not yet sampled (Metrics updates periodically)",
                                        ),
                                    }
                                }
                                None => crate::widgets::kv_row(
                                    ui,
                                    "SpeciesMembership",
                                    "Not Available",
                                ),
                            }
                        });
                });

            // --- RELATIONSHIPS / HISTORY ---
            // "Nearby organisms" uses spatial proximity (a distance query
            // against `sensing::HeadVision.range` when present), not a
            // spring-graph BFS over physically-connected entities (same
            // body, or a colony bud-link) — that BFS would show nothing at
            // all for the common case of two separate, unlinked organisms
            // simply near each other. Spatial distance is the mechanism
            // that actually answers "who else is around me right now,"
            // which is what this section and "interaction radius" are both
            // about.
            egui::CollapsingHeader::new(format!(
                "{} Relationships / History",
                egui_remixicon::icons::TEAM_LINE
            ))
            .default_open(false)
            .show(ui, |ui| {
                let mut node_q = world
                    .ecs
                    .query::<(&physics::ParticleNode, Option<&sensing::HeadVision>)>();
                let Ok((node, vision)) = node_q.get(&world.ecs, entity) else {
                    crate::widgets::empty_state(ui, "No position data for this entity.");
                    return;
                };
                let my_pos = node.position;
                let radius = vision.map(|v| v.range);

                egui::Grid::new("insp_relationships_summary")
                    .striped(true)
                    .show(ui, |ui| {
                        match radius {
                            Some(r) => crate::widgets::kv_row_mono(
                                ui,
                                "InteractionRadius",
                                &format!("{r:.1} units (vision range)"),
                            ),
                            None => crate::widgets::kv_row(
                                ui,
                                "InteractionRadius",
                                "Not Available (no HeadVision)",
                            ),
                        }
                        crate::widgets::kv_row_mono(
                            ui,
                            "TrajectorySamples",
                            &state.trajectory_history.len().to_string(),
                        );
                    });

                ui.add_space(crate::theme::SPACE_SM);
                ui.label(egui::RichText::new("Nearby Organisms").strong());
                let search_radius = radius.unwrap_or(250.0);
                let mut all_q = world
                    .ecs
                    .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode, &ecology::Diet)>();
                let mut nearby: Vec<(bevy_ecs::entity::Entity, ecology::Diet, f32)> = all_q
                    .iter(&world.ecs)
                    .filter(|&(e, ..)| e != entity)
                    .map(|(e, other_node, diet)| {
                        (e, diet.clone(), my_pos.distance(other_node.position))
                    })
                    .filter(|&(_, _, dist)| dist <= search_radius)
                    .collect();
                nearby.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                if nearby.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("None within {search_radius:.0} units."))
                            .small()
                            .color(crate::theme::DISABLED_FG),
                    );
                } else {
                    egui::Grid::new("insp_nearby")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Diet").strong());
                            ui.label(egui::RichText::new("Distance").strong());
                            ui.end_row();
                            for (_e, diet, dist) in nearby.iter().take(10) {
                                ui.colored_label(
                                    crate::theme::chart_color(diet),
                                    format!("{diet:?}"),
                                );
                                ui.monospace(format!("{dist:.1}"));
                                ui.end_row();
                            }
                        });
                    if nearby.len() > 10 {
                        ui.label(
                            egui::RichText::new(format!("…and {} more.", nearby.len() - 10))
                                .small()
                                .color(crate::theme::DISABLED_FG),
                        );
                    }
                }

                ui.add_space(crate::theme::SPACE_SM);
                ui.label(egui::RichText::new("Trajectory History").strong());
                if state.tracked_entity != Some(entity) {
                    ui.label(
                        egui::RichText::new(
                            "Enable \"Track\" above to start recording this organism's path.",
                        )
                        .small()
                        .color(crate::theme::DISABLED_FG),
                    );
                } else if state.trajectory_history.len() < 2 {
                    ui.label(
                        egui::RichText::new("Not enough samples yet.")
                            .small()
                            .color(crate::theme::DISABLED_FG),
                    );
                } else {
                    let start = state.trajectory_history.front().unwrap();
                    let end = state.trajectory_history.back().unwrap();
                    let net_displacement = start.distance(*end);
                    let path_length: f32 = state
                        .trajectory_history
                        .iter()
                        .zip(state.trajectory_history.iter().skip(1))
                        .map(|(a, b)| a.distance(*b))
                        .sum();
                    egui::Grid::new("insp_trajectory")
                        .striped(true)
                        .show(ui, |ui| {
                            crate::widgets::kv_row_mono(
                                ui,
                                "PathLength",
                                &format!("{path_length:.1} units"),
                            );
                            crate::widgets::kv_row_mono(
                                ui,
                                "NetDisplacement",
                                &format!("{net_displacement:.1} units"),
                            );
                        });
                }
            });

            // --- BODY PLAN ---
            egui::CollapsingHeader::new(format!("{} Body Plan", egui_remixicon::icons::NODE_TREE))
                .default_open(false)
                .show(ui, |ui| {
                    render_body_plan(ui, state, world, entity);
                });
        });
}

/// Renders the selected organism's segment/spring tree (`utils::draw_segment_tree`)
/// rooted at its head node — completes a feature that was fully implemented
/// but never wired into the Inspector (see `IMPLEMENTATION_STATUS.md`'s
/// dead-code finding). Clicking a row in the tree re-selects that segment,
/// same as clicking it in the viewport.
fn render_body_plan(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    world: &mut world::World,
    entity: bevy_ecs::entity::Entity,
) {
    let mut node_q = world.ecs.query::<&physics::ParticleNode>();
    let Ok(node) = node_q.get(&world.ecs, entity) else {
        crate::widgets::empty_state(ui, "Not a physical body segment.");
        return;
    };
    let organism_id = node.organism_id;

    // The tree always renders rooted at the organism's head, regardless of
    // which segment is currently selected, so the shape reads the same way
    // no matter what a user clicked on in the viewport.
    let mut all_nodes = world
        .ecs
        .query::<(bevy_ecs::entity::Entity, &physics::ParticleNode)>();
    let head = all_nodes
        .iter(&world.ecs)
        .find(|(_, n)| n.organism_id == organism_id && n.segment_type == 0)
        .map(|(e, _)| e)
        .unwrap_or(entity);

    let mut adj: std::collections::HashMap<
        bevy_ecs::entity::Entity,
        Vec<(bevy_ecs::entity::Entity, physics::Spring)>,
    > = std::collections::HashMap::new();
    let mut spring_q = world.ecs.query::<&physics::Spring>();
    for spring in spring_q.iter(&world.ecs) {
        adj.entry(spring.node_a)
            .or_default()
            .push((spring.node_b, spring.clone()));
        adj.entry(spring.node_b)
            .or_default()
            .push((spring.node_a, spring.clone()));
    }

    let mut visited = std::collections::HashSet::new();
    crate::utils::draw_segment_tree(
        ui,
        head,
        &adj,
        &world.ecs,
        &mut visited,
        &mut state.selected_entity,
    );
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
            // Same single Follow pathway as the organism Inspector's own
            // "Track" checkbox above.
            state.set_follow(is_tracked.then_some(entity));
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
