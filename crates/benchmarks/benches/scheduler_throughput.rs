use criterion::{criterion_group, criterion_main, Criterion};
use scheduler::{SimulationScheduler, SystemOrder};

/// Benchmarks the scheduler's tick throughput with no registered systems.
fn bench_empty_tick(c: &mut Criterion) {
    let cfg = config::PhylonConfig::default();
    let mut sched = SimulationScheduler::new(&cfg);

    c.bench_function("scheduler_empty_tick", |b| {
        b.iter(|| sched.step().expect("step must not fail"));
    });
}

/// Benchmarks the scheduler's tick throughput with one no-op system per phase.
fn bench_noop_systems(c: &mut Criterion) {
    let cfg = config::PhylonConfig::default();
    let mut sched = SimulationScheduler::new(&cfg);

    for &order in scheduler::SystemOrder::all_ordered() {
        sched.register(order, Box::new(|_tick, _bus| Ok(())));
    }

    c.bench_function("scheduler_noop_systems", |b| {
        b.iter(|| sched.step().expect("step must not fail"));
    });
}

criterion_group!(benches, bench_empty_tick, bench_noop_systems);
criterion_main!(benches);
