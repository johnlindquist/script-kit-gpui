//! Oracle-Session `protocol-builtin-boundary-refactor-plan` PR5b +
//! PR5c: pure, window-free planner from [`TriggerBuiltin`] to a narrow
//! [`AppRoute`] description of the intended UI transition, now wired
//! into the live dispatcher.
//!
//! The dispatcher in `trigger_builtin_dispatch.rs` mixes three
//! concerns inside a single match:
//!
//! 1. Decide *which* view the app should move to (pure).
//! 2. Seed per-view caches / placeholder strings / focus targets
//!    (mostly pure, some fallible IO for the cache seeds).
//! 3. Mutate `self` and deferred-resize via `window`/`Context`
//!    (imperative, GPUI-bound).
//!
//! Concern (1) is the piece that should be testable without a real
//! window, and it is also the piece where a future enum addition is
//! most likely to silently drift from the dispatcher. This module
//! extracts it as pure data.
//!
//! As of PR5c the live `apply_trigger_builtin` in
//! `trigger_builtin_dispatch.rs` matches on
//! [`plan_trigger_builtin_route`]'s output, so a new `TriggerBuiltin`
//! variant that forgets to produce a route is a compile break here
//! AND in the dispatcher — not a runtime no-op. The inline audit test
//! `apply_trigger_builtin_is_wired_through_planner` pins that wiring
//! so a future refactor cannot silently re-inline the match.

use crate::builtins::trigger_registry::TriggerBuiltin;

/// Narrow description of the route the app should enter after a
/// `triggerBuiltin` succeeds.
///
/// Variants intentionally omit `Window`/`Context` handles, cache
/// seed data, and `update_window_size_deferred` calls — those are
/// the imperative half that stays in the dispatcher for now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppRoute {
    /// Open a filterable list view of `kind`. The dispatcher is
    /// responsible for seeding any view-specific cache, resetting
    /// `filter_text` / `pending_filter_sync` / `hovered_index`, and
    /// issuing the window resize.
    ShowFilterableView(FilterableView),
    /// Open the shared file-search prompt. The dispatcher calls
    /// [`crate::app_impl`] `ScriptListApp::open_file_search`.
    OpenFileSearch,
    /// Open the Tab-AI Agent Chat surface with no entry intent.
    OpenTabAi,
    /// Open the "Do in current app" commands list via the existing
    /// menu-bar helper.
    OpenCurrentAppCommands,
    /// Execute a regular launcher built-in by its canonical command id.
    ExecuteBuiltin(&'static str),
}

/// The filterable views reachable from `triggerBuiltin`. A separate
/// enum from [`AppRoute`] so follow-up refactors can widen it (e.g.
/// Favorites, Settings) without forcing every `AppRoute` caller to
/// grow new arms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilterableView {
    ClipboardHistory,
    AppLauncher,
    BrowserTabs,
    EmojiPicker,
    WindowSwitcher,
    ProcessManager,
    KitStoreBrowse,
    KitStoreInstalled,
    Favorites,
    SearchAiPresets,
}

impl FilterableView {
    /// All variants in declaration order. Used by the reverse
    /// coverage test to pin that every `FilterableView` is the
    /// target of *some* `TriggerBuiltin`.
    pub const ALL: &'static [FilterableView] = &[
        FilterableView::ClipboardHistory,
        FilterableView::AppLauncher,
        FilterableView::BrowserTabs,
        FilterableView::EmojiPicker,
        FilterableView::WindowSwitcher,
        FilterableView::ProcessManager,
        FilterableView::KitStoreBrowse,
        FilterableView::KitStoreInstalled,
        FilterableView::Favorites,
        FilterableView::SearchAiPresets,
    ];

    /// Stable variant name used by the golden-JSONL fixture. Keep
    /// these strings exactly in sync with the Rust identifier — the
    /// golden trace in `tests/golden/trigger_builtin/routes.jsonl`
    /// renders as `"ShowFilterableView::{name}"` and a rename here
    /// without updating the fixture is intentionally a test failure.
    pub const fn name(self) -> &'static str {
        match self {
            FilterableView::ClipboardHistory => "ClipboardHistory",
            FilterableView::AppLauncher => "AppLauncher",
            FilterableView::BrowserTabs => "BrowserTabs",
            FilterableView::EmojiPicker => "EmojiPicker",
            FilterableView::WindowSwitcher => "WindowSwitcher",
            FilterableView::ProcessManager => "ProcessManager",
            FilterableView::KitStoreBrowse => "KitStoreBrowse",
            FilterableView::KitStoreInstalled => "KitStoreInstalled",
            FilterableView::Favorites => "Favorites",
            FilterableView::SearchAiPresets => "SearchAiPresets",
        }
    }
}

/// Stable one-line rendering of an [`AppRoute`] for
/// golden-transcript tests. The format is intentionally narrow:
///
/// * `AppRoute::ShowFilterableView(v)` → `"ShowFilterableView::{v}"`
/// * `AppRoute::OpenFileSearch` → `"OpenFileSearch"`
/// * `AppRoute::OpenTabAi` → `"OpenTabAi"`
/// * `AppRoute::OpenCurrentAppCommands` → `"OpenCurrentAppCommands"`
/// * `AppRoute::ExecuteBuiltin(id)` → `"ExecuteBuiltin::{id}"`
///
/// Changing these strings is a breaking change to the route
/// golden fixture and will fail
/// `tests/trigger_builtin_route_golden.rs` until the fixture is
/// regenerated intentionally.
pub fn render_route(route: &AppRoute) -> String {
    match route {
        AppRoute::ShowFilterableView(view) => format!("ShowFilterableView::{}", view.name()),
        AppRoute::OpenFileSearch => "OpenFileSearch".to_string(),
        AppRoute::OpenTabAi => "OpenTabAi".to_string(),
        AppRoute::OpenCurrentAppCommands => "OpenCurrentAppCommands".to_string(),
        AppRoute::ExecuteBuiltin(id) => format!("ExecuteBuiltin::{id}"),
    }
}

/// Inverse of [`render_route`]. Parses the stable wire format back
/// into an [`AppRoute`], returning `None` for anything the planner
/// cannot produce. The unit test `parse_route_round_trips_render_route`
/// pins that the two functions are bidirectional for every route the
/// planner can emit, so the rendered form is a true serialization
/// contract — Bun or MCP consumers can encode a route string and
/// Rust will ingest it without a second lookup table.
pub fn parse_route(rendered: &str) -> Option<AppRoute> {
    match rendered {
        "OpenFileSearch" => Some(AppRoute::OpenFileSearch),
        "OpenTabAi" => Some(AppRoute::OpenTabAi),
        "OpenCurrentAppCommands" => Some(AppRoute::OpenCurrentAppCommands),
        other if other.starts_with("ExecuteBuiltin::builtin/") => {
            Some(AppRoute::ExecuteBuiltin(Box::leak(
                other["ExecuteBuiltin::".len()..]
                    .to_string()
                    .into_boxed_str(),
            )))
        }
        other => {
            let view_name = other.strip_prefix("ShowFilterableView::")?;
            let view = FilterableView::ALL
                .iter()
                .copied()
                .find(|v| v.name() == view_name)?;
            Some(AppRoute::ShowFilterableView(view))
        }
    }
}

/// Pure planner: decide which [`AppRoute`] a resolved
/// [`TriggerBuiltin`] should enter. No GPUI context, no IO, no
/// `self`-mutation — just a total function over the enum.
///
/// Adding a new `TriggerBuiltin` variant forces a new arm here,
/// which means the dispatcher cannot silently drop the new variant
/// into a no-op `_` branch.
pub const fn plan_trigger_builtin_route(id: TriggerBuiltin) -> AppRoute {
    match id {
        TriggerBuiltin::ClipboardHistory => {
            AppRoute::ShowFilterableView(FilterableView::ClipboardHistory)
        }
        TriggerBuiltin::AppLauncher => AppRoute::ShowFilterableView(FilterableView::AppLauncher),
        TriggerBuiltin::FileSearch => AppRoute::OpenFileSearch,
        TriggerBuiltin::BrowserTabs => AppRoute::ShowFilterableView(FilterableView::BrowserTabs),
        TriggerBuiltin::EmojiPicker => AppRoute::ShowFilterableView(FilterableView::EmojiPicker),
        TriggerBuiltin::WindowSwitcher => {
            AppRoute::ShowFilterableView(FilterableView::WindowSwitcher)
        }
        TriggerBuiltin::TabAi => AppRoute::OpenTabAi,
        TriggerBuiltin::ProcessManager => {
            AppRoute::ShowFilterableView(FilterableView::ProcessManager)
        }
        TriggerBuiltin::CurrentAppCommands => AppRoute::OpenCurrentAppCommands,
        TriggerBuiltin::NewScript => AppRoute::ExecuteBuiltin("builtin/new-script"),
        TriggerBuiltin::SdkReference => AppRoute::ExecuteBuiltin("builtin/sdk-reference"),
        TriggerBuiltin::AiVault => AppRoute::ExecuteBuiltin("builtin/vault"),
        TriggerBuiltin::Settings => AppRoute::ExecuteBuiltin("builtin/settings"),
        TriggerBuiltin::BrowseKitStore => {
            AppRoute::ShowFilterableView(FilterableView::KitStoreBrowse)
        }
        TriggerBuiltin::ManageInstalledKits => {
            AppRoute::ShowFilterableView(FilterableView::KitStoreInstalled)
        }
        TriggerBuiltin::Favorites => AppRoute::ShowFilterableView(FilterableView::Favorites),
        TriggerBuiltin::SearchAiPresets => {
            AppRoute::ShowFilterableView(FilterableView::SearchAiPresets)
        }
        TriggerBuiltin::ScriptTemplateCatalog => {
            AppRoute::ExecuteBuiltin("builtin/new-script-from-template")
        }
        TriggerBuiltin::Dictation => AppRoute::ExecuteBuiltin("builtin/dictation"),
        TriggerBuiltin::DictationHistory => AppRoute::ExecuteBuiltin("builtin/dictation-history"),
        TriggerBuiltin::ChooseTheme => AppRoute::ExecuteBuiltin("builtin/choose-theme"),
        TriggerBuiltin::ScriptKitSelfie => AppRoute::ExecuteBuiltin("builtin/script-kit-selfie"),
        TriggerBuiltin::MainWindow => AppRoute::ExecuteBuiltin("builtin/main-window"),
        TriggerBuiltin::QuickTerminal => AppRoute::ExecuteBuiltin("builtin/quick-terminal"),
        TriggerBuiltin::Webcam => AppRoute::ExecuteBuiltin("builtin/webcam"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trigger_builtin_has_a_route() {
        for &id in TriggerBuiltin::ALL {
            // Just calling the const fn exhaustively is the whole
            // point — if a variant goes missing we fail to compile.
            let _route = plan_trigger_builtin_route(id);
        }
    }

    #[test]
    fn every_filterable_view_is_reachable() {
        use std::collections::BTreeSet;
        let reached: BTreeSet<FilterableView> = TriggerBuiltin::ALL
            .iter()
            .filter_map(|&id| match plan_trigger_builtin_route(id) {
                AppRoute::ShowFilterableView(v) => Some(v),
                _ => None,
            })
            .collect();
        let declared: BTreeSet<FilterableView> = FilterableView::ALL.iter().copied().collect();
        assert_eq!(
            reached, declared,
            "FilterableView::ALL and the set of views produced by the planner must agree — \
             either a trigger-builtin stopped opening a filterable view, or a FilterableView \
             variant was added without a trigger-builtin to route to it."
        );
    }

    #[test]
    fn non_filterable_routes_are_one_to_one() {
        use std::collections::BTreeMap;
        // Each non-filterable AppRoute variant must correspond to
        // exactly one TriggerBuiltin, so renaming a route without
        // touching the planner surfaces here.
        let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
        for &id in TriggerBuiltin::ALL {
            let tag = match plan_trigger_builtin_route(id) {
                AppRoute::ShowFilterableView(_) => continue,
                AppRoute::OpenFileSearch => "OpenFileSearch",
                AppRoute::OpenTabAi => "OpenTabAi",
                AppRoute::OpenCurrentAppCommands => "OpenCurrentAppCommands",
                AppRoute::ExecuteBuiltin(id) => id,
            };
            *counts.entry(tag).or_default() += 1;
        }
        for (tag, n) in &counts {
            assert_eq!(
                *n, 1,
                "non-filterable AppRoute `{tag}` must be produced by exactly one TriggerBuiltin, got {n}"
            );
        }
        let expected: BTreeMap<&'static str, usize> = [
            ("OpenFileSearch", 1usize),
            ("OpenTabAi", 1),
            ("OpenCurrentAppCommands", 1),
            ("builtin/choose-theme", 1),
            ("builtin/dictation", 1),
            ("builtin/main-window", 1),
            ("builtin/dictation-history", 1),
            ("builtin/new-script", 1),
            ("builtin/new-script-from-template", 1),
            ("builtin/quick-terminal", 1),
            ("builtin/script-kit-selfie", 1),
            ("builtin/sdk-reference", 1),
            ("builtin/settings", 1),
            ("builtin/vault", 1),
            ("builtin/webcam", 1),
        ]
        .into_iter()
        .collect();
        assert_eq!(counts, expected);
    }

    #[test]
    fn render_route_produces_stable_strings() {
        // Pin the rendering format used by the golden-JSONL fixture.
        assert_eq!(render_route(&AppRoute::OpenFileSearch), "OpenFileSearch");
        assert_eq!(render_route(&AppRoute::OpenTabAi), "OpenTabAi");
        assert_eq!(
            render_route(&AppRoute::OpenCurrentAppCommands),
            "OpenCurrentAppCommands"
        );
        assert_eq!(
            render_route(&AppRoute::ShowFilterableView(
                FilterableView::ClipboardHistory
            )),
            "ShowFilterableView::ClipboardHistory"
        );
        // Every FilterableView must produce a non-empty `name()` and
        // must round-trip through `render_route` without swallowing
        // the view identity.
        for &v in FilterableView::ALL {
            let name = v.name();
            assert!(
                !name.is_empty(),
                "FilterableView::{v:?}.name() must be non-empty"
            );
            let rendered = render_route(&AppRoute::ShowFilterableView(v));
            assert_eq!(rendered, format!("ShowFilterableView::{name}"));
        }
    }

    #[test]
    fn specific_known_routes_are_stable() {
        // Belt-and-braces: pin a few concrete mappings so a silent
        // rewire (e.g. FileSearch → a filterable view) fails loudly.
        assert_eq!(
            plan_trigger_builtin_route(TriggerBuiltin::FileSearch),
            AppRoute::OpenFileSearch
        );
        assert_eq!(
            plan_trigger_builtin_route(TriggerBuiltin::TabAi),
            AppRoute::OpenTabAi
        );
        assert_eq!(
            plan_trigger_builtin_route(TriggerBuiltin::CurrentAppCommands),
            AppRoute::OpenCurrentAppCommands
        );
        assert_eq!(
            plan_trigger_builtin_route(TriggerBuiltin::ClipboardHistory),
            AppRoute::ShowFilterableView(FilterableView::ClipboardHistory)
        );
    }

    #[test]
    fn parse_route_round_trips_render_route() {
        for &id in TriggerBuiltin::ALL {
            let route = plan_trigger_builtin_route(id);
            let rendered = render_route(&route);
            assert_eq!(
                parse_route(&rendered),
                Some(route),
                "parse_route must invert render_route for {id:?} → {rendered}"
            );
        }
    }

    #[test]
    fn apply_trigger_builtin_is_wired_through_planner() {
        // Source-text audit: PR5c wired `apply_trigger_builtin` to
        // match on `plan_trigger_builtin_route(id)` so a new
        // `TriggerBuiltin` variant forces a planner arm AND a
        // dispatcher arm. If a future refactor re-inlines the match
        // (matching on `id` directly), the exhaustiveness guarantee
        // from this module is silently lost. Catch that here, not at
        // a merge-later runtime no-op.
        let path = "src/app_impl/trigger_builtin_dispatch.rs";
        let source =
            std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path}"));
        assert!(
            source.contains("match plan_trigger_builtin_route(id)"),
            "{path} must route `apply_trigger_builtin` through \
             `plan_trigger_builtin_route` (see src/app_impl/routes.rs)"
        );
    }

    #[test]
    fn dispatch_trigger_builtin_name_delegates_to_typed_entry() {
        // Source-text audit for Oracle-Session
        // `protocol-builtin-boundary-engineering-plan` Pass 4 (rank
        // #3, sub-pass 1). The string-entry dispatcher must forward
        // resolved `TriggerBuiltin`s to the typed entry point so the
        // string↔enum boundary stays a one-way door. A future
        // refactor that re-inlines `apply_trigger_builtin(resolved,
        // …)` directly inside `dispatch_trigger_builtin_name` would
        // defeat the whole point of Oracle #3: the plan is to
        // eventually delete the string entry point and move
        // resolution into ingress, which is only safe while the
        // typed entry point is the sole bridge to
        // `apply_trigger_builtin`.
        let path = "src/app_impl/trigger_builtin_dispatch.rs";
        let source =
            std::fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read {path}"));
        assert!(
            source.contains("pub fn dispatch_trigger_builtin_enum"),
            "{path} must expose typed entry point `dispatch_trigger_builtin_enum`"
        );
        assert!(
            source.contains("self.dispatch_trigger_builtin_enum(resolved"),
            "{path} `dispatch_trigger_builtin_name` must forward resolved \
             TriggerBuiltin into the typed entry point, not call \
             `apply_trigger_builtin` directly"
        );
    }

    #[test]
    fn parse_route_rejects_unknown_strings() {
        assert_eq!(parse_route(""), None);
        assert_eq!(parse_route("Unknown"), None);
        assert_eq!(parse_route("ShowFilterableView"), None);
        assert_eq!(parse_route("ShowFilterableView::"), None);
        assert_eq!(parse_route("ShowFilterableView::NotAView"), None);
        // Case-sensitive: lowercased tags must not match.
        assert_eq!(parse_route("openfilesearch"), None);
        assert_eq!(
            parse_route("ShowFilterableView::clipboardhistory"),
            None,
            "case-sensitive view names must not round-trip"
        );
    }
}
