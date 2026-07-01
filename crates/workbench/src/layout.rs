use bevy::prelude::*;

#[derive(Component)]
pub struct ToolbarNode;
#[derive(Component)]
pub struct SidebarNode;
#[derive(Component)]
pub struct ViewportNode;
#[derive(Component)]
pub struct InspectorNode;
#[derive(Component)]
pub struct StatusBarNode;
#[derive(Component)]
pub struct MetricsNode;
#[derive(Component)]
pub struct EventLogNode;
#[derive(Component)]
pub struct BottomPanelNode;
#[derive(Component)]
pub struct MainAreaNode;
#[derive(Component)]
pub struct MenuBarNode;
#[derive(Component)]
pub struct NavigationRailNode;

pub fn setup_workbench_layout(mut commands: Commands) {
    // Root container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("WorkbenchRoot"),
        ))
        .with_children(|parent| {
            // Main Menu Bar (Topmost)
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                MenuBarNode,
                Name::new("MenuBar"),
            ));

            // Workspace Container (Row: Nav Rail + Content Area)
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        ..default()
                    },
                    Name::new("WorkspaceContainer"),
                ))
                .with_children(|workspace| {
                    // Navigation Rail (Leftmost)
                    workspace.spawn((
                        Node {
                            width: Val::Px(48.0),
                            height: Val::Percent(100.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::vertical(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
                        NavigationRailNode,
                        Name::new("NavigationRail"),
                    ));

                    // Content Area (Column: Toolbar + Main Area + Bottom Panel)
                    workspace
                        .spawn((
                            Node {
                                flex_grow: 1.0,
                                height: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            },
                            Name::new("ContentArea"),
                        ))
                        .with_children(|content| {
                            // Top Toolbar (Height: 40px)
                            content.spawn((
                                Node {
                                    width: Val::Percent(100.0),
                                    height: Val::Px(40.0),
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                                ToolbarNode,
                                Name::new("Toolbar"),
                            ));

                            // Main Area (Remaining Height)
                            content
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0), // Flex-grow will handle this
                                        flex_grow: 1.0,
                                        flex_direction: FlexDirection::Row,
                                        ..default()
                                    },
                                    MainAreaNode,
                                    Name::new("MainArea"),
                                ))
                                .with_children(|main| {
                                    // Left Sidebar
                                    main.spawn((
                                        Node {
                                            width: Val::Px(250.0),
                                            height: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
                                        SidebarNode,
                                        Name::new("Sidebar"),
                                    ));

                                    // Viewport (Transparent space for Camera2d to render through)
                                    main.spawn((
                                        Node {
                                            flex_grow: 1.0,
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },
                                        ViewportNode,
                                        Name::new("Viewport"),
                                    ));

                                    // Right Inspector
                                    main.spawn((
                                        Node {
                                            width: Val::Px(300.0),
                                            height: Val::Percent(100.0),
                                            flex_direction: FlexDirection::Column,
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
                                        InspectorNode,
                                        Name::new("Inspector"),
                                    ));
                                });

                            // Bottom Panel
                            content
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Px(200.0),
                                        flex_direction: FlexDirection::Column,
                                        ..default()
                                    },
                                    BottomPanelNode,
                                    Name::new("BottomPanel"),
                                ))
                                .with_children(|bottom| {
                                    // Middle Content (Row)
                                    bottom
                                        .spawn((
                                            Node {
                                                width: Val::Percent(100.0),
                                                flex_grow: 1.0,
                                                flex_direction: FlexDirection::Row,
                                                ..default()
                                            },
                                            Name::new("BottomPanelRow"),
                                        ))
                                        .with_children(|row| {
                                            // Metrics
                                            row.spawn((
                                                Node {
                                                    width: Val::Percent(50.0),
                                                    flex_grow: 1.0,
                                                    padding: UiRect::all(Val::Px(5.0)),
                                                    ..default()
                                                },
                                                BackgroundColor(Color::srgb(0.18, 0.18, 0.18)),
                                                MetricsNode,
                                                Name::new("Metrics"),
                                            ));

                                            // Event Log
                                            row.spawn((
                                                Node {
                                                    width: Val::Percent(50.0),
                                                    flex_grow: 1.0,
                                                    padding: UiRect::all(Val::Px(5.0)),
                                                    flex_direction: FlexDirection::Column,
                                                    ..default()
                                                },
                                                BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                                                EventLogNode,
                                                Name::new("EventLog"),
                                            ));
                                        });

                                    // Status Bar
                                    bottom.spawn((
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Px(25.0),
                                            flex_direction: FlexDirection::Row,
                                            justify_content: JustifyContent::SpaceBetween,
                                            align_items: AlignItems::Center,
                                            padding: UiRect::horizontal(Val::Px(8.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                                        StatusBarNode,
                                        Name::new("StatusBar"),
                                    ));
                                });
                        });
                });
        });
}
