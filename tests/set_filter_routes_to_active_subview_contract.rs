//! Source-level contract for the Run 2 Pass #34
//! `tool-setfilter-active-view-routing` user story.
//!
//! Pass #14 recorded sub-gap (2) of `empty-clipboard-state`: the
//! `SetFilter` stdin command's receiver — `set_filter_text_immediate`
//! in `src/app_impl/filter_input_updates.rs` — updated only
//! `self.filter_text` + the main-menu ranker path, never writing the
//! text into the active subview's variant `filter` field. The
//! user-facing consequence: stdin-driven `setFilter "xyz"` on
//! `ClipboardHistoryView` / `EmojiPickerView` / `AppLauncherView` /
//! `WindowSwitcherView` (and siblings) silently left the subview's
//! filter stale, so `getState.visibleChoiceCount` — computed in
//! `prompt_handler/mod.rs` from the *variant's* `filter` field
//! (see the `ClipboardHistoryView` arm around line 2120) — could
//! never drop to zero on a filter-miss, making the `empty-clipboard-state`
//! story's corrected acceptance clause (Pass #33's `visibleChoiceCount=0`)
//! structurally unreachable even after Pass #33 pinned the protocol field.
//!
//! The complementary path — UI text-change events via
//! `handle_filter_input_change` — already routes correctly through
//! per-variant match arms that call `sync_builtin_query_state` on the
//! subview's `filter` + `selected_index`. The asymmetry between the two
//! entry points is exactly what sub-gap (2) documented.
//!
//! Pass #34 fixes this by introducing
//! `ScriptListApp::write_filter_to_current_subview(&mut self, text: &str) -> bool`
//! and wiring it into `set_filter_text_immediate` so stdin-driven filter
//! writes land on the active subview's own field. A return of `true`
//! also suppresses the ScriptList-only fallback-mode branch, since
//! `get_filtered_results_cached` and `collect_fallbacks` both key on
//! the script list and would incorrectly flip a subview into fallback
//! rendering.
//!
//! This contract test pins the invariants at source level so a future
//! mechanical refactor cannot silently regress the routing or erase
//! the fallback-mode gate.

const FILTER_INPUT_UPDATES: &str = include_str!("../src/app_impl/filter_input_updates.rs");

#[test]
fn set_filter_text_immediate_delegates_to_subview_router_helper() {
    // The stdin-driven receiver MUST call `write_filter_to_current_subview`
    // before kicking off ScriptList-specific reconcile + fallback work.
    // Dropping this call would immediately reintroduce the Pass #14
    // sub-gap: stdin `setFilter` on a subview would go back to updating
    // only `self.filter_text`, leaving the variant's own `filter` field
    // stale and `getState.visibleChoiceCount` stuck on the pre-filter
    // dataset size.
    let receiver_pos = FILTER_INPUT_UPDATES
        .find("fn set_filter_text_immediate(")
        .expect(
            "src/app_impl/filter_input_updates.rs `set_filter_text_immediate` \
             receiver must exist — this is the single choke-point the \
             stdin `SetFilter` command routes through across all three \
             embedded dispatchers.",
        );
    // Scope the search to the receiver body so a stray usage elsewhere
    // cannot satisfy the contract accidentally.
    let body = &FILTER_INPUT_UPDATES[receiver_pos..];
    let body_end = body
        .find("\n    pub(crate) fn write_filter_to_current_subview(")
        .or_else(|| body.find("\n    pub(crate) fn clear_filter("))
        .unwrap_or(body.len());
    let receiver_body = &body[..body_end];

    assert!(
        receiver_body.contains("self.write_filter_to_current_subview(&text)"),
        "src/app_impl/filter_input_updates.rs `set_filter_text_immediate` must call \
         `self.write_filter_to_current_subview(&text)` so stdin-driven `setFilter` \
         writes to the active subview's variant `filter` field. Removing this \
         call regresses Pass #14 sub-gap (2) of `empty-clipboard-state`: \
         `getState.visibleChoiceCount` would never drop to zero on a \
         filter-miss because the variant's `filter` field stays stale."
    );

    assert!(
        receiver_body.contains("let handled_by_subview ="),
        "src/app_impl/filter_input_updates.rs `set_filter_text_immediate` must \
         bind the subview-router result into a `handled_by_subview` flag so \
         the ScriptList-only fallback-mode block below can gate on it. \
         Discarding the return value lets `collect_fallbacks` run on a \
         builtin subview, incorrectly flipping the subview into \
         script-list fallback rendering."
    );

    let compact_receiver_body: String = receiver_body.split_whitespace().collect();
    assert!(
        compact_receiver_body.contains("if!handled_by_subview&&!text.is_empty()"),
        "src/app_impl/filter_input_updates.rs `set_filter_text_immediate` must \
         guard its fallback-mode branch with `!handled_by_subview && !text.is_empty()`. \
         `get_filtered_results_cached` and `collect_fallbacks` are ScriptList-only; \
         running them on a builtin subview would cache stale script results and \
         activate the main-menu fallback state against the wrong surface."
    );
}

#[test]
fn set_filter_at_stays_in_shared_spine_route() {
    let receiver_pos = FILTER_INPUT_UPDATES
        .find("fn set_filter_text_immediate(")
        .expect("set_filter_text_immediate receiver must exist");
    let body = &FILTER_INPUT_UPDATES[receiver_pos..];
    let body_end = body
        .find("\n    pub(crate) fn write_filter_to_current_subview(")
        .or_else(|| body.find("\n    pub(crate) fn clear_filter("))
        .unwrap_or(body.len());
    let receiver_body = &body[..body_end];

    for needle in [
        "Self::special_entry_from_script_list_filter(&text)",
        "self.route_script_list_special_entry(entry, &text, window, cx)",
        "return;",
    ] {
        assert!(
            receiver_body.contains(needle),
            "stdin/devtools setFilter \"@\" must delegate ScriptList special entries to the shared router: {needle}"
        );
    }
}

#[test]
fn write_filter_to_current_subview_covers_all_shared_input_builtin_views() {
    // The router helper MUST have an arm for every shared-input builtin
    // subview documented in `current_view_uses_shared_filter_input`
    // (src/app_impl/filter_input_core.rs). Missing an arm would mean
    // stdin `setFilter` silently no-ops on that surface while visibly
    // updating the input field — the exact UX regression Pass #14
    // caught for clipboard history.
    //
    // `ScriptList` is intentionally NOT in this list — it is the
    // default-fallthrough (`_ => false`) so the main-menu ranker path
    // still runs on the script list.
    //
    // `FileSearchView` is also intentionally excluded — it has a
    // dedicated streaming router via `restart_file_search_stream_for_query`
    // that owns directory navigation + spotlight queries, and naively
    // writing `text` into `query` without kicking off a stream would
    // leave the displayed rows stale against the new query.
    let router_pos = FILTER_INPUT_UPDATES
        .find("fn write_filter_to_current_subview(")
        .expect(
            "src/app_impl/filter_input_updates.rs must define \
             `write_filter_to_current_subview(&mut self, text: &str) -> bool` — \
             this is the single dispatch point that routes stdin-driven \
             filter writes into the active subview's variant field.",
        );
    let router_body = &FILTER_INPUT_UPDATES[router_pos..];
    // Terminate the scan at the next `pub(crate) fn` sibling declaration
    // (e.g. `clear_filter`) so the assertions below stay scoped to the
    // router body.
    let next_fn_rel = router_body[1..].find("\n    pub(crate) fn ").expect(
        "`write_filter_to_current_subview` must be followed by another \
             `pub(crate) fn` sibling in the same impl block — a missing \
             sibling implies the impl block was truncated or the router \
             was left dangling.",
    );
    let router_section = &router_body[..=next_fn_rel];
    assert!(
        router_section.contains("_ => false,"),
        "src/app_impl/filter_input_updates.rs `write_filter_to_current_subview` \
         must terminate with a `_ => false,` fallthrough arm so unhandled \
         views (ScriptList, FileSearchView, AgentChatView, etc.) fall back \
         to the ScriptList-only path."
    );

    let required_subviews = [
        "AppView::ClipboardHistoryView",
        "AppView::AppLauncherView",
        "AppView::WindowSwitcherView",
        "AppView::BrowserTabsView",
        "AppView::ThemeChooserView",
        "AppView::ProcessManagerView",
        "AppView::SettingsView",
        "AppView::SearchAiPresetsView",
        "AppView::FavoritesBrowseView",
        "AppView::CurrentAppCommandsView",
        "AppView::AgentChatHistoryView",
        "AppView::BrowserHistoryView",
        "AppView::DictationHistoryView",
        "AppView::NotesBrowseView",
        "AppView::EmojiPickerView",
    ];
    for view in required_subviews {
        assert!(
            router_section.contains(view),
            "src/app_impl/filter_input_updates.rs `write_filter_to_current_subview` \
             is missing an arm for `{view}`. Every shared-input builtin \
             subview listed in `current_view_uses_shared_filter_input` \
             (src/app_impl/filter_input_core.rs) must have a dispatch arm \
             here; otherwise stdin `setFilter` on that surface will \
             silently no-op and `getState.visibleChoiceCount` will stay \
             stuck on the pre-filter dataset size."
        );
    }

    // Every dispatch arm must call `sync_builtin_query_state` so the
    // `{filter, selected_index}` pair is updated atomically (filter
    // replaced, selection reset to 0). A half-update — writing the
    // filter but not resetting `selected_index` — would leave the
    // caret pointing at an out-of-range row after a narrowing filter,
    // a bug class `handle_filter_input_change` already defends against.
    let arm_count = router_section
        .match_indices("Self::sync_builtin_query_state(filter, selected_index, text);")
        .count();
    assert!(
        arm_count >= required_subviews.len(),
        "src/app_impl/filter_input_updates.rs `write_filter_to_current_subview` \
         must call `Self::sync_builtin_query_state(filter, selected_index, text)` \
         in every arm — counted {arm_count} call sites but need at least \
         {} for the shared-input builtin subviews. Writing the filter \
         without also resetting `selected_index = 0` can leave the caret \
         pointing past the end of the narrowed list.",
        required_subviews.len(),
    );
}

#[test]
fn write_filter_to_current_subview_returns_false_for_script_list_and_file_search() {
    // The fallthrough arm `_ => false` is load-bearing: it tells
    // `set_filter_text_immediate` that for `ScriptList` (and any future
    // non-shared-input view like `AgentChatView` or `ScratchPadView`)
    // the ScriptList-only ranker + fallback-state work MUST still run.
    // Converting this to `_ => true` would silently disable the
    // script-list filter pipeline — every keystroke in the main menu
    // would stop narrowing scripts while still updating `self.filter_text`.
    let router_pos = FILTER_INPUT_UPDATES
        .find("fn write_filter_to_current_subview(")
        .expect("`write_filter_to_current_subview` must exist");
    let router_body = &FILTER_INPUT_UPDATES[router_pos..];

    assert!(
        router_body.contains("_ => false,"),
        "src/app_impl/filter_input_updates.rs `write_filter_to_current_subview` \
         must have a `_ => false` fallthrough arm so ScriptList (the default \
         view) returns false and the ScriptList-only ranker + fallback-state \
         branch in `set_filter_text_immediate` still runs. Flipping this \
         to `_ => true` would silently disable main-menu filtering."
    );

    // FileSearchView has dedicated streaming routing that owns directory
    // navigation and cancellation — naively writing `new_text` into its
    // `query` field without kicking off a stream would leave displayed
    // rows stale. Pin its absence from the router helper.
    let next_fn_rel = router_body[1..]
        .find("\n    pub(crate) fn ")
        .expect("router must be followed by a sibling `pub(crate) fn`");
    let router_section = &router_body[..=next_fn_rel];
    assert!(
        !router_section.contains("AppView::FileSearchView"),
        "src/app_impl/filter_input_updates.rs `write_filter_to_current_subview` \
         must NOT include an `AppView::FileSearchView` arm. FileSearchView \
         has its own streaming router (`restart_file_search_stream_for_query`) \
         that owns directory navigation + spotlight cancellation; writing \
         into the `query` field here would leave the displayed rows stale \
         against the new query. The stdin `setFilter` path for file search \
         is intentionally a no-op and should remain one until a dedicated \
         stdin command exists."
    );
}
