use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    Initializing,
    Running,
    Paused,
    Shutdown,
}

pub struct PhylonPlugins {
    pub gpu_pos_tx: crossbeam_channel::Sender<Vec<u8>>,
    pub gpu_node_entities_tx: crossbeam_channel::Sender<Vec<Entity>>,
    pub brain_data_tx: crossbeam_channel::Sender<Vec<u8>>,
    pub diffusion_data_tx: crossbeam_channel::Sender<Vec<f32>>,
}

impl PluginGroup for PhylonPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(CorePlugin)
            .add(SimulationPlugin)
            .add(EnvironmentPlugin)
            .add(crate::render::RenderingPlugin {
                gpu_pos_tx: self.gpu_pos_tx,
                gpu_node_entities_tx: self.gpu_node_entities_tx,
                brain_data_tx: self.brain_data_tx,
                diffusion_data_tx: self.diffusion_data_tx,
            })
            .add(workbench::WorkbenchPlugin)
            .add(crate::camera::CameraPlugin)
            .add(InputPlugin)
            .add(crate::selection::SelectionPlugin)
            .add(DiagnosticsPlugin)
            .add(PersistencePlugin)
            .add(ThemePlugin)
            .add(UiAssetsPlugin)
    }
}

// Placeholder plugin implementations

pub struct CorePlugin;
impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, start_simulation.run_if(in_state(AppState::Loading)));
    }
}

fn start_simulation(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Running);
}

pub struct SimulationPlugin;
impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<evolution::LineageTracker>()
            .init_resource::<genetics::GlobalInnovationTracker>()
            .init_resource::<behavior::BehaviorConfig>()
            .init_resource::<metabolism::GlobalAtmosphere>()
            .init_resource::<ecology::EcologyConfig>()
            .add_message::<reproduction::BirthRequest>();

        // We move the old systems here to keep it compiling as requested
        app.add_systems(OnEnter(AppState::Running), sandbox_setup_headless);
        app.add_systems(
            Update,
            (
                crate::systems::simulation_control_listener,
                overlay_changed_listener,
                crate::systems::process_births_system,
                crate::systems::process_narrative_events_system,
                crate::systems::process_deaths_system,
                crate::systems::sync_gpu_positions_to_cpu,
                // sync_brain_system removed temporarily as they depend on internal fn in main.rs
                // We'll move them back or properly implement them in RenderingPlugin later.
                metabolism::day_night_cycle_system,
                ecology::food_spawner_system,
                organisms::systems::growth_system,
                organisms::systems::producer_growth_system,
                sensing::sensing_system,
                behavior::behavior_system,
                behavior::behavior_logging_system,
                ecology::photosynthesis_system,
                ecology::foraging_system,
                metabolism::metabolism_system,
                reproduction::reproduction_system,
                ecology::catastrophe_system,
            )
                .chain()
                .run_if(in_state(AppState::Running)),
        );
        app.add_systems(
            Update,
            (
                crate::systems::update_status_bar_system,
                crate::systems::update_sidebar_system,
                crate::systems::update_inspector_system,
                crate::systems::update_metrics_system,
            ),
        );
    }
}

fn sandbox_setup_headless(
    mut commands: Commands,
    mut lineage_tracker: ResMut<evolution::LineageTracker>,
    _tracker: ResMut<genetics::GlobalInnovationTracker>,
) {
    let mut rng = rand::thread_rng();
    use rand::Rng;

    let producer_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(6),
        common::EntityId(0),
        genetics::HoxSequence::plant([0.068, 0.730, 0.216]),
    );

    let lineage_id = lineage_tracker.new_lineage_id();

    for _ in 0..100 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);

        let entity = commands
            .spawn((
                physics::ParticleNode::new(common::Vec2::new(px, py), 1.0, 0, 0),
                Transform::from_translation(Vec3::new(px, py, 0.0)),
                producer_genome.clone(),
                ecology::Diet::Producer,
            ))
            .id();

        lineage_tracker.register_birth(
            common::EntityId(entity.to_bits()),
            None,
            lineage_id,
            evolution::SpeciesId(0),
            0,
            0,
        );
    }

    let herbivore_genome = genetics::Genome::new_hox_driven(
        genetics::GenomeId(7),
        common::EntityId(0),
        genetics::HoxSequence::new(
            vec![
                // A basic herbivore sequence with multiple nodes
                genetics::HoxGene::head(),
                genetics::HoxGene::torso(),
                genetics::HoxGene::muscle(0.8, 0.0),
                genetics::HoxGene::tail(),
            ],
            [0.8, 0.2, 0.1],
        ),
    );

    for _ in 0..20 {
        let px = rng.gen_range(-1000.0..1000.0);
        let py = rng.gen_range(-1000.0..1000.0);
        commands.queue(crate::systems::SpawnOrganismCommand {
            parent_id: None,
            genome: herbivore_genome.clone(),
            position: common::Vec2::new(px, py),
            diet: ecology::Diet::Herbivore,
            category: ecology::EcologicalCategory::None,
        });
    }
}

pub struct EnvironmentPlugin;
impl Plugin for EnvironmentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<environment::EnvironmentManager>()
            .init_resource::<ecology::catastrophe::CatastropheManager>()
            .init_resource::<ecology::catastrophe::CatastropheConfig>()
            .init_resource::<diffusion::CpuHazardFieldState>()
            .init_resource::<diffusion::CpuFieldState>()
            .init_resource::<diffusion::CpuSignalFieldState>()
            .add_message::<ecology::catastrophe::HazardSpawned>()
            .add_systems(
                Update,
                sync_diffusion_field_system.run_if(in_state(AppState::Running)),
            );
    }
}

fn sync_diffusion_field_system(
    receiver: Res<crate::DiffusionDataReceiver>,
    mut cpu_field: ResMut<diffusion::CpuFieldState>,
) {
    if let Ok(data) = receiver.0.try_recv() {
        cpu_field.data = data;
    }
}

fn overlay_changed_listener(
    mut events: bevy::prelude::MessageReader<workbench::events::OverlayChangedEvent>,
    mut overlay: ResMut<crate::ActiveOverlay>,
) {
    for ev in events.read() {
        match ev {
            workbench::events::OverlayChangedEvent::NextOverlay => {
                // Cycle through overlays: None -> Energy -> O2 -> CO2 -> Pheromones -> None...
                let next = match overlay.0 {
                    None => Some(diffusion::FieldLayer::Energy),
                    Some(diffusion::FieldLayer::Energy) => Some(diffusion::FieldLayer::O2),
                    Some(diffusion::FieldLayer::O2) => Some(diffusion::FieldLayer::CO2),
                    Some(diffusion::FieldLayer::CO2) => Some(diffusion::FieldLayer::Pheromones),
                    Some(diffusion::FieldLayer::Pheromones) => None,
                };
                overlay.0 = next;
                bevy::prelude::info!("Overlay changed to {:?}", overlay.0);
            }
        }
    }
}

pub struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, _app: &mut App) {}
}

pub struct DiagnosticsPlugin;
impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<analytics::NarrationLog>()
            .init_resource::<analytics::MetricsState>()
            .add_systems(Update, metrics_gathering_system);
    }
}

fn metrics_gathering_system(
    mut metrics: ResMut<analytics::MetricsState>,
    organism_query: Query<&ecology::Diet>,
    corpse_query: Query<&ecology::Corpse>,
    food_query: Query<&ecology::FoodPellet>,
    mineral_query: Query<&ecology::MineralPellet>,
    time: Res<Time>,
) {
    let mut counts = analytics::PopulationCounts::default();

    for diet in organism_query.iter() {
        match diet {
            ecology::Diet::Producer => counts.producers += 1,
            ecology::Diet::Herbivore => counts.herbivores += 1,
            ecology::Diet::Carnivore => counts.carnivores += 1,
            ecology::Diet::Omnivore => counts.omnivores += 1,
            ecology::Diet::Decomposer => counts.decomposers += 1,
        }
    }

    counts.corpses = corpse_query.iter().count();
    counts.food_pellets = food_query.iter().count();
    counts.minerals = mineral_query.iter().count();

    let dt = time.delta_secs_f64();
    metrics.record_frame(counts, dt, dt); // Real dt is used for FPS, sim dt for internal time
    metrics.record_env_perf(1.0 / dt.max(0.001), 0.0, 1.0, 0.21, 0.04, 25.0); // Placeholder environment values
}

pub struct PersistencePlugin;
impl Plugin for PersistencePlugin {
    fn build(&self, _app: &mut App) {}
}

pub struct ThemePlugin;
impl Plugin for ThemePlugin {
    fn build(&self, _app: &mut App) {}
}

pub struct UiAssetsPlugin;
impl Plugin for UiAssetsPlugin {
    fn build(&self, _app: &mut App) {}
}
