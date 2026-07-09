//! Cross-system ecology tests (Phase 7, W5d) — extracted verbatim from
//! `lib.rs`'s `foraging_feeding_effect_tests` module. Kept as one file
//! (rather than distributed per-system) since it already spans multiple
//! systems (`foraging_system`, `catastrophe_system`, `food_spawner_system`).

use crate::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;

fn sample_chem(atp: f32, glucose: f32) -> metabolism::ChemicalEconomy {
    metabolism::ChemicalEconomy {
        glucose,
        o2: 0.0,
        co2: 0.0,
        atp,
        max_glucose: 1000.0,
        max_o2: 100.0,
        max_co2: 100.0,
        max_atp: 100.0,
    }
}

fn base_world() -> World {
    let mut world = World::new();
    world.insert_resource(events::TimedEffects::default());
    world.insert_resource(metabolism::GlobalAtmosphere::default());
    world.insert_resource(ResourceSpatialGrids::new(50.0));
    world
}

/// Phase 5, SX-2c: a successful organism-vs-organism predation should
/// spawn a real `TimedEffects` burst at the predator's position — the
/// gap this milestone closes (previously nothing marked the moment of
/// the attack itself, only the prey's eventual death).
#[test]
fn predation_spawns_a_feeding_effect_at_the_predator_position() {
    let mut world = base_world();
    let predator_pos = common::Vec3::new(100.0, 100.0, 0.0);
    world.spawn((
        physics::ParticleNode::new(predator_pos, 1.0, 0, 0),
        sample_chem(50.0, 0.0),
        Diet::Carnivore,
    ));
    world.spawn((
        physics::ParticleNode::new(predator_pos, 1.0, 0, 1),
        sample_chem(50.0, 10.0),
        Diet::Herbivore,
    ));

    world.run_system_once(foraging_system);

    let effects = &world.resource::<events::TimedEffects>().active;
    assert_eq!(effects.len(), 1);
    let events::TimedEffectKind::FloatingText { text, color } = &effects[0].kind;
    assert_eq!(text, "Hunted!");
    assert_eq!(*color, Diet::Carnivore.standard_color());
    assert_eq!(effects[0].position, predator_pos);
}

/// Herbivory (Herbivore-eats-Producer) is a distinct case with its own
/// text, per `feeding_text`'s exhaustive-enough match.
#[test]
fn herbivory_spawns_a_grazed_effect_at_the_herbivore_position() {
    let mut world = base_world();
    let herbivore_pos = common::Vec3::new(-40.0, 20.0, 0.0);
    world.spawn((
        physics::ParticleNode::new(herbivore_pos, 1.0, 0, 0),
        sample_chem(50.0, 0.0),
        Diet::Herbivore,
    ));
    world.spawn((
        physics::ParticleNode::new(herbivore_pos, 1.0, 0, 1),
        sample_chem(50.0, 10.0),
        Diet::Producer,
    ));

    world.run_system_once(foraging_system);

    let effects = &world.resource::<events::TimedEffects>().active;
    assert_eq!(effects.len(), 1);
    let events::TimedEffectKind::FloatingText { text, color } = &effects[0].kind;
    assert_eq!(text, "Grazed!");
    assert_eq!(*color, Diet::Herbivore.standard_color());
}

/// Two organisms too far apart to interact must not spawn any effect.
#[test]
fn no_effect_when_out_of_range() {
    let mut world = base_world();
    world.spawn((
        physics::ParticleNode::new(common::Vec3::new(0.0, 0.0, 0.0), 1.0, 0, 0),
        sample_chem(50.0, 0.0),
        Diet::Carnivore,
    ));
    world.spawn((
        physics::ParticleNode::new(common::Vec3::new(1000.0, 1000.0, 0.0), 1.0, 0, 1),
        sample_chem(50.0, 10.0),
        Diet::Herbivore,
    ));

    world.run_system_once(foraging_system);

    assert!(world.resource::<events::TimedEffects>().active.is_empty());
}

/// Phase 6, Epic A: `catastrophe_system` used to read a per-call
/// `Local<u64>` tick counter that reset to `0` on every `run_system_once`
/// invocation, so `elapsed = tick - start_tick` was always `0` regardless
/// of how many real ticks had passed — a hazard could never reach
/// `impending_duration` and would stay `Impending` forever. This proves
/// the fix: a hazard whose `start_tick` is far enough in the past
/// (measured via the real `GlobalAtmosphere::ticks` counter) must
/// transition to `Active` the moment `catastrophe_system` runs.
#[test]
fn hazard_transitions_to_active_once_impending_duration_has_really_elapsed() {
    let mut world = World::new();
    world.insert_resource(common::SimRng::from_seed(1));
    world.insert_resource(metabolism::GlobalAtmosphere {
        ticks: 1000,
        ..Default::default()
    });
    world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
    world.insert_resource(diffusion::CpuHazardFieldState::default());
    world.insert_resource(bevy_ecs::event::Events::<catastrophe::HazardSpawned>::default());
    let config = catastrophe::CatastropheConfig {
        spawn_probability: 0.0, // don't let a second hazard spawn mid-test
        ..Default::default()
    };
    let impending_duration = config.impending_duration;
    world.insert_resource(config);
    let mut manager = catastrophe::CatastropheManager::default();
    manager.hazards.push(catastrophe::LocalHazard {
        center: common::Vec2::new(0.0, 0.0),
        state: catastrophe::HazardState::Impending {
            start_tick: common::Tick(1000 - impending_duration as u64),
        },
    });
    world.insert_resource(manager);

    world.run_system_once(catastrophe_system);

    let manager = world.resource::<catastrophe::CatastropheManager>();
    assert_eq!(manager.hazards.len(), 1);
    assert!(matches!(
        manager.hazards[0].state,
        catastrophe::HazardState::Active { .. }
    ));
}

/// Same fixed seed must produce the same hazard-spawn decision and
/// position across two independent `World`s — proving the `fastrand`→
/// `SimRng` migration preserved (rather than broke) this system's
/// determinism guarantee.
#[test]
fn catastrophe_system_is_deterministic_for_a_given_seed() {
    fn run_once() -> Vec<common::Vec2> {
        let mut world = World::new();
        world.insert_resource(common::SimRng::from_seed(42));
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
        world.insert_resource(diffusion::CpuHazardFieldState::default());
        world.insert_resource(bevy_ecs::event::Events::<catastrophe::HazardSpawned>::default());
        world.insert_resource(catastrophe::CatastropheConfig {
            spawn_probability: 1.0, // always spawn, isolating the position draw
            ..Default::default()
        });
        world.insert_resource(catastrophe::CatastropheManager::default());

        world.run_system_once(catastrophe_system);

        world
            .resource::<catastrophe::CatastropheManager>()
            .hazards
            .iter()
            .map(|h| h.center)
            .collect()
    }

    assert_eq!(run_once(), run_once());
}

/// Same fixed seed must produce the same food-spawn decision (position,
/// or consistent absence of one) across two independent `World`s —
/// proving `food_spawner_system`'s `fastrand`→`SimRng` migration
/// preserved determinism.
#[test]
fn food_spawner_system_is_deterministic_for_a_given_seed() {
    fn run_once() -> Vec<common::Vec3> {
        let mut world = World::new();
        world.insert_resource(common::SimRng::from_seed(7));
        world.insert_resource(metabolism::GlobalAtmosphere::default());
        world.insert_resource(environment::EnvironmentManager::new(1, false, 500.0, 500.0));
        world.insert_resource(EcologyConfig::default());

        world.run_system_once(food_spawner_system);

        let mut query = world.query::<&FoodPellet>();
        query.iter(&world).map(|p| p.position).collect()
    }

    assert_eq!(run_once(), run_once());
}

/// Phase 6, Epic J (Milestone J5): `Diet::Omnivore`'s color was changed
/// specifically to increase separation from `Diet::Carnivore` under a
/// Deuteranopia simulation (see `docs/design/accessibility.md`). This
/// doesn't re-run the full colorblindness simulation (that measurement
/// tool was a throwaway example, deleted after use, per this project's
/// convention) — it's a cheap, permanent guard against silently
/// reverting to the old amber value or picking a new one that's
/// trivially identical to Carnivore in plain sRGB terms, which would
/// undo this milestone's fix without any test catching it.
#[test]
fn omnivore_color_is_not_the_old_amber_and_stays_visibly_distinct_from_carnivore() {
    let omnivore = Diet::Omnivore.standard_color();
    let carnivore = Diet::Carnivore.standard_color();

    let old_amber = [1.0, 0.482, 0.0];
    assert_ne!(
        omnivore, old_amber,
        "Omnivore must not silently revert to the pre-Phase-6 amber that collided with Carnivore under deuteranopia"
    );

    let distance = ((omnivore[0] - carnivore[0]).powi(2)
        + (omnivore[1] - carnivore[1]).powi(2)
        + (omnivore[2] - carnivore[2]).powi(2))
    .sqrt();
    assert!(
        distance > 0.3,
        "Omnivore and Carnivore should read as clearly distinct colors in linear RGB; got distance {distance}"
    );
}
