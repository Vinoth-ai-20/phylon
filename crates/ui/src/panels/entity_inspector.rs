use common::EntityId;
use egui::{Color32, RichText, Ui};

pub fn render_entity_inspector(ui: &mut Ui, selected: &[EntityId], world: &mut world::PhylonWorld) {
    ui.heading("Entity Inspector");

    if selected.is_empty() {
        ui.label("Click an organism to inspect");
        return;
    }

    let entity_id = selected[0];
    ui.label(format!("Inspecting Entity {}", entity_id.0));
    ui.separator();

    let e = hecs::Entity::from_bits(entity_id.0).unwrap();
    if !world.ecs.contains(e) {
        ui.label("Entity no longer exists (dead).");
        return;
    }

    if let Ok((energy, health, age, speed, genome, species)) = world.ecs.query_one_mut::<(
        &organisms::Energy,
        &organisms::Health,
        &organisms::Age,
        &physics::Velocity,
        &genetics::Genome,
        &organisms::SpeciesId,
    )>(e)
    {
        egui::Grid::new("entity_stats_grid")
            .striped(true)
            .num_columns(2)
            .show(ui, |ui| {
                let labels = [
                    ("State", "Alive".to_string()),
                    ("Species ID", format!("{}", species.0)),
                    ("Age", format!("{}", age.0)),
                    ("Health", format!("{:.1}%", health.0 * 100.0)),
                    ("Energy", format!("{:.1}", energy.0)),
                    ("Speed", format!("{:.2}", speed.0.length())),
                    ("Size", format!("{:.2}", genome.size)),
                    (
                        "Diet",
                        match genome.diet {
                            genetics::Diet::Herbivore => "Herbivore".to_string(),
                            genetics::Diet::Carnivore => "Carnivore".to_string(),
                            genetics::Diet::Omnivore => "Omnivore".to_string(),
                        },
                    ),
                ];

                for (label, val) in labels {
                    ui.label(RichText::new(label).color(Color32::from_rgb(150, 150, 170)));
                    ui.label(RichText::new(val).color(Color32::WHITE));
                    ui.end_row();
                }
            });
    } else {
        ui.label("Not an organism.");
    }
}
