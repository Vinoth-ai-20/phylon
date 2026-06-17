# Physics and Diffusion

Phylon operates on an abstract set of dimensionless simulation units. Real-world physical mapping is defined per-experiment, but internally, the engine remains mathematically abstract to avoid precision artifacts at extreme scales.

## Simulation Units

- `SimLength (su)`: Base spatial unit.
- `SimMass (smu)`: Base mass unit.
- `SimTime (st)`: Base time unit (one tick).
- `SimEnergy (seu)`: Base energy unit.

## Physics Integrator

**Chosen Integrator**: Symplectic Euler.
*Justification*: Symplectic Euler perfectly preserves energy in oscillatory systems. Because Phylon organisms are modeled as **Dynamic Graphs** rather than rigid monolithic bodies, they rely entirely on particle-spring constraints (elasticity, reflection, tension, and repulsion) acting on discrete nodes (cells/particles). The stability and computational cheapness of Symplectic Euler vastly outperform standard explicit Euler when processing tens of millions of structural tissue links across 100,000+ organisms.

## Node-Edge Constraint Physics

Phylon explicitly rejects top-down rigid body dynamics for biology. Movement and structure emerge from:

- **Tension & Elasticity**: Edges between nodes act as structural springs that attempt to maintain rest lengths.
- **Repulsion**: Nodes that are not structurally bound exert strong short-range repulsion to simulate collision and volume.
- **Motor Nodes**: Behavior components don't apply an abstract "move organism" force; instead, specific nodes act as muscles, contracting or expanding spring edges to create locomotion.
This physically grounded approach naturally yields fluid-like behavior, realistic soft-body collisions, and modular body plans.

## Field Diffusion PDE

Field diffusion is governed by a standard 2D diffusion equation:
∂u/∂t = D ∇²u - λu + S
Where:

- D = Diffusion coefficient
- ∇²u = Discrete Laplacian
- λ = Decay constant
- S = Source term (emissions from entities)

*Numerical Method*: Explicit Euler using a 5-point discrete Laplacian stencil. GPU compute shaders parallelize the calculation across the grid. The `diffusion_step_size` configures the integration step limit to ensure the Courant-Friedrichs-Lewy (CFL) condition is not violated, preventing unstable oscillating gradients.

## Boundary Handling: Neumann Boundaries

The world handles field diffusion at boundaries using **Neumann (reflecting) boundary conditions**. The field gradients at the edges of the active simulation area are assumed to be zero, preventing mass/energy loss across the boundary and ensuring conservation within the active zone.

When chunks are loaded dynamically, boundary ghost cells apply these Neumann conditions if the adjacent chunk is not loaded. If adjacent chunks are loaded, they seamlessly exchange gradients.
