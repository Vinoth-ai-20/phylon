use brain::Intention;
use hecs::World;
use physics::{Acceleration, Velocity};

pub fn process_behavior(world: &mut World) {
    puffin::profile_function!();

    let speed_factor = 20.0;

    for (_entity, (vel, acc, intention)) in
        world.query_mut::<(&mut Velocity, &mut Acceleration, &Intention)>()
    {
        let target_vel = intention.target_velocity;

        // simple steering towards target_velocity
        let diff = target_vel - vel.0;
        acc.0 = diff * speed_factor;
    }
}
