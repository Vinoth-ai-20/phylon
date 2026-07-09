//! Life-stage transitions and re-entrant growth (Phase 4, `PHASE4_ROADMAP.md`
//! milestone P4-L1, ADR-P4-03).

use crate::components::{GrowthState, LifeStage};
use crate::developmental_graph::DevelopmentalGraph;
use bevy_ecs::prelude::{Commands, Entity, Query, Without};

/// Fraction of `metabolism::Age.max_lifespan` at which a `Juvenile` organism
/// matures into an `Adult` — untuned placeholder, same status as every
/// other Phase 4 rate/threshold constant introduced this phase.
const MATURITY_AGE_FRACTION: f32 = 0.1;

/// Distance between adjacent spine nodes for resumed growth — must match
/// `organisms::spawning::spawn_organism`'s own `segment_length` literal
/// (`20.0`), since nothing persists the organism's original value once
/// `GrowthState` is first removed.
const RESUMED_SEGMENT_LENGTH: f32 = 20.0;

/// ~0.5s per segment bud at 60Hz — matches `spawn_organism`'s own bud
/// interval, for the same reason as `RESUMED_SEGMENT_LENGTH`.
const RESUMED_BUD_INTERVAL: u64 = 30;

/// # Life Stage System
///
/// ## 1. What Happens
/// Promotes a `Juvenile` organism to `Adult` once it clears a maturity age
/// threshold, and — the "re-entrant growth" ADR-P4-03 calls for —
/// re-inserts a `GrowthState`, resumed from the organism's *current* body
/// (via its persistent `DevelopmentalGraph`), so `growth_system` picks up
/// growing it again from exactly where it left off — typically just past an
/// early `Tail` decode, since `next_segment_index` was already advanced past
/// whatever position stopped juvenile growth (see `growth_system`, which
/// increments it before checking for `Tail`). The resumed growth passes the
/// `Adult` life-stage signal into
/// `genetics::develop_at_position_with_life_stage` (see that function's doc
/// comment), so these newly-reached positions are decoded under a genuinely
/// different context than a juvenile-context decode of the same position
/// index would have produced — not a deterministic continuation of what
/// juvenile growth would have done had it not stopped early. An organism
/// whose juvenile growth used its entire `MAX_SEGMENTS` budget (rather than
/// stopping via an early `Tail`) has no further room to grow as an adult —
/// this is treated as correct, in-universe behavior (it matured to its full
/// possible size already), not a gap to patch.
///
/// ## 2. Why It Happens
/// Before this milestone, `growth_system`'s "grow once, wire a `Brain`,
/// remove `GrowthState` forever" transition was strictly one-way — there
/// was no life-cycle concept at all. ADR-P4-03 requires re-entrancy on the
/// *same* entity (preserving lineage/identity continuity) rather than
/// spawning a wholly separate "adult" organism, which it explicitly
/// rejected as an alternative.
///
/// ## 3. How It Happens
/// Only organisms *not* currently mid-growth (`Without<GrowthState>` —
/// growth must have completed at least once already) and still `Juvenile`
/// are considered. On promotion:
/// 1. The organism's own persisted `genetics::Genome` component (attached
///    standalone at spawn, independent of `GrowthState` — see
///    `spawning::spawn_organism`) seeds the resumed state; no genome is
///    reconstructed or guessed.
/// 2. `next_segment_index`/`parent_spine_node`/`current_pos`/`heading` are
///    derived from the last spine node in the `DevelopmentalGraph` — not
///    reset to zero — so growth resumes exactly where the body currently
///    ends, in the same direction it was already growing.
/// 3. `effectors` is *reseeded* from every `Elastic`/`Rotational` spring
///    already attached to any entity this organism's graph owns (the same
///    filter `growth_system` itself uses when it first discovers an
///    effector), not left empty — so the eventual brain rebuild wires the
///    whole adult body, not just newly-grown segments.
///
/// **Brain reconciliation is a full rebuild, not an in-place extension** —
/// `growth_system`'s existing completion branch is entirely unchanged by
/// this milestone; when growth finishes a second time, it simply overwrites
/// the existing `Brain`/`Neuromodulators`/`SensoryState`/`MotorSystem`/
/// `SignalEmitter` components, exactly as if this were the organism's
/// first-ever completion. An in-place topology *extension* would need a
/// `Brain`/`CtrnnNode` mutation API that doesn't exist today, and would
/// raise open questions about what a hidden node "means" once the topology
/// around it changes — this milestone does not attempt to resolve that;
/// see `PHASE4_ROADMAP.md`'s P4-L1 execution log for the full reasoning.
///
/// **Known limitation, stated plainly:** any Hebbian-adapted synapse
/// weights the juvenile brain accumulated via
/// `crate::hebbian_plasticity_system` are lost at rebuild — the adult brain
/// starts fresh from the genome's CPPN, at the adult life-stage signal,
/// exactly as the organism's first-ever brain did at the juvenile one.
/// Preserving learned weights across a life-stage transition is future
/// work, not attempted here.
pub fn life_stage_system(
    mut commands: Commands,
    mut candidates: Query<
        (
            Entity,
            &mut LifeStage,
            &metabolism::Age,
            &genetics::Genome,
            &DevelopmentalGraph,
        ),
        Without<GrowthState>,
    >,
    node_query: Query<&physics::ParticleNode>,
    spring_query: Query<(Entity, &physics::Spring)>,
) {
    for (entity, mut life_stage, age, genome, graph) in candidates.iter_mut() {
        if *life_stage != LifeStage::Juvenile {
            continue;
        }
        let maturity_age = (age.max_lifespan as f32 * MATURITY_AGE_FRACTION) as u64;
        if age.ticks < maturity_age {
            continue;
        }

        // Every non-branch (spine) node, in growth order — the tail of this
        // slice is where resumed growth continues from.
        let spine: Vec<&crate::developmental_graph::DevelopmentalNode> =
            graph.nodes.iter().filter(|n| !n.is_branch).collect();
        let Some(last_spine) = spine.last() else {
            continue; // Unreachable in practice (every graph has a head), never panic.
        };
        let Some(last_entity) = last_spine.entity else {
            continue; // Not a real, materialized graph (shouldn't happen post-spawn).
        };
        let Ok(last_node) = node_query.get(last_entity) else {
            continue;
        };

        // Heading: direction from the second-to-last spine node toward the
        // last one, so resumed growth continues the same way the body was
        // already growing. A single-segment (head-only) body has no
        // direction to infer — default to 0.0.
        let heading = if spine.len() >= 2 {
            spine[spine.len() - 2]
                .entity
                .and_then(|prev_entity| node_query.get(prev_entity).ok())
                .map(|prev_node| {
                    let delta = last_node.position - prev_node.position;
                    if delta.length() > 0.0001 {
                        delta.y.atan2(delta.x)
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };

        // Reseed `effectors` from every actuated spring already attached to
        // this organism's body, so the eventual brain rebuild wires the
        // whole adult body, not just newly-grown segments.
        let graph_entities: std::collections::HashSet<bevy_ecs::entity::Entity> =
            graph.nodes.iter().filter_map(|n| n.entity).collect();
        let effectors: Vec<bevy_ecs::entity::Entity> = spring_query
            .iter()
            .filter(|(_, spring)| {
                (graph_entities.contains(&spring.node_a) || graph_entities.contains(&spring.node_b))
                    && (spring.constraint_type == physics::ConstraintType::Elastic
                        || spring.constraint_type == physics::ConstraintType::Rotational)
            })
            .map(|(s, _)| s)
            .collect();

        *life_stage = LifeStage::Adult;

        commands.entity(entity).insert(GrowthState {
            genome: genome.clone(),
            next_segment_index: spine.len(),
            ticks_until_next_bud: RESUMED_BUD_INTERVAL,
            base_bud_interval: RESUMED_BUD_INTERVAL,
            parent_spine_node: Some(last_entity),
            current_pos: last_node.position
                + common::Vec3::new(heading.cos(), heading.sin(), 0.0) * -RESUMED_SEGMENT_LENGTH,
            segment_length: RESUMED_SEGMENT_LENGTH,
            effectors,
            is_organism_complete: false,
            heading,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use rand::SeedableRng;

    /// Runs `growth_system` repeatedly until `GrowthState` is removed (growth
    /// completion) or a fixed iteration ceiling is hit — mirrors
    /// `systems::tests::run_growth_to_completion` exactly (kept as a small,
    /// separate copy rather than exporting the original, to avoid widening
    /// that helper's visibility just for this module's tests).
    fn run_growth_to_completion(world: &mut World, entity: Entity) {
        for _ in 0..(crate::MAX_SEGMENTS * 40) {
            if world.get::<GrowthState>(entity).is_none() {
                return;
            }
            world.run_system_once(crate::growth_system);
        }
        panic!("growth_system did not complete within the iteration ceiling");
    }

    fn spawn_and_grow_to_adulthood(world: &mut World, genome: genetics::Genome) -> Entity {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let entity = crate::spawn_organism(
            world,
            &genome,
            common::Vec3::new(0.0, 0.0, 0.0),
            ecology::Diet::Herbivore,
            ecology::EcologicalCategory::None,
            0,
            0,
            &mut rng,
        );
        run_growth_to_completion(world, entity);
        assert_eq!(
            world.get::<LifeStage>(entity).copied(),
            Some(LifeStage::Juvenile),
            "sanity check: still juvenile immediately after first growth completes"
        );

        // Advance age past the maturity threshold without ticking the whole
        // simulation — `max_lifespan` is 10_000 at spawn (`spawning.rs`), so
        // `MATURITY_AGE_FRACTION * 10_000 = 1_000`.
        world.get_mut::<metabolism::Age>(entity).unwrap().ticks = 1_500;

        world.run_system_once(life_stage_system);
        entity
    }

    #[test]
    fn life_stage_system_does_not_promote_before_maturity_age() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let entity = crate::spawn_organism(
            &mut world,
            &genome,
            common::Vec3::new(0.0, 0.0, 0.0),
            ecology::Diet::Herbivore,
            ecology::EcologicalCategory::None,
            0,
            0,
            &mut rng,
        );
        run_growth_to_completion(&mut world, entity);
        // Age is 0 immediately after spawn/growth — well under the
        // maturity threshold.
        world.run_system_once(life_stage_system);

        assert_eq!(
            world.get::<LifeStage>(entity).copied(),
            Some(LifeStage::Juvenile)
        );
        assert!(
            world.get::<GrowthState>(entity).is_none(),
            "growth must not resume before maturity"
        );
    }

    #[test]
    fn life_stage_system_promotes_and_resumes_growth_at_maturity_age() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_and_grow_to_adulthood(&mut world, genome);

        assert_eq!(
            world.get::<LifeStage>(entity).copied(),
            Some(LifeStage::Adult)
        );
        assert!(
            world.get::<GrowthState>(entity).is_some(),
            "growth should have resumed on the maturity transition"
        );

        let graph = world.get::<DevelopmentalGraph>(entity).unwrap();
        let spine_count = graph.nodes.iter().filter(|n| !n.is_branch).count();
        let state = world.get::<GrowthState>(entity).unwrap();
        assert_eq!(
            state.next_segment_index, spine_count,
            "resumed growth must continue from the current body's actual length"
        );
        assert!(!state.is_organism_complete);
    }

    #[test]
    fn resumed_growth_reaches_completion_and_rebuilds_the_brain() {
        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let genome = genetics::Genome::new_minimal(genetics::GenomeId(1), common::EntityId(0));
        let entity = spawn_and_grow_to_adulthood(&mut world, genome);

        assert!(world.get::<GrowthState>(entity).is_some());
        run_growth_to_completion(&mut world, entity);

        assert!(
            world.get::<GrowthState>(entity).is_none(),
            "resumed growth should complete and remove GrowthState again"
        );
        assert!(
            world.get::<brain::Brain>(entity).is_some(),
            "the brain should have been rebuilt for the adult body"
        );
    }

    #[test]
    fn adult_growth_decodes_a_position_differently_than_a_juvenile_decode_would_have() {
        // The roadmap's own verification ask for P4-L1: "a fixture-genome
        // test asserting a life-stage transition actually changes decoded
        // segment sequence." Uses the same hand-built, linearly-sensitive
        // CPPN fixture `genetics::develop`'s own
        // `nonzero_life_stage_signal_can_change_the_decode` test relies on,
        // confirming `growth_system` actually threads a `LifeStage::Adult`
        // entity's signal through to `develop_at_position_with_life_stage`,
        // not just defaulting to `0.0` regardless of life stage.
        use genetics::cppn::DEFAULT_MUTATION_RATE;
        use genetics::{Cppn, CppnConnection, CppnNode};

        let regulatory_cppn = Cppn {
            nodes: vec![
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 0,
                },
                CppnNode {
                    activation: brain::ActivationFn::Linear,
                    bias: 0.0,
                    layer: 1,
                },
            ],
            connections: vec![CppnConnection {
                source: 0,
                target: 1,
                weight: 10.0,
                enabled: true,
                innovation: 0,
                mutation_rate: DEFAULT_MUTATION_RATE,
            }],
        };

        let position = 5;
        let juvenile_counterfactual = genetics::develop_at_position_with_life_stage(
            &regulatory_cppn,
            position,
            crate::MAX_SEGMENTS,
            LifeStage::Juvenile.developmental_signal(),
        );

        let mut world = World::new();
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        let entity = world
            .spawn((
                GrowthState {
                    genome: genetics::Genome::new_minimal(
                        genetics::GenomeId(1),
                        common::EntityId(0),
                    ),
                    next_segment_index: position,
                    ticks_until_next_bud: 0,
                    base_bud_interval: 30,
                    parent_spine_node: None,
                    current_pos: common::Vec3::new(0.0, 0.0, 0.0),
                    segment_length: RESUMED_SEGMENT_LENGTH,
                    effectors: Vec::new(),
                    is_organism_complete: false,
                    heading: 0.0,
                },
                DevelopmentalGraph::new(),
                LifeStage::Adult,
            ))
            .id();
        world
            .get_mut::<GrowthState>(entity)
            .unwrap()
            .genome
            .regulatory_cppn = regulatory_cppn;

        world.run_system_once(crate::growth_system);

        let graph = world.get::<DevelopmentalGraph>(entity).unwrap();
        let decoded_node = graph
            .nodes
            .iter()
            .find(|n| n.position == position && !n.is_branch)
            .expect("growth_system should have decoded and pushed this position");

        assert_ne!(
            decoded_node.outputs, juvenile_counterfactual,
            "an Adult-context decode must differ from what a Juvenile-context decode of the same position would have produced"
        );
    }
}
