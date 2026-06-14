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
    pub _padding: [u32; 2],
    pub diffusion_rate: [f32; 4], // R=Oxygen, G=Carbon, B=Scent, A=Temperature
    pub decay_rate: [f32; 4],
}

pub struct DiffusionField {
    pub width: u32,
    pub height: u32,
    pub uniforms: wgpu::Buffer,
    pub buffer_a: wgpu::Buffer,
    pub buffer_b: wgpu::Buffer,
    pub staging_buffer: wgpu::Buffer,
    pub cpu_buffer: Vec<[f32; 4]>,
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
        diffusion_rate: [f32; 4],
        decay_rate: [f32; 4],
    ) -> Self {
        let initial_data = vec![[0.0f32; 4]; (width * height) as usize];
        let _size = (initial_data.len() * mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress;

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

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Diffusion Staging Buffer"),
            size: _size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniforms = DiffusionUniforms {
            grid_width: width,
            grid_height: height,
            _padding: [0; 2],
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
            staging_buffer,
            cpu_buffer: initial_data,
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
        compute_pass.dispatch_workgroups(self.width.div_ceil(16), self.height.div_ceil(16), 1);

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

    /// Downloads the current GPU field state into the CPU buffer.
    pub fn download(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        puffin::profile_function!();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Diffusion Download Encoder"),
        });

        let size =
            (self.width * self.height * mem::size_of::<[f32; 4]>() as u32) as wgpu::BufferAddress;

        encoder.copy_buffer_to_buffer(self.current_buffer(), 0, &self.staging_buffer, 0, size);
        queue.submit(Some(encoder.finish()));

        let buffer_slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        device.poll(wgpu::Maintain::Wait);

        if rx.recv().unwrap().is_ok() {
            let data = buffer_slice.get_mapped_range();
            self.cpu_buffer.copy_from_slice(bytemuck::cast_slice(&data));
            drop(data);
            self.staging_buffer.unmap();
        }
    }

    /// Uploads the CPU buffer to the current GPU field state.
    pub fn upload(&mut self, queue: &wgpu::Queue) {
        puffin::profile_function!();
        queue.write_buffer(
            self.current_buffer(),
            0,
            bytemuck::cast_slice(&self.cpu_buffer),
        );
    }

    pub fn get_cell(&self, x: u32, y: u32) -> [f32; 4] {
        if x >= self.width || y >= self.height {
            return [0.0; 4];
        }
        self.cpu_buffer[(y * self.width + x) as usize]
    }

    pub fn set_cell(&mut self, x: u32, y: u32, val: [f32; 4]) {
        if x < self.width && y < self.height {
            self.cpu_buffer[(y * self.width + x) as usize] = val;
        }
    }

    pub fn add_to_cell(&mut self, x: u32, y: u32, val: [f32; 4]) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            for (i, v) in val.iter().enumerate() {
                self.cpu_buffer[idx][i] += v;
            }
        }
    }
}
