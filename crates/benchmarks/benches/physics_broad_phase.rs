//! Phase 8, Epic 8.10's own disclosed gap, closed here: the roadmap's
//! stated verification for the GPU steric-hindrance broad-phase moving
//! from a dense `128x128` grid to a fixed-size 3D spatial hash
//! (ADR-P8-04) asked for "a new GPU broad-phase benchmark... at multiple
//! population sizes, before/after" — deferred at the time since the ADR's
//! core memory-safety concern (avoiding a ~128x blowup) was satisfied by
//! construction (the hash table's total bucket count was chosen to exactly
//! match the pre-8.10 dense grid's, not measured). This benchmark gives a
//! real number for the hash-based design's own steady-state cost, so a
//! future epic that revisits or tunes the broad-phase has a baseline to
//! compare against, per this project's own "profile before optimizing"
//! rule — it does not itself optimize anything.
//!
//! Mirrors `foraging_scaling.rs`'s own structure and population range
//! (1,000 / 5,000 / 10,000) for direct comparability with the CPU-side
//! broad-phase benchmark, even though the two measure different systems.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use gpu::physics_pipeline::{GpuParticleNode, GpuPhysicsSpring, PhysicsComputePipeline};

/// Requests a minimal headless `wgpu` device — no surface, no rendering
/// pipeline — mirroring `app::app::init_gpu_headless`'s own adapter/device
/// request incantation (the only other place in this workspace that needs
/// a surfaceless GPU device).
fn create_headless_gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no GPU adapter available for benchmarking — see ADR-P8-09's CI-posture notes");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("BenchmarkDevice"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
        },
        None,
    ))
    .expect("failed to create a GPU device for benchmarking");

    (device, queue)
}

/// Builds `n` nodes spread across a 2D grid (so the spatial-hash broad-
/// phase actually has to do meaningful bucketing work, matching
/// `foraging_scaling.rs`'s own spread pattern) plus exactly one `Passive`
/// (force-free) spring — `PhysicsComputePipeline::dispatch` early-returns
/// without doing any GPU work if either `nodes` or `springs` is empty, and
/// this benchmark's interest is specifically the steric-hindrance
/// broad-phase (`bin_nodes`/`integrate`'s neighbor scan), not spring-force
/// computation, so one inert spring is enough to make the real dispatch
/// path run.
fn build_nodes_and_springs(n: u32) -> (Vec<GpuParticleNode>, Vec<GpuPhysicsSpring>) {
    let side = (n as f32).sqrt().ceil() as u32;
    let mut nodes = Vec::with_capacity(n as usize);
    for i in 0..n {
        let x = (i % side) as f32 * 30.0;
        let y = (i / side) as f32 * 30.0;
        nodes.push(GpuParticleNode {
            position: [x, y, 0.0],
            _pad0: 0.0,
            velocity: [0.0, 0.0, 0.0],
            _pad1: 0.0,
            force: [0.0, 0.0, 0.0],
            _pad2: 0.0,
            mass: 1.0,
            // Spread across ~100 distinct organism ids so the repulsion
            // loop exercises both the same-organism and cross-organism
            // branches, matching a real population's mix.
            organism_id: i % 100,
            _pad3: [0.0, 0.0],
        });
    }

    let springs = vec![GpuPhysicsSpring {
        node_a: 0,
        node_b: n.saturating_sub(1).max(1),
        constraint_type: 2, // Passive
        rest_length: 30.0,
        base_length: 30.0,
        stiffness: 0.0,
        damping: 0.0,
        actuation_amplitude: 0.0,
        actuation_phase: 0.0,
        breaking_strain: 0.0,
        is_fin: 0,
        _padding_2: 0,
    }];

    (nodes, springs)
}

/// Benchmarks one full `PhysicsComputePipeline::compute_step` (all 5
/// passes, including the spatial-hash broad-phase bin/scan) at increasing
/// population sizes. A single `PhysicsComputePipeline`/node-buffer pair is
/// reused across all timed iterations within a population size (matching
/// `compute_step`'s own real usage pattern: buffers grow once, then stay
/// steady-state) — an untimed warm-up call absorbs the one-time buffer-
/// allocation cost before timing begins, so what's measured is the
/// steady-state per-tick GPU cost, not allocation.
fn bench_physics_broad_phase(c: &mut Criterion) {
    let (device, queue) = create_headless_gpu();

    let mut group = c.benchmark_group("physics_broad_phase");
    // GPU dispatch + blocking readback is much slower per-iteration than
    // the CPU-side benchmarks in this crate; a smaller sample size keeps
    // total benchmark run time reasonable without sacrificing a stable
    // estimate.
    group.sample_size(20);

    for &count in &[1_000u32, 5_000, 10_000] {
        let (nodes, springs) = build_nodes_and_springs(count);
        let mut pipeline = PhysicsComputePipeline::new(&device);

        // Warm-up: absorbs `ensure_capacity`'s one-time buffer allocation.
        let _ = pipeline.compute_step(&device, &queue, &nodes, &springs, 1.0 / 60.0, 0.0, None);

        group.bench_with_input(BenchmarkId::new("nodes", count), &count, |b, _| {
            b.iter(|| {
                pipeline.compute_step(&device, &queue, &nodes, &springs, 1.0 / 60.0, 0.0, None)
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_physics_broad_phase);
criterion_main!(benches);
