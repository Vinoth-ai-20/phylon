//! # Phylon Plugins
//!
//! Embedded `rhai` scripting engine for scenario authoring and scripted
//! god-mode interventions.
//!
//! Deliberately depends on nothing but `rhai`/`common`/`thiserror` — no
//! `bevy_ecs`, no simulation-domain crates — matching this workspace's
//! "only `app` may depend on everything" rule (see `main.rs`'s doc
//! comment). Scripts don't mutate simulation state directly: every exposed
//! function just pushes a [`ScriptCommand`] onto a queue, which the host
//! (`app::scripting`) drains and applies via the exact same
//! `PhylonApp::apply_*` methods `events.rs`'s live menu handler and
//! `app::replay`'s playback driver already use — this is the same
//! deferred-command pattern `storage::replay::ReplayAction` uses, for the
//! same reason: nothing here needs to know how to spawn an organism, only
//! that a spawn was requested.
//!
//! Also why scripts are safe by construction: `rhai::Engine` has no
//! filesystem/network/process access unless a host explicitly registers
//! such a function, and this crate registers only the four
//! [`ScriptCommand`] variants — a script literally cannot do anything else.

#![warn(missing_docs)]
#![warn(clippy::all)]

use rhai::{Dynamic, Engine, Scope};
use std::cell::RefCell;
use std::rc::Rc;

/// Errors from the plugin subsystem.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// The script file could not be read.
    #[error("I/O error: {source}")]
    Io {
        /// Underlying I/O error.
        #[from]
        source: std::io::Error,
    },
    /// A rhai script failed to run (compile error or runtime panic/error).
    #[error("script error: {message}")]
    RuntimeError {
        /// Description of the script error.
        message: String,
    },
}

impl common::PhylonError for PluginError {}

/// One deferred action a script requested via a registered function call.
/// Deliberately the same four non-Entity-referencing actions as
/// `storage::replay::ReplayAction` (see that type's doc comment for why
/// only these are safe) — a scripted intervention and a manually-recorded
/// one carry exactly the same risk profile.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCommand {
    /// Requests a full ecosystem reseed.
    ReseedEcosystem,
    /// Requests spawning a named sandbox preset at `(x, y)`.
    SpawnPreset {
        /// The preset's name.
        name: String,
        /// World-space X coordinate.
        x: f32,
        /// World-space Y coordinate.
        y: f32,
    },
    /// Requests spawning a deterministic "Proto-Fish" at `(x, y)`.
    SpawnProtoFish {
        /// World-space X coordinate.
        x: f32,
        /// World-space Y coordinate.
        y: f32,
    },
    /// Requests spawning a manual catastrophe hazard at `(x, y)`.
    SpawnManualHazard {
        /// World-space X coordinate.
        x: f32,
        /// World-space Y coordinate.
        y: f32,
    },
}

/// # Embedded Scripting Engine
///
/// ## 1. What Happens
/// Wraps a `rhai::Engine` with four registered functions
/// (`reseed_ecosystem`, `spawn_preset`, `spawn_proto_fish`, `spawn_hazard`)
/// that queue a [`ScriptCommand`] rather than mutating anything directly.
///
/// ## 2. Why It Happens
/// Researchers need to author scenarios and scripted interventions without
/// recompiling the engine (the spec's "God Mode" scripting API) — but the
/// script must never be able to reach into the ECS `World` directly, both
/// because `plugins` has no dependency on `bevy_ecs` to do so, and because
/// a queued-command model is trivially safe regardless of what a script
/// does (there's no live mutable state for it to corrupt).
///
/// ## 3. How It Happens
/// [`PluginEngine::run_script`]/[`PluginEngine::run_file`] clear the queue,
/// run the script (optionally seeded with read-only context variables via
/// [`PluginEngine::run_script_with_context`] — e.g. exposing the current
/// population count so a script can make conditional decisions), then
/// drain and return every queued command in call order for the host to
/// apply.
pub struct PluginEngine {
    engine: Engine,
    commands: Rc<RefCell<Vec<ScriptCommand>>>,
}

impl PluginEngine {
    /// Creates a new engine with the four god-mode functions registered.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let commands: Rc<RefCell<Vec<ScriptCommand>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let commands = commands.clone();
            engine.register_fn("reseed_ecosystem", move || {
                commands.borrow_mut().push(ScriptCommand::ReseedEcosystem);
            });
        }
        {
            let commands = commands.clone();
            engine.register_fn("spawn_preset", move |name: &str, x: f64, y: f64| {
                commands.borrow_mut().push(ScriptCommand::SpawnPreset {
                    name: name.to_string(),
                    x: x as f32,
                    y: y as f32,
                });
            });
        }
        {
            let commands = commands.clone();
            engine.register_fn("spawn_proto_fish", move |x: f64, y: f64| {
                commands.borrow_mut().push(ScriptCommand::SpawnProtoFish {
                    x: x as f32,
                    y: y as f32,
                });
            });
        }
        {
            let commands = commands.clone();
            engine.register_fn("spawn_hazard", move |x: f64, y: f64| {
                commands
                    .borrow_mut()
                    .push(ScriptCommand::SpawnManualHazard {
                        x: x as f32,
                        y: y as f32,
                    });
            });
        }

        Self { engine, commands }
    }

    /// Runs `script` with `context` variables pre-populated in scope (e.g.
    /// `[("population", 42_i64.into())]`), returning every command the
    /// script requested, in call order.
    pub fn run_script_with_context(
        &self,
        script: &str,
        context: &[(&str, Dynamic)],
    ) -> Result<Vec<ScriptCommand>, PluginError> {
        self.commands.borrow_mut().clear();
        let mut scope = Scope::new();
        for (name, value) in context {
            scope.push(name.to_string(), value.clone());
        }
        self.engine
            .run_with_scope(&mut scope, script)
            .map_err(|e| PluginError::RuntimeError {
                message: e.to_string(),
            })?;
        Ok(self.commands.borrow_mut().drain(..).collect())
    }

    /// Runs `script` with no context variables. See
    /// [`PluginEngine::run_script_with_context`].
    pub fn run_script(&self, script: &str) -> Result<Vec<ScriptCommand>, PluginError> {
        self.run_script_with_context(script, &[])
    }

    /// Reads and runs the `.rhai` script at `path`. See
    /// [`PluginEngine::run_script`].
    pub fn run_file(&self, path: &std::path::Path) -> Result<Vec<ScriptCommand>, PluginError> {
        let script = std::fs::read_to_string(path)?;
        self.run_script(&script)
    }
}

impl Default for PluginEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reseed_ecosystem_queues_command() {
        let engine = PluginEngine::new();
        let commands = engine.run_script("reseed_ecosystem();").unwrap();
        assert_eq!(commands, vec![ScriptCommand::ReseedEcosystem]);
    }

    #[test]
    fn spawn_preset_queues_command_with_resolved_position() {
        let engine = PluginEngine::new();
        let commands = engine
            .run_script(r#"spawn_preset("Herbivore", 10.0, -5.0);"#)
            .unwrap();
        assert_eq!(
            commands,
            vec![ScriptCommand::SpawnPreset {
                name: "Herbivore".to_string(),
                x: 10.0,
                y: -5.0,
            }]
        );
    }

    #[test]
    fn multiple_calls_queue_in_order() {
        let engine = PluginEngine::new();
        let commands = engine
            .run_script("spawn_proto_fish(1.0, 2.0); spawn_hazard(3.0, 4.0);")
            .unwrap();
        assert_eq!(
            commands,
            vec![
                ScriptCommand::SpawnProtoFish { x: 1.0, y: 2.0 },
                ScriptCommand::SpawnManualHazard { x: 3.0, y: 4.0 },
            ]
        );
    }

    #[test]
    fn queue_is_cleared_between_runs() {
        let engine = PluginEngine::new();
        engine.run_script("reseed_ecosystem();").unwrap();
        let commands = engine.run_script("spawn_hazard(0.0, 0.0);").unwrap();
        assert_eq!(
            commands,
            vec![ScriptCommand::SpawnManualHazard { x: 0.0, y: 0.0 }]
        );
    }

    #[test]
    fn context_variables_are_readable_by_the_script() {
        let engine = PluginEngine::new();
        let commands = engine
            .run_script_with_context(
                r#"if population > 10 { reseed_ecosystem(); }"#,
                &[("population", Dynamic::from(20_i64))],
            )
            .unwrap();
        assert_eq!(commands, vec![ScriptCommand::ReseedEcosystem]);
    }

    #[test]
    fn context_variables_can_suppress_the_conditional_branch() {
        let engine = PluginEngine::new();
        let commands = engine
            .run_script_with_context(
                r#"if population > 10 { reseed_ecosystem(); }"#,
                &[("population", Dynamic::from(5_i64))],
            )
            .unwrap();
        assert!(commands.is_empty());
    }

    #[test]
    fn compile_error_surfaces_as_runtime_error() {
        let engine = PluginEngine::new();
        let result = engine.run_script("this is not valid rhai (((");
        assert!(result.is_err());
    }

    #[test]
    fn unregistered_function_call_surfaces_as_runtime_error() {
        let engine = PluginEngine::new();
        let result = engine.run_script("delete_all_files();");
        assert!(result.is_err());
    }
}
