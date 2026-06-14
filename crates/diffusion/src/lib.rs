//! Field Diffusion management using WGSL compute shaders.

use bytemuck::{Pod, Zeroable};
use gpu::compute::DiffusionPipeline;
use std::mem;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct DiffusionUniforms {
    pub grid_width: u32,
    pub grid_height: u32,
    pub diffusion_rate: f32,
    pub decay_rate: f32,
}

pub struct DiffusionField {
    pub width: u32,
    pub height: u32,
    pub uniforms: wgpu::Buffer,
    pub buffer_a: wgpu::Buffer,
    pub buffer_b: wgpu::Buffer,
    pub bind_group_a: wgpu::BindGroup,
    pub bind_group_b: wgpu::BindGroup,
    pub flip: bool,
}

impl DiffusionField {
    pub fn new(
        device: &wgpu::Device,
        pipeline: &DiffusionPipeline,
        width: u32,
        height: u32,
        diffusion_rate: f32,
        decay_rate: f32,
    ) -> Self {
        let initial_data = vec![0.0f32; (width * height) as usize];
        let size = (initial_data.len() * mem::size_of::<f32>()) as wgpu::BufferAddress;

        let buffer_a = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Diffusion Buffer A"),
            contents: bytemuck::cast_slice(&initial_data),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let buffer_b = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Diffusion Buffer B"),
            contents: bytemuck::cast_slice(&initial_data),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });

        let uniforms = DiffusionUniforms {
            grid_width: width,
            grid_height: height,
            diffusion_rate,
            decay_rate,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Diffusion Uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_a = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Diffusion Bind Group A"),
            layout: &pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer_a.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_b.as_entire_binding(),
                },
            ],
        });

        let bind_group_b = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Diffusion Bind Group B"),
            layout: &pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffer_b.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffer_a.as_entire_binding(),
                },
            ],
        });

        Self {
            width,
            height,
            uniforms: uniform_buffer,
            buffer_a,
            buffer_b,
            bind_group_a,
            bind_group_b,
            flip: false,
        }
    }

    pub fn dispatch(&mut self, encoder: &mut wgpu::CommandEncoder, pipeline: &DiffusionPipeline) {
        puffin::profile_function!();
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Diffusion Compute Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&pipeline.pipeline);
        if self.flip {
            compute_pass.set_bind_group(0, &self.bind_group_b, &[]);
        } else {
            compute_pass.set_bind_group(0, &self.bind_group_a, &[]);
        }

        // Dispatch (width/16, height/16, 1)
        compute_pass.dispatch_workgroups((self.width + 15) / 16, (self.height + 15) / 16, 1);

        // Swap buffers for next tick
        self.flip = !self.flip;
    }

    pub fn current_buffer(&self) -> &wgpu::Buffer {
        if self.flip {
            &self.buffer_b
        } else {
            &self.buffer_a
        }
    }
}
