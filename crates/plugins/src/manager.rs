use crate::api::{apply_commands, GodModeApi};
use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use rhai::{Engine, Scope, AST};
use std::path::PathBuf;
use std::sync::mpsc;
use tracing::{error, info, warn};
use world::PhylonWorld;

pub struct ScriptManager {
    engine: Engine,
    api: GodModeApi,
    current_script_path: Option<PathBuf>,
    current_ast: Option<AST>,
    _watcher: Option<notify::RecommendedWatcher>,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    tx: mpsc::Sender<Result<Event, notify::Error>>,
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptManager {
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Register GodModeApi type and methods
        engine
            .register_type_with_name::<GodModeApi>("GodModeApi")
            .register_fn("spawn_food", GodModeApi::spawn_food)
            .register_fn("kill_radius", GodModeApi::kill_radius)
            .register_fn("flood_field", GodModeApi::flood_field);

        let (tx, rx) = mpsc::channel();

        Self {
            engine,
            api: GodModeApi::new(),
            current_script_path: None,
            current_ast: None,
            _watcher: None,
            rx,
            tx,
        }
    }

    pub fn load_script(&mut self, path: impl Into<PathBuf>) -> Result<()> {
        let path = path.into();
        info!("Loading script: {:?}", path);

        let ast = self
            .engine
            .compile_file(path.clone())
            .map_err(|e| anyhow::anyhow!("Compile error: {}", e))?;
        self.current_ast = Some(ast);
        self.current_script_path = Some(path.clone());

        // Setup hot-reload watcher
        let tx_clone = self.tx.clone();
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx_clone.send(res);
        })?;

        // Watch the specific file
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        self._watcher = Some(watcher);

        Ok(())
    }

    pub fn run_active_script(&mut self, world: &mut PhylonWorld) {
        // Check for file modifications
        while let Ok(res) = self.rx.try_recv() {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() {
                        if let Some(path) = &self.current_script_path {
                            info!("Script changed on disk, hot-reloading: {:?}", path);
                            if let Ok(ast) = self.engine.compile_file(path.clone()) {
                                self.current_ast = Some(ast);
                            } else {
                                warn!("Failed to recompile script");
                            }
                        }
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        }

        // Evaluate AST
        if let Some(ast) = &self.current_ast {
            let mut scope = Scope::new();
            // Bind the api to a variable `api` in the script
            scope.push("api", self.api.clone());

            if let Err(e) = self.engine.run_ast_with_scope(&mut scope, ast) {
                error!("Script execution error: {}", e);
            }

            // Extract the updated api from the scope
            if let Some(_updated_api) = scope.get_value::<GodModeApi>("api") {
                // Technically it's the same shared Rc<RefCell>, but we can just use our local reference
                let commands = self.api.drain();
                if !commands.is_empty() {
                    apply_commands(world, commands);
                }
            }
        }
    }
}
