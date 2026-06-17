//! # Phylon Ecology
//!
//! Food chain dynamics, predation, disease spread, fungal networks,
//! decomposition, and oxygen / carbon cycles.
//!
//! Ecology is the layer that turns individual organism actions into population-
//! level outcomes. It processes feeding interactions, disease transmission,
//! decay of dead organisms, and publishes significant events to the event bus.
//!
//! ## Phase 0 scope
//!
//! Interaction type enumeration. Implementation: Phase 4.

#![warn(missing_docs)]
#![warn(clippy::all)]

use common::{EntityId, Tick};

/// The type of ecological interaction between two organisms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    /// Predator consumes prey.
    Predation,
    /// Organism consumes plant or resource field.
    Herbivory,
    /// Parasite drains energy from host.
    Parasitism,
    /// Disease transmits between individuals.
    Transmission,
    /// Organism decomposes a dead entity.
    Decomposition,
    /// Fungal network redistributes nutrients.
    FungalTransfer,
}

/// A record of a single ecological interaction during one tick.
#[allow(dead_code)]
pub struct EcologyInteraction {
    /// The tick this interaction occurred.
    tick: Tick,
    /// The kind of interaction.
    kind: InteractionKind,
    /// The initiating entity (predator, parasite, fungus, etc.).
    initiator: EntityId,
    /// The target entity (prey, host, carcass, etc.).
    target: EntityId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_kind_is_copy() {
        let k = InteractionKind::Predation;
        let _k2 = k;
    }
}
