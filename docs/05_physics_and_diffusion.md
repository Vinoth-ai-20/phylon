# Physics and Diffusion

Phylon operates on an abstract set of dimensionless simulation units. Real-world physical mapping is defined per-experiment, but internally, the engine remains mathematically abstract to avoid precision artifacts at extreme scales.

## Simulation Units
- `SimLength (su)`: Base spatial unit.
- `SimMass (smu)`: Base mass unit.
- `SimTime (st)`: Base time unit (one tick).
- `SimEnergy (seu)`: Base energy unit.

## Physics Integrator

**Chosen Integrator**: Symplectic Euler.
*Justification*: Symplectic Euler perfectly preserves energy in oscillatory systems (like spring-based colonies or soft-body approximations) and is much cheaper than Runge-Kutta. For a simulation aiming for 100,000 agents with structural tissue links, the stability and computational cheapness of Symplectic Euler vastly outperform standard explicit Euler.

## Field Diffusion PDE

Field diffusion is governed by a standard 2D diffusion equation:
∂u/∂t = D ∇²u - λu + S
Where:
- D = Diffusion coefficient
- ∇²u = Discrete Laplacian
- λ = Decay constant
- S = Source term (emissions from entities)

*Numerical Method*: Explicit Euler using a 5-point discrete Laplacian stencil. GPU compute shaders parallelize the calculation across the grid. The `diffusion_step_size` configures the integration step limit to ensure the Courant-Friedrichs-Lewy (CFL) condition is not violated, preventing unstable oscillating gradients.

## Boundary Handling: Ghost Cells

The world is infinite, but computation is bounded by active chunks.
When diffusion occurs at a chunk edge, the compute shader reads from a 1-cell wide padding border called a "ghost cell". Ghost cells mirror the state of adjacent chunks or resolve to 0 (or a configurable ambient boundary condition) if the adjacent chunk is not loaded. Before the diffusion shader runs, the CPU copies overlap data between adjacent loaded chunks into their respective GPU ghost cell buffers to ensure smooth continuous gradients across arbitrary chunk seams.
