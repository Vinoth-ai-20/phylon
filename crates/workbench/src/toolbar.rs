use crate::events::{CameraControlEvent, OverlayChangedEvent, SimulationControlEvent};
use crate::layout::ToolbarNode;
use bevy::prelude::*;

#[derive(Component)]
pub enum ToolbarAction {
    Play,
    Pause,
    Reset,
    SpeedUp,
    Step,
    SpectatorToggle,
    CameraReset,
    OverlayToggle,
}

pub struct ToolbarPlugin;

impl Plugin for ToolbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            setup_toolbar.after(crate::layout::setup_workbench_layout),
        )
        .add_systems(Update, handle_toolbar_interactions);
    }
}

fn setup_toolbar(
    mut commands: Commands,
    query: Query<Entity, With<ToolbarNode>>,
    ui_assets: Res<crate::UiAssets>,
) {
    let Some(toolbar_entity) = query.iter().next() else {
        return;
    };

    let button_colors = (
        Color::srgb(0.2, 0.2, 0.2),
        Color::srgb(0.3, 0.3, 0.3),
        Color::srgb(0.4, 0.4, 0.4),
    );

    let spawn_btn = |commands: &mut Commands, action: ToolbarAction, text: &str| {
        commands
            .spawn((
                (
                    Button,
                    Node {
                        padding: UiRect::all(Val::Px(5.0)),
                        margin: UiRect::all(Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(button_colors.0),
                ),
                action,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Text::new(text),
                    TextFont {
                        font: bevy::prelude::FontSource::Handle(ui_assets.inter_regular.clone()),
                        font_size: bevy::prelude::FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            })
            .id()
    };

    let btns = vec![
        spawn_btn(&mut commands, ToolbarAction::Play, "Play"),
        spawn_btn(&mut commands, ToolbarAction::Pause, "Pause"),
        spawn_btn(&mut commands, ToolbarAction::Reset, "Reset"),
        spawn_btn(&mut commands, ToolbarAction::SpeedUp, "Speed"),
        spawn_btn(&mut commands, ToolbarAction::Step, "Step"),
        spawn_btn(&mut commands, ToolbarAction::SpectatorToggle, "Spectator"),
        spawn_btn(&mut commands, ToolbarAction::CameraReset, "Reset Cam"),
        spawn_btn(&mut commands, ToolbarAction::OverlayToggle, "Overlay"),
    ];

    commands.entity(toolbar_entity).add_children(&btns);
}

#[allow(clippy::type_complexity)]
fn handle_toolbar_interactions(
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor, &ToolbarAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut sim_evw: MessageWriter<SimulationControlEvent>,
    mut cam_evw: MessageWriter<CameraControlEvent>,
    mut over_evw: MessageWriter<OverlayChangedEvent>,
) {
    for (interaction, mut color, action) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
                match action {
                    ToolbarAction::Play => {
                        sim_evw.write(SimulationControlEvent::Play);
                    }
                    ToolbarAction::Pause => {
                        sim_evw.write(SimulationControlEvent::Pause);
                    }
                    ToolbarAction::Reset => {
                        sim_evw.write(SimulationControlEvent::Reset);
                    }
                    ToolbarAction::SpeedUp => {
                        sim_evw.write(SimulationControlEvent::SetSpeed(2.0));
                    }
                    ToolbarAction::Step => {
                        sim_evw.write(SimulationControlEvent::StepOneTick);
                    }
                    ToolbarAction::SpectatorToggle => {
                        cam_evw.write(CameraControlEvent::ToggleSpectator);
                    }
                    ToolbarAction::CameraReset => {
                        cam_evw.write(CameraControlEvent::ResetCamera);
                    }
                    ToolbarAction::OverlayToggle => {
                        over_evw.write(OverlayChangedEvent::NextOverlay);
                    }
                }
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.2, 0.2, 0.2));
            }
        }
    }
}
