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

/// # Embedded Scripting Engine
///
/// ## 1. What Happens
/// The `PluginEngine` is a placeholder for the `rhai` embedded scripting context.
///
/// ## 2. Why It Happens
/// Researchers often need to inject custom scenarios, trigger ecological disasters, or
/// modify genome populations on the fly without recompiling the Rust engine. An embedded
/// scripting engine provides a sandboxed "God Mode" API.
///
/// ## 3. How It Happens
/// In Phase 12, this struct will wrap a `rhai::Engine`, exposing tightly controlled
/// bindings to the Bevy ECS `Commands` queue and `EventBus` so scripts can spawn entities
/// or alter weather systems.
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
