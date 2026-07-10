use crate::capsule_mesh::{build_capsule_mesh, CapsuleVertex};
use wgpu::util::DeviceExt;

/// # Capsule Instance
///
/// ## 1. What Happens
/// `CapsuleInstance` holds the GPU-side per-bone payload for the mesh-based
/// capsule renderer (Phase 8, ADR-P8-03) — the direct successor to
/// `SdfBoneInstance`, widened from `Vec2` to `Vec3` endpoints.
///
/// ## 2. Why It Happens
/// ADR-P8-03: a shared, tiny, procedurally-generated capsule mesh, instanced
/// per bone via an oriented-look-at vertex shader — nearly the same
/// *instance data* as the old `SdfBoneInstance`, only the *shader algorithm*
/// (oriented rasterized mesh vs. metaball density accumulation) changes.
///
/// ## 3. How It Happens
/// `capsule.wgsl`'s vertex shader reconstructs each mesh vertex's world
/// position from `pos_a`/`pos_b`/`radius` directly — see that shader's own
/// doc comment.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CapsuleInstance {
    /// World-space endpoint A (e.g. the node with smaller index).
    pub pos_a: [f32; 3],
    /// World-space endpoint B.
    pub pos_b: [f32; 3],
    /// Capsule skin radius in world units.
    pub radius: f32,
    /// RGB tint.
    pub color: [f32; 3],
    /// Vitality dimming factor in `[0, 1]` — see `SdfBoneInstance::health`'s
    /// doc comment (unchanged rationale, carried over verbatim).
    pub health: f32,
}

impl CapsuleInstance {
    // Locations 0-1 are the shared mesh's own vertex attributes
    // (`CapsuleVertex::desc()`); instance attributes start at 2.
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        2 => Float32x3, // pos_a
        3 => Float32x3, // pos_b
        4 => Float32,   // radius
        5 => Float32x3, // color
        6 => Float32,   // health
    ];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CapsuleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuCamera {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _pad0: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    /// The directional light's view-projection matrix (Epic 8.3) — used
    /// both to render the shadow map (`vs_shadow`) and to sample it back
    /// (`fs_main`'s `sample_shadow`). First field so `sun_dir`/`sunlight`
    /// naturally pack into the remaining 16-byte slot after it with no
    /// explicit padding (mirrors `GpuCamera`'s own layout reasoning).
    light_view_proj: [[f32; 4]; 4],
    sun_dir: [f32; 3],
    sunlight: f32,
}

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Shadow map resolution — untuned-but-reasonable, same status as every
/// other not-yet-measured constant introduced this phase.
const SHADOW_MAP_SIZE: u32 = 2048;

/// # Mesh-Based Capsule Organism Renderer
///
/// ## 1. What Happens
/// `OrganismRenderer` draws every organism/pellet bone as an instanced,
/// depth-correct, lit capsule mesh — the Phase 8 (ADR-P8-03) replacement
/// for the retired `SdfSkinRenderer`.
///
/// ## 2. Why It Happens
/// See ADR-P8-03 (`PHASE8_NATIVE_3D_ENGINE_ROADMAP.md`): the old 2-pass SDF
/// metaball technique has no depth buffer, can't support PBR/shadows/LOD/
/// clipping planes natively, and its "skeleton" already exists — the mesh
/// pipeline reuses the exact same per-bone data, just rasterized instead of
/// density-accumulated.
///
/// ## 3. How It Happens
/// One shared, procedurally-generated capsule mesh (`capsule_mesh`) is
/// instanced per bone; `capsule.wgsl`'s vertex shader orients/scales each
/// instance from `pos_a`/`pos_b`/`radius` (no per-instance rotation stored).
/// A real depth buffer (`Depth32Float`) is owned by this renderer — the
/// first depth-consuming pass anywhere in the codebase (ADR-P8-03). Shading
/// is single-light Cook-Torrance PBR (`capsule.wgsl`'s fragment shader),
/// modulated by a directional shadow map (Epic 8.3): every frame first
/// renders a depth-only pass of the same instances from the light's point
/// of view (`shadow_texture`), then the main pass samples it back to
/// determine which fragments are in shadow. One shared `PipelineLayout`
/// (camera/light, highlight color, shadow map — 3 groups) backs all three
/// pipelines (main, highlight, shadow-writing), so no pipeline layout is
/// duplicated across them.
pub struct OrganismRenderer {
    pipeline: wgpu::RenderPipeline,
    highlight_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,

    camera_buffer: wgpu::Buffer,
    light_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    highlight_color_buffer: wgpu::Buffer,
    highlight_color_bind_group: wgpu::BindGroup,

    // Never resized (fixed `SHADOW_MAP_SIZE`) and never read directly after
    // construction — kept only so the texture (and thus `shadow_view`,
    // which borrows from it internally) stays alive for the renderer's
    // lifetime, the same ownership relationship `depth_texture`/`depth_view`
    // already have.
    #[allow(dead_code)]
    shadow_texture: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    shadow_bind_group: wgpu::BindGroup,

    mesh_vertex_buffer: wgpu::Buffer,
    mesh_index_buffer: wgpu::Buffer,
    mesh_index_count: u32,

    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    current_width: u32,
    current_height: u32,

    instance_capacity: usize,
    instance_buffer: Option<wgpu::Buffer>,
    highlight_instance_capacity: usize,
    highlight_instance_buffer: Option<wgpu::Buffer>,
}

impl OrganismRenderer {
    /// Creates a new `OrganismRenderer`.
    ///
    /// `surface_format` must be the swapchain format (used for the main
    /// pipeline's colour target).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("OrganismCameraBuffer"),
            contents: bytemuck::bytes_of(&GpuCamera {
                view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
                camera_pos: [0.0; 3],
                _pad0: 0.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("OrganismLightBuffer"),
            contents: bytemuck::bytes_of(&GpuLight {
                light_view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
                sun_dir: [0.4, -0.3, -0.85],
                sunlight: 1.0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OrganismCameraBGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    // `vs_shadow` reads `light.light_view_proj`; `fs_main`
                    // reads all three fields.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OrganismCameraBindGroup"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        let highlight_color_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("OrganismHighlightColorBuffer"),
            contents: bytemuck::cast_slice(&[0.0f32; 4]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let highlight_color_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("OrganismHighlightColorBGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let highlight_color_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OrganismHighlightColorBindGroup"),
            layout: &highlight_color_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: highlight_color_buffer.as_entire_binding(),
            }],
        });

        // Shadow map (Epic 8.3) — a fixed-size depth-only texture,
        // rendered from the directional light's point of view each frame
        // and sampled back by the main pass's fragment shader. Unlike the
        // main scene's depth texture, this never resizes with the window.
        let shadow_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OrganismShadowTexture"),
            size: wgpu::Extent3d {
                width: SHADOW_MAP_SIZE,
                height: SHADOW_MAP_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("OrganismShadowSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // A comparison sampler with linear filtering gives cheap,
            // standard bilinear-filtered PCF (one `textureSampleCompare`
            // tap, hardware-interpolated) — "basic shadow mapping," not a
            // multi-tap PCF kernel this epic doesn't call for.
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("OrganismShadowBGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("OrganismShadowBindGroup"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CapsuleShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("capsule.wgsl").into()),
        });

        // One shared layout (camera/light, highlight color, shadow map)
        // backs all three pipelines below — none of `capsule.wgsl`'s entry
        // points needs every group, but declaring one consistent layout
        // (rather than a bespoke subset per pipeline) means every pipeline
        // binds the same 3 groups uniformly at draw time, and no pipeline
        // layout is duplicated (Epic 8.3's own architecture rule).
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("OrganismPipelineLayout"),
            bind_group_layouts: &[&camera_bgl, &highlight_color_bgl, &shadow_bgl],
            push_constant_ranges: &[],
        });
        let depth_stencil_state = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let vertex_buffers = [CapsuleVertex::desc(), CapsuleInstance::desc()];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OrganismPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // No culling: the mesh-generation winding order isn't
                // independently verified per-triangle, and this mesh is
                // small enough that drawing both faces costs nothing
                // measurable — correctness (a visible, complete capsule)
                // over a micro-optimization this epic doesn't need.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth_stencil_state.clone()),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Highlight pipeline: the "inverted hull" outline technique (this
        // module's own architecture note in the roadmap) — draw only the
        // *back* faces of a slightly inflated capsule, depth-tested against
        // (not writing into) the main pass's already-populated depth
        // buffer, so the outline only shows past the main silhouette. Reuses
        // the same shared 3-group `pipeline_layout` as `pipeline` — group 2
        // (shadow map) is bound but unread by `fs_highlight`.
        let highlight_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OrganismHighlightPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_highlight",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                ..depth_stencil_state
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Shadow pass: depth-only, writes into `shadow_texture` from the
        // light's point of view. A small constant + slope-scaled depth bias
        // mitigates shadow-acne self-shadowing artifacts on the capsule's
        // curved surface; both are cheap, standard, untuned starting values
        // (this epic's "measure before optimizing" rule applies to visual
        // acne/peter-panning correction if the interactive pass finds it
        // wrong, not to guessing a better constant up front).
        // Its own minimal layout (group 0 only) — `vs_shadow` reads nothing
        // from groups 1/2, and critically must NOT declare group 2 (the
        // shadow map itself): binding `shadow_bind_group` in this same pass
        // while its depth attachment writes into that same texture is a
        // self-referential read+write wgpu correctly rejects.
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("OrganismShadowPipelineLayout"),
                bind_group_layouts: &[&camera_bgl],
                push_constant_ranges: &[],
            });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("OrganismShadowPipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_shadow",
                buffers: &vertex_buffers,
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let (mesh_vertices, mesh_indices) = build_capsule_mesh();
        let mesh_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CapsuleMeshVertexBuffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mesh_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("CapsuleMeshIndexBuffer"),
            contents: bytemuck::cast_slice(&mesh_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let mesh_index_count = mesh_indices.len() as u32;

        let (depth_texture, depth_view) = Self::create_depth_texture(device, width, height);

        Self {
            pipeline,
            highlight_pipeline,
            shadow_pipeline,
            camera_buffer,
            light_buffer,
            camera_bind_group,
            highlight_color_buffer,
            highlight_color_bind_group,
            shadow_texture,
            shadow_view,
            shadow_bind_group,
            mesh_vertex_buffer,
            mesh_index_buffer,
            mesh_index_count,
            depth_texture,
            depth_view,
            current_width: width,
            current_height: height,
            instance_capacity: 0,
            instance_buffer: None,
            highlight_instance_capacity: 0,
            highlight_instance_buffer: None,
        }
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("OrganismDepthTexture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        (tex, view)
    }

    /// Recreates the depth texture when the surface is resized.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.current_width && height == self.current_height {
            return;
        }
        let (tex, view) = Self::create_depth_texture(device, width, height);
        self.depth_texture = tex;
        self.depth_view = view;
        self.current_width = width;
        self.current_height = height;
    }

    /// Exposes this renderer's depth buffer view so other passes (e.g.
    /// `DebugRenderer`'s camera-facing billboards, Epic 8.3) can depth-test
    /// against the same scene depth without owning a redundant depth
    /// texture of their own.
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    fn ensure_instance_buffer<'a>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &'a mut Option<wgpu::Buffer>,
        capacity: &mut usize,
        label: &'static str,
        instances: &[CapsuleInstance],
    ) -> &'a wgpu::Buffer {
        if instances.len() > *capacity || buffer.is_none() {
            *capacity = instances.len().max(*capacity * 2).max(256);
            *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (*capacity * std::mem::size_of::<CapsuleInstance>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buf = buffer.as_ref().unwrap();
        queue.write_buffer(buf, 0, bytemuck::cast_slice(instances));
        buf
    }

    /// Writes this frame's camera/light uniforms — shared by `render` and
    /// `render_highlight` since both draw into the same frame's depth
    /// buffer with the same view.
    fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        view_proj: glam::Mat4,
        camera_pos: glam::Vec3,
        light_view_proj: glam::Mat4,
        sun_dir: glam::Vec3,
        sunlight: f32,
    ) {
        queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&GpuCamera {
                view_proj: view_proj.to_cols_array_2d(),
                camera_pos: camera_pos.into(),
                _pad0: 0.0,
            }),
        );
        queue.write_buffer(
            &self.light_buffer,
            0,
            bytemuck::bytes_of(&GpuLight {
                light_view_proj: light_view_proj.to_cols_array_2d(),
                sun_dir: sun_dir.into(),
                sunlight,
            }),
        );
    }

    /// Computes the directional light's view-projection matrix for the
    /// shadow pass: an orthographic frustum (directional lights have no
    /// meaningful perspective/position) that comfortably bounds the whole
    /// simulated world, looking along the fixed `sun_dir`.
    ///
    /// `world_half_extent` is the caller's own world-bounds constant (e.g.
    /// `app/render.rs`'s `WORLD_BOUNDS`) — passed in rather than duplicated,
    /// so this module never invents its own notion of "how big is the
    /// world."
    fn compute_light_view_proj(sun_dir: glam::Vec3, world_half_extent: f32) -> glam::Mat4 {
        let light_distance = world_half_extent * 2.5;
        let eye = -sun_dir * light_distance;
        let view = glam::Mat4::look_to_rh(eye, sun_dir, glam::Vec3::Y);
        // Slightly larger than the world itself so bones near the boundary
        // don't clip out of the shadow frustum.
        let half = world_half_extent * 1.15;
        let near = 0.1;
        let far = light_distance * 2.0;
        let proj = glam::Mat4::orthographic_rh(-half, half, -half, half, near, far);
        proj * view
    }

    /// Renders the lit, depth-correct organism capsules for this frame.
    ///
    /// `target_view` must be the current swapchain surface view; existing
    /// colour data (field background) is preserved (`LoadOp::Load`). This
    /// is the first pass in the frame to touch this renderer's depth
    /// buffer — it clears it.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        instances: &[CapsuleInstance],
        screen_size: [f32; 2],
        view_proj: glam::Mat4,
        camera_pos: glam::Vec3,
        sunlight: f32,
        world_half_extent: f32,
        viewport: Option<[u32; 4]>,
    ) {
        self.resize(device, screen_size[0] as u32, screen_size[1] as u32);
        let sun_dir = glam::Vec3::new(0.4, -0.3, -0.85).normalize();
        let light_view_proj = Self::compute_light_view_proj(sun_dir, world_half_extent);
        self.update_uniforms(
            queue,
            view_proj,
            camera_pos,
            light_view_proj,
            sun_dir,
            sunlight,
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("OrganismEncoder"),
        });

        // Instance buffer is uploaded once here and reused by both the
        // shadow pass and the main pass below (same bones, same frame).
        let instance_buffer = (!instances.is_empty()).then(|| {
            Self::ensure_instance_buffer(
                device,
                queue,
                &mut self.instance_buffer,
                &mut self.instance_capacity,
                "CapsuleInstanceBuffer",
                instances,
            )
        });

        // Shadow pass: depth-only render of the same instances from the
        // light's point of view, before the main color+depth pass.
        if let Some(instance_buffer) = instance_buffer {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OrganismShadowPass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            shadow_pass.set_pipeline(&self.shadow_pipeline);
            shadow_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            shadow_pass.set_vertex_buffer(0, self.mesh_vertex_buffer.slice(..));
            shadow_pass.set_vertex_buffer(1, instance_buffer.slice(..));
            shadow_pass
                .set_index_buffer(self.mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            shadow_pass.draw_indexed(0..self.mesh_index_count, 0, 0..instances.len() as u32);
        }

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OrganismPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some([vx, vy, vw, vh]) = viewport {
                if vw > 0 && vh > 0 {
                    rpass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                    rpass.set_scissor_rect(vx, vy, vw, vh);
                }
            }

            if let Some(instance_buffer) = instance_buffer {
                rpass.set_pipeline(&self.pipeline);
                rpass.set_bind_group(0, &self.camera_bind_group, &[]);
                rpass.set_bind_group(1, &self.highlight_color_bind_group, &[]);
                rpass.set_bind_group(2, &self.shadow_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.mesh_vertex_buffer.slice(..));
                rpass.set_vertex_buffer(1, instance_buffer.slice(..));
                rpass.set_index_buffer(self.mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.mesh_index_count, 0, 0..instances.len() as u32);
            }
        }

        queue.submit(Some(encoder.finish()));
    }

    /// Renders a highlight outline for the provided (already slightly
    /// inflated by the caller, matching `SdfSkinRenderer::render_highlight`'s
    /// existing convention) instances — depth-tested against, but not
    /// overwriting, whatever `render()` already wrote to the depth buffer
    /// this frame (must be called after `render()` in the same frame).
    #[allow(clippy::too_many_arguments)]
    pub fn render_highlight(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_view: &wgpu::TextureView,
        instances: &[CapsuleInstance],
        color: [f32; 4],
        screen_size: [f32; 2],
        viewport: Option<[u32; 4]>,
    ) {
        if instances.is_empty() {
            return;
        }
        // Camera and light uniforms (including `light_view_proj`, which the
        // light doesn't move within a frame) were already written by the
        // preceding `render()` call this same frame — only the highlight
        // color changes here.
        let _ = screen_size;
        queue.write_buffer(
            &self.highlight_color_buffer,
            0,
            bytemuck::cast_slice(&color),
        );

        let instance_buffer = Self::ensure_instance_buffer(
            device,
            queue,
            &mut self.highlight_instance_buffer,
            &mut self.highlight_instance_capacity,
            "CapsuleHighlightInstanceBuffer",
            instances,
        );

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("OrganismHighlightEncoder"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OrganismHighlightPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some([vx, vy, vw, vh]) = viewport {
                if vw > 0 && vh > 0 {
                    rpass.set_viewport(vx as f32, vy as f32, vw as f32, vh as f32, 0.0, 1.0);
                    rpass.set_scissor_rect(vx, vy, vw, vh);
                }
            }

            rpass.set_pipeline(&self.highlight_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_bind_group(1, &self.highlight_color_bind_group, &[]);
            rpass.set_bind_group(2, &self.shadow_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.mesh_vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, instance_buffer.slice(..));
            rpass.set_index_buffer(self.mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..self.mesh_index_count, 0, 0..instances.len() as u32);
        }

        queue.submit(Some(encoder.finish()));
    }
}
