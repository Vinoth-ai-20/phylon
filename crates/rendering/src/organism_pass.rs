use glam::Mat4;
use wgpu::util::DeviceExt;

use genetics::{Diet, Genome};
use organisms::{Age, Disease, Energy, Health, SpeciesId};
use physics::{Heading, Position};

use crate::instance_buffer::InstanceData;

const VERTICES: &[[f32; 2]] = &[[-0.5, -0.5], [0.5, -0.5], [-0.5, 0.5], [0.5, 0.5]];
const INDICES: &[u16] = &[0, 1, 2, 2, 1, 3];

pub struct OrganismPass {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_capacity: u32,
    instance_count: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    camera_bind_group_layout: wgpu::BindGroupLayout,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl OrganismPass {
    pub fn new(device: &wgpu::Device, _config: &wgpu::SurfaceConfiguration) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Organism Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/rendering/organism.wgsl").into(),
            ),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of::<CameraUniform>() as u64,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Organism Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        // The pipeline uses MRT.
        // Location 0: Main Screen (config.format)
        // Location 1: Trail Texture (Rgba16Float)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Organism Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                    },
                    InstanceData::desc(),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[
                    // Location 0: Main screen
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // Location 1: Trail
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        // Additive blending for the trail
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::One, // additive over the trail
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Organism Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Organism Quad Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let initial_capacity = 10_000;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Organism Instance Buffer"),
            size: (initial_capacity * std::mem::size_of::<InstanceData>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_capacity: initial_capacity as u32,
            instance_count: 0,
            camera_buffer,
            camera_bind_group,
            camera_bind_group_layout,
        }
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    pub fn camera_buffer(&self) -> &wgpu::Buffer {
        &self.camera_buffer
    }

    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        world: &mut world::PhylonWorld,
        config: &wgpu::SurfaceConfiguration,
    ) {
        puffin::profile_function!();

        // Update Camera
        let aspect_ratio = config.width as f32 / config.height as f32;
        let width = 1000.0;
        let height = width / aspect_ratio;

        let proj = Mat4::orthographic_rh(
            -width / 2.0,
            width / 2.0,
            -height / 2.0,
            height / 2.0,
            -1.0,
            1.0,
        );

        let camera_uniform = CameraUniform {
            view_proj: proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        let mut instances = Vec::new();

        for (_id, (pos, vel, heading, energy, health, age, species, genome, disease)) in
            world.ecs.query_mut::<(
                &Position,
                &physics::Velocity,
                &Heading,
                &Energy,
                &Health,
                &Age,
                &SpeciesId,
                &Genome,
                Option<&Disease>,
            )>()
        {
            let is_infected = if disease.is_some() { 1 } else { 0 };
            let diet_val = match genome.diet {
                Diet::Herbivore => 0,
                Diet::Carnivore => 1,
                Diet::Omnivore => 2, // Map Omnivore to Scavenger visual
            };

            instances.push(InstanceData {
                position: pos.0.into(),
                heading: heading.0,
                speed: vel.0.length(),
                size: genome.size,
                base_color: genome.color,
                diet: diet_val,
                energy: energy.0,
                health: health.0,
                is_infected,
                tick_age: age.0 as f32, // Passed directly as f32 for shader
                species_id: species.0,
                _pad: [0.0; 2],
            });
        }

        // Sort by size ascending, so larger organisms are drawn last (on top)
        instances.sort_by(|a, b| {
            a.size
                .partial_cmp(&b.size)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.instance_count = instances.len() as u32;

        if self.instance_count > 0 {
            if self.instance_count > self.instance_capacity {
                self.instance_capacity = self.instance_count.next_power_of_two();
                self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Organism Instance Buffer"),
                    size: (self.instance_capacity as usize * std::mem::size_of::<InstanceData>())
                        as wgpu::BufferAddress,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        if self.instance_count > 0 {
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..self.instance_count);
        }
    }
}
