use brain::Intention;
use common::Vec2;
use genetics::Genome;
use hecs::World;
use physics::{Acceleration, Heading};

/// Maps the organism's neural intentions into physical forces and rotations.
pub fn process_behavior(world: &mut World) {
    puffin::profile_function!();

    for (_entity, (intention, heading, acc, genome)) in
        world.query_mut::<(&Intention, &mut Heading, &mut Acceleration, &Genome)>()
    {
        // intention.data[0] is turn amount in [-1.0, 1.0] (thanks to tanh)
        // intention.data[1] is forward thrust in [-1.0, 1.0]

        let turn_amount = intention.data[0];
        let thrust = intention.data[1];

        // Max turn speed per tick (e.g., 0.2 radians)
        let max_turn = 0.2;
        heading.0 += turn_amount * max_turn;

        // Normalize heading just in case
        while heading.0 > std::f32::consts::PI {
            heading.0 -= std::f32::consts::TAU;
        }
        while heading.0 < -std::f32::consts::PI {
            heading.0 += std::f32::consts::TAU;
        }

        // Forward thrust scaled by genome max speed
        // To allow stopping or reversing, we can allow negative thrust, or just bound to [0, 1].
        // Since tanh is [-1, 1], let's map [-1, 1] to [-max_speed, max_speed].
        let force_magnitude = thrust * genome.max_speed;

        let force_dir = Vec2::new(heading.0.cos(), heading.0.sin());
        acc.0 += force_dir * force_magnitude;
    }
}
