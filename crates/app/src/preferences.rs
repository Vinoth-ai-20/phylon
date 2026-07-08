//! # User Preferences Persistence
//!
//! ## 1. What Happens
//! `Preferences` is a small, `.ron`-serialized settings file — distinct from
//! `config::PhylonConfig` (which describes one simulation *experiment*'s
//! setup: tick rate, RNG seed, headless mode) — covering cosmetic,
//! cross-session UI preferences a person would expect to persist: High
//! Contrast Mode, the UI scale factor, and whether the first-run onboarding
//! hints dialog (Phase 5, SX-9a) has ever been shown.
//!
//! ## 2. Why It Happens
//! Phase 6's audit found no application-preferences persistence mechanism
//! existed anywhere in this codebase — `WorkbenchState::show_onboarding_hints`
//! was, by necessity, session-scoped only (re-shown every restart), and
//! toggling High Contrast Mode or the UI scale slider never survived closing
//! the app. This closes that gap for the smallest, clearest set of settings
//! actually worth remembering — not an attempt to persist every
//! `WorkbenchState` field (most of it, like camera position or selection, is
//! legitimately session-only).
//!
//! ## 3. How It Happens
//! Loaded once at [`crate::app::PhylonApp::new`] and applied to the initial
//! [`ui::WorkbenchState`]. Saved at the two real exit paths this app has —
//! `MenuAction::Quit` and the window's `CloseRequested` event (see
//! `events.rs`) — mirroring `research::ExperimentManifest`'s own
//! `ron::ser::to_string_pretty`/`ron::de::from_str` save/load pattern. A
//! missing or corrupt preferences file is not a hard error (unlike
//! `PhylonConfig::load`, where a bad simulation config legitimately should
//! fail loudly) — cosmetic preferences default and log a warning instead,
//! since nothing about the simulation itself depends on them.

use std::path::{Path, PathBuf};

/// Cross-session UI preferences. See this module's doc comment for what is
/// (and deliberately isn't) covered.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Preferences {
    /// Mirrors `ui::WorkbenchState::high_contrast`.
    pub(crate) high_contrast: bool,
    /// Mirrors `ui::WorkbenchState::ui_scale`.
    pub(crate) ui_scale: f32,
    /// Whether the first-run onboarding hints dialog (Phase 5, SX-9a) has
    /// ever been shown, across all sessions — distinct from
    /// `WorkbenchState::show_onboarding_hints`, which only tracks whether
    /// it's showing *right now* in the current session.
    pub(crate) onboarding_seen: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            high_contrast: false,
            ui_scale: 1.0,
            onboarding_seen: false,
        }
    }
}

/// Where the preferences file lives — a flat file next to `data/default.ron`,
/// the same relative-to-cwd convention every other on-disk artifact in this
/// app already uses (`data/experiments/`, `./screenshots/`, `./recordings/`).
pub(crate) fn preferences_path() -> PathBuf {
    PathBuf::from("data/preferences.ron")
}

impl Preferences {
    /// Loads preferences from `path`, falling back to
    /// [`Preferences::default`] (with a warning logged, not a hard error) if
    /// the file is absent, unreadable, or fails to parse.
    pub(crate) fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(e) => {
                tracing::warn!("failed to read preferences file {path:?}: {e}, using defaults");
                return Self::default();
            }
        };
        match ron::de::from_str(&text) {
            Ok(prefs) => prefs,
            Err(e) => {
                tracing::warn!("failed to parse preferences file {path:?}: {e}, using defaults");
                Self::default()
            }
        }
    }

    /// Saves preferences to `path`, creating the parent directory if needed.
    /// Failures are logged, not propagated — losing a cosmetic-preference
    /// write is not worth interrupting shutdown over.
    pub(crate) fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("failed to create preferences directory {parent:?}: {e}");
                return;
            }
        }
        match ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()) {
            Ok(text) => {
                if let Err(e) = std::fs::write(path, text) {
                    tracing::warn!("failed to write preferences file {path:?}: {e}");
                }
            }
            Err(e) => tracing::warn!("failed to serialize preferences: {e}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_missing_file_returns_default() {
        let prefs = Preferences::load(Path::new("does_not_exist_preferences.ron"));
        assert!(!prefs.high_contrast);
        assert_eq!(prefs.ui_scale, 1.0);
        assert!(!prefs.onboarding_seen);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = std::env::temp_dir().join(format!("phylon_prefs_test_{}", std::process::id()));
        let path = dir.join("preferences.ron");

        let prefs = Preferences {
            high_contrast: true,
            ui_scale: 1.5,
            onboarding_seen: true,
        };
        prefs.save(&path);

        let loaded = Preferences::load(&path);
        assert!(loaded.high_contrast);
        assert_eq!(loaded.ui_scale, 1.5);
        assert!(loaded.onboarding_seen);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_corrupt_file_returns_default_not_a_panic() {
        let dir =
            std::env::temp_dir().join(format!("phylon_prefs_corrupt_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("preferences.ron");
        std::fs::write(&path, "not valid ron at all {{{").unwrap();

        let prefs = Preferences::load(&path);
        assert!(!prefs.high_contrast);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
