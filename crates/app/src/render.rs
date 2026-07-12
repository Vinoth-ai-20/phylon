//! # Frame Rendering
//!
//! This module owns [`PhylonApp::render`], called once per OS-requested
//! frame redraw: it advances the simulation to catch up to real time (via
//! `simulation::advance_simulation_for_frame`), gathers ECS state into
//! renderable instances, and drives the GPU render passes (field/heatmap
//! background, organism capsules, debug badges, selection highlights, egui)
//! before presenting the frame.
//!
//! See `render::world_instances` and `render::organism_visuals` for how ECS
//! state becomes renderable instance data — this file itself is the
//! per-frame orchestration that calls into them and issues the actual GPU
//! render passes.

use anyhow::Result;

use crate::app::PhylonApp;

/// Organism visual-instance builders — the "what to draw" half of this
/// file's per-node/per-spring loops. See its own module doc comment for the
/// extraction discipline. `pub(crate)` so `app.rs`'s `pick_entity` can reuse
/// its pellet-radius constants for ray-vs-capsule picking, rather than
/// duplicating those literals.
pub(crate) mod organism_visuals;
/// Per-frame world-instance gathering — the per-node/per-spring/per-pellet
/// orchestration that calls `organism_visuals`'s builders, kept separate
/// from `render()` itself so gathering-what-to-draw and issuing-the-actual-
/// GPU-passes can be read (and changed) independently.
pub(crate) mod world_instances;

impl PhylonApp {
    /// Renders one frame: advances the simulation to catch up to real time,
    /// gathers ECS state into render instances, and issues the GPU render
    /// passes (field/heatmap background, organism capsules, debug badges,
    /// selection highlights, egui), presenting the result.
    ///
    /// Simulation physics runs at a fixed, deterministic timestep (`DT`, the
    /// configured [`common::TickRate`]) to keep biological processes (energy
    /// decay, neuron membrane potentials) numerically stable regardless of
    /// how the monitor's refresh rate fluctuates. This method decouples the
    /// render framerate from the biological tick rate using a
    /// fixed-timestep accumulator, delegated to
    /// `simulation::advance_simulation_for_frame`:
    ///
    /// $$ t_{accum} = t_{accum} + (speed \times \Delta t_{frame}) $$
    ///
    /// While $t_{accum} \ge 1.0$, the engine calls `update_simulation()` to
    /// step the ECS forward by the fixed `DT` seconds, decrementing
    /// $t_{accum}$. Once caught up, it builds the `wgpu::CommandEncoder`,
    /// executes the field/splat and organism render passes, and renders the
    /// `egui` UI on top.
    pub(crate) fn render(&mut self) -> Result<()> {
        if self.gpu.is_none() || self.physics_compute.is_none() {
            return Ok(());
        }

        // 1. Camera Tracking

        // Advances any in-progress Frame Selected/Frame All transition —
        // see `last_camera_animation_instant`'s own doc comment for why
        // this uses its own dedicated timing field.
        let now = std::time::Instant::now();
        let camera_animation_dt = (now - self.last_camera_animation_instant).as_secs_f32();
        self.last_camera_animation_instant = now;
        self.ui.tick_frame_animation(camera_animation_dt);

        if let Some(tracked) = self.ui.tracked_entity {
            if let Ok(node) = self
                .world
                .ecs
                .query::<&physics::ParticleNode>()
                .get(&self.world.ecs, tracked)
            {
                // Smoothly follow the target — only meaningful in `Orbit`
                // mode (lerps the focus point); `Fly` mode has no
                // equivalent "focus point" concept to follow.
                if let ui::camera::CameraController::Orbit(orbit) = &mut self.ui.camera_controller {
                    orbit.focus = orbit.focus.lerp(node.position, 0.1);
                }
            } else {
                // Entity no longer exists (e.g. died), drop tracking
                self.ui.set_follow(None);
            }
        }

        // 2-5. Fixed-timestep simulation catch-up + this frame's perf/
        // demographic telemetry — see
        // `simulation::advance_simulation_for_frame`'s doc comment.
        self.advance_simulation_for_frame();

        // 6. Gather rendering instances — see `render::world_instances`'s
        // module doc comment.
        let world_instances = self.gather_world_render_instances();
        let debug_instances = world_instances.debug_instances;
        let capsule_instances = world_instances.capsule_instances;
        let hover_bones = world_instances.hover_bones;
        let selected_bones = world_instances.selected_bones;

        let gpu = self.gpu.as_mut().unwrap();

        // Prepare render frame
        let output = match gpu.surface.as_ref().unwrap().get_current_texture() {
            Ok(tex) => tex,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                gpu.surface
                    .as_ref()
                    .unwrap()
                    .configure(&gpu.device, gpu.config.as_ref().unwrap());
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(()),
            Err(e) => return Err(anyhow::anyhow!("surface error: {e}")),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut central_rect_px = None;

        let mut full_output = None;
        let mut egui_context = None;

        let mut interaction = ui::CanvasInteraction::default();
        let mut scale = 1.0;

        let mut ui_actions = Vec::new();

        if let (Some(egui_state), Some(window)) = (&mut self.egui_state, &self.window) {
            let raw_input = egui_state.take_egui_input(window);
            let ctx = egui_state.egui_ctx().clone();

            let output = ctx.run(raw_input, |ctx| {
                let (canvas_interact, acts) =
                    ui::render_ui(ctx, &mut self.app_state, &mut self.world, &mut self.ui);
                ui_actions.extend(acts);
                interaction = canvas_interact;
            });

            scale = window.scale_factor() as f32;

            egui_state.handle_platform_output(window, output.platform_output.clone());

            let ui_rect = interaction.rect;

            let x = (ui_rect.min.x * scale).round() as u32;
            let y = (ui_rect.min.y * scale).round() as u32;
            let mut w = (ui_rect.width() * scale).round() as u32;
            let mut h = (ui_rect.height() * scale).round() as u32;

            if let Some(config) = gpu.config.as_ref() {
                if x + w > config.width {
                    w = config.width.saturating_sub(x);
                }
                if y + h > config.height {
                    h = config.height.saturating_sub(y);
                }
            }

            if w > 0 && h > 0 {
                central_rect_px = Some([x, y, w, h]);
                self.ui.canvas_rect = central_rect_px;
            }

            if let Some(pos) = interaction.hover_pos {
                self.ui.current_hover_pos = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            } else {
                self.ui.current_hover_pos = None;
            }

            full_output = Some(output);
            egui_context = Some(ctx);
        }

        // Process native interactions from the transparent canvas, routed
        // through the one canonical `ui::viewport_input` layer (egui
        // adapter + `ViewportController`) rather than interpreting
        // `CanvasInteraction` directly here, so there is exactly one place
        // that translates raw input into camera/viewport behavior.
        let viewport_input =
            ui::viewport_input::ViewportInput::from_canvas_interaction(&interaction);
        ui::viewport_input::apply_to_camera(&mut self.ui, &viewport_input, scale);

        if interaction.clicked {
            if let Some(pos) = interaction.click_pos {
                self.ui.pending_click = Some(common::Vec2::new(pos.x * scale, pos.y * scale));
            }
        }

        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        // Get sunlight for background color — the same scalar also drives
        // the organism renderer's directional-light intensity below, so
        // day/night lighting on the background and on organisms stays in
        // sync.
        let mut clear_color = wgpu::Color {
            r: 0.001,
            g: 0.001,
            b: 0.004,
            a: 1.0,
        };
        let mut sunlight = 1.0f32;
        if let Some(atmosphere) = self
            .world
            .ecs
            .get_resource::<metabolism::GlobalAtmosphere>()
        {
            sunlight = atmosphere.sunlight;
            let s = atmosphere.sunlight as f64;
            clear_color = wgpu::Color {
                r: 0.001 * (1.0 - s) + 0.010 * s,
                g: 0.001 * (1.0 - s) + 0.070 * s,
                b: 0.004 * (1.0 - s) + 0.184 * s,
                a: 1.0,
            };
        }

        let heatmap_state = self
            .world
            .ecs
            .get_resource::<ui::HeatmapState>()
            .cloned()
            .unwrap_or_default();
        let mut field_view_to_render: Option<&wgpu::TextureView> = None;

        // Use the cropped central viewport (not the full window) so the
        // heatmap's world-space<->screen-space conversion matches the
        // organism (sdf_skin) projection below and the two don't drift
        // apart ("parallax") when panning with a sidebar/toolbar open.
        let (screen_w, screen_h) = central_rect_px
            .map(|[_, _, w, h]| (w as f32, h as f32))
            .unwrap_or_else(|| {
                gpu.config
                    .as_ref()
                    .map(|c| (c.width as f32, c.height as f32))
                    .unwrap_or((1280.0, 720.0))
            });

        // Half-extent of the simulation world in world-space units — must
        // match `field_overlay.wgsl`'s `world_bounds` (below) exactly, since
        // that shader maps screen->world->grid-UV assuming this same value.
        // The Glucose/ATP splat step below maps organism positions into grid
        // space using this same constant so the two stay in registration;
        // using the viewport's pixel size there instead (as it previously
        // did) scaled the mapping by an arbitrary, resize-dependent factor,
        // which is what made the heatmap appear misaligned/tiled well
        // outside the actual world bounds.
        const WORLD_BOUNDS: f32 = 1500.0;

        // Computed above the heatmap/field section so `FieldConfig`'s
        // plane-slice sampler can reuse the same `Camera3d`/`view_proj`
        // every other renderer uses, rather than needing its own separate
        // flat 2D camera projection.
        let aspect = if screen_h > 0.0 {
            screen_w / screen_h
        } else {
            1.0
        };
        let camera = self.ui.camera();
        let view_proj = camera.view_proj(aspect);
        let inv_view_proj = view_proj.inverse();

        // For Glucose/ATP, min/max are recomputed fresh below from this
        // frame's actual values (rather than using `heatmap_state`'s stored
        // min/max, which default to a fixed 0.0..1.0 that nothing updates —
        // organism glucose/ATP commonly run into the tens of thousands, so
        // normalizing against 0..1 clipped everything to the top of the
        // colormap instead of showing a gradient).
        let mut dynamic_min = heatmap_state.min_val;
        let mut dynamic_max = heatmap_state.max_val;

        if heatmap_state.active != ui::ActiveHeatmap::None {
            match heatmap_state.active {
                ui::ActiveHeatmap::Pheromones => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(0));
                    }
                }
                ui::ActiveHeatmap::EnergyDensity => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(1));
                    }
                }
                ui::ActiveHeatmap::O2 => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(2));
                    }
                }
                ui::ActiveHeatmap::CO2 => {
                    if let Some(diffusion) = self.diffusion_compute.as_ref() {
                        field_view_to_render = Some(diffusion.current_layer_view(3));
                    }
                }
                ui::ActiveHeatmap::Glucose | ui::ActiveHeatmap::ATP => {
                    if let Some(splat_compute) = self.splat_compute.as_mut() {
                        let mut splats = Vec::new();
                        let mut sample_max = 0.0f32;
                        let mut query = self
                            .world
                            .ecs
                            .query::<(&physics::ParticleNode, &metabolism::ChemicalEconomy)>();
                        for (node, chem) in query.iter(&self.world.ecs) {
                            let value = if heatmap_state.active == ui::ActiveHeatmap::Glucose {
                                chem.glucose
                            } else {
                                chem.atp
                            };
                            sample_max = sample_max.max(value);

                            // Map world space to grid space — must use the
                            // same WORLD_BOUNDS the fragment shader assumes,
                            // not the viewport's pixel size (see comment on
                            // WORLD_BOUNDS above).
                            let grid_x = (node.position.x / WORLD_BOUNDS) * 128.0 + 128.0;
                            let grid_y = (-node.position.y / WORLD_BOUNDS) * 128.0 + 128.0;

                            splats.push(rendering::GpuSplat {
                                grid_pos: [grid_x, grid_y],
                                value,
                                grid_radius: 8.0,
                            });
                        }
                        splat_compute.step(&gpu.device, &gpu.queue, &splats);
                        field_view_to_render = Some(&splat_compute.view);
                        dynamic_min = 0.0;
                        dynamic_max = sample_max.max(1.0);
                    }
                }
                _ => {}
            }
        }

        if let (Some(field_renderer), Some(view_to_render)) =
            (self.field_renderer.as_ref(), field_view_to_render)
        {
            field_renderer.update_config(
                &gpu.queue,
                rendering::FieldConfig {
                    inv_view_proj: inv_view_proj.to_cols_array_2d(),
                    min_val: dynamic_min,
                    max_val: dynamic_max,
                    slice_z: 0.0,
                    colormap: heatmap_state.colormap,
                    world_bounds: [WORLD_BOUNDS, WORLD_BOUNDS],
                    _pad: [0.0; 2],
                },
            );

            field_renderer.render(
                &gpu.device,
                &mut encoder,
                &view,
                view_to_render,
                central_rect_px,
                clear_color,
            );
        } else if let Some(field_renderer) = self.field_renderer.as_ref() {
            // Render nothing but clear the screen
            field_renderer.update_config(
                &gpu.queue,
                rendering::FieldConfig {
                    inv_view_proj: inv_view_proj.to_cols_array_2d(),
                    min_val: 0.0,
                    max_val: -1.0, // Ensures range < 0.0001, alpha = 0.0
                    slice_z: 0.0,
                    colormap: heatmap_state.colormap,
                    world_bounds: [WORLD_BOUNDS, WORLD_BOUNDS],
                    _pad: [0.0; 2],
                },
            );
            if let Some(diffusion) = self.diffusion_compute.as_ref() {
                field_renderer.render(
                    &gpu.device,
                    &mut encoder,
                    &view,
                    diffusion.current_layer_view(0),
                    central_rect_px,
                    clear_color,
                );
            }
        }

        // Submit the field renderer (which clears the screen and draws the background) BEFORE
        // the other renderers, which rely on LoadOp::Load and submit their own encoders.
        gpu.queue.submit(std::iter::once(encoder.finish()));

        let (view_w, view_h) = (screen_w, screen_h);
        // The depth attachment must exactly match the color attachment's
        // (`target_view`, the full swapchain texture) extent — `view_w`/
        // `view_h` is the cropped *viewport* rect (used correctly above for
        // the projection's aspect ratio), which is smaller whenever a
        // sidebar/panel is open, and wgpu rejects a render pass whose
        // attachments have differing sizes.
        let surface_size = gpu
            .config
            .as_ref()
            .map(|c| [c.width as f32, c.height as f32])
            .unwrap_or([view_w, view_h]);

        // ── Organism rendering — mesh-based capsule instancing. Always run
        // if there are bones.
        if !capsule_instances.is_empty() {
            if let Some(organism_renderer) = self.organism_renderer.as_mut() {
                let clip_plane = self.ui.clip_plane;
                organism_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &capsule_instances,
                    surface_size,
                    view_proj,
                    camera.position,
                    rendering::ClipPlane {
                        enabled: clip_plane.enabled,
                        height: clip_plane.height,
                        keep_above: clip_plane.keep_above,
                    },
                    sunlight,
                    WORLD_BOUNDS,
                    central_rect_px,
                );
            }
        }

        // `debug_instances` (Health/Disease/Category badges — Priority
        // 2/3/5 biological signals) must draw *before* the hover/selection
        // highlight, not after: drawing debug instances last would let a
        // low-health ring or disease badge paint *over* (and visually
        // obscure) the Priority-1 selection/hover outline wherever they
        // overlap — a violation of "higher-priority signals must always
        // remain readable." Selection/hover always paints last, below.
        // Debug badges are camera-facing billboards, depth-tested against
        // `OrganismRenderer`'s shared depth buffer — only rendered once
        // that renderer exists (it owns the only depth buffer in the
        // frame).
        if !debug_instances.is_empty() {
            if let (Some(debug_renderer), Some(organism_renderer)) = (
                self.debug_renderer.as_mut(),
                self.organism_renderer.as_ref(),
            ) {
                debug_renderer.render(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &debug_instances,
                    organism_renderer.depth_view(),
                    view_proj,
                    camera.right(),
                    camera.up(),
                    central_rect_px,
                );
            }
        }

        if !hover_bones.is_empty() {
            if let Some(organism_renderer) = self.organism_renderer.as_mut() {
                organism_renderer.render_highlight(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &hover_bones,
                    [0.0, 1.0, 0.0, 1.0],
                    surface_size,
                    central_rect_px,
                );
            }
        }

        if !selected_bones.is_empty() {
            if let Some(organism_renderer) = self.organism_renderer.as_mut() {
                // Deliberately a fixed value, not a wall-clock sine
                // oscillation and not Health-fraction-driven. This
                // project's visual-language rules prohibit decorative
                // pulsing — every animation must be driven by a real,
                // current simulation value, and a wall-clock oscillation
                // would carry no biological meaning, animating identically
                // whether the selected organism is thriving or dying. It
                // is also deliberately *not* tied to Health: `docs/design/
                // biological_visual_language.md`'s numeric priority
                // hierarchy places Selection at Priority 1, the highest —
                // tying its alpha to Health (Priority 2) would blur that
                // ordering and create a second, competing Health signal
                // (opacity-by-health-fraction is already used elsewhere for
                // that). A static, maximum-alpha outline keeps Selection
                // unambiguous and undiminished.
                let pulse = 1.0;
                organism_renderer.render_highlight(
                    &gpu.device,
                    &gpu.queue,
                    &view,
                    &selected_bones,
                    [1.0, 1.0, 1.0, pulse],
                    surface_size,
                    central_rect_px,
                );
            }
        }

        if let (Some(egui_renderer), Some(window), Some(output), Some(ctx)) = (
            &mut self.egui_renderer,
            &self.window,
            full_output,
            egui_context,
        ) {
            let clipped_primitives = ctx.tessellate(output.shapes, window.scale_factor() as f32);
            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [
                    gpu.config.as_ref().map(|c| c.width).unwrap_or(1280),
                    gpu.config.as_ref().map(|c| c.height).unwrap_or(720),
                ],
                pixels_per_point: window.scale_factor() as f32,
            };

            let mut egui_encoder =
                gpu.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("egui_encoder"),
                    });

            for (id, image_delta) in &output.textures_delta.set {
                egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, image_delta);
            }

            egui_renderer.update_buffers(
                &gpu.device,
                &gpu.queue,
                &mut egui_encoder,
                &clipped_primitives,
                &screen_descriptor,
            );

            {
                let mut render_pass = egui_encoder
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("egui_render_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    })
                    .forget_lifetime();
                egui_renderer.render(&mut render_pass, &clipped_primitives, &screen_descriptor);
            }

            gpu.queue.submit(std::iter::once(egui_encoder.finish()));

            for id in &output.textures_delta.free {
                egui_renderer.free_texture(id);
            }
        }

        // Screenshot/recording readback — must happen here, after the egui
        // pass has been submitted (so captured frames include the UI) but
        // before `output.present()` below, since `output.texture` is only
        // valid until it's presented.
        let capture_size = gpu
            .config
            .as_ref()
            .map(|c| (c.width, c.height))
            .unwrap_or((0, 0));
        let capture_format = gpu
            .config
            .as_ref()
            .map(|c| c.format)
            .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb);

        if self.pending_screenshot && capture_size.0 > 0 && capture_size.1 > 0 {
            self.pending_screenshot = false;
            match crate::capture::read_texture_to_image(
                &gpu.device,
                &gpu.queue,
                &output.texture,
                capture_format,
                capture_size.0,
                capture_size.1,
            )
            .map(|img| crate::capture::save_screenshot(&img))
            {
                Some(Ok(path)) => self.ui.push_toast(
                    format!("Saved screenshot to {}", path.display()),
                    ui::ToastSeverity::Success,
                    3.0,
                ),
                Some(Err(e)) => {
                    tracing::error!("Failed to save screenshot: {e}");
                    self.ui.push_toast(
                        format!("Failed to save screenshot: {e}"),
                        ui::ToastSeverity::Error,
                        5.0,
                    );
                }
                None => tracing::error!("Screenshot readback produced no image"),
            }
        }

        // Chart PNG export readback — same timing constraint as the
        // screenshot above (must run before `present()`), just cropped to
        // one Metrics chart's rect instead of the whole window.
        if let Some((x, y, width, height)) = self.pending_chart_export.take() {
            if capture_size.0 > 0 && capture_size.1 > 0 {
                match crate::capture::read_texture_to_image(
                    &gpu.device,
                    &gpu.queue,
                    &output.texture,
                    capture_format,
                    capture_size.0,
                    capture_size.1,
                ) {
                    Some(img) => {
                        let x = x.min(img.width().saturating_sub(1));
                        let y = y.min(img.height().saturating_sub(1));
                        let width = width.min(img.width() - x).max(1);
                        let height = height.min(img.height() - y).max(1);
                        match crate::capture::save_chart_png(&img, x, y, width, height) {
                            Ok(path) => self.ui.push_toast(
                                format!("Saved chart to {}", path.display()),
                                ui::ToastSeverity::Success,
                                3.0,
                            ),
                            Err(e) => {
                                tracing::error!("Failed to save chart PNG: {e}");
                                self.ui.push_toast(
                                    format!("Chart export failed: {e}"),
                                    ui::ToastSeverity::Error,
                                    5.0,
                                );
                            }
                        }
                    }
                    None => tracing::error!("Chart export readback produced no image"),
                }
            }
        }

        if let Some(recording) = self.recording.as_mut() {
            if capture_size.0 > 0
                && capture_size.1 > 0
                && recording.last_capture.elapsed() >= crate::capture::CAPTURE_INTERVAL
            {
                if let Some(img) = crate::capture::read_texture_to_image(
                    &gpu.device,
                    &gpu.queue,
                    &output.texture,
                    capture_format,
                    capture_size.0,
                    capture_size.1,
                ) {
                    recording.frames.push(img);
                    recording.last_capture = std::time::Instant::now();
                }
            }
        }

        // Hit the recording cap — stop and save. Checked as a separate step
        // (rather than inline above) so `self.recording.take()` and
        // `self.ui.push_toast(...)` are plain disjoint-field accesses, not a
        // `&mut self` method call, which would conflict with the `gpu`
        // borrow (from `self.gpu.as_mut()`) still live in this scope.
        if matches!(&self.recording, Some(r) if r.frames.len() >= crate::capture::MAX_RECORDING_FRAMES)
        {
            let recording = self.recording.take().unwrap();
            self.ui.recording_active = false;
            self.ui.recording_started_at = None;
            crate::capture::finish_recording(&recording.frames, &mut self.ui);
        }

        output.present();

        self.handle_menu_actions(ui_actions);

        Ok(())
    }
}
