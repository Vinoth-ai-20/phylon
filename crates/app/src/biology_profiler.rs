//! # Hierarchical Per-Tick Biology Profiler (P9.1b)
//!
//! Measures, rather than assumes, which per-tick biology systems actually
//! dominate CPU time — the direct follow-on to the P9.1 finding that the
//! bottleneck is CPU-side ECS system execution, not GPU throughput or
//! diffusion. Where P9.1's own instrumentation measured two large,
//! monolithic "biology block" chunks, this module breaks the same call
//! sequence down into the 18 named categories a researcher would actually
//! want to compare (Growth, Development, Morphogenesis, Hormones,
//! Diffusion, Immune, Metabolism, Sensing, Brain Gather, GPU Brain Dispatch,
//! Behavior, Reproduction, Ecology, Disease, Predation, Spatial Update,
//! Cleanup, Events), each with average time, max time, percentage of tick,
//! call count, an entities-processed figure, and time-per-entity.
//!
//! **Opt-in, zero-cost when off:** gated behind the `PHYLON_BIOLOGY_PROFILE`
//! environment variable, checked once at startup via
//! `BiologyProfilerConfig::from_env` — matching the same pattern
//! `crate::motion_diagnostic`/`crate::behavior_validation` already use.
//! When unset, every instrumented call site pays exactly one `bool` check
//! and nothing else (no `Instant::now()`, no map insert).
//!
//! **Why this lives as a plain `PhylonApp` field, not an ECS `Resource`:**
//! unlike `motion_diagnostic`/`behavior_validation` (which are themselves
//! `bevy_ecs` systems run via `run_system_once`, and therefore need
//! `Resource` storage to persist state across ticks — see
//! `motion_diagnostic`'s own doc comment for why `Local` doesn't work
//! there), this profiler wraps the *call sites* inside
//! `simulation::update_simulation` directly. It is plain Rust code running
//! on `&mut PhylonApp`, so an ordinary struct field (the same pattern as
//! `sim_scratch`/`render_scratch`) persists across ticks with no ECS
//! involvement at all.
//!
//! **What "entities processed" means here, precisely:** rather than compute
//! a separate filtered query count per category (which would itself cost
//! real time — exactly the kind of per-tick overhead this profiler exists
//! to measure elsewhere, not add to), this module uses one cheap, per-tick
//! total-organism head count (computed once, only when the profiler is
//! enabled) as a shared context for every category's time-per-entity
//! figure. This is a deliberate, disclosed simplification: for a system
//! that only touches a subset of organisms (e.g. `growth_system` only
//! processes organisms still growing), the reported "ms/entity" is an
//! upper-bound-ish approximation scaled against the *whole* population, not
//! an exact per-touched-entity cost. This is still the right tool for the
//! P9.1b question ("which systems dominate, and does cost scale linearly
//! with population") — it is not the right tool for "what is the exact
//! marginal cost of processing one more growing organism specifically,"
//! which would need per-category filtered counts if ever needed.

use std::collections::HashMap;

/// One category in the hierarchical per-tick breakdown. Several individual
/// ECS systems are bucketed under one category where they address the same
/// biological concern — see `simulation::update_simulation`'s call sites
/// (search for `profiled!`) for the exact system-to-category mapping.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum BiologyCategory {
    Growth,
    Development,
    Morphogenesis,
    Hormones,
    Diffusion,
    Immune,
    Metabolism,
    Sensing,
    BrainGather,
    GpuBrainDispatch,
    Behavior,
    Reproduction,
    Ecology,
    Disease,
    Predation,
    SpatialUpdate,
    Cleanup,
    Events,
}

impl BiologyCategory {
    /// Every category, in the display order used by the hierarchical
    /// report — not alphabetical, but roughly "body plan and growth, then
    /// signaling, then cognition, then population-level concerns," so the
    /// unsorted report reads in a sensible order even before the by-cost
    /// sort is applied.
    pub(crate) const ALL: [BiologyCategory; 18] = [
        BiologyCategory::Growth,
        BiologyCategory::Development,
        BiologyCategory::Morphogenesis,
        BiologyCategory::Hormones,
        BiologyCategory::Diffusion,
        BiologyCategory::Immune,
        BiologyCategory::Metabolism,
        BiologyCategory::Sensing,
        BiologyCategory::BrainGather,
        BiologyCategory::GpuBrainDispatch,
        BiologyCategory::Behavior,
        BiologyCategory::Reproduction,
        BiologyCategory::Ecology,
        BiologyCategory::Disease,
        BiologyCategory::Predation,
        BiologyCategory::SpatialUpdate,
        BiologyCategory::Cleanup,
        BiologyCategory::Events,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            BiologyCategory::Growth => "Growth",
            BiologyCategory::Development => "Development",
            BiologyCategory::Morphogenesis => "Morphogenesis",
            BiologyCategory::Hormones => "Hormones",
            BiologyCategory::Diffusion => "Diffusion",
            BiologyCategory::Immune => "Immune",
            BiologyCategory::Metabolism => "Metabolism",
            BiologyCategory::Sensing => "Sensing",
            BiologyCategory::BrainGather => "Brain Gather",
            BiologyCategory::GpuBrainDispatch => "GPU Brain Dispatch",
            BiologyCategory::Behavior => "Behavior",
            BiologyCategory::Reproduction => "Reproduction",
            BiologyCategory::Ecology => "Ecology",
            BiologyCategory::Disease => "Disease",
            BiologyCategory::Predation => "Predation",
            BiologyCategory::SpatialUpdate => "Spatial Update",
            BiologyCategory::Cleanup => "Cleanup",
            BiologyCategory::Events => "Events",
        }
    }
}

/// Reads `PHYLON_BIOLOGY_PROFILE` once at startup; call at app startup and
/// carry the result on `PhylonApp`, so every per-tick call site pays only a
/// plain `bool` read.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BiologyProfilerConfig {
    pub(crate) enabled: bool,
}

impl BiologyProfilerConfig {
    pub(crate) fn from_env() -> Self {
        Self {
            enabled: std::env::var("PHYLON_BIOLOGY_PROFILE").is_ok(),
        }
    }
}

/// Accumulated timing for one category across the current reporting window.
#[derive(Default, Clone, Copy)]
struct CategoryStat {
    total_ms: f64,
    max_ms: f64,
    call_count: u64,
    /// Sum of the shared per-tick population figure across every call this
    /// category received this window — divided by `call_count` at report
    /// time to get an average "entities processed" figure. See this
    /// module's doc comment for why this is a shared per-tick count, not a
    /// category-specific filtered one.
    entities_total: u64,
}

/// Ticks between printed reports — roughly every 5 seconds at the default
/// 60Hz tick rate, coarse enough that the report itself is never a
/// meaningful cost, frequent enough to watch a metric change as population
/// grows during a live run.
const REPORT_INTERVAL_TICKS: u64 = 300;

/// Cross-tick accumulator — see this module's doc comment for why this is
/// a plain struct field on `PhylonApp`, not an ECS `Resource`.
#[derive(Default)]
pub(crate) struct BiologyProfilerState {
    stats: HashMap<BiologyCategory, CategoryStat>,
    tick_count: u64,
}

impl BiologyProfilerState {
    /// Records one timed call under `category`. `entities` is this tick's
    /// shared population-count context (see module doc), not a
    /// category-specific filtered count.
    pub(crate) fn record(&mut self, category: BiologyCategory, elapsed_ms: f64, entities: u64) {
        let stat = self.stats.entry(category).or_default();
        stat.total_ms += elapsed_ms;
        stat.max_ms = stat.max_ms.max(elapsed_ms);
        stat.call_count += 1;
        stat.entities_total += entities;
    }

    /// Call once per tick, only when the profiler is enabled (checked by
    /// the caller — see `simulation::update_simulation`). Advances the
    /// window counter and prints + resets the accumulated report every
    /// [`REPORT_INTERVAL_TICKS`] ticks, so each printed report reflects a
    /// recent window rather than an ever-growing whole-run average that
    /// would blur an early, cheap, small-population period together with
    /// a later, expensive, large-population one.
    pub(crate) fn end_tick(&mut self) {
        self.tick_count += 1;
        if self.tick_count.is_multiple_of(REPORT_INTERVAL_TICKS) {
            self.report();
            self.stats.clear();
        }
    }

    fn report(&self) {
        let tick_total_ms: f64 = self.stats.values().map(|s| s.total_ms).sum();
        if tick_total_ms <= 0.0 {
            return;
        }

        let mut rows: Vec<(BiologyCategory, CategoryStat)> = BiologyCategory::ALL
            .into_iter()
            .filter_map(|cat| self.stats.get(&cat).map(|s| (cat, *s)))
            .collect();
        // Ranked by total time (share of tick), highest first — directly
        // answers "which systems dominate," not just "list every category."
        rows.sort_by(|a, b| b.1.total_ms.partial_cmp(&a.1.total_ms).unwrap());

        let mut out = String::new();
        out.push_str(&format!(
            "\n{:<20} {:>9} {:>9} {:>7} {:>7} {:>10} {:>12}\n",
            "System", "avg_ms", "max_ms", "pct", "calls", "entities", "ms/entity"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');

        let mut running_pct = 0.0;
        let mut hotspot_note = String::new();
        for (cat, stat) in &rows {
            let calls = stat.call_count.max(1) as f64;
            let avg_ms = stat.total_ms / calls;
            let pct = 100.0 * stat.total_ms / tick_total_ms;
            let avg_entities = stat.entities_total as f64 / calls;
            let per_entity = if avg_entities > 0.0 {
                avg_ms / avg_entities
            } else {
                0.0
            };
            out.push_str(&format!(
                "{:<20} {:>9.3} {:>9.3} {:>6.1}% {:>7} {:>10.0} {:>12.5}\n",
                cat.label(),
                avg_ms,
                stat.max_ms,
                pct,
                stat.call_count,
                avg_entities,
                per_entity
            ));
            running_pct += pct;
            if running_pct <= 80.0 {
                if !hotspot_note.is_empty() {
                    hotspot_note.push_str(", ");
                }
                hotspot_note.push_str(cat.label());
            }
        }
        out.push_str(&"-".repeat(80));
        out.push_str(&format!(
            "\ntick_total_ms={tick_total_ms:.3}  systems_covering_80pct=[{hotspot_note}]\n"
        ));

        tracing::info!(target: "biology_profiler", "{}", out);
    }
}
