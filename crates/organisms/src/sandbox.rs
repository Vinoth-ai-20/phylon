//! Sandbox mode: manual entity tagging, named presets, and procedural test
//! fixtures for debugging and demonstration, independent of the
//! genome/CPPN/regulatory-network growth pipeline the rest of this crate
//! implements.

use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};

/// Flag-based traits for the sandbox and debugging tools.
///
/// These traits operate independently of the Hox/CPPN genetics pipeline. They allow
/// the user to manually tag entities with specific behaviors via the Inspector UI.
#[derive(Component, Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxTraits {
    /// Entity acts as a seed for a membrane wall.
    pub is_membrane_seed: bool,
    /// Entity duplicates itself when linked.
    pub link_duplicate: bool,
    /// Entity actively sends energy to linked neighbors.
    pub sends_energy: bool,
    /// Entity consumes oxygen/emits CO2.
    pub respires: bool,
    /// Entity generates energy from sunlight and consumes CO2.
    pub photosynthesis: bool,
    /// Entity possesses a functional tail for locomotion.
    pub has_tail: bool,
    /// Entity kills and eats animals on contact.
    pub kills_animals: bool,
    /// Entity acts as an edible plant source.
    pub edible_plant: bool,
    /// Entity acts as an edible animal source (meat).
    pub edible_animal: bool,
    /// Entity exerts a repulsive force on others.
    pub repels: bool,
    /// Entity can be grabbed by the user.
    pub grabbable: bool,
    /// Entity can be fixed in place (stationary).
    pub fixable: bool,
    /// Entity causes tearing in the velocity field.
    pub velocity_tear: bool,
    /// Entity is part of a structural mesh.
    pub mesh: bool,
}

/// A definition for a named organism or structure preset.
#[derive(Debug, Clone)]
pub struct PresetDefinition {
    /// Human-readable name of the preset.
    pub name: String,
    /// Whether this preset is a living, reproducing organism (true)
    /// or a static test fixture (false).
    pub evolvable: bool,
    /// Sandbox trait flags applied to the root node of this preset.
    pub traits: SandboxTraits,
    /// Optional: the specific diet type (if applicable).
    pub diet: Option<ecology::Diet>,
    /// Optional: the specific ecological category.
    pub category: Option<ecology::EcologicalCategory>,
}

impl PresetDefinition {
    /// Returns the standard predefined presets.
    pub fn standard_presets() -> Vec<PresetDefinition> {
        vec![
            PresetDefinition {
                name: "Herbivore (Evolvable)".to_string(),
                evolvable: true,
                traits: SandboxTraits {
                    respires: true,
                    grabbable: true,
                    ..Default::default()
                },
                diet: Some(ecology::Diet::Herbivore),
                category: Some(ecology::EcologicalCategory::None),
            },
            PresetDefinition {
                name: "Hunter (Evolvable)".to_string(),
                evolvable: true,
                traits: SandboxTraits {
                    respires: true,
                    kills_animals: true,
                    grabbable: true,
                    ..Default::default()
                },
                diet: Some(ecology::Diet::Carnivore),
                category: Some(ecology::EcologicalCategory::None),
            },
            PresetDefinition {
                name: "Edible Plant (Evolvable)".to_string(),
                evolvable: true,
                traits: SandboxTraits {
                    photosynthesis: true,
                    edible_plant: true,
                    grabbable: true,
                    ..Default::default()
                },
                diet: Some(ecology::Diet::Producer),
                category: Some(ecology::EcologicalCategory::None),
            },
            PresetDefinition {
                name: "Membrane Seed (Stationary)".to_string(),
                evolvable: false,
                traits: SandboxTraits {
                    is_membrane_seed: true,
                    fixable: true,
                    ..Default::default()
                },
                diet: None,
                category: None,
            },
            PresetDefinition {
                name: "Membrane Seed (Sealed)".to_string(),
                evolvable: false,
                traits: SandboxTraits {
                    is_membrane_seed: true,
                    fixable: true,
                    repels: true,
                    ..Default::default()
                },
                diet: None,
                category: None,
            },
            PresetDefinition {
                name: "Structure Node (Mesh)".to_string(),
                evolvable: false,
                traits: SandboxTraits {
                    mesh: true,
                    fixable: true,
                    ..Default::default()
                },
                diet: None,
                category: None,
            },
        ]
    }
}

/// Generates a hexagonal mesh of interconnected nodes.
///
/// This is used to build structural test fixtures like pressure tanks
/// and collision mazes.
pub fn generate_hex_mesh(
    world: &mut bevy_ecs::world::World,
    center: common::Vec2,
    cols: usize,
    rows: usize,
    spacing: f32,
    stiffness: f32,
    is_fixed: bool,
) {
    use physics::{ConstraintType, ParticleNode, Spring};

    let mut nodes = Vec::new();
    let mesh_id = world.spawn_empty().id();

    // Spawn nodes
    for row in 0..rows {
        let mut row_nodes = Vec::new();
        for col in 0..cols {
            // Offset odd rows by half a spacing unit to create a hex grid
            let x_offset = if row % 2 != 0 { spacing * 0.5 } else { 0.0 };
            let y_offset = row as f32 * spacing * 0.866; // sqrt(3)/2

            let pos = center.extend(0.0)
                + common::Vec3::new(
                    col as f32 * spacing + x_offset - (cols as f32 * spacing * 0.5),
                    y_offset - (rows as f32 * spacing * 0.866 * 0.5),
                    0.0,
                );

            let color = [0.4, 0.4, 0.5]; // Grey-blue for structure

            let entity = world
                .spawn((
                    ParticleNode::new(pos, 5.0, 1, mesh_id.index()),
                    crate::OrganismColor(color),
                    SandboxTraits {
                        mesh: true,
                        fixable: is_fixed,
                        ..Default::default()
                    },
                    // Biological components for inspector
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
                ))
                .id();

            // Set fixed state
            if let Some(mut node) = world.get_mut::<ParticleNode>(entity) {
                node.is_fixed = is_fixed;
            }

            row_nodes.push(entity);
        }
        nodes.push(row_nodes);
    }

    // Connect nodes
    for row in 0..rows {
        for col in 0..cols {
            let current = nodes[row][col];

            // Connect to right neighbor
            if col + 1 < cols {
                let right = nodes[row][col + 1];
                world.spawn((
                    Spring {
                        node_a: current,
                        node_b: right,
                        constraint_type: ConstraintType::Rigid,
                        rest_length: spacing,
                        base_length: spacing,
                        stiffness,
                        damping: 0.5,
                        actuation_amplitude: 0.0,
                        actuation_phase: 0.0,
                        breaking_strain: 5.0,
                        is_fin: 0,
                    },
                    crate::OrganismColor([0.3, 0.3, 0.4]),
                ));
            }

            // Connect to bottom neighbors (hex lattice)
            if row + 1 < rows {
                // Bottom left or straight down (depending on odd/even row)
                if row % 2 == 0 {
                    // Even row: bottom left is col-1, bottom right is col
                    if col > 0 {
                        let bottom_left = nodes[row + 1][col - 1];
                        world.spawn((
                            Spring {
                                node_a: current,
                                node_b: bottom_left,
                                constraint_type: ConstraintType::Rigid,
                                rest_length: spacing,
                                base_length: spacing,
                                stiffness,
                                damping: 0.5,
                                actuation_amplitude: 0.0,
                                actuation_phase: 0.0,
                                breaking_strain: 5.0,
                                is_fin: 0,
                            },
                            crate::OrganismColor([0.3, 0.3, 0.4]),
                        ));
                    }

                    let bottom_right = nodes[row + 1][col];
                    world.spawn((
                        Spring {
                            node_a: current,
                            node_b: bottom_right,
                            constraint_type: ConstraintType::Rigid,
                            rest_length: spacing,
                            base_length: spacing,
                            stiffness,
                            damping: 0.5,
                            actuation_amplitude: 0.0,
                            actuation_phase: 0.0,
                            breaking_strain: 5.0,
                            is_fin: 0,
                        },
                        crate::OrganismColor([0.3, 0.3, 0.4]),
                    ));
                } else {
                    // Odd row: bottom left is col, bottom right is col+1
                    let bottom_left = nodes[row + 1][col];
                    world.spawn((
                        Spring {
                            node_a: current,
                            node_b: bottom_left,
                            constraint_type: ConstraintType::Rigid,
                            rest_length: spacing,
                            base_length: spacing,
                            stiffness,
                            damping: 0.5,
                            actuation_amplitude: 0.0,
                            actuation_phase: 0.0,
                            breaking_strain: 5.0,
                            is_fin: 0,
                        },
                        crate::OrganismColor([0.3, 0.3, 0.4]),
                    ));

                    if col + 1 < cols {
                        let bottom_right = nodes[row + 1][col + 1];
                        world.spawn((
                            Spring {
                                node_a: current,
                                node_b: bottom_right,
                                constraint_type: ConstraintType::Rigid,
                                rest_length: spacing,
                                base_length: spacing,
                                stiffness,
                                damping: 0.5,
                                actuation_amplitude: 0.0,
                                actuation_phase: 0.0,
                                breaking_strain: 5.0,
                                is_fin: 0,
                            },
                            crate::OrganismColor([0.3, 0.3, 0.4]),
                        ));
                    }
                }
            }
        }
    }
}
