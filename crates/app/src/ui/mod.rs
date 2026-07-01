use bevy::prelude::*;
use genetics;
use physics;
pub mod graph;

#[derive(Component)]
pub struct SimControlBtn(pub workbench::events::SimulationControlEvent);

pub struct WorkbenchPlugin;

impl Plugin for WorkbenchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(
                Update,
                (
                    button_interaction_system,
                    sim_control_button_system,
                    inspector_update_system,
                    diagnostics_update_system,
                    metrics_update_system,
                    event_log_update_system,
                ),
            )
            .add_plugins((
                ToolbarPlugin,
                SidebarPlugin,
                InspectorPlugin,
                MetricsPlugin,
                graph::GraphsPlugin,
                StatusBarPlugin,
                NotificationPlugin,
                LayoutPlugin,
                DialogPlugin,
            ));
    }
}

pub struct ToolbarPlugin;
impl Plugin for ToolbarPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct SidebarPlugin;
impl Plugin for SidebarPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct InspectorPlugin;
impl Plugin for InspectorPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct MetricsPlugin;
impl Plugin for MetricsPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct StatusBarPlugin;
impl Plugin for StatusBarPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct NotificationPlugin;
impl Plugin for NotificationPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct LayoutPlugin;
impl Plugin for LayoutPlugin {
    fn build(&self, _app: &mut App) {}
}
pub struct DialogPlugin;
impl Plugin for DialogPlugin {
    fn build(&self, _app: &mut App) {}
}

// Color palette
const COLOR_SIDEBAR: Color = Color::srgba(0.08, 0.08, 0.1, 0.95);
const COLOR_TOPBAR: Color = Color::srgba(0.1, 0.1, 0.12, 0.95);
const COLOR_BUTTON_NORMAL: Color = Color::srgb(0.2, 0.2, 0.25);
const COLOR_BUTTON_HOVERED: Color = Color::srgb(0.3, 0.3, 0.35);
const COLOR_BUTTON_PRESSED: Color = Color::srgb(0.4, 0.4, 0.45);
const COLOR_TEXT: Color = Color::srgb(0.9, 0.9, 0.9);

fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = asset_server.load("fonts/Inter-Regular.ttf");

    // Root Node (100% width and height, flex column)
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();

    // Top Bar
    let top_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(50.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(20.0)),
                ..default()
            },
            BackgroundColor(COLOR_TOPBAR),
        ))
        .id();
    commands.entity(root).add_child(top_bar);

    commands.entity(top_bar).with_children(|p| {
        p.spawn((
            Text::new("PHYLON Engine"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(24.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
        ));

        p.spawn((
            Text::new("FPS: -- | TPS: -- | Organisms: --"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(18.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
            DiagnosticsText,
        ));
    });

    // Main Content Area (Row, flexes to fill space)
    let main_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0), // Takes up remaining vertical space
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    commands.entity(root).add_child(main_area);

    // Left side: Empty placeholder for simulation view (transparent)
    let sim_view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    commands.entity(main_area).add_child(sim_view);

    // Right side: Sidebar (Fixed width)
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(15.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(COLOR_SIDEBAR),
        ))
        .id();
    commands.entity(main_area).add_child(sidebar);

    commands.entity(sidebar).with_children(|p| {
        p.spawn((
            Text::new("Controls"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(20.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
            Node {
                margin: UiRect::bottom(Val::Px(10.0)),
                ..default()
            },
        ));
    });

    // Replace old buttons with new widget buttons
    let play_btn = ui::widgets::spawn_widget_button(
        &mut commands,
        sidebar,
        ui::widgets::WidgetButton {
            default_color: COLOR_BUTTON_NORMAL,
            ..default()
        },
        font.clone(),
        "Play",
    );
    commands.entity(play_btn).insert(SimControlBtn(
        workbench::events::SimulationControlEvent::Play,
    ));

    let pause_btn = ui::widgets::spawn_widget_button(
        &mut commands,
        sidebar,
        ui::widgets::WidgetButton {
            default_color: COLOR_BUTTON_NORMAL,
            ..default()
        },
        font.clone(),
        "Pause",
    );
    commands.entity(pause_btn).insert(SimControlBtn(
        workbench::events::SimulationControlEvent::Pause,
    ));

    let reset_btn = ui::widgets::spawn_widget_button(
        &mut commands,
        sidebar,
        ui::widgets::WidgetButton {
            default_color: COLOR_BUTTON_NORMAL,
            ..default()
        },
        font.clone(),
        "Reset Simulation",
    );
    commands.entity(reset_btn).insert(SimControlBtn(
        workbench::events::SimulationControlEvent::Reset,
    ));

    // Add a Slider Widget for simulation speed (example)
    let slider_label = commands
        .spawn((
            Text::new("Sim Speed"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(16.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
        ))
        .id();
    commands.entity(sidebar).add_child(slider_label);

    ui::widgets::spawn_widget_slider(&mut commands, sidebar, ui::widgets::WidgetSlider::default());

    // Add a Toggle Widget for rendering mode
    ui::widgets::spawn_widget_toggle(
        &mut commands,
        sidebar,
        ui::widgets::WidgetToggle::default(),
        font.clone(),
        "Show Overlay",
    );

    commands.entity(sidebar).with_children(|p| {
        // Inspector Area
        p.spawn((
            Text::new("Inspector\nNo entity selected"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(16.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
            InspectorText,
        ));

        // Metrics Area
        p.spawn((
            Text::new("Metrics\nLoading..."),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(14.0),
                ..default()
            },
            TextColor(COLOR_TEXT),
            MetricsText,
        ));

        // Event Log Area (Wait, we will spawn this outside with commands)
    });

    let scroll_view = ui::widgets::spawn_widget_scroll_view(
        &mut commands,
        sidebar,
        ui::widgets::WidgetScrollView::default(),
        Color::srgba(0.0, 0.0, 0.0, 0.5),
        Val::Px(150.0),
    );

    commands.entity(scroll_view).with_children(|inner| {
        inner.spawn((
            Text::new("Event Log\n"),
            TextFont {
                font: bevy::prelude::FontSource::Handle(font.clone()),
                font_size: bevy::prelude::FontSize::Px(14.0),
                ..default()
            },
            TextColor(Color::srgba(0.8, 0.8, 0.8, 0.8)),
            EventLogText,
        ));
    });

    commands.entity(sidebar).with_children(|p| {
        // Graphs
        crate::spawn_graph!(
            p,
            graph::MetricType::Population,
            "Producers Population",
            Color::srgb(0.2, 0.8, 0.2),
            128,
            font.clone()
        );
        crate::spawn_graph!(
            p,
            graph::MetricType::FPS,
            "Framerate (FPS)",
            Color::srgb(0.8, 0.6, 0.1),
            128,
            font.clone()
        );
    });
}

#[allow(clippy::type_complexity)]
fn button_interaction_system(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(COLOR_BUTTON_PRESSED);
            }
            Interaction::Hovered => {
                *color = BackgroundColor(COLOR_BUTTON_HOVERED);
            }
            Interaction::None => {
                *color = BackgroundColor(COLOR_BUTTON_NORMAL);
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn sim_control_button_system(
    interaction_query: Query<(&Interaction, &SimControlBtn), (Changed<Interaction>, With<Button>)>,
    mut ev_writer: bevy::prelude::MessageWriter<workbench::events::SimulationControlEvent>,
) {
    for (interaction, btn) in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            ev_writer.write(btn.0);
        }
    }
}

#[derive(Component)]
pub struct InspectorText;

fn inspector_update_system(
    selected: Res<crate::selection::SelectedEntity>,
    organism_query: Query<(Entity, &physics::ParticleNode, Option<&genetics::Genome>)>,
    mut text_query: Query<&mut Text, With<InspectorText>>,
) {
    if !selected.is_changed() {
        return;
    }

    if let Some(mut text) = text_query.iter_mut().next() {
        if let Some(entity) = selected.0 {
            if let Ok((_, node, genome)) = organism_query.get(entity) {
                let genome_str = if let Some(g) = genome {
                    let genes = g.brain_cppn.connections.len() + g.morph_cppn.connections.len();
                    format!("Genome ID: {}\nGenes: {}", g.id.0, genes)
                } else {
                    "No Genome".to_string()
                };

                text.0 = format!(
                    "Inspector\nEntity: {:?}\nPos: {:.1}, {:.1}\n{}",
                    entity, node.position.x, node.position.y, genome_str
                );
            } else {
                text.0 = "Inspector\nEntity not found".to_string();
            }
        } else {
            text.0 = "Inspector\nNo entity selected".to_string();
        }
    }
}

#[derive(Component)]
pub struct DiagnosticsText;

fn diagnostics_update_system(
    metrics: Res<analytics::MetricsState>,
    mut text_query: Query<&mut Text, With<DiagnosticsText>>,
) {
    if let Some(mut text) = text_query.iter_mut().next() {
        let total = metrics.producers_history.back().map_or(0.0, |v| v[1])
            + metrics.herbivores_history.back().map_or(0.0, |v| v[1])
            + metrics.carnivores_history.back().map_or(0.0, |v| v[1])
            + metrics.omnivores_history.back().map_or(0.0, |v| v[1])
            + metrics.decomposers_history.back().map_or(0.0, |v| v[1]);

        text.0 = format!(
            "FPS: {:.1} | TPS: {:.1} | Organisms: {}",
            metrics.smoothed_fps, metrics.smoothed_tps, total as u64
        );
    }
}

#[derive(Component)]
pub struct MetricsText;

fn metrics_update_system(
    metrics: Res<analytics::MetricsState>,
    mut text_query: Query<&mut Text, With<MetricsText>>,
) {
    if let Some(mut text) = text_query.iter_mut().next() {
        text.0 = format!(
            "Metrics\nProducers: {}\nHerbivores: {}\nCarnivores: {}\nOmnivores: {}\nDecomposers: {}\nFood: {}\nMinerals: {}",
            metrics.producers_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.herbivores_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.carnivores_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.omnivores_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.decomposers_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.food_history.back().map_or(0.0, |v| v[1]) as u64,
            metrics.minerals_history.back().map_or(0.0, |v| v[1]) as u64,
        );
    }
}

#[derive(Component)]
pub struct EventLogText;

fn event_log_update_system(
    narration: Res<analytics::NarrationLog>,
    mut text_query: Query<&mut Text, With<EventLogText>>,
) {
    if !narration.is_changed() {
        return;
    }

    if let Some(mut text) = text_query.iter_mut().next() {
        let mut log_str = String::from("Event Log\n");
        for event in narration.events.iter().rev().take(5).rev() {
            log_str.push_str(&format!(
                "[{}] {}: {}\n",
                event.tick, event.event_type, event.description
            ));
        }
        text.0 = log_str;
    }
}
