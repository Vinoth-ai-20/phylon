//! Environment and Terrain generation.

use common::{ChunkId, Vec2};
use noise::{NoiseFn, OpenSimplex};

pub const CHUNK_SIZE: u32 = 256;

#[derive(Debug, Clone)]
pub struct ChunkTerrain {
    pub id: ChunkId,
    pub heights: Vec<f32>,
}

impl ChunkTerrain {
    pub fn generate(id: ChunkId, seed: u32) -> Self {
        puffin::profile_function!();
        let noise = OpenSimplex::new(seed);
        let mut heights = Vec::with_capacity((CHUNK_SIZE * CHUNK_SIZE) as usize);

        let base_x = id.0 as f32 * CHUNK_SIZE as f32;
        let base_y = id.1 as f32 * CHUNK_SIZE as f32;

        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let wx = base_x + x as f32;
                let wy = base_y + y as f32;
                
                // Sample noise at low frequency
                let nx = wx * 0.01;
                let ny = wy * 0.01;
                let val = noise.get([nx as f64, ny as f64]) as f32;
                
                // Map from [-1, 1] to [0, 1]
                heights.push((val + 1.0) * 0.5);
            }
        }

        Self { id, heights }
    }

    /// Gets the height at a specific local coordinate [0, CHUNK_SIZE).
    pub fn get_height(&self, local_x: u32, local_y: u32) -> f32 {
        if local_x >= CHUNK_SIZE || local_y >= CHUNK_SIZE {
            return 0.0;
        }
        self.heights[(local_y * CHUNK_SIZE + local_x) as usize]
    }
}
