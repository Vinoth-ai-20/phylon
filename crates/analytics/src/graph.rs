//! Graph connectivity analysis over an arbitrary undirected edge list —
//! deliberately generic (`usize` node indices, not `Entity`), so this stays
//! decoupled from `bevy_ecs`/`physics` the same way [`crate::shannon_index`]
//! stays decoupled from `evolution`. Callers (see
//! `app::analytics_bridge::colony_connectivity_system`) map `Entity` IDs to
//! dense `usize` indices before calling in, and map results back afterward.

use std::collections::{HashSet, VecDeque};

/// Undirected adjacency list built from an edge list — used by both
/// [`connected_components`] and [`diameter`].
struct AdjacencyList {
    neighbors: Vec<Vec<usize>>,
}

impl AdjacencyList {
    fn build(node_count: usize, edges: &[(usize, usize)]) -> Self {
        let mut neighbors = vec![Vec::new(); node_count];
        for &(a, b) in edges {
            if a < node_count && b < node_count {
                neighbors[a].push(b);
                neighbors[b].push(a);
            }
        }
        Self { neighbors }
    }
}

/// # Connected Components
///
/// ## 1. What Happens
/// Partitions `node_count` nodes (indexed `0..node_count`) into connected
/// components given an undirected `edges` list, returning each node's
/// component ID (`result[node] == result[other]` iff they're in the same
/// component).
///
/// ## 2. Why It Happens
/// A "colony" (per the spec's graph connectivity analysis requirement) is
/// exactly a connected component of the budding-spring graph (see
/// `reproduction::ReproductionMode::Budding`, Epic 10) — this is the
/// primitive that lets `colony_size_distribution` and other colony-level
/// analytics exist at all.
///
/// ## 3. How It Happens
/// Breadth-first search from every not-yet-visited node, assigning the same
/// component ID to everything reached. O(nodes + edges).
pub fn connected_components(node_count: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let adjacency = AdjacencyList::build(node_count, edges);
    let mut component = vec![usize::MAX; node_count];
    let mut next_component = 0;

    for start in 0..node_count {
        if component[start] != usize::MAX {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        component[start] = next_component;
        while let Some(node) = queue.pop_front() {
            for &neighbor in &adjacency.neighbors[node] {
                if component[neighbor] == usize::MAX {
                    component[neighbor] = next_component;
                    queue.push_back(neighbor);
                }
            }
        }
        next_component += 1;
    }

    component
}

/// Groups [`connected_components`]' output into a size-per-component
/// distribution — e.g. "3 colonies of size 1 (solitary organisms), 2
/// colonies of size 4".
pub fn colony_size_distribution(components: &[usize]) -> Vec<usize> {
    if components.is_empty() {
        return Vec::new();
    }
    let component_count = components.iter().max().map(|&m| m + 1).unwrap_or(0);
    let mut sizes = vec![0usize; component_count];
    for &c in components {
        sizes[c] += 1;
    }
    sizes
}

/// # Network Diameter
///
/// ## 1. What Happens
/// Computes the diameter (longest shortest path, in edge count) of the
/// single connected component containing `start` — not the whole graph,
/// since diameter is only meaningful within one connected component (see
/// [`connected_components`]).
///
/// ## 2. Why It Happens
/// Per the spec's "graph connectivity analysis (colony cohesion, network
/// diameter)" — diameter is a cohesion proxy: a colony that's a tight
/// cluster has a small diameter relative to its size, while a long chain of
/// budded organisms has a diameter close to its member count.
///
/// ## 3. How It Happens
/// BFS from every node *within the starting node's component only* (found
/// via one BFS pass first), tracking the maximum eccentricity seen — the
/// standard "double BFS is a diameter lower bound, all-pairs BFS is exact"
/// tradeoff; this uses the exact all-pairs-within-component version since
/// colonies are expected to stay small (tens of members, not thousands).
pub fn diameter(node_count: usize, edges: &[(usize, usize)], start: usize) -> usize {
    if start >= node_count {
        return 0;
    }
    let adjacency = AdjacencyList::build(node_count, edges);

    let bfs_farthest = |from: usize, component: &HashSet<usize>| -> usize {
        let mut visited = vec![false; node_count];
        let mut queue = VecDeque::from([(from, 0usize)]);
        visited[from] = true;
        let mut max_dist = 0;
        while let Some((node, dist)) = queue.pop_front() {
            max_dist = max_dist.max(dist);
            for &neighbor in &adjacency.neighbors[node] {
                if component.contains(&neighbor) && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back((neighbor, dist + 1));
                }
            }
        }
        max_dist
    };

    // First BFS to find every node in `start`'s component.
    let mut component = HashSet::new();
    let mut queue = VecDeque::from([start]);
    component.insert(start);
    while let Some(node) = queue.pop_front() {
        for &neighbor in &adjacency.neighbors[node] {
            if component.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }

    component
        .iter()
        .map(|&node| bfs_farthest(node, &component))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_nodes_are_each_their_own_component() {
        let components = connected_components(3, &[]);
        assert_eq!(components[0], components[0]);
        assert_ne!(components[0], components[1]);
        assert_ne!(components[1], components[2]);
    }

    #[test]
    fn connected_nodes_share_a_component() {
        let components = connected_components(4, &[(0, 1), (1, 2)]);
        assert_eq!(components[0], components[1]);
        assert_eq!(components[1], components[2]);
        assert_ne!(components[0], components[3]);
    }

    #[test]
    fn colony_size_distribution_counts_members_per_component() {
        // 0-1-2 form one colony of size 3; 3 is solitary.
        let components = connected_components(4, &[(0, 1), (1, 2)]);
        let mut sizes = colony_size_distribution(&components);
        sizes.sort_unstable();
        assert_eq!(sizes, vec![1, 3]);
    }

    #[test]
    fn diameter_of_a_chain_equals_its_length() {
        // 0-1-2-3 is a chain of 4 nodes, 3 edges apart end to end.
        let d = diameter(4, &[(0, 1), (1, 2), (2, 3)], 0);
        assert_eq!(d, 3);
    }

    #[test]
    fn diameter_of_a_single_node_is_zero() {
        assert_eq!(diameter(1, &[], 0), 0);
    }

    #[test]
    fn diameter_ignores_other_components() {
        // 0-1 is one colony (diameter 1); 2-3-4 is a separate, longer one.
        let d = diameter(5, &[(0, 1), (2, 3), (3, 4)], 0);
        assert_eq!(d, 1);
    }
}
