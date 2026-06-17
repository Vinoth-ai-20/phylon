use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 2],
    pub heading: f32,
    pub speed: f32,
    pub size: f32,
    pub base_color: [f32; 3],
    pub diet: f32,        // 0=Herbivore, 1=Carnivore, 2=Scavenger
    pub energy: f32,      // 0.0–1.0 normalised
    pub health: f32,      // 0.0–1.0 normalised
    pub is_infected: f32, // 0.0 or 1.0
    pub tick_age: f32,    // Age(u64) cast to f32, raw tick count
    pub species_id: f32,  // SpeciesId(u32) as procedural seed
    pub death_age: f32,   // 0.0 if alive, ticks since death

    // HOX gene data packed for GPU
    // hox_genes_a: segments 0-3 packed as bytes
    // hox_genes_b: segments 4-6 packed as bytes + hox_count
    pub hox_genes_a: u32, // [seg0, seg1, seg2, seg3]
    pub hox_genes_b: u32, // [seg4, seg5, seg6, count]

    // HOX size factors — 7 floats
    pub hox_sizes: [f32; 7],

    // Appendage data packed as u32s
    // Lower byte: appendage type, upper byte: count
    pub hox_appends: [u32; 7],

    pub _pad: [f32; 2], // maintain 16-byte alignment
}

impl InstanceData {
    const ATTRIBUTES: [wgpu::VertexAttribute; 9] = [
        wgpu::VertexAttribute {
            // pos, heading, speed
            offset: 0,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            // size, base_color
            offset: 16,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            // diet, energy, health, is_infected
            offset: 32,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            // tick_age, species_id, death_age
            offset: 48,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            // hox_genes_a, hox_genes_b
            offset: 60,
            shader_location: 5,
            format: wgpu::VertexFormat::Uint32x2,
        },
        wgpu::VertexAttribute {
            // hox_sizes 0..4
            offset: 68,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32x4,
        },
        wgpu::VertexAttribute {
            // hox_sizes 4..7
            offset: 84,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            // hox_appends 0..4
            offset: 96,
            shader_location: 8,
            format: wgpu::VertexFormat::Uint32x4,
        },
        wgpu::VertexAttribute {
            // hox_appends 4..7
            offset: 112,
            shader_location: 9,
            format: wgpu::VertexFormat::Uint32x3,
        },
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBUTES,
        }
    }
}
