//! Confirms the rayon-parallelized `metabolism_system` actually accelerates
//! at scale, not just compiles — distinct from the correctness check already
//! covered by
//! `metabolism::tests::metabolism_is_deterministic_regardless_of_thread_count`.

use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use metabolism::{Age, ChemicalEconomy, GlobalAtmosphere, Metabolism};
use physics::ParticleNode;

fn build_world_with_organisms(n: u32) -> World {
    let mut world = World::new();
    world.insert_resource(GlobalAtmosphere::default());
    for i in 0..n {
        world.spawn((
            ParticleNode::new(common::Vec3::new(i as f32 * 3.0, 0.0, 0.0), 1.0, 0, i),
            ChemicalEconomy {
                glucose: 500.0,
                o2: 300.0,
                co2: 50.0,
                atp: 400.0,
                max_glucose: 1000.0,
                max_o2: 1000.0,
                max_co2: 1000.0,
                max_atp: 1000.0,
            },
            Age {
                ticks: 0,
                max_lifespan: 10_000_000,
            },
            Metabolism {
                mass: 5.0 + (i as f32 * 0.01),
                base_rate: 0.001,
                is_plant: i % 3 == 0,
            },
        ));
    }
    world
}

/// Benchmarks one `metabolism_system` tick at increasing population sizes,
/// once pinned to a single rayon thread (the pre-Epic-6 serial baseline)
/// and once on the default multi-threaded pool.
fn bench_metabolism_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("metabolism_system");

    for &count in &[1_000u32, 10_000, 50_000] {
        group.bench_with_input(BenchmarkId::new("1_thread", count), &count, |b, &count| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build()
                .unwrap();
            b.iter_batched(
                || build_world_with_organisms(count),
                |mut world| {
                    pool.install(|| {
                        world.run_system_once(metabolism::metabolism_system);
                    });
                },
                criterion::BatchSize::LargeInput,
            );
        });

        group.bench_with_input(
            BenchmarkId::new("default_pool", count),
            &count,
            |b, &count| {
                b.iter_batched(
                    || build_world_with_organisms(count),
                    |mut world| {
                        world.run_system_once(metabolism::metabolism_system);
                    },
                    criterion::BatchSize::LargeInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_metabolism_scaling);
criterion_main!(benches);
