//! Reusable recent-items tracking (Phase 7, W0d) — a generic, capped,
//! most-recently-used list, usable by any category of "things the user
//! recently opened." Only `Files` has a real producer today (`SaveState`/
//! `LoadState` in `crates/app/src/events.rs`); the other categories are
//! named extension points, not built features — see `RecentCategory`'s
//! doc comment.
//!
//! Replaces the pre-Phase-7 `WorkbenchState::recent_files: Vec<String>`,
//! which was never actually populated by anything (a confirmed dead
//! control — the "Open Recent" submenu could never render, and even if it
//! had, clicking an entry opened a generic file picker instead of that
//! entry's own path). This module is the fix, plus the reusable shape so
//! the same bug pattern doesn't get re-invented per category.
//!
//! ## Policies (binding for every consumer of this module)
//!
//! - **Ordering**: most-recently-used first. [`RecentItemsService::record`]
//!   always moves an entry to the front, whether it's new or already
//!   present.
//! - **Duplicate handling**: never duplicated. Recording an already-present
//!   path removes the old entry before reinserting at the front, rather
//!   than allowing two entries for the same path.
//! - **Maximum history size**: `RecentItemsList::MAX_ITEMS` (private —
//!   internal implementation detail, not a public link target). Oldest
//!   entries are silently dropped past this cap — normal LRU eviction, not
//!   an error condition.
//! - **Missing-file behavior**: this module does zero filesystem I/O and
//!   never removes an entry just because a file might be missing —
//!   silently pruning history behind the user's back would be exactly the
//!   kind of surprising behavior this milestone is fixing, not adding.
//!   Checking whether a path still exists, and presenting a distinct
//!   disabled/"missing" state for entries that don't, is the UI layer's
//!   job (see `crates/ui/src/plugins/menu.rs`); explicit removal via
//!   [`RecentItemsService::remove`] is always a user action, never
//!   automatic.
//! - **Persistence**: serialized as part of `app::preferences::Preferences`
//!   (RON) — the same mechanism `high_contrast`/`ui_scale`/`onboarding_seen`
//!   already use, loaded on startup and saved at the same exit paths.
//! - **Future extension points**: add a [`RecentCategory`] variant; nothing
//!   else in this module changes shape. Wiring a real producer for that
//!   category (e.g. recording on replay-bundle open) is a separate, later
//!   change — this module only owns the generic list/cap/dedupe mechanics.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Which category of "recently used" list an item belongs to. Only
/// [`RecentCategory::Files`] has a real producer as of Phase 7, W0d — the
/// rest are named now so their storage/persistence shape already exists
/// once a real source for them is built, rather than each needing a new
/// ad hoc field added to `WorkbenchState` later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RecentCategory {
    /// Simulation state save/load paths (`.bin` snapshots).
    Files,
    /// Replay bundle paths — not yet wired to a producer.
    Replays,
    /// Research experiment manifests — not yet wired to a producer.
    Experiments,
    /// Exported artifact paths (CSV/JSON/PNG) — not yet wired to a producer.
    Exports,
    /// Saved workspace/panel-layout presets — not yet wired to a producer;
    /// pairs naturally with Epic W3's layout-persistence work once that
    /// lands.
    WorkspaceLayouts,
}

/// One category's capped, ordered, deduplicated recent-items list. Not
/// constructed directly by consumers — see [`RecentItemsService`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RecentItemsList {
    items: Vec<String>,
}

impl RecentItemsList {
    /// Maximum history size per category (see module doc comment's policy
    /// list). Ten was chosen to match how many entries comfortably fit in
    /// a menu submenu without scrolling — not independently tuned.
    const MAX_ITEMS: usize = 10;

    fn record(&mut self, path: String) {
        self.items.retain(|p| p != &path);
        self.items.insert(0, path);
        self.items.truncate(Self::MAX_ITEMS);
    }

    fn remove(&mut self, path: &str) {
        self.items.retain(|p| p != path);
    }
}

/// The single pathway for recording and reading recent-items state —
/// `menu.rs` and any future consumer should only ever go through this, per
/// this module's own doc-comment policy list. Mirrors the "one canonical
/// mutation API" discipline `ADR-W0-01` established for selection state,
/// applied here to recent-items state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentItemsService {
    lists: HashMap<RecentCategory, RecentItemsList>,
}

impl RecentItemsService {
    /// Records `path` as just-used in `category` (ordering/duplicate/cap
    /// policy — see module doc comment).
    pub fn record(&mut self, category: RecentCategory, path: impl Into<String>) {
        self.lists.entry(category).or_default().record(path.into());
    }

    /// Explicitly removes `path` from `category` — the only way an entry
    /// ever disappears from history (see the module doc comment's
    /// missing-file policy: never automatic).
    pub fn remove(&mut self, category: RecentCategory, path: &str) {
        if let Some(list) = self.lists.get_mut(&category) {
            list.remove(path);
        }
    }

    /// Most-recently-used first; an empty iterator if nothing has been
    /// recorded for `category` yet.
    pub fn items(&self, category: RecentCategory) -> impl Iterator<Item = &str> {
        self.lists
            .get(&category)
            .into_iter()
            .flat_map(|l| l.items.iter().map(String::as_str))
    }

    /// Whether `category` currently has no recorded items.
    pub fn is_empty(&self, category: RecentCategory) -> bool {
        self.lists
            .get(&category)
            .map(|l| l.items.is_empty())
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_orders_most_recently_used_first() {
        let mut service = RecentItemsService::default();
        service.record(RecentCategory::Files, "a.bin");
        service.record(RecentCategory::Files, "b.bin");
        service.record(RecentCategory::Files, "c.bin");
        let items: Vec<&str> = service.items(RecentCategory::Files).collect();
        assert_eq!(items, vec!["c.bin", "b.bin", "a.bin"]);
    }

    #[test]
    fn recording_an_existing_path_moves_it_to_front_without_duplicating() {
        let mut service = RecentItemsService::default();
        service.record(RecentCategory::Files, "a.bin");
        service.record(RecentCategory::Files, "b.bin");
        service.record(RecentCategory::Files, "a.bin");
        let items: Vec<&str> = service.items(RecentCategory::Files).collect();
        assert_eq!(
            items,
            vec!["a.bin", "b.bin"],
            "must not contain a duplicate a.bin entry"
        );
    }

    #[test]
    fn history_is_capped_at_max_items_dropping_the_oldest() {
        let mut service = RecentItemsService::default();
        for i in 0..(RecentItemsList::MAX_ITEMS + 3) {
            service.record(RecentCategory::Files, format!("{i}.bin"));
        }
        let items: Vec<&str> = service.items(RecentCategory::Files).collect();
        assert_eq!(items.len(), RecentItemsList::MAX_ITEMS);
        // The most recent MAX_ITEMS entries survive; the oldest 3 dropped.
        assert_eq!(items[0], format!("{}.bin", RecentItemsList::MAX_ITEMS + 2));
        assert!(!items.contains(&"0.bin"));
    }

    #[test]
    fn remove_deletes_only_the_named_entry() {
        let mut service = RecentItemsService::default();
        service.record(RecentCategory::Files, "a.bin");
        service.record(RecentCategory::Files, "b.bin");
        service.remove(RecentCategory::Files, "a.bin");
        let items: Vec<&str> = service.items(RecentCategory::Files).collect();
        assert_eq!(items, vec!["b.bin"]);
    }

    #[test]
    fn categories_are_independent() {
        let mut service = RecentItemsService::default();
        service.record(RecentCategory::Files, "a.bin");
        service.record(RecentCategory::Replays, "a.replay");
        assert_eq!(
            service.items(RecentCategory::Files).collect::<Vec<_>>(),
            vec!["a.bin"]
        );
        assert_eq!(
            service.items(RecentCategory::Replays).collect::<Vec<_>>(),
            vec!["a.replay"]
        );
        assert!(service.is_empty(RecentCategory::Experiments));
    }

    #[test]
    fn a_never_recorded_category_is_empty_not_a_panic() {
        let service = RecentItemsService::default();
        assert!(service.is_empty(RecentCategory::Exports));
        assert_eq!(service.items(RecentCategory::Exports).count(), 0);
    }
}
