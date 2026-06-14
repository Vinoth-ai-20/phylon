//! wgpu rendering pipeline for debugging and visualisation.

pub mod field_pass;
pub mod food_pass;
pub mod instance_buffer;
pub mod organism_pass;
pub mod post_pass;
pub mod trail_pass;

pub use field_pass::FieldPass;
pub use food_pass::FoodPass;
pub use organism_pass::OrganismPass;
pub use post_pass::PostPass;
pub use trail_pass::TrailPass;
