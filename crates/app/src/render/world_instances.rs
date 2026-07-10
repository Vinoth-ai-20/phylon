//! Per-frame world-instance gathering (Phase 7, W2d) — answers "what's in
//! the world this frame," the same way `organism_visuals` (its sibling
//! module) answers "what does one entity look like." Extracted verbatim
//! from `render.rs`'s prior inline body: every query, closure, and
//! `organism_visuals` builder call moves unchanged — this milestone is
//! architectural separation, not a behavior change.
//!
//! [`PhylonApp::gather_world_render_instances`] reads only `&self.world`/
//! `&self.ui` (confirmed no `self` mutation anywhere in this body before
//! this extraction) and returns the four instance lists the GPU passes in
//! `render()` consume. It touches no `wgpu` state — the actual drawing
//! stays in `render()`, unchanged in order and content.

use super::organism_visuals::{self, BoneKind};
use crate::app::PhylonApp;

/// The four render-instance lists one frame's world state produces —
/// see `render()`'s own doc comment for how each is consumed by a GPU pass.
pub(crate) struct WorldRenderInstances {
    pub(crate) debug_instances: Vec<rendering::DebugInstance>,
    pub(crate) capsule_instances: Vec<rendering::CapsuleInstance>,
    pub(crate) hover_bones: Vec<rendering::CapsuleInstance>,
    pub(crate) selected_bones: Vec<rendering::CapsuleInstance>,
}

impl PhylonApp {
    /// Gathers this frame's render instances from `World` state: selection/
    /// hover highlighting, frustum culling, and every organism/food/mineral/
    /// corpse visual `organism_visuals`'s builders decide — see this
    /// module's doc comment for the extraction discipline.
    ///
    /// Takes `&mut self` only because `bevy_ecs::World::query` requires
    /// `&mut World` to construct a `QueryState` (internal caching), even
    /// though every use here is read-only — no `self` field is ever
    /// assigned in this function's body.
    pub(crate) fn gather_world_render_instances(&mut self) -> WorldRenderInstances {
        let mut debug_instances = Vec::new();
        let mut capsule_instances = Vec::new();
        let mut hover_bones = Vec::new();
        let mut selected_bones = Vec::new();

        let mut get_connected_component = |entity: bevy_ecs::entity::Entity| {
            let mut adj: std::collections::HashMap<
                bevy_ecs::entity::Entity,
                Vec<bevy_ecs::entity::Entity>,
            > = std::collections::HashMap::new();
            let mut query_springs = self.world.ecs.query::<&physics::Spring>();
            for spring in query_springs.iter(&self.world.ecs) {
                adj.entry(spring.node_a).or_default().push(spring.node_b);
                adj.entry(spring.node_b).or_default().push(spring.node_a);
            }

            let mut queue = std::collections::VecDeque::new();
            let mut visited = std::collections::HashSet::new();
            queue.push_back(entity);
            visited.insert(entity);

            while let Some(curr) = queue.pop_front() {
                if let Some(neighbors) = adj.get(&curr) {
                    for neighbor in neighbors {
                        if visited.insert(*neighbor) {
                            queue.push_back(*neighbor);
                        }
                    }
                }
            }
            visited
        };

        let selected_component = self.ui.selected_entity.map(&mut get_connected_component);
        // `panel_hover_entity` (Phase 2, M9) lets a non-viewport panel (e.g.
        // the Lineage Explorer) drive the same highlight a viewport-hover
        // would — `hovered_entity` itself stays viewport-picking-only since
        // it's unconditionally overwritten every frame in `events.rs`.
        let hovered_component = self
            .ui
            .hovered_entity
            .or(self.ui.panel_hover_entity)
            .map(&mut get_connected_component);

        // Camera-frustum culling for the (potentially whole-population-sized)
        // SDF bone list: skip gathering/uploading/instancing bones that fall
        // entirely outside the visible viewport. Uses the last known canvas
        // rect (one frame stale at worst — negligible) since the current
        // frame's egui layout hasn't run yet at this point in `render()`.
        let (cull_w, cull_h) = self
            .ui
            .canvas_rect
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or_else(|| {
                self.gpu
                    .as_ref()
                    .and_then(|g| g.config.as_ref())
                    .map(|c| (c.width as f32, c.height as f32))
                    .unwrap_or((1280.0, 720.0))
            });
        const CULL_MARGIN: f32 = 100.0; // generous slack for bone radius + node_radius
        let cull_camera_pos = self.ui.camera_pos_2d();
        let cull_camera_zoom = self.ui.camera_zoom_2d();
        let cull_half_w = cull_w / 2.0 / cull_camera_zoom + CULL_MARGIN;
        let cull_half_h = cull_h / 2.0 / cull_camera_zoom + CULL_MARGIN;
        let cull_min_x = cull_camera_pos.x - cull_half_w;
        let cull_max_x = cull_camera_pos.x + cull_half_w;
        let cull_min_y = cull_camera_pos.y - cull_half_h;
        let cull_max_y = cull_camera_pos.y + cull_half_h;
        let bone_visible = |pa: [f32; 2], pb: [f32; 2]| -> bool {
            let min_x = pa[0].min(pb[0]);
            let max_x = pa[0].max(pb[0]);
            let min_y = pa[1].min(pb[1]);
            let max_y = pa[1].max(pb[1]);
            max_x >= cull_min_x && min_x <= cull_max_x && max_y >= cull_min_y && min_y <= cull_max_y
        };

        // Build node position lookup for bone endpoint resolution
        let mut node_positions: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 2]> =
            std::collections::HashMap::new();
        // Phase 8 (ADR-P8-03): the mesh-based capsule renderer needs real
        // `Vec3` endpoints (organisms still grow with `z` fixed at `0.0`
        // until Epic 8.6, but the renderer itself is now genuinely 3D) —
        // kept as a second map rather than widening `node_positions` itself
        // since every other consumer of that map (culling, spotlight
        // nearby-lookup, colony-link debug instances) is still flat 2D
        // logic, unaffected by and out of scope for this epic.
        let mut node_positions_3d: std::collections::HashMap<bevy_ecs::entity::Entity, [f32; 3]> =
            std::collections::HashMap::new();

        // Per-node organism-id lookup (Phase 5, SX-3d) — `physics::ParticleNode.organism_id`
        // is already stored per node; no BFS/adjacency-map traversal is
        // needed to find colony (budding) links, only a same-tick
        // comparison of two nodes' `organism_id` on either end of a spring
        // (see the spring loop below). Built in the same query as
        // `node_positions` below, at no extra query cost.
        let mut entity_organism_id: std::collections::HashMap<bevy_ecs::entity::Entity, u32> =
            std::collections::HashMap::new();

        // Per-segment vitality lookup (Phase 5, SX-1c) — `metabolism::Health`
        // lives only on an organism's head entity (`organisms::spawning`), not
        // every segment, so this maps every segment entity in that organism's
        // `DevelopmentalGraph` to the same head-derived health fraction,
        // mirroring `render_physiology_overlay`'s existing graph-walk pattern
        // rather than inventing a new one. Absent from this map (sandbox
        // structures with no `Health`) defaults to `1.0` (fully vital) at the
        // lookup site below, not inserted here.
        let mut entity_health_fraction: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            std::collections::HashMap::new();
        let mut query_health_graphs = self
            .world
            .ecs
            .query::<(&organisms::DevelopmentalGraph, &metabolism::Health)>();
        for (graph, health) in query_health_graphs.iter(&self.world.ecs) {
            let fraction = if health.max > 0.0 {
                (health.current / health.max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            for graph_node in &graph.nodes {
                if let Some(seg_entity) = graph_node.entity {
                    entity_health_fraction.insert(seg_entity, fraction);
                }
            }
        }

        // Per-organism average infection severity lookup (Phase 5, SX-1d),
        // keyed by head entity (the only entity `ecology::disease::Infection`
        // lives on, mirroring `metabolism::Health`'s own placement) — walks
        // the same `DevelopmentalGraph` used above for Health, averaging
        // `SegmentInfection.severity` across segments rather than reading a
        // single value, since severity is a genuinely per-segment quantity
        // (P4-F5).
        let mut entity_avg_severity: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            std::collections::HashMap::new();
        let mut query_segment_infection = self
            .world
            .ecs
            .query::<&ecology::disease::SegmentInfection>();
        let mut query_infection_graphs = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &organisms::DevelopmentalGraph,
            &ecology::disease::Infection,
        )>();
        for (head_entity, graph, _infection) in query_infection_graphs.iter(&self.world.ecs) {
            let mut total = 0.0f32;
            let mut count = 0u32;
            for graph_node in &graph.nodes {
                if let Some(seg_entity) = graph_node.entity {
                    if let Ok(seg_infection) =
                        query_segment_infection.get(&self.world.ecs, seg_entity)
                    {
                        total += seg_infection.severity;
                        count += 1;
                    }
                }
            }
            if count > 0 {
                entity_avg_severity.insert(head_entity, total / count as f32);
            }
        }

        let mut query_nodes_render = self.world.ecs.query::<(
            bevy_ecs::entity::Entity,
            &physics::ParticleNode,
            Option<&ecology::EcologicalCategory>,
            Option<&metabolism::Health>,
            Option<&ecology::disease::Infection>,
        )>();
        for (entity, node, category, health, infection) in query_nodes_render.iter(&self.world.ecs)
        {
            node_positions.insert(entity, [node.position.x, node.position.y]);
            node_positions_3d.insert(entity, node.position.into());
            entity_organism_id.insert(entity, node.organism_id);

            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);

            // Low-health ring (Phase 5, SX-1c) — Primary tier, always visible,
            // not gated behind `debug_structural` (unlike the category ring
            // below, which stays debug-only). `Health` only exists on the
            // head entity (`organisms::spawning`), so this naturally draws
            // once per organism. Amber below 40%, red below 15%, per
            // `docs/design/biological_visual_language.md`'s Health entry;
            // nothing drawn above 40% — absence is the encoding for the
            // common healthy case, matching SX-1b's Idle precedent.
            if let Some(health) = health {
                if let Some(instance) = organism_visuals::health_ring_instance(
                    health,
                    node.position.into(),
                    self.ui.node_radius,
                ) {
                    debug_instances.push(instance);
                }
            }

            // Disease badge (Phase 5, SX-1d) — Primary tier, always visible,
            // offset up-and-left from the head position so it never blends
            // with the Health disk above into an ambiguous combined color
            // (both are opaque-ish filled disks on the shared
            // `debug_quad.wgsl` primitive — see this document's own note in
            // `biological_visual_language.md`'s Disease entry on why this
            // isn't a true concentric ring). No animation — a fully static
            // function of current `Infection.state`/severity, per this
            // milestone's explicit "no pulses" instruction.
            if let Some(infection) = infection {
                let avg_severity = entity_avg_severity.get(&entity).copied().unwrap_or(0.0);
                let health_fraction = health.map_or(1.0, |h| {
                    if h.max > 0.0 {
                        (h.current / h.max).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                });
                debug_instances.push(organism_visuals::disease_badge_instance(
                    infection,
                    avg_severity,
                    health_fraction,
                    node.position.into(),
                    self.ui.node_radius,
                ));
            }

            if should_draw_debug {
                debug_instances.push(organism_visuals::segment_debug_dot_instance(
                    node,
                    self.ui.node_radius,
                ));

                if let Some(cat) = category {
                    if let Some(instance) =
                        organism_visuals::category_ring_instance(cat, node, self.ui.node_radius)
                    {
                        debug_instances.push(instance);
                    }
                }
            }
        }

        // Developmental growth visual (Phase 5, SX-2d): a just-formed segment
        // scales in from near-0 to full radius over a short fixed window,
        // rather than popping into existence at full size. `SpawnTick`
        // (reused, not a new component — see `organisms::systems::growth_system`'s
        // doc comment) is looked up per spring's `node_b`, which is always
        // the newer of the two endpoints under this codebase's own `Spring`
        // convention (`node_a` is always the pre-existing parent/anchor;
        // confirmed by reading every `Spring` construction site in
        // `growth_system`/`producer_growth_system`, not assumed).
        const GROWTH_FADE_IN_TICKS: u64 = 30; // 0.5s at 60Hz — brief, per the design doc
        let current_tick = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
            .map_or(0, |a| a.ticks);
        let mut query_spawn_ticks = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &organisms::SpawnTick)>();
        let entity_growth_progress: std::collections::HashMap<bevy_ecs::entity::Entity, f32> =
            query_spawn_ticks
                .iter(&self.world.ecs)
                .map(|(e, spawn_tick)| {
                    let age_ticks = current_tick.saturating_sub(spawn_tick.0);
                    let progress = (age_ticks as f32 / GROWTH_FADE_IN_TICKS as f32).clamp(0.0, 1.0);
                    (e, progress)
                })
                .collect();

        // Spotlight mode (Phase 5, SX-5b) — dims every organism except the
        // selected entity, its connected body/colony (reusing the exact
        // `selected_component` BFS already computed above for selection
        // highlighting, not a second traversal), and any other organism
        // whose head falls within the selected organism's interaction
        // radius (`sensing::HeadVision.range`, falling back to `250.0` —
        // the same fallback SX-4c/5a's nearby-organism lookups already use,
        // for consistency). `None` (not gated at all) when the mode is off
        // or nothing is selected, so this costs nothing in the common case.
        let spotlight_dim_entities: Option<std::collections::HashSet<bevy_ecs::entity::Entity>> =
            if self.ui.spotlight_mode {
                self.ui.selected_entity.map(|selected| {
                    let selected_pos = node_positions.get(&selected).copied();
                    let radius = {
                        let mut vision_q = self.world.ecs.query::<&sensing::HeadVision>();
                        vision_q
                            .get(&self.world.ecs, selected)
                            .map(|v| v.range)
                            .unwrap_or(250.0)
                    };

                    let mut nearby_organism_ids: std::collections::HashSet<u32> =
                        std::collections::HashSet::new();
                    if let Some(&sel_pos) = selected_pos.as_ref() {
                        let mut diet_q = self
                            .world
                            .ecs
                            .query::<(&physics::ParticleNode, &ecology::Diet)>();
                        for (node, _diet) in diet_q.iter(&self.world.ecs) {
                            let d = ((node.position.x - sel_pos[0]).powi(2)
                                + (node.position.y - sel_pos[1]).powi(2))
                            .sqrt();
                            if d <= radius {
                                nearby_organism_ids.insert(node.organism_id);
                            }
                        }
                    }

                    // Every entity to *keep lit*: the BFS-connected body/
                    // colony, plus every segment belonging to a nearby
                    // organism_id.
                    let mut lit: std::collections::HashSet<bevy_ecs::entity::Entity> =
                        selected_component.clone().unwrap_or_default();
                    for (&e, &oid) in entity_organism_id.iter() {
                        if nearby_organism_ids.contains(&oid) {
                            lit.insert(e);
                        }
                    }

                    // Return the complement — everything to *dim* — since
                    // that's what the render loop below actually looks up
                    // per-bone (dimming is the exception, not the rule).
                    entity_organism_id
                        .keys()
                        .filter(|e| !lit.contains(e))
                        .copied()
                        .collect()
                })
            } else {
                None
            };
        const SPOTLIGHT_DIM_FACTOR: f32 = 0.15;
        let spotlight_factor = |e: bevy_ecs::entity::Entity| -> f32 {
            match &spotlight_dim_entities {
                Some(dim_set) if dim_set.contains(&e) => SPOTLIGHT_DIM_FACTOR,
                _ => 1.0,
            }
        };

        // Collect springs for SDF capsule rendering.
        let mut query_springs_render = self
            .world
            .ecs
            .query::<(&physics::Spring, Option<&organisms::OrganismColor>)>();
        for (spring, opt_color) in query_springs_render.iter(&self.world.ecs) {
            // Absent from the map (segments that existed before this
            // milestone shipped, or non-organism structures) defaults to
            // `1.0` — fully grown, not "always freshly spawned."
            let growth_scale = entity_growth_progress
                .get(&spring.node_b)
                .copied()
                .unwrap_or(1.0);

            // Colony/migration visualization (Phase 5, SX-3d) — a spring
            // whose two endpoints belong to different organisms *is* a
            // colony (budding) link, by the same definition
            // `analytics_bridge_system`'s colony-connectivity graph already
            // uses. Population-wide, always visible (not gated by
            // selection) — Priority 4 (Ecological status) per the Numeric
            // priority hierarchy, so drawn via `debug_instances` alongside
            // Health/Disease, beneath the Priority-1 selection/hover
            // highlight (unaffected — draw order for `debug_instances`
            // itself was already fixed at SX-1e).
            if let (Some(&org_a), Some(&org_b), Some(&pa3), Some(&pb3)) = (
                entity_organism_id.get(&spring.node_a),
                entity_organism_id.get(&spring.node_b),
                node_positions_3d.get(&spring.node_a),
                node_positions_3d.get(&spring.node_b),
            ) {
                if org_a != org_b {
                    debug_instances.push(organism_visuals::colony_link_instance(
                        pa3,
                        pb3,
                        self.ui.skin_thickness,
                    ));
                }
            }

            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&spring.node_a) && comp.contains(&spring.node_b));
            let is_in_hovered = hovered_component
                .as_ref()
                .is_some_and(|comp| comp.contains(&spring.node_a) && comp.contains(&spring.node_b));

            if let (Some(&pa3), Some(&pb3)) = (
                node_positions_3d.get(&spring.node_a),
                node_positions_3d.get(&spring.node_b),
            ) {
                let (hover, selected) = organism_visuals::bone_highlight_instances(
                    pa3,
                    pb3,
                    spring.is_fin == 1,
                    spring.constraint_type,
                    self.ui.skin_thickness,
                    is_in_hovered,
                    is_in_selected,
                );
                if let Some(instance) = hover {
                    hover_bones.push(instance);
                }
                if let Some(instance) = selected {
                    selected_bones.push(instance);
                }
            }

            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);

            // Skip springs that have no associated organism color (e.g. broken/detached).
            let bone_kind = if spring.constraint_type == physics::ConstraintType::Passive
                && spring.is_fin == 0
            {
                BoneKind::PassiveTail
            } else if spring.constraint_type == physics::ConstraintType::Elastic {
                BoneKind::ElasticMuscle
            } else if spring.constraint_type == physics::ConstraintType::Rigid
                || spring.constraint_type == physics::ConstraintType::Rotational
            {
                BoneKind::RigidOrRotational {
                    is_fin: spring.is_fin == 1,
                }
            } else {
                continue;
            };

            if let (Some(&pa), Some(&pb), Some(&pa3), Some(&pb3)) = (
                node_positions.get(&spring.node_a),
                node_positions.get(&spring.node_b),
                node_positions_3d.get(&spring.node_a),
                node_positions_3d.get(&spring.node_b),
            ) {
                let health_fraction = entity_health_fraction
                    .get(&spring.node_a)
                    .copied()
                    .unwrap_or(1.0);
                let (sdf, debug) = organism_visuals::bone_visual_instances(
                    bone_kind,
                    pa3,
                    pb3,
                    opt_color.map(|c| c.0),
                    health_fraction,
                    spotlight_factor(spring.node_a),
                    growth_scale,
                    self.ui.skin_thickness,
                    self.ui.bone_line_thickness,
                    should_draw_sdf,
                    should_draw_debug,
                    bone_visible(pa, pb),
                );
                if let Some(instance) = sdf {
                    capsule_instances.push(instance);
                }
                if let Some(instance) = debug {
                    debug_instances.push(instance);
                }
            }
        }

        // Render food pellets (always shown in debug view)
        let mut query_food = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::FoodPellet)>();
        for (entity, food) in query_food.iter(&self.world.ecs) {
            let pos = [food.position.x, food.position.y];
            let pos3: [f32; 3] = food.position.into();
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);
            let is_hovered = hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));

            let instances = organism_visuals::pellet_like_instances(
                pos3,
                [1.000, 0.665, 0.078, 1.0], // #FFD54F
                [1.000, 0.665, 0.078],
                organism_visuals::FOOD_PELLET_RADIUS,
                should_draw_debug,
                should_draw_sdf,
                bone_visible(pos, pos),
                is_hovered,
                is_in_selected,
            );
            if let Some(instance) = instances.debug {
                debug_instances.push(instance);
            }
            if let Some(instance) = instances.sdf {
                capsule_instances.push(instance);
            }
            if let Some(instance) = instances.hover {
                hover_bones.push(instance);
            }
            if let Some(instance) = instances.selected {
                selected_bones.push(instance);
            }
        }

        // Render mineral pellets
        let mut query_mineral = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::MineralPellet)>();
        for (entity, mineral) in query_mineral.iter(&self.world.ecs) {
            let pos = [mineral.position.x, mineral.position.y];
            let pos3: [f32; 3] = mineral.position.into();
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);
            let is_hovered = hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));

            let instances = organism_visuals::pellet_like_instances(
                pos3,
                [0.397, 0.539, 0.584, 1.0], // #A9C2C9
                [0.397, 0.539, 0.584],
                organism_visuals::MINERAL_PELLET_RADIUS,
                should_draw_debug,
                should_draw_sdf,
                bone_visible(pos, pos),
                is_hovered,
                is_in_selected,
            );
            if let Some(instance) = instances.debug {
                debug_instances.push(instance);
            }
            if let Some(instance) = instances.sdf {
                capsule_instances.push(instance);
            }
            if let Some(instance) = instances.hover {
                hover_bones.push(instance);
            }
            if let Some(instance) = instances.selected {
                selected_bones.push(instance);
            }
        }

        // Render corpses
        let mut query_corpse = self
            .world
            .ecs
            .query::<(bevy_ecs::entity::Entity, &ecology::Corpse)>();
        for (entity, corpse) in query_corpse.iter(&self.world.ecs) {
            let pos = [corpse.position.x, corpse.position.y];
            let pos3: [f32; 3] = corpse.position.into();
            let is_in_selected = selected_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));
            let should_draw_debug =
                self.ui.debug_structural && (selected_component.is_none() || is_in_selected);
            let should_draw_sdf =
                !self.ui.debug_structural || (selected_component.is_some() && !is_in_selected);
            let is_hovered = hovered_component
                .as_ref()
                .is_some_and(|c| c.contains(&entity));

            let instances = organism_visuals::pellet_like_instances(
                pos3,
                [0.153, 0.136, 0.156, 1.0], // #6D676E
                [0.153, 0.136, 0.156],
                organism_visuals::CORPSE_RADIUS,
                should_draw_debug,
                should_draw_sdf,
                bone_visible(pos, pos),
                is_hovered,
                is_in_selected,
            );
            if let Some(instance) = instances.debug {
                debug_instances.push(instance);
            }
            if let Some(instance) = instances.sdf {
                capsule_instances.push(instance);
            }
            if let Some(instance) = instances.hover {
                hover_bones.push(instance);
            }
            if let Some(instance) = instances.selected {
                selected_bones.push(instance);
            }
        }

        WorldRenderInstances {
            debug_instances,
            capsule_instances,
            hover_bones,
            selected_bones,
        }
    }
}
