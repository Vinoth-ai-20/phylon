//! # Phylon Sensing
//!
//! All sensor modalities: vision, olfaction, hearing, tactile contact,
//! thermoreception, proprioception, electroreception, and magnetoreception.
//!
//! Sensors read from local field values and nearby entity positions. They
//! produce a flat float vector fed into the neural brain as input.
//!
//! ## Current scope
//!
//! Of the 12 declared [`SensorModality`] variants, `sensing_system` actually
//! populates 3: Vision (a binned-cone heuristic, not true raycasting — see
//! [`HeadVision`]'s doc comment), Olfaction, and Proprioception (plus two
//! non-spec extras, Signal and Hazard). The other 7 (Hearing, Touch,
//! Thermoreception, Baroreception, Electroreception, Magnetoreception,
//! Nociception) exist only as unused enum variants today.

#![allow(missing_docs)]
#![warn(clippy::all)]

use bevy_ecs::entity::Entity;
use common::{Vec2, Vec3};
use std::collections::HashMap;

/// The sensory modalities available to organisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorModality {
    Vision,
    Olfaction,
    Hearing,
    Touch,
    Thermoreception,
    Proprioception,
    Baroreception,
    Electroreception,
    Magnetoreception,
    Nociception,
    Signal,
    Hazard,
}

/// # Ecological Sensory State
///
/// ## 1. What Happens
/// The `SensoryState` component stores the flattened float vector ($\mathbb{R}^N$) representing
/// the organism's current environmental perception.
///
/// ## 2. Why It Happens
/// Neural networks (like CTRNNs or neat-based brains) require normalized numeric arrays as
/// input. We decouple the biological sensors (eyes, noses) from the neural network topology.
/// The biology writes to this array, and the brain reads from it.
///
/// ## 3. How It Happens
/// During the `Sensing` phase, `sensing_system` iterates over active sensor modalities, computes
/// their values (e.g., sampling diffusion fields or raycasting for vision), and overwrites the
/// indices in `inputs`.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct SensoryState {
    /// Array of float inputs corresponding to the active sensor modalities.
    pub inputs: Vec<f32>,
}

impl SensoryState {
    /// Creates a new, empty sensory state.
    pub fn new(input_count: usize) -> Self {
        Self {
            inputs: vec![0.0; input_count],
        }
    }
}

/// # Organism Visual Cortex
///
/// ## 1. What Happens
/// The `HeadVision` component defines the geometric capabilities of an organism's eyes.
///
/// ## 2. Why It Happens
/// Blind organisms stumble aimlessly. Vision allows targeted predation, foraging, and mating.
/// However, true raycasting is too expensive for thousands of agents. We use a lightweight
/// binned-cone heuristic instead.
///
/// ## 3. How It Happens
/// The system checks the angle to nearby entities. If the entity falls within the `fov` cone,
/// its inverse-distance is accumulated into a 3×3 azimuth×elevation grid of bins (Phase 8,
/// Epic 8.7, ADR-P8-07 — extended from the pre-8.7 3-bin Left/Center/Right azimuth-only
/// model). This gives the neural network enough gradient information to turn toward or away
/// from a target, and (once organisms have real vertical body-plan variation — not yet, see
/// `dorsal`'s own doc comment) to pitch up/down toward it too.
#[derive(bevy_ecs::prelude::Component, Debug, Clone)]
pub struct HeadVision {
    /// Maximum distance the organism can see.
    pub range: f32,
    /// Field of view angle in radians.
    pub fov: f32,
    /// Last known forward (direction-of-travel) unit vector, used when
    /// velocity is near zero. `Vec3` since Phase 8, Epic 8.7 (ADR-P8-07) —
    /// still confined to the `Z = 0` plane in practice today, since it's
    /// derived from `physics::ParticleNode::velocity`, which is itself
    /// always `Z = 0` (no code path gives an organism vertical velocity).
    pub last_forward: Vec3,
    /// Body-fixed dorsal ("up") reference (Phase 8, Epic 8.7, ADR-P8-07) —
    /// together with `last_forward`, forms the body frame the azimuth/
    /// elevation calculation is computed in (`crate::vision_azimuth_elevation`),
    /// mirroring `organisms::GrowthState::dorsal`'s same role and same
    /// `Vec3::Z` default. Since every sensed position is also `Z = 0` today
    /// (no world-space vertical variation exists anywhere yet), elevation
    /// is provably always the "mid" bin in practice — a real, disclosed
    /// limitation of today's flat world, not a bug in the math itself (see
    /// `vision_azimuth_elevation`'s own tests for the genuine 3D case).
    pub dorsal: Vec3,
    /// Distance within which nodes are ignored (self-occlusion heuristic).
    pub self_occlusion_radius: f32,
    /// The food/prey entity currently being steered towards, if any.
    ///
    /// Kept from tick to tick so the organism commits to one target instead
    /// of flickering between whichever candidate happens to dominate a
    /// vision bin that tick — the lock is only dropped (and a new target
    /// picked) once this entity is no longer a valid candidate (eaten,
    /// despawned, or out of range/FOV).
    pub locked_target: Option<bevy_ecs::entity::Entity>,
}

/// Azimuth (angle in the forward-right plane) and elevation (angle in the
/// forward-up/dorsal plane) of a unit direction `dir`, relative to the body
/// frame defined by `forward`/`dorsal` (Phase 8, Epic 8.7, ADR-P8-07) —
/// replaces the pre-8.7 `Vec2::angle_to`, a signed 2D angle with no direct
/// 3D analogue (see the ADR's own Context section).
///
/// `azimuth` reproduces the pre-8.7 formula exactly whenever `dorsal ==
/// Vec3::Z` and `forward`/`dir` are confined to the XY plane (true for
/// every real call site today): `atan2((forward × dir) · dorsal, forward ·
/// dir)` reduces to the 2D cross-product/dot-product form `Vec2::angle_to`
/// used. `elevation` is `asin(dir · dorsal)` — the signed angle `dir` makes
/// above (`> 0`) or below (`< 0`) the horizontal (forward-right) plane;
/// always `0.0` today since every `dir` this crate constructs has `z ==
/// 0.0` and `dorsal == Vec3::Z`.
///
/// All three of `forward`, `dorsal`, and `dir` must be unit vectors.
pub fn vision_azimuth_elevation(forward: Vec3, dorsal: Vec3, dir: Vec3) -> (f32, f32) {
    let azimuth = forward.cross(dir).dot(dorsal).atan2(forward.dot(dir));
    let elevation = dir.dot(dorsal).clamp(-1.0, 1.0).asin();
    (azimuth, elevation)
}

/// Grid cell size used for the broad-phase spatial indices built each tick.
///
/// Correctness doesn't depend on this value (queries always filter by exact
/// distance afterwards) — it only affects how many candidates fall in each
/// bucket, so it's picked close to typical `HeadVision::range` values.
const SPATIAL_CELL_SIZE: f32 = 100.0;

/// Everything one organism's sensing computation needs, captured by value
/// so it can be computed on any thread with no live ECS access — see
/// `sensing_system`'s doc comment for why the system is split this way.
struct EntitySnapshot {
    entity: Entity,
    position: Vec2,
    velocity: Vec2,
    input_len: usize,
    vision: Option<VisionSnapshot>,
    energy: Option<(f32, f32)>, // (atp, max_atp)
    age: Option<(u64, u64)>,    // (ticks, max_lifespan)
    diet: Option<ecology::Diet>,
}

#[derive(Clone, Copy)]
struct VisionSnapshot {
    range: f32,
    fov: f32,
    last_forward: Vec3,
    dorsal: Vec3,
    self_occlusion_radius: f32,
    locked_target: Option<Entity>,
}

/// Result of one organism's sensing computation — pure data, applied back
/// to the ECS by `sensing_system` in a second, sequential pass.
struct SensingResult {
    entity: Entity,
    inputs: Vec<f32>,
    /// `Some` only when the entity has a [`HeadVision`] component.
    vision_update: Option<(Vec3, Option<Entity>)>, // (new last_forward, new locked_target)
}

/// Read-only snapshots of everything `compute_sensing` needs to look up
/// about *other* entities — plain data structures only (no live `Query`),
/// so they're trivially `Sync` and safe to share across `rayon` worker
/// threads.
struct WorldSnapshot<'a> {
    diet_map: &'a HashMap<u32, ecology::Diet>,
    organism_grid: &'a spatial::UniformGrid,
    node_positions: &'a HashMap<Entity, (Vec2, u32)>,
    resource_grids: &'a ecology::ResourceSpatialGrids,
    food_positions: &'a HashMap<Entity, Vec2>,
    mineral_positions: &'a HashMap<Entity, Vec2>,
    corpse_positions: &'a HashMap<Entity, Vec2>,
    cpu_field: Option<&'a diffusion::CpuFieldState>,
    cpu_signal_field: Option<&'a diffusion::CpuSignalFieldState>,
    cpu_hazard_field: Option<&'a diffusion::CpuHazardFieldState>,
    tick: u64,
}

/// Pure per-entity sensing computation — reads only `snap` and the
/// read-only `world` snapshot, touches no shared mutable state. Safe to
/// call from any thread; `sensing_system` runs this via `rayon`'s
/// `par_iter`. Ports the exact same field-order/logic as the
/// pre-parallelization implementation.
fn compute_sensing(snap: &EntitySnapshot, world: &WorldSnapshot) -> SensingResult {
    let mut inputs = vec![0.0f32; snap.input_len];
    if snap.input_len == 0 {
        return SensingResult {
            entity: snap.entity,
            inputs,
            vision_update: None,
        };
    }

    let mut idx = 0;

    // 1. Chemical sensor (Olfaction) - reads diffusion field
    if let Some(field) = world.cpu_field {
        let val = field.sample(snap.position, 0);
        if idx < inputs.len() {
            inputs[idx] = val;
            idx += 1;
        }
    }

    // 1.5. Signal sensor - reads emergent signal field
    if let Some(field) = world.cpu_signal_field {
        let val = field.sample(snap.position);
        if idx < inputs.len() {
            inputs[idx] = val;
            idx += 1;
        }
    }

    // 1.6. Hazard sensor - reads "impending doom" field
    if let Some(field) = world.cpu_hazard_field {
        let val = field.sample(snap.position);
        if idx < inputs.len() {
            inputs[idx] = val;
            idx += 1;
        }
    }

    // 2. Proprioception (ATP level)
    if let Some((atp, max_atp)) = snap.energy {
        if idx < inputs.len() {
            inputs[idx] = atp / max_atp.max(1.0);
            idx += 1;
        }
    }

    // 3. Proprioception (Age)
    if let Some((ticks, max_lifespan)) = snap.age {
        if idx < inputs.len() {
            inputs[idx] = ticks as f32 / max_lifespan.max(1) as f32;
            idx += 1;
        }
    }

    let mut vision_update = None;

    // 4-12. Vision (Phase 8, Epic 8.7, ADR-P8-07: a 3×3 azimuth×elevation
    // grid of bins — Left/Center/Right crossed with Up/Mid/Down — extended
    // from the pre-8.7 3-bin azimuth-only model).
    if let Some(vision) = snap.vision {
        // Update forward direction based on velocity — still Z=0-confined
        // in practice (`snap.velocity` is a `Vec2`, extended to `Vec3` with
        // `z = 0.0`), since nothing gives an organism vertical velocity.
        let mut last_forward = vision.last_forward;
        if snap.velocity.length_squared() > 0.01 {
            last_forward = snap.velocity.extend(0.0).normalize();
        } else if last_forward.length_squared() < 0.01 {
            last_forward = Vec3::X; // Fallback
        }
        let forward = last_forward;
        let dorsal = vision.dorsal;
        let half_fov = vision.fov / 2.0;
        let third_fov = half_fov / 1.5; // Divide FOV into 3 bins per axis

        // Classifies a (azimuth, elevation) pair into one of the 9 grid
        // bins, row-major by elevation (Down=0, Mid=1, Up=2) then azimuth
        // (Left=0, Center=1, Right=2) — matches `BIN_LABELS` in this
        // module's tests.
        let bin_index = |azimuth: f32, elevation: f32| -> usize {
            let az_bin = if azimuth < -third_fov {
                0
            } else if azimuth > third_fov {
                2
            } else {
                1
            };
            let el_bin = if elevation < -third_fov {
                0
            } else if elevation > third_fov {
                2
            } else {
                1
            };
            el_bin * 3 + az_bin
        };

        let mut obs_bins = [0.0f32; 9];

        // Returns `Some((azimuth, elevation, strength))` if `target_pos` is
        // visible (outside self-occlusion radius, within range and FOV).
        let vision_check = |target_pos: Vec2| -> Option<(f32, f32, f32)> {
            let diff = (target_pos - snap.position).extend(0.0);
            let dist = diff.length();
            if dist < vision.self_occlusion_radius || dist > vision.range {
                return None;
            }
            let dir = diff / dist;
            let (azimuth, elevation) = vision_azimuth_elevation(forward, dorsal, dir);
            if azimuth < -half_fov || azimuth > half_fov {
                return None;
            }
            let strength = 1.0 - (dist / vision.range);
            Some((azimuth, elevation, strength))
        };

        // Candidate food/prey targets seen this tick — the actual bin
        // values are populated from a single *chosen* candidate below, not
        // accumulated across all of them, so the organism commits to one
        // target (see `HeadVision::locked_target`) instead of flip-flopping
        // between whichever candidate is momentarily strongest.
        let mut food_candidates: Vec<(Entity, f32, f32, f32)> = Vec::new();

        // 1. See other organisms (mating, collision avoidance, predation)
        for other_entity in world
            .organism_grid
            .query_radius(snap.position.extend(0.0), vision.range)
        {
            let Some(&(other_pos, other_organism_id)) = world.node_positions.get(&other_entity)
            else {
                continue;
            };
            let mut is_food = false;
            if let (Some(my_diet), Some(other_diet)) =
                (&snap.diet, world.diet_map.get(&other_organism_id))
            {
                is_food = matches!(
                    (my_diet, other_diet),
                    (
                        ecology::Diet::Carnivore,
                        ecology::Diet::Herbivore | ecology::Diet::Omnivore
                    ) | (
                        ecology::Diet::Herbivore | ecology::Diet::Omnivore,
                        ecology::Diet::Producer
                    )
                );
            }
            let Some((azimuth, elevation, strength)) = vision_check(other_pos) else {
                continue;
            };
            if is_food {
                food_candidates.push((other_entity, azimuth, elevation, strength));
            } else {
                let bin = bin_index(azimuth, elevation);
                obs_bins[bin] = obs_bins[bin].max(strength);
            }
        }

        // 2. Diet-specific target vision (all treated as food candidates)
        if let Some(diet) = &snap.diet {
            match diet {
                ecology::Diet::Producer => {
                    for entity in world
                        .resource_grids
                        .minerals
                        .query_radius(snap.position.extend(0.0), vision.range)
                    {
                        if let Some(&pos) = world.mineral_positions.get(&entity) {
                            if let Some((azimuth, elevation, strength)) = vision_check(pos) {
                                food_candidates.push((entity, azimuth, elevation, strength));
                            }
                        }
                    }
                }
                ecology::Diet::Herbivore | ecology::Diet::Omnivore => {
                    for entity in world
                        .resource_grids
                        .food
                        .query_radius(snap.position.extend(0.0), vision.range)
                    {
                        if let Some(&pos) = world.food_positions.get(&entity) {
                            if let Some((azimuth, elevation, strength)) = vision_check(pos) {
                                food_candidates.push((entity, azimuth, elevation, strength));
                            }
                        }
                    }
                }
                ecology::Diet::Decomposer => {
                    for entity in world
                        .resource_grids
                        .corpses
                        .query_radius(snap.position.extend(0.0), vision.range)
                    {
                        if let Some(&pos) = world.corpse_positions.get(&entity) {
                            if let Some((azimuth, elevation, strength)) = vision_check(pos) {
                                food_candidates.push((entity, azimuth, elevation, strength));
                            }
                        }
                    }
                }
                ecology::Diet::Carnivore => {
                    // Carnivores look at other organisms which is already done above.
                }
            }
        }

        // Keep steering at the locked target as long as it's still a valid
        // candidate this tick; only pick a new one (closest/strongest) once
        // the lock is lost.
        let locked_candidate = vision
            .locked_target
            .and_then(|locked| food_candidates.iter().find(|(e, _, _, _)| *e == locked))
            .copied();

        let chosen = locked_candidate.or_else(|| {
            food_candidates
                .iter()
                .copied()
                .max_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
        });

        let new_locked_target = chosen.map(|(entity, _, _, _)| entity);

        let mut food_bins = [0.0f32; 9];
        if let Some((_, azimuth, elevation, strength)) = chosen {
            food_bins[bin_index(azimuth, elevation)] = strength;
        }

        for bin in 0..9 {
            if idx < inputs.len() {
                inputs[idx] = food_bins[bin] - obs_bins[bin];
                idx += 1;
            }
        }

        // Internal Pacemaker (CPG)
        if idx < inputs.len() {
            // At 60 ticks/sec, * 0.2 gives ~2 Hz frequency.
            let pacemaker_signal = (world.tick as f32 * 0.2).sin();
            inputs[idx] = pacemaker_signal;
        }

        vision_update = Some((forward, new_locked_target));
    }

    SensingResult {
        entity: snap.entity,
        inputs,
        vision_update,
    }
}

/// # Sensory Acquisition System
///
/// ## 1. What Happens
/// The `sensing_system` collects environmental data (Chemical fields, visual targets) and
/// internal data (ATP, Age) and populates the `SensoryState` array for every organism.
///
/// ## 2. Why It Happens
/// The brain needs a snapshot of the world to make decisions. By combining field sampling
/// (for gradients/pheromones), distance-based vision (for hunting/fleeing), and an internal
/// Pacemaker (for gait generation), we provide the necessary basis for complex behavior.
///
/// ## 3. How It Happens
/// For every organism with a `SensoryState`:
/// 1. Sample CPU diffusion fields (Olfaction, Signals, Hazards) at the node's position.
/// 2. Read internal components (`ChemicalEconomy::atp`, `Age::ticks`) for proprioception.
/// 3. Iterate over the spatial index (or all nodes/food/corpses) and bin them into the `HeadVision` FOV.
/// 4. Generate a sine wave `Pacemaker` signal ($\approx 2Hz$) to drive rhythmic locomotion.
///
/// ## Parallel/sequential split (determinism)
///
/// Unlike `metabolism::metabolism_system` (Epic 6, M6.1), this system has
/// **no shared mutable state accumulated across entities at all** — every
/// organism reads only read-only snapshots (other organisms' positions, the
/// diet map, field samples, the pacemaker tick) and writes only to its own
/// `SensoryState`/`HeadVision` components. So there's no reduction-ordering
/// hazard to guard against here, just the mechanical requirement that nothing
/// touched inside the parallel phase can be a live `bevy_ecs` `Query`/`Res`
/// (those aren't safely shareable across a `rayon` closure the way plain
/// `HashMap`/`Vec` data is) — hence the snapshot-into-plain-data step before
/// the parallel phase, and the writeback-via-`get_mut` step after it.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub fn sensing_system(
    mut query: bevy_ecs::prelude::Query<(
        Entity,
        &mut SensoryState,
        &physics::ParticleNode,
        Option<&mut HeadVision>,
        Option<&metabolism::ChemicalEconomy>,
        Option<&metabolism::Age>,
        Option<&ecology::Diet>,
    )>,
    node_query: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &physics::ParticleNode)>,
    diet_query: bevy_ecs::prelude::Query<(&physics::ParticleNode, &ecology::Diet)>,
    food_query: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &ecology::FoodPellet)>,
    mineral_query: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &ecology::MineralPellet)>,
    corpse_query: bevy_ecs::prelude::Query<(bevy_ecs::entity::Entity, &ecology::Corpse)>,
    resource_grids: bevy_ecs::prelude::Res<ecology::ResourceSpatialGrids>,
    cpu_field: Option<bevy_ecs::prelude::Res<diffusion::CpuFieldState>>,
    cpu_signal_field: Option<bevy_ecs::prelude::Res<diffusion::CpuSignalFieldState>>,
    cpu_hazard_field: Option<bevy_ecs::prelude::Res<diffusion::CpuHazardFieldState>>,
    mut local_tick: bevy_ecs::prelude::Local<u64>,
) {
    use rayon::prelude::*;

    let mut diet_map = HashMap::new();
    for (node, diet) in diet_query.iter() {
        diet_map.insert(node.organism_id, diet.clone());
    }

    // Broad-phase spatial index over organisms, rebuilt fresh each tick from
    // current positions — replaces the O(N * M) "scan every node for every
    // organism" pattern with a bucketed radius query. Food/mineral/corpse
    // grids are shared via `ecology::ResourceSpatialGrids` (built once per
    // tick by `build_resource_grids_system`) since `foraging_system` needs
    // the exact same indices — no reason to rebuild them twice.
    // Vision remains 2D math throughout this module (angle-based FOV
    // checks) even though the underlying grids/components are `Vec3` since
    // Phase 8 (ADR-P8-01) — every position is truncated to its XY plane at
    // this snapshot boundary, mirroring the same pattern used at the
    // metabolism/catastrophe diffusion-field boundaries.
    let mut organism_grid = spatial::UniformGrid::new(SPATIAL_CELL_SIZE).unwrap();
    let mut node_positions = HashMap::new();
    for (entity, node) in node_query.iter() {
        let _ = organism_grid.insert(entity, node.position);
        node_positions.insert(entity, (node.position.truncate(), node.organism_id));
    }

    // Snapshot resource entity positions into plain maps — see this
    // function's doc comment on why the parallel phase can't hold a live
    // `Query`.
    let food_positions: HashMap<Entity, Vec2> = food_query
        .iter()
        .map(|(e, f)| (e, f.position.truncate()))
        .collect();
    let mineral_positions: HashMap<Entity, Vec2> = mineral_query
        .iter()
        .map(|(e, m)| (e, m.position.truncate()))
        .collect();
    let corpse_positions: HashMap<Entity, Vec2> = corpse_query
        .iter()
        .map(|(e, c)| (e, c.position.truncate()))
        .collect();

    let world_snapshot = WorldSnapshot {
        diet_map: &diet_map,
        organism_grid: &organism_grid,
        node_positions: &node_positions,
        resource_grids: &resource_grids,
        food_positions: &food_positions,
        mineral_positions: &mineral_positions,
        corpse_positions: &corpse_positions,
        cpu_field: cpu_field.as_deref(),
        cpu_signal_field: cpu_signal_field.as_deref(),
        cpu_hazard_field: cpu_hazard_field.as_deref(),
        tick: *local_tick,
    };

    // Snapshot phase (sequential): each organism's own state, by value.
    let snapshots: Vec<EntitySnapshot> = query
        .iter()
        .map(
            |(entity, state, node, vision_opt, energy_opt, age_opt, diet_opt)| EntitySnapshot {
                entity,
                position: node.position.truncate(),
                velocity: node.velocity.truncate(),
                input_len: state.inputs.len(),
                vision: vision_opt.map(|v| VisionSnapshot {
                    range: v.range,
                    fov: v.fov,
                    last_forward: v.last_forward,
                    dorsal: v.dorsal,
                    self_occlusion_radius: v.self_occlusion_radius,
                    locked_target: v.locked_target,
                }),
                energy: energy_opt.map(|c| (c.atp, c.max_atp)),
                age: age_opt.map(|a| (a.ticks, a.max_lifespan)),
                diet: diet_opt.cloned(),
            },
        )
        .collect();

    // Parallel phase: pure per-entity computation, no shared state touched.
    let results: Vec<SensingResult> = snapshots
        .par_iter()
        .map(|snap| compute_sensing(snap, &world_snapshot))
        .collect();

    // Sequential writeback — order doesn't matter here (see doc comment),
    // but a single deterministic pass keeps the pattern consistent with
    // `metabolism_system`.
    for result in results {
        if let Ok((_, mut state, _, mut vision_opt, _, _, _)) = query.get_mut(result.entity) {
            if state.inputs.is_empty() {
                continue;
            }
            state.inputs = result.inputs;
            if let (Some(vision), Some((new_forward, new_locked))) =
                (&mut vision_opt, result.vision_update)
            {
                vision.last_forward = new_forward;
                vision.locked_target = new_locked;
            }
        }
    }

    // Advance the pacemaker tick globally once per frame
    *local_tick += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-P8-07's own named regression check: for the default `Vec3::Z`
    /// dorsal and any `forward`/`dir` confined to the XY plane (every real
    /// call site today), the new azimuth formula must reproduce the pre-8.7
    /// `Vec2::angle_to` result exactly, and elevation must be exactly zero.
    #[test]
    fn vision_azimuth_elevation_with_z_dorsal_matches_the_pre_8_7_2d_angle() {
        let forward = Vec3::new(1.0, 0.0, 0.0);
        for angle_deg in [-150, -90, -45, 0, 30, 89, 90, 150] {
            let angle = (angle_deg as f32).to_radians();
            let dir = Vec3::new(angle.cos(), angle.sin(), 0.0);
            let (azimuth, elevation) = vision_azimuth_elevation(forward, Vec3::Z, dir);

            let old_angle = Vec2::X.angle_to(Vec2::new(dir.x, dir.y));
            assert!(
                (azimuth - old_angle).abs() < 1e-4,
                "angle {angle_deg}: expected azimuth {old_angle}, got {azimuth}"
            );
            assert!(
                elevation.abs() < 1e-6,
                "angle {angle_deg}: expected zero elevation, got {elevation}"
            );
        }
    }

    /// Genuine 3D correctness: a target directly "above" the organism along
    /// its `dorsal` axis must read as maximal elevation (π/2) regardless of
    /// azimuth, and a target exactly on the horizon must read as zero
    /// elevation — proving the formula is a real, well-defined 3D
    /// generalization, not merely re-deriving the 2D case.
    #[test]
    fn vision_azimuth_elevation_reads_maximal_elevation_straight_up() {
        let forward = Vec3::new(1.0, 0.0, 0.0);
        let dorsal = Vec3::Z;
        let (_, elevation) = vision_azimuth_elevation(forward, dorsal, dorsal);
        assert!((elevation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    /// A tilted `dorsal` (not the default `Vec3::Z`) still produces a
    /// coherent azimuth/elevation split — the two angles remain independent
    /// (a purely horizontal offset in the tilted frame reads as zero
    /// elevation), confirming the math doesn't silently assume a
    /// world-space-vertical dorsal.
    #[test]
    fn vision_azimuth_elevation_works_with_a_tilted_dorsal() {
        let forward = Vec3::new(1.0, 0.0, 0.0);
        let dorsal = Vec3::new(0.0, 1.0, 1.0).normalize(); // tilted 45° off Z
        let right = forward.cross(dorsal).normalize();
        // A direction purely along `right` (in the tilted horizontal plane)
        // should read as ~90° azimuth, ~0 elevation.
        let (azimuth, elevation) = vision_azimuth_elevation(forward, dorsal, right);
        assert!((azimuth.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
        assert!(elevation.abs() < 1e-3);
    }

    #[test]
    fn sensor_modality_is_copy() {
        let s = SensorModality::Vision;
        let _s2 = s;
    }

    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;

    /// (inputs, last_forward, locked_target) for one organism after a
    /// `sensing_system` tick.
    type OrganismOutcome = (Vec<f32>, Vec3, Option<Entity>);

    fn build_world_with_organisms(n: u32) -> World {
        let mut world = World::new();
        world.insert_resource(ecology::ResourceSpatialGrids::new(50.0));

        for i in 0..n {
            // Alternate carnivore/herbivore so the predation vision path
            // (the most complex branch) actually engages, not just the
            // field-sampling/proprioception branches.
            let diet = if i % 2 == 0 {
                ecology::Diet::Carnivore
            } else {
                ecology::Diet::Herbivore
            };
            world.spawn((
                physics::ParticleNode::new(
                    common::Vec3::new((i as f32) * 15.0, 0.0, 0.0),
                    1.0,
                    0,
                    i,
                ),
                SensoryState::new(15),
                HeadVision {
                    range: 250.0,
                    fov: std::f32::consts::PI * 0.8,
                    last_forward: common::Vec3::X,
                    dorsal: common::Vec3::Z,
                    self_occlusion_radius: 5.0,
                    locked_target: None,
                },
                metabolism::ChemicalEconomy {
                    glucose: 500.0,
                    o2: 300.0,
                    co2: 50.0,
                    atp: 400.0,
                    max_glucose: 1000.0,
                    max_o2: 1000.0,
                    max_co2: 1000.0,
                    max_atp: 1000.0,
                },
                metabolism::Age {
                    ticks: i as u64,
                    max_lifespan: 10_000,
                },
                diet,
            ));
        }
        world
    }

    fn run_sensing_with_thread_count(
        n_threads: usize,
        organism_count: u32,
    ) -> Vec<OrganismOutcome> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(n_threads)
            .build()
            .unwrap();
        let mut world = build_world_with_organisms(organism_count);

        pool.install(|| {
            world.run_system_once(sensing_system);
        });

        let mut query = world.query::<(&physics::ParticleNode, &SensoryState, &HeadVision)>();
        let mut results: Vec<_> = query
            .iter(&world)
            .map(|(node, state, vision)| {
                (
                    node.organism_id,
                    (
                        state.inputs.clone(),
                        vision.last_forward,
                        vision.locked_target,
                    ),
                )
            })
            .collect();
        results.sort_by_key(|(id, _)| *id);
        results.into_iter().map(|(_, r)| r).collect()
    }

    #[test]
    fn sensing_is_deterministic_regardless_of_thread_count() {
        // 150 organisms, alternating carnivore/herbivore in a line within
        // vision range of each other, so predation targeting (the branch
        // most likely to be order-sensitive) is actually exercised.
        let results_1 = run_sensing_with_thread_count(1, 150);
        let results_8 = run_sensing_with_thread_count(8, 150);
        assert_eq!(
            results_1, results_8,
            "sensory inputs/vision state diverged between 1 and 8 threads"
        );
    }
}
