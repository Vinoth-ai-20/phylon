//! Measures `ecology::foraging_system` — the most complex per-tick ecology
//! system, an O(N) broad-phase spatial-grid rebuild plus nested predation/
//! consumption resolution — since it runs every simulation tick and has no
//! other benchmark coverage in this crate. This benchmark exists to give any
//! future optimization work real before/after numbers to measure against; it
//! does not itself optimize anything.

use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use ecology::{Diet, ResourceSpatialGrids};
use metabolism::{ChemicalEconomy, GlobalAtmosphere};
use physics::ParticleNode;

fn sample_chem() -> ChemicalEconomy {
    ChemicalEconomy {
        glucose: 500.0,
        o2: 300.0,
        co2: 50.0,
        atp: 400.0,
        max_glucose: 1000.0,
        max_o2: 1000.0,
        max_co2: 1000.0,
        max_atp: 1000.0,
    }
}

/// Builds a `World` with `n` organisms spread across a 2D grid (so the
/// spatial broad-phase actually has to do meaningful bucketing work, not
/// just degenerate to one cell) and an empty `ResourceSpatialGrids` — this
/// benchmark isolates organism-vs-organism predation cost specifically,
/// the phase the re-audit found least measured.
fn build_world_with_organisms(n: u32) -> World {
    let mut world = World::new();
    world.insert_resource(GlobalAtmosphere::default());
    world.insert_resource(events::TimedEffects::default());
    world.insert_resource(ResourceSpatialGrids::new(50.0));

    let side = (n as f32).sqrt().ceil() as u32;
    for i in 0..n {
        let x = (i % side) as f32 * 30.0;
        let y = (i / side) as f32 * 30.0;
        let diet = match i % 4 {
            0 => Diet::Carnivore,
            1 => Diet::Herbivore,
            2 => Diet::Omnivore,
            _ => Diet::Producer,
        };
        world.spawn((
            ParticleNode::new(common::Vec3::new(x, y, 0.0), 1.0, 0, i),
            sample_chem(),
            diet,
        ));
    }
    world
}

/// Benchmarks one `foraging_system` tick at increasing population sizes —
/// the population range spans typical (1,000) to stress-test (10,000)
/// counts, matching the range `metabolism_parallel`'s existing benchmark
/// already uses for comparability.
fn bench_foraging_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("foraging_system");

    for &count in &[1_000u32, 5_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("organisms", count), &count, |b, &count| {
            b.iter_batched(
                || build_world_with_organisms(count),
                |mut world| {
                    world.run_system_once(ecology::foraging_system);
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_foraging_scaling);
criterion_main!(benches);
