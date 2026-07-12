//! Ecology systems, one file per system. Each module's own doc comment
//! documents its system in full: what it does, why it exists, and how it
//! works.

/// Catastrophe/hazard-field lifecycle and organism energy drain.
pub mod catastrophe_system;
/// Corpse decay/outgassing and mineralization.
pub mod corpse_decay;
/// Food pellet spawning up to the population cap.
pub mod food_spawner;
/// Predation, herbivory, and pellet/mineral/corpse consumption.
pub mod foraging;
/// Producer (plant) photosynthesis.
pub mod photosynthesis;
/// Per-tick spatial-grid rebuild for food/mineral/corpse broad-phase queries.
pub mod resource_grids;

pub use catastrophe_system::catastrophe_system;
pub use corpse_decay::corpse_decay_system;
pub use food_spawner::food_spawner_system;
pub use foraging::foraging_system;
pub use photosynthesis::photosynthesis_system;
pub use resource_grids::build_resource_grids_system;
