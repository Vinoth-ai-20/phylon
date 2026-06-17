//! # Phylon UI
//!
//! `egui`-based research interface: entity inspector, analytics dashboard,
//! experiment controls, replay timeline, and debug overlay toggles.
//!
//! The UI crate renders on top of the simulation frame using egui's wgpu
//! backend. It reads from the simulation state (via shared snapshots) and
//! publishes intervention events to the event bus.
//!
//! ## Phase 0 scope
//!
//! Placeholder panel type. Full egui integration: Phase 7.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the UI subsystem.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// An egui widget encountered an invalid state.
    #[error("UI state error: {message}")]
    StateError {
        /// Description of the invalid state.
        message: String,
    },
}

impl common::PhylonError for UiError {}

/// Placeholder for the UI context.
///
/// TODO(phase-7): Implement full egui panel system.
pub struct UiContext;

impl UiContext {
    /// Creates a placeholder UI context.
    pub fn placeholder() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_context_placeholder_creates() {
        let _ui = UiContext::placeholder();
    }
}
