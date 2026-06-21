# Ecology & Biology

Phylon bridges the gap between neural simulation and biological constraint by treating organisms as metabolizing physical entities within a strict chemical economy.

## The 4-Variable Chemical Economy

Organisms cannot simply "move"; movement costs physical energy. Every entity tracks four chemical pools in its `ChemicalEconomy`:

1. **Glucose**: The long-term energy storage.
2. **O2**: Oxygen, required for respiration.
3. **CO2**: Carbon dioxide, a toxic byproduct of respiration.
4. **ATP**: The immediate kinetic currency required for muscle actuation and neural inference.

### Metabolism & Cellular Respiration

To generate ATP, an organism must burn Glucose in the presence of Oxygen. This process is modeled via a minimum limiting function—if either Glucose or Oxygen is depleted, ATP production bottlenecks, and the base metabolic cost will begin to drain the organism's reserves.

$$ \Delta ATP = \min \left( \eta \cdot \text{Glucose}, \kappa \cdot O_2 \right) - \mu_{\text{base}} \cdot \text{mass} $$

Where:

- $\eta$ is the glycolysis conversion efficiency.
- $\kappa$ is the oxygen respiration limit.
- $\mu_{\text{base}}$ is the base metabolic cost per unit mass.

If ATP drops to $0.0$, the organism "suffocates" and dies.
If Glucose drops to $0.0$, the organism starves.

## Lotka-Volterra Trophic Dynamics

Because energy is strictly conserved, Phylon naturally expresses Lotka-Volterra predator-prey dynamics.

The ecosystem is tiered:

1. **Producers**: Tie their Glucose production directly to the `GlobalAtmosphere` sunlight intensity ($I_{sun}$). They can only generate energy during the Day cycle.
2. **Herbivores & Carnivores**: Must physically collide with appropriate nutrient sources (`FoodPellets` or other organisms) to steal their Glucose reserves.
3. **Decomposers**: Specialized organisms that break down `Corpses` back into `MineralPellets`, closing the ecological loop.

When night falls ($I_{sun} = 0.0$), producers stop generating Glucose. If the night lasts too long, producers starve, cascading starvation up the trophic ladder.

## L-System Morphology and Hox Genes

An organism's physical structure is not hardcoded; it is grown fractally.

The `HoxSequence` within the `genetics::Genome` acts as a 1D L-System grammar. When an organism is born (spawned as a single "Zygote" Head node), the `GrowthState` machine reads the Hox genes sequentially over several ticks.

- **Head Gene**: Spawns the central processing node.
- **Torso Gene**: Extends the rigid spine.
- **Muscle Gene**: Spawns an actuated spring capable of expanding/contracting based on the CPPN brain's output.
- **Fin/Branch Gene**: Forks the L-System grammar to grow symmetrical lateral appendages.

Because larger bodies have a higher `mass`, their $\mu_{\text{base}}$ (base metabolic cost) is higher. Therefore, the evolutionary algorithms must balance the neural capacity required to move a complex body against the metabolic cost of sustaining it.
