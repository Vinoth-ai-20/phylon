use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub position: [f32; 2],
    pub heading: f32,
    pub size: f32,
    pub base_color: [f32; 3],
    pub diet: u32,
    pub energy: f32,
    pub health: f32,
    pub is_infected: u32,
    pub tick_age: f32,
    pub genome_id: u32,
}

impl InstanceData {
    const ATTRIBUTES: [wgpu::VertexAttribute; 10] = [
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x2,
        },
        wgpu::VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<f32>())
                as wgpu::BufferAddress,
            shader_location: 3,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<f32>() * 2)
                as wgpu::BufferAddress,
            shader_location: 4,
            format: wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<f32>() * 5)
                as wgpu::BufferAddress,
            shader_location: 5,
            format: wgpu::VertexFormat::Uint32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>()
                + std::mem::size_of::<f32>() * 5
                + std::mem::size_of::<u32>()) as wgpu::BufferAddress,
            shader_location: 6,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>()
                + std::mem::size_of::<f32>() * 6
                + std::mem::size_of::<u32>()) as wgpu::BufferAddress,
            shader_location: 7,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>()
                + std::mem::size_of::<f32>() * 7
                + std::mem::size_of::<u32>()) as wgpu::BufferAddress,
            shader_location: 8,
            format: wgpu::VertexFormat::Uint32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>()
                + std::mem::size_of::<f32>() * 7
                + std::mem::size_of::<u32>() * 2) as wgpu::BufferAddress,
            shader_location: 9,
            format: wgpu::VertexFormat::Float32,
        },
        wgpu::VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>()
                + std::mem::size_of::<f32>() * 8
                + std::mem::size_of::<u32>() * 2) as wgpu::BufferAddress,
            shader_location: 10,
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
