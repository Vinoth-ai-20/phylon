//! # Phylon Plugins
//!
//! Embedded `rhai` scripting engine, plugin loader, and simulation API
//! bindings for scenario authoring and god-mode interventions.
//!
//! ## Phase 0 scope
//!
//! Placeholder. Implementation: Phase 12.

#![warn(missing_docs)]
#![warn(clippy::all)]

/// Errors from the plugin subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// A rhai script failed to compile.
    #[error("script compile error: {message}")]
    CompileError {
        /// Description of the compile error.
        message: String,
    },
}

impl common::PhylonError for PluginError {}

/// Placeholder for the plugin engine.
///
/// TODO(phase-12): Implement rhai engine initialisation, script loading,
/// and simulation API exposure.
pub struct PluginEngine;

impl PluginEngine {
    /// Creates a placeholder plugin engine.
    pub fn placeholder() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_engine_placeholder_creates() {
        let _eng = PluginEngine::placeholder();
    }
}
