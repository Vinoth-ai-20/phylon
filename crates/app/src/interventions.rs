//! The subset of `ui::MenuAction` handlers that mutate simulation state in a
//! way that's safe to record and replay (see `storage::replay::ReplayAction`'s
//! doc comment for which four, and why only these). Factored into methods
//! here — rather than left inline in `events.rs`'s menu-action match arm —
//! so `events.rs`'s live handler and `replay::run_replay`'s playback driver
//! call the exact same code, instead of two copies that could silently
//! drift apart.

use crate::app::PhylonApp;

impl PhylonApp {
    /// Despawns every entity, resets time/atmosphere/metrics/lineage
    /// tracking, and reseeds the initial populations — the full body of
    /// `MenuAction::ReseedEcosystem`'s original handler.
    pub(crate) fn apply_reseed_ecosystem(&mut self) {
        // Despawn all entities
        let entities: Vec<_> = self.world.ecs.iter_entities().map(|e| e.id()).collect();
        for entity in entities {
            self.world.ecs.despawn(entity);
        }

        // Reset tracking
        self.ui.selected_entity = None;
        self.ui.tracked_entity = None;

        // Reset time/atmosphere/metrics — without this, a "fresh"
        // simulation kept the old tick count, day-night phase, and Metrics
        // history, so the status bar and graphs looked like nothing had
        // actually reset.
        self.total_sim_time = 0.0;
        self.accumulated_time = 0.0;
        self.world
            .ecs
            .insert_resource(metabolism::GlobalAtmosphere::default());
        self.world
            .ecs
            .insert_resource(analytics::MetricsState::new());

        // Clear lineage tracker
        if let Some(mut tracker) = self
            .world
            .ecs
            .get_resource_mut::<evolution::LineageTracker>()
        {
            *tracker = evolution::LineageTracker::new();
        }

        // Respawn defaults
        let mut tracker = evolution::LineageTracker::new();
        let mut species_registry = evolution::SpeciesRegistry::default();
        let mut global_tracker = genetics::GlobalInnovationTracker::default();
        self.world
            .ecs
            .resource_scope::<common::SimRng, _>(|ecs, mut sim_rng| {
                crate::app::seed_ecosystem(
                    ecs,
                    &mut tracker,
                    &mut species_registry,
                    &mut global_tracker,
                    &mut sim_rng.0,
                );
            });
        self.world.ecs.insert_resource(tracker);
        self.world.ecs.insert_resource(species_registry);
        self.world.ecs.insert_resource(global_tracker);
    }

    /// Spawns a named sandbox preset at `position` — the full body of
    /// `MenuAction::SpawnPreset`'s original handler (minus resolving
    /// `position` from `self.ui.camera_pos`, since replay passes an
    /// already-resolved recorded position instead).
    pub(crate) fn apply_spawn_preset(&mut self, name: &str, position: common::Vec2) {
        let preset_opt = organisms::sandbox::PresetDefinition::standard_presets()
            .into_iter()
            .find(|p| p.name == name);

        let Some(preset) = preset_opt else {
            return;
        };

        if preset.evolvable {
            let diet = preset.diet.unwrap_or(ecology::Diet::Herbivore);
            // Evolvable presets get a seed regulatory genome matching the
            // corresponding starter species' body-plan tendency (Phase 3
            // M4 — see `app::seed_ecosystem`'s doc comment; pigmentation is
            // emergent, so this no longer forces the diet's canonical
            // color the way the retired `HoxSequence`-driven path did).
            // Phase 5, SX-2a (ADR-P5-07): mirrors `seed_ecosystem`'s own
            // swept `RegulatorySeedWeights` combinations for the
            // corresponding starter species — see
            // `crate::app::seed_regulatory_cppn`'s doc comment.
            let weights = match name {
                "Herbivore (Evolvable)" => crate::app::RegulatorySeedWeights {
                    output_bias: -4.45,
                    hox_weight: 8.97,
                    differentiation_weight: 7.07,
                    effector_weight: 3.12,
                    pigment_weight: 1.22,
                    sine_coarse_weight: 2.15,
                    sine_fine_weight: 1.76,
                },
                "Hunter (Evolvable)" => crate::app::RegulatorySeedWeights {
                    output_bias: -4.40,
                    hox_weight: 6.21,
                    differentiation_weight: 6.27,
                    effector_weight: 6.99,
                    pigment_weight: 0.88,
                    sine_coarse_weight: 0.34,
                    sine_fine_weight: 1.95,
                },
                "Edible Plant (Evolvable)" => crate::app::RegulatorySeedWeights {
                    output_bias: -3.0,
                    hox_weight: 0.0,
                    differentiation_weight: 0.0,
                    effector_weight: 0.0,
                    pigment_weight: 1.0,
                    sine_coarse_weight: 0.0,
                    sine_fine_weight: 0.0,
                },
                _ => crate::app::RegulatorySeedWeights {
                    output_bias: -4.45,
                    hox_weight: 8.97,
                    differentiation_weight: 7.07,
                    effector_weight: 3.12,
                    pigment_weight: 1.22,
                    sine_coarse_weight: 2.15,
                    sine_fine_weight: 1.76,
                },
            };
            let genome = genetics::Genome::seed(
                genetics::GenomeId(0), // Would normally be a unique ID
                common::EntityId(0),
                crate::app::seed_brain_cppn(),
                genetics::Cppn::new(),
                crate::app::seed_regulatory_cppn(weights),
            );

            let category = preset.category.unwrap_or(ecology::EcologicalCategory::None);

            self.world
                .ecs
                .resource_scope::<common::SimRng, _>(|ecs, mut sim_rng| {
                    organisms::spawn_organism(
                        ecs,
                        &genome,
                        position,
                        diet,
                        category,
                        0,
                        0,
                        &mut sim_rng.0,
                    );
                });
        } else {
            // Non-evolvable structures get a fixed static node topology.
            // For Membrane Seed or Structure Node, just spawn a single node.
            let seg_type = if preset.traits.is_membrane_seed { 1 } else { 0 };
            let color = if preset.traits.is_membrane_seed {
                [0.5, 0.5, 0.9]
            } else {
                [0.7, 0.7, 0.7]
            };

            let entity = self.world.ecs.spawn_empty().id();
            let mut node = physics::ParticleNode::new(position, 5.0, seg_type, entity.index());
            node.is_fixed = preset.traits.fixable;
            self.world.ecs.entity_mut(entity).insert((
                node,
                organisms::OrganismColor(color),
                preset.traits, // Attach SandboxTraits
            ));

            // Attach biological components so Inspector can view it
            self.world.ecs.entity_mut(entity).insert((
                metabolism::ChemicalEconomy {
                    glucose: 10000.0,
                    o2: 10000.0,
                    co2: 0.0,
                    atp: 10000.0,
                    max_glucose: 100000.0,
                    max_o2: 10000.0,
                    max_co2: 10000.0,
                    max_atp: 100000.0,
                },
                metabolism::Age {
                    ticks: 0,
                    max_lifespan: 10000,
                },
            ));
        }
    }

    /// Spawns a deterministic "Proto-Fish" at `position` — the full body of
    /// `MenuAction::SpawnProtoFish`'s original handler.
    pub(crate) fn apply_spawn_proto_fish(&mut self, position: common::Vec2) {
        let fish_genome = genetics::Genome::seed(
            genetics::GenomeId(100),
            common::EntityId(0),
            crate::app::seed_brain_cppn(),
            genetics::Cppn::new(),
            crate::app::seed_regulatory_cppn(crate::app::RegulatorySeedWeights {
                output_bias: -4.40,
                hox_weight: 6.21,
                differentiation_weight: 6.27,
                effector_weight: 6.99,
                pigment_weight: 0.88,
                sine_coarse_weight: 0.34,
                sine_fine_weight: 1.95,
            }),
        );
        self.world
            .ecs
            .resource_scope::<common::SimRng, _>(|ecs, mut sim_rng| {
                organisms::spawn_organism(
                    ecs,
                    &fish_genome,
                    position,
                    ecology::Diet::Carnivore,
                    ecology::EcologicalCategory::None,
                    0,
                    0,
                    &mut sim_rng.0,
                );
            });
    }

    /// Spawns a manual catastrophe hazard at `position`/`tick` — the full
    /// body of `MenuAction::SpawnManualHazard`'s original handler.
    pub(crate) fn apply_spawn_manual_hazard(&mut self, position: common::Vec2, tick: u64) {
        let mut manager = self
            .world
            .ecs
            .resource_mut::<ecology::catastrophe::CatastropheManager>();
        manager.spawn_hazard(common::Tick(tick), position);
    }
}
