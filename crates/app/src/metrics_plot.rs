use analytics::MetricsState;
use bevy::prelude::*;
use bevy_vector_shapes::prelude::*;

pub struct MetricsPlotPlugin;

impl Plugin for MetricsPlotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MetricsState>()
            .add_plugins(ShapePlugin::default())
            .add_systems(Update, draw_metrics_plot);
    }
}

fn draw_metrics_plot(
    mut painter: ShapePainter,
    metrics: Res<MetricsState>,
    windows: Query<&Window>,
) {
    let window = if let Some(w) = windows.iter().next() {
        w
    } else {
        return;
    };

    // Plot settings
    let plot_width = 250.0;
    let plot_height = 100.0;

    // Position plot in bottom-right corner of screen, above UI
    let x_offset = window.width() / 2.0 - plot_width / 2.0 - 20.0;
    let y_offset = -window.height() / 2.0 + plot_height / 2.0 + 20.0;

    painter.set_translation(Vec3::new(x_offset, y_offset, 10.0));

    // Draw background
    painter.color = Color::srgba(0.05, 0.05, 0.07, 0.8);
    painter.rect(Vec2::new(plot_width, plot_height));

    let time_window = 10.0; // Show last 10 seconds
    let current_time = metrics.sim_time;
    let min_time = (current_time - time_window).max(0.0);

    if metrics.fps_history.is_empty() {
        return;
    }

    // Draw FPS line
    painter.color = Color::srgb(0.2, 0.8, 0.2);
    painter.thickness = 2.0;

    let max_fps = 120.0;

    let mut prev_point: Option<Vec2> = None;

    for [t, fps] in metrics.fps_history.iter() {
        if *t < min_time {
            continue;
        }

        // Normalize coordinates relative to plot center
        let nx = ((*t - min_time) / time_window) as f32; // 0 to 1
        let ny = (*fps / max_fps).clamp(0.0, 1.0) as f32;

        let px = (nx - 0.5) * plot_width;
        let py = (ny - 0.5) * plot_height;
        let pos = Vec2::new(px, py);

        if let Some(prev) = prev_point {
            painter.line(prev.extend(0.0), pos.extend(0.0));
        }
        prev_point = Some(pos);
    }
}
