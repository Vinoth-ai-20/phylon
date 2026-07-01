use crate::layout::EventLogNode;
use bevy::prelude::*;

#[derive(Component)]
pub struct EventLogEntry;

pub struct EventLogPlugin;

impl Plugin for EventLogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_narrative_events);
    }
}

fn handle_narrative_events(
    mut commands: Commands,
    narration_log: Option<Res<analytics::NarrationLog>>,
    query: Query<Entity, With<EventLogNode>>,
    ui_assets: Res<crate::UiAssets>,
    children_query: Query<&Children>,
) {
    let Some(log_node) = query.iter().next() else {
        return;
    };
    let Some(log) = narration_log else { return };

    // Simple implementation: despawn all and respawn to keep it synced.
    // In a real app we'd only append new ones, but for now this is robust.
    if let Ok(children) = children_query.get(log_node) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let mut entries = vec![];

    // We only want to show the last 15 events to fit in the box
    let count = log.events.len();
    let start = count.saturating_sub(15);

    for i in start..count {
        if let Some(ev) = log.events.get(i) {
            let msg = format!("[Tick {}] {}", ev.tick, ev.description);
            let entry = commands
                .spawn((
                    Text::new(msg),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.jetbrains_mono.clone()),
                        font_size: bevy::prelude::FontSize::Px(12.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.7)),
                    EventLogEntry,
                ))
                .id();
            entries.push(entry);
        }
    }

    commands.entity(log_node).add_children(&entries);
}
