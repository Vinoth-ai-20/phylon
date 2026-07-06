//! CSV/JSON export of [`crate::MetricsState`]'s time-series histories —
//! closes the spec's "CSV and JSON export for all time-series data"
//! requirement for the *live* metrics ring buffers (population, FPS/TPS,
//! environment, diversity). Epic 11's `storage` crate export covers
//! organisms/lineages/events instead — this is the other half.

use crate::MetricsState;
use serde::Serialize;

/// One named time series, paired with its `[sim_time_s, value]` points —
/// the unit both CSV and JSON export operate on.
struct NamedSeries<'a> {
    name: &'static str,
    points: &'a std::collections::VecDeque<[f64; 2]>,
}

fn all_series(metrics: &MetricsState) -> Vec<NamedSeries<'_>> {
    vec![
        NamedSeries {
            name: "producers",
            points: &metrics.producers_history,
        },
        NamedSeries {
            name: "herbivores",
            points: &metrics.herbivores_history,
        },
        NamedSeries {
            name: "carnivores",
            points: &metrics.carnivores_history,
        },
        NamedSeries {
            name: "omnivores",
            points: &metrics.omnivores_history,
        },
        NamedSeries {
            name: "decomposers",
            points: &metrics.decomposers_history,
        },
        NamedSeries {
            name: "food_pellets",
            points: &metrics.food_history,
        },
        NamedSeries {
            name: "minerals",
            points: &metrics.minerals_history,
        },
        NamedSeries {
            name: "corpses",
            points: &metrics.corpses_history,
        },
        NamedSeries {
            name: "fps",
            points: &metrics.fps_history,
        },
        NamedSeries {
            name: "tps",
            points: &metrics.tps_history,
        },
        NamedSeries {
            name: "memory_mb",
            points: &metrics.memory_history,
        },
        NamedSeries {
            name: "sunlight",
            points: &metrics.sunlight_history,
        },
        NamedSeries {
            name: "o2",
            points: &metrics.o2_history,
        },
        NamedSeries {
            name: "co2",
            points: &metrics.co2_history,
        },
        NamedSeries {
            name: "temperature",
            points: &metrics.temp_history,
        },
        NamedSeries {
            name: "shannon_index",
            points: &metrics.shannon_history,
        },
        NamedSeries {
            name: "simpson_index",
            points: &metrics.simpson_history,
        },
        NamedSeries {
            name: "species_richness",
            points: &metrics.species_richness_history,
        },
        NamedSeries {
            name: "species_turnover",
            points: &metrics.species_turnover_history,
        },
    ]
}

/// Renders every named time series in `metrics` as one long-form CSV:
/// `series,sim_time_s,value`. Long form (rather than one column per series)
/// avoids needing every series to share the same sample times, since they
/// don't — `record_frame`/`record_env_perf`/`record_diversity` are called
/// at different cadences.
pub fn metrics_to_csv(metrics: &MetricsState) -> String {
    let mut out = String::from("series,sim_time_s,value\n");
    for series in all_series(metrics) {
        for point in series.points {
            out.push_str(&format!("{},{},{}\n", series.name, point[0], point[1]));
        }
    }
    out
}

#[derive(Serialize)]
struct JsonSeries {
    name: &'static str,
    points: Vec<[f64; 2]>,
}

#[derive(Serialize)]
struct JsonExport {
    series: Vec<JsonSeries>,
}

/// Renders every named time series in `metrics` as JSON: `{"series":
/// [{"name": "...", "points": [[t, v], ...]}, ...]}`.
pub fn metrics_to_json(metrics: &MetricsState) -> Result<String, serde_json::Error> {
    let export = JsonExport {
        series: all_series(metrics)
            .into_iter()
            .map(|s| JsonSeries {
                name: s.name,
                points: s.points.iter().copied().collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&export)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PopulationCounts;

    #[test]
    fn csv_export_includes_header_and_series_names() {
        let mut m = MetricsState::new();
        m.record_frame(
            PopulationCounts {
                producers: 5,
                ..Default::default()
            },
            0.016,
            0.016,
        );
        let csv = metrics_to_csv(&m);
        assert!(csv.starts_with("series,sim_time_s,value\n"));
        assert!(csv.contains("producers,"));
        assert!(csv.contains("fps,"));
    }

    #[test]
    fn json_export_round_trips_series_names_and_points() {
        let mut m = MetricsState::new();
        m.record_frame(
            PopulationCounts {
                herbivores: 3,
                ..Default::default()
            },
            0.016,
            0.016,
        );
        let json = metrics_to_json(&m).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let series = parsed["series"].as_array().unwrap();
        let herbivores = series
            .iter()
            .find(|s| s["name"] == "herbivores")
            .expect("herbivores series present");
        assert_eq!(herbivores["points"][0][1], 3.0);
    }
}
