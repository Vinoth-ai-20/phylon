// Example God Mode script

// Spawn some food at the center
api.spawn_food(0.0, 0.0, 50.0);
api.spawn_food(10.0, 10.0, 50.0);
api.spawn_food(-10.0, -10.0, 50.0);

// Kill anything within a radius of 200 units around the center
api.kill_radius(0.0, 0.0, 200.0);

// Flood oxygen channel (0)
api.flood_field(0, 1.0);
