use bevy_ecs::prelude::*;

/// How often (in simulation ticks) the analytics bridge runs — these are
/// O(population) aggregations (species/age/generation distributions,
/// colony graph construction), so this follows the same "periodic, not
/// per-tick" principle `organisms::hebbian_plasticity_system`'s pruning
/// uses, rather than paying the cost every tick.
const ANALYTICS_SAMPLE_INTERVAL_TICKS: u64 = 60; // once per second at 60 Hz

/// Bridges `evolution::LineageTracker` (species/age/generation per
/// currently-alive organism) and the inter-organism `physics::Spring` graph
/// (colony links formed by `reproduction::ReproductionMode::Budding`) into
/// `analytics::MetricsState` — computing Shannon/Simpson diversity, species
/// richness/turnover, age/generation distributions, and colony
/// size/diameter.
///
/// `analytics` deliberately doesn't depend on `evolution`/`physics` (see
/// `analytics::shannon_index`'s doc comment) so it stays a decoupled
/// math/storage crate, callable from contexts that don't have a live ECS
/// `World` at all (tests, offline report generation). Something has to sit
/// between `analytics` and the simulation-domain crates — `app`, the
/// composition root, is that something, the same pattern `app::batch` uses
/// to bridge `research`.
///
/// A "colony" is a connected component of the graph formed by springs whose
/// two endpoint nodes belong to *different* organisms (`ParticleNode.organism_id`
/// differs) — ordinary intra-body springs (bones, muscles) connect nodes of
/// the *same* organism and are correctly excluded. Solitary organisms with
/// no budding links still appear as size-1 colonies, so
/// `colony_size_distribution`'s length always equals the live population.
pub fn analytics_bridge_system(
    lineage_tracker: Option<Res<evolution::LineageTracker>>,
    atmosphere: Option<Res<metabolism::GlobalAtmosphere>>,
    mut metrics: ResMut<analytics::MetricsState>,
    node_query: Query<(Entity, &physics::ParticleNode)>,
    spring_query: Query<&physics::Spring>,
) {
    let Some(atmosphere) = atmosphere else {
        return;
    };
    if !atmosphere
        .ticks
        .is_multiple_of(ANALYTICS_SAMPLE_INTERVAL_TICKS)
    {
        return;
    }

    if let Some(tracker) = lineage_tracker {
        let mut species_counts: std::collections::HashMap<u64, usize> =
            std::collections::HashMap::new();
        let mut ages = Vec::new();
        let mut generations = Vec::new();
        for record in tracker.active_records() {
            *species_counts.entry(record.species.0).or_insert(0) += 1;
            ages.push(atmosphere.ticks.saturating_sub(record.birth_tick));
            generations.push(record.generation);
        }
        // A single paired `Vec` from one `.iter()` pass, not two separately
        // collected `.keys()`/`.values()` vectors — avoids relying on both
        // iterators staying positionally aligned across two separate calls,
        // and lets `record_diversity` retain the species-id to count
        // pairing.
        let species_distribution: Vec<(u64, usize)> = species_counts
            .iter()
            .map(|(&id, &count)| (id, count))
            .collect();
        metrics.record_diversity(&species_distribution);
        metrics.record_distributions(ages, generations);
    }

    // Colony connectivity: one node per distinct organism_id, edges from
    // springs whose two endpoints belong to different organisms.
    let entity_organism: std::collections::HashMap<Entity, u32> =
        node_query.iter().map(|(e, n)| (e, n.organism_id)).collect();

    let mut organism_ids: Vec<u32> = entity_organism.values().copied().collect();
    organism_ids.sort_unstable();
    organism_ids.dedup();
    let index_of: std::collections::HashMap<u32, usize> = organism_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    let mut edges: Vec<(usize, usize)> = Vec::new();
    for spring in spring_query.iter() {
        let (Some(&org_a), Some(&org_b)) = (
            entity_organism.get(&spring.node_a),
            entity_organism.get(&spring.node_b),
        ) else {
            continue;
        };
        if org_a == org_b {
            continue;
        }
        let (Some(&idx_a), Some(&idx_b)) = (index_of.get(&org_a), index_of.get(&org_b)) else {
            continue;
        };
        edges.push((idx_a, idx_b));
    }

    let components = analytics::graph::connected_components(organism_ids.len(), &edges);
    let sizes = analytics::graph::colony_size_distribution(&components);

    let largest_diameter = sizes
        .iter()
        .enumerate()
        .max_by_key(|(_, &size)| size)
        .map(|(component_id, _)| {
            let representative = components
                .iter()
                .position(|&c| c == component_id)
                .unwrap_or(0);
            analytics::graph::diameter(organism_ids.len(), &edges, representative)
        })
        .unwrap_or(0);

    metrics.record_colony_connectivity(sizes, largest_diameter);
}
