use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 2],
    pub heading: f32,
    pub speed: f32,
    pub size: f32,
    pub base_color: [f32; 3],
    pub diet: u32,        // 0=Herbivore, 1=Carnivore, 2=Scavenger
    pub energy: f32,      // 0.0–1.0 normalised
    pub health: f32,      // 0.0–1.0 normalised
    pub is_infected: u32, // 0 or 1
    pub tick_age: f32,    // Age(u64) cast to f32, raw tick count
    pub species_id: u32,  // SpeciesId(u32) as procedural seed
    pub _pad: [f32; 2],   // align to 16 bytes
}

impl InstanceData {
    const ATTRIBUTES: [wgpu::VertexAttribute; 11] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: 8,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 12,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 20,
            shader_location: 5,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 6,
            format: wgpu::VertexFormat::Uint32,
        },
        wgpu::VertexAttribute {
            offset: 36,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 40,
            shader_location: 8,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 44,
            shader_location: 9,
            format: wgpu::VertexFormat::Uint32,
        },
        wgpu::VertexAttribute {
            offset: 48,
            shader_location: 10,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: 52,
            shader_location: 11,
            format: wgpu::VertexFormat::Uint32,
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
