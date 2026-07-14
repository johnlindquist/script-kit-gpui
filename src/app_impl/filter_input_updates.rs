use super::*;

const FILTER_COMPUTE_DEFER: std::time::Duration = std::time::Duration::from_millis(16);

/// Leading characters that put the launcher list into a structurally
/// different presentation (sigil hint surfaces, capture composer, command
/// heads, spine projections, fallback "Use X with..." sections) rather than
/// just narrowing the fuzzy rows.
const LIST_STRUCTURE_SIGILS: &[char] = &['@', '/', ';', ':', '!', '~', '|', '>', '+', '.'];

/// Which structural "family" of launcher list a filter query renders.
#[derive(Debug, PartialEq, Eq)]
enum FilterListFamily {
    /// Empty query: default sections (Brain Inbox, Suggested, ...).
    Default,
    /// Sigil-headed query (`@`, `/`, `;`, ...): hint/fallback/projection
    /// surfaces keyed to that sigil.
    Sigil(char),
    /// Menu-syntax qualifier head that owns the main list (`has:`, `t:`, ...).
    QualifierOwned,
    /// Plain fuzzy-filter text.
    Plain,
}

fn filter_list_family(text: &str) -> FilterListFamily {
    if text.is_empty() {
        return FilterListFamily::Default;
    }
    if let Some(head) = text.trim_start().chars().next() {
        if LIST_STRUCTURE_SIGILS.contains(&head) {
            return FilterListFamily::Sigil(head);
        }
    }
    if crate::menu_syntax::active_filter_head_owns_main_list(text) {
        FilterListFamily::QualifierOwned
    } else {
        FilterListFamily::Plain
    }
}

/// The input echoes every keystroke immediately, while the grouped list
/// follows `computed_filter_text` after the coalescer defer. Within one
/// list family a one-frame-stale list is imperceptible (fuzzy rows narrow
/// slightly), but across families (default sections vs the `Use "@" with...`
/// fallback vs a sigil-owned surface) the stale frame reads as a flash of
/// the wrong screen. Those transitions must apply in the same frame as the
/// input echo instead of waiting out `FILTER_COMPUTE_DEFER`.
pub(crate) fn filter_change_flips_list_structure(old: &str, new: &str) -> bool {
    filter_list_family(old) != filter_list_family(new)
}

pub(crate) fn menu_syntax_filter_only_escape_should_clear(
    raw: &str,
    mode: &crate::menu_syntax::MenuSyntaxMode,
) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }

    if let Some(rest) = trimmed.strip_prefix(':') {
        return !rest.chars().any(char::is_whitespace);
    }

    if crate::menu_syntax::active_filter_head_owns_main_list(trimmed) {
        return true;
    }

    trimmed.split_whitespace().count() == 1
        && mode
            .advanced_query_for(trimmed)
            .is_some_and(|query| query.free_text.trim().is_empty() && query.has_predicates())
}

impl ScriptListApp {
    #[inline]
    fn filter_change_can_affect_window_size(&self) -> bool {
        // Mini ScriptList height depends on grouped results.
        // Non-ScriptList views may also size from filtered item counts.
        // Full ScriptList uses the normal fixed launcher size, so typing
        // should not recalculate/defer resize every keystroke.
        !matches!(self.current_view, AppView::ScriptList)
            || self.main_window_mode == MainWindowMode::Mini
    }

    pub(crate) fn cancel_history_filter_render_pending_if_obsolete(&mut self, next_filter: &str) {
        if self
            .history_filter_render_pending
            .as_deref()
            .is_some_and(|pending| pending != next_filter)
        {
            tracing::info!(
                target: "script_kit::input_history",
                event = "history_filter_render_pending_cancelled_obsolete",
                next_filter_len = next_filter.len(),
                history_index = ?self.input_history.current_index(),
                selected_index = self.selected_index,
            );
            self.history_filter_render_pending = None;
        }
    }

    /// Single authoritative post-filter reconciliation path for ScriptList.
    ///
    /// Called after `computed_filter_text` changes (both debounced and immediate).
    /// Syncs the GPUI list model, resets selection to the first selectable row,
    /// reveals it, and rebuilds preflight — all outside `render()`.
    pub(crate) fn reconcile_script_list_after_filter_change(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }

        // Filter changes intentionally restart from the first selectable row.
        self.reset_main_menu_selection_user_moved();
        self.snap_main_menu_selection_to_first();

        // Keep GPUI's list model aligned with the newly computed grouped results.
        // Filter changes may replace every row while preserving the same count,
        // so force measured item rebuilding after the new selection is known.
        self.sync_list_state_for_filter_replacement();
        self.validate_selection_bounds(cx);

        // Clear last_scrolled_index so the reveal is never skipped —
        // filter changes always need a fresh scroll even if selected_index == 0.
        self.last_scrolled_index = None;

        // Reveal the final selected row after selection coercion.
        self.scroll_to_selected_if_needed(reason);

        // Preflight depends on filter, selection, and fallback state. Immediate
        // typing paths finish their final state after reconciliation, so those
        // callers own the single final rebuild.
        let caller_rebuilds_preflight_after_final_state =
            matches!(reason, "filter_immediate" | "set_filter_text_immediate");
        if !caller_rebuilds_preflight_after_final_state {
            self.rebuild_main_window_preflight_if_needed();
        }

        self.refresh_ghost_with_input(cx);
    }

    /// Reconcile ScriptList after async result providers refresh the current query.
    ///
    /// Unlike a real filter change, provider completions should keep the user's
    /// current row selected when that row still exists in the refreshed list.
    pub(crate) fn reconcile_script_list_after_results_refresh(
        &mut self,
        reason: &'static str,
        selection_before: MainMenuSelectionSnapshot,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }

        self.sync_list_state_for_filter_replacement();
        self.validate_selection_bounds(cx);

        let restored = match main_menu_refresh_selection_policy(self.main_menu_selection_user_moved)
        {
            MainMenuRefreshSelectionPolicy::RestoreIdentity => {
                self.restore_main_menu_selection_from_snapshot(selection_before)
            }
            MainMenuRefreshSelectionPolicy::SnapToFirst => {
                self.snap_main_menu_selection_to_first();
                tracing::debug!(
                    target: "script_kit::selection",
                    event = "main_menu_refresh_snap_to_first_untouched_selection",
                    reason,
                    selected_index = self.selected_index,
                );
                false
            }
        };
        if restored {
            tracing::debug!(
                target: "script_kit::selection",
                event = "main_menu_async_refresh_selection_reconciled",
                reason,
                selected_index = self.selected_index,
            );
        } else if self.selected_index == 0 {
            tracing::warn!(
                target: "script_kit::selection",
                event = "main_menu_async_refresh_selected_first_without_restore",
                reason,
                selected_index = self.selected_index,
            );
        } else {
            tracing::debug!(
                target: "script_kit::selection",
                event = "main_menu_async_refresh_selection_reconciled",
                reason,
                selected_index = self.selected_index,
                restored = false,
            );
        }

        self.scroll_to_selected_if_needed(reason);
        self.rebuild_main_window_preflight_if_needed();
        self.refresh_ghost_with_input(cx);
    }

    fn apply_filter_compute_now(&mut self, value: String, cx: &mut Context<Self>) {
        if self.computed_filter_text == value {
            tracing::debug!(
                target: "script_kit::filter",
                event = "apply_filter_compute_exact_query_noop",
                filter_len = value.len(),
            );
            return;
        }
        if logging::filter_perf_trace_enabled() {
            logging::log(
                "FILTER_PERF",
                &format!("[2/5] APPLY_FILTER value='{}' len={}", value, value.len()),
            );
        }
        if self.computed_filter_text != value {
            let update_start = std::time::Instant::now();
            self.filter_coalescer.reset();
            self.computed_filter_text = value.clone();
            if crate::menu_syntax::active_filter_head_owns_main_list(&value) {
                self.main_menu_fallback_state.clear();
                self.invalidate_grouped_cache();
            }
            self.maybe_start_root_file_search(&value, cx);
            self.maybe_start_root_brain_semantic_search(&value, cx);
            self.refresh_root_brain_inbox_if_stale(false, cx);
            self.maybe_start_root_windows_refresh_for_query(&value, cx);
            self.maybe_start_root_browser_tabs_refresh_for_query(&value, cx);
            self.maybe_start_root_browser_history_refresh_for_query(&value, cx);
            self.reconcile_script_list_after_filter_change("filter_immediate", cx);
            self.rebuild_main_window_preflight_if_needed();
            if self.filter_change_can_affect_window_size() {
                self.update_window_size();
            }
            let update_elapsed = update_start.elapsed();
            if logging::filter_perf_trace_enabled()
                || update_elapsed >= std::time::Duration::from_millis(8)
            {
                logging::log(
                    "FILTER_PERF",
                    &format!(
                        "[3/5] APPLY_FILTER_DONE in {:.2}ms for '{}'",
                        update_elapsed.as_secs_f64() * 1000.0,
                        value
                    ),
                );
            }
            cx.notify();
        }
    }

    pub(crate) fn queue_filter_compute(&mut self, value: String, cx: &mut Context<Self>) {
        if self.computed_filter_text == value {
            tracing::debug!(
                target: "script_kit::filter",
                event = "queue_filter_compute_exact_query_noop",
                filter_len = value.len(),
            );
            self.filter_coalescer.reset();
            return;
        }

        // Structural flips (empty <-> sigil/fallback/qualifier surfaces) must
        // not publish even one stale-list frame: the input already echoed the
        // new text, so compute now instead of deferring. Same-family typing
        // keeps the coalescer so rapid keystrokes still skip intermediate
        // computes.
        if filter_change_flips_list_structure(&self.computed_filter_text, &value) {
            self.filter_coalescer.reset();
            self.apply_filter_compute_now(value, cx);
            return;
        }

        let should_spawn = self.filter_coalescer.queue(value);
        if !should_spawn {
            tracing::debug!(
                target: "script_kit::filter",
                event = "queue_filter_compute_coalesced",
            );
            return;
        }

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FILTER_COMPUTE_DEFER).await;

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    let Some(latest) = app.filter_coalescer.take_latest() else {
                        return;
                    };
                    app.apply_filter_compute_now(latest, cx);
                })
            });
        })
        .detach();
    }

    /// Apply a filter text change synchronously, without coalescer delay.
    ///
    /// Verbatim-echo contract (Run 4 Pass #8 attacker probe
    /// `stdin-setfilter-inputvalue-unbounded`, closed Run 8 Pass #23):
    /// `text` is stored into `self.filter_text` with no length cap,
    /// truncation, or encoding transformation — whatever the stdin
    /// `setFilter` command supplied arrives in `getState.inputValue`
    /// byte-for-byte. The only enforced bound is the stdin line cap at
    /// `MAX_STDIN_COMMAND_BYTES` (16 * 1024 bytes), applied by
    /// `read_stdin_line_bounded` in `src/stdin_commands/mod.rs:1003`.
    /// Callers consuming `getState.inputValue` MUST handle payloads up
    /// to that cap. Pinned by
    /// `tests/stdin_setfilter_input_value_verbatim_contract.rs`.
    pub(crate) fn set_filter_text_immediate(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The filter input is single-line; GPUI's text shaper panics on
        // newlines (`vendor/gpui/src/text_system.rs:414`). Sanitize early so
        // pasted multi-line content cannot crash the app.
        let text = if text.chars().any(|c| matches!(c, '\n' | '\r')) {
            text.replace("\r\n", " ").replace(['\n', '\r'], " ")
        } else {
            text
        };

        // Token-atomic delete parity with the Agent Chat composer: a single
        // backspace inside an alias-registered `@file:` token (any keyboard
        // routing path, including legacy simulateKey) removes the whole token
        // instead of leaving a damaged mention.
        let text = if matches!(self.current_view, AppView::ScriptList) {
            self.spine_mention_atomic_delete_fixup(&self.filter_text, &text)
                .unwrap_or(text)
        } else {
            text
        };

        self.pending_menu_syntax_ai_proposal = None;

        if let AppView::AgentChatView { entity } = &self.current_view {
            self.suppress_filter_events = true;
            self.filter_text = text.clone();
            self.pending_programmatic_filter_echo = Some(text.clone());
            self.gpui_input_state.update(cx, |state, cx| {
                state.set_highlight_ranges_with_roles(Vec::new());
                state.set_value(text.clone(), window, cx);
                let len = text.len();
                state.set_selection(len, len, window, cx);
            });
            self.suppress_filter_events = false;
            self.pending_filter_sync = false;
            entity.update(cx, |chat, cx| {
                chat.set_input(text.clone(), cx);
                chat.refresh_agent_chat_spine_from_composer(cx);
            });
            cx.notify();
            return;
        }

        let input_already_matches = self.gpui_input_state.read(cx).value().to_string() == text;
        if matches!(self.current_view, AppView::ScriptList)
            && self.filter_text == text
            && self.computed_filter_text == text
            && input_already_matches
            && !self.pending_filter_sync
        {
            self.pending_programmatic_filter_echo = None;
            tracing::debug!(
                target: "script_kit::filter",
                event = "set_filter_text_immediate_exact_query_noop",
                filter_len = text.len(),
            );
            return;
        }

        self.suppress_filter_events = true;
        self.filter_text = text.clone();
        self.pending_programmatic_filter_echo = Some(text.clone());
        self.gpui_input_state.update(cx, |state, cx| {
            state.set_highlight_ranges_with_roles(Vec::new());
            state.set_value(text.clone(), window, cx);
            let len = text.len();
            state.set_selection(len, len, window, cx);
        });
        // The input's highlight ranges were just cleared, so the render-side
        // cache must be invalidated too. Otherwise, when the recomputed ranges
        // happen to equal the cached ones (e.g. the `@file:` prefix accent is
        // byte-identical before and after a file selection rewrites the
        // input), the render guard skips reapplying them and the input stays
        // unhighlighted.
        self.main_menu_render_diagnostics.last_input_highlight_text = String::new();
        self.main_menu_render_diagnostics
            .last_input_highlight_ranges = Vec::new();
        self.suppress_filter_events = false;
        self.pending_filter_sync = false;

        // Route filter to the active subview's variant field when current_view
        // is a builtin subview (ClipboardHistoryView, EmojiPickerView, etc.).
        // Without this, stdin `setFilter` on a subview would only update
        // `self.filter_text` and leave the subview's own `filter` field stale,
        // so `getState.visibleChoiceCount` (computed from the variant's filter)
        // would never reflect the narrowed dataset. Sub-gap (2) of the
        // `empty-clipboard-state` story.
        let handled_by_subview = self.write_filter_to_current_subview(&text);
        if handled_by_subview && matches!(self.current_view, AppView::ThemeChooserView { .. }) {
            // Protocol setFilter must drive the same live preview side effects
            // (hex paste accent preview, first-match repreview, list re-splice)
            // as real typing, which is suppressed on this path.
            self.computed_filter_text = text.clone();
            self.filter_coalescer.reset();
            self.apply_theme_chooser_filter_change_effects(cx);
            if self.filter_change_can_affect_window_size() {
                self.update_window_size_deferred(window, cx);
            }
            cx.notify();
            return;
        }
        if handled_by_subview && matches!(self.current_view, AppView::ProfileSearchView { .. }) {
            self.computed_filter_text = text.clone();
            self.filter_coalescer.reset();
            if self.filter_change_can_affect_window_size() {
                self.update_window_size_deferred(window, cx);
            }
            cx.notify();
            return;
        }

        // Menu bar items are now pre-fetched by frontmost_app_tracker
        // No lazy loading needed - items are already in cache when we open

        // stdin `setFilter` on FileSearchView needs to drive the file-search
        // stream the same way real keystrokes do (the GPUI handler at
        // `handle_filter_input_change` line ~511 is suppressed here). Open
        // the view at the new query so directory navigation works under
        // protocol automation.
        if !handled_by_subview && matches!(self.current_view, AppView::FileSearchView { .. }) {
            let presentation =
                if let AppView::FileSearchView { presentation, .. } = &self.current_view {
                    *presentation
                } else {
                    FileSearchPresentation::Full
                };
            self.open_file_search_view_preserving_current_results(text.clone(), presentation, cx);
            return;
        }

        let mut handler_form_owns_input = false;
        if !handled_by_subview && matches!(self.current_view, AppView::ScriptList) {
            if let Some(entry) = Self::special_entry_from_script_list_filter(&text) {
                if self.route_script_list_special_entry(entry, &text, window, cx) {
                    return;
                }
            }
            self.set_menu_syntax_mode_from_filter(&text);
            if self.spine_enabled {
                self.set_spine_parse_from_filter_and_cursor(&text, text.len());
                self.maybe_start_spine_file_subsearch_for_current_projection(cx);
                let has_cwd_segment = self.spine_parse.segments.iter().any(|s| {
                    matches!(s.kind, crate::spine::SpineSegmentKind::ProjectCwd { .. })
                        && matches!(
                            s.resolution,
                            crate::spine::SpineSegmentResolution::Resolved { .. }
                        )
                });
                // Note: CWD is no longer auto-cleared when the parsed input
                // lacks a `>:` segment. The CWD now lives in the footer chip
                // (set on Enter against a directory row) and is independent
                // of the input bar. The user changes it by typing `>` again
                // and picking a different directory, or by clicking the
                // chip.
                let _ = has_cwd_segment;
            }
            handler_form_owns_input = self.menu_syntax_capture_form_owns_input_for(&text);
            self.sync_menu_syntax_form_inputs_from_filter(window, cx);
            let handler_form_field_owns_input =
                self.menu_syntax_form_input_active && handler_form_owns_input;
            if handler_form_field_owns_input {
                self.menu_syntax_object_selector_state = Default::default();
                self.menu_syntax_trigger_picker_state = Default::default();
                self.sync_menu_syntax_form_inputs_from_filter(window, cx);
            } else {
                self.run_menu_syntax_object_selector_state_machine(&text, window, cx);
            }
            if !handler_form_field_owns_input
                && self.menu_syntax_object_selector_state.snapshot.is_none()
            {
                self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);
            }
            self.invalidate_grouped_cache();
        } else {
            self.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::default();
            self.sync_menu_syntax_form_inputs_from_filter(window, cx);
        }

        if !handled_by_subview
            && matches!(self.current_view, AppView::ScriptList)
            && self.menu_syntax_trigger_picker_state.snapshot.is_none()
            && self.menu_syntax_object_selector_state.snapshot.is_none()
        {
            let picker_ctx = self.menu_syntax_trigger_picker_context(&text);
            if crate::menu_syntax::build_trigger_picker_snapshot(&text, &picker_ctx).is_some() {
                self.run_menu_syntax_trigger_picker_state_machine(&text, window, cx);
                self.invalidate_grouped_cache();
            }
        }

        if self.menu_syntax_mode.is_menu_syntax_for(&text)
            || self.menu_syntax_trigger_picker_state.snapshot.is_some()
            || self.menu_syntax_object_selector_state.snapshot.is_some()
            || self.menu_syntax_capture_form_owns_input_for(&text)
            || crate::menu_syntax::active_filter_head_owns_main_list(&text)
        {
            // Menu syntax owns the result list entirely — clear any stale
            // fallback items so pressing Enter routes to execute_selected,
            // not execute_selected_fallback. Also clear when the trigger
            // picker is active for a partial trigger like `;t` (where
            // `is_menu_syntax_for` still returns false because the parser
            // doesn't yet recognize `;t` as a full target).
            self.main_menu_fallback_state.clear();
        }

        self.computed_filter_text = text.clone();
        self.filter_coalescer.reset();
        self.maybe_start_root_file_search(&text, cx);
        self.maybe_start_root_brain_semantic_search(&text, cx);
        self.refresh_root_brain_inbox_if_stale(false, cx);
        self.maybe_start_root_windows_refresh_for_query(&text, cx);
        self.maybe_start_root_browser_tabs_refresh_for_query(&text, cx);
        self.maybe_start_root_browser_history_refresh_for_query(&text, cx);
        self.reconcile_script_list_after_filter_change("set_filter_text_immediate", cx);

        // Update fallback state immediately based on filter results
        // This ensures SimulateKey commands can check fallback state correctly
        // NOTE: validate_selection_bounds already clears main_menu_fallback_state,
        // but we need special handling for legacy SimulateKey compatibility.
        // Skip when a subview handled the filter: `get_filtered_results_cached`
        // and `collect_fallbacks` are ScriptList-only and would incorrectly
        // flip a builtin subview into the script-list fallback mode.
        if !handled_by_subview && !text.is_empty() {
            if !handler_form_owns_input
                && !self.menu_syntax_mode.is_menu_syntax_for(&text)
                && !crate::menu_syntax::active_filter_head_owns_main_list(&text)
                && self.menu_syntax_trigger_picker_state.snapshot.is_none()
                && self.menu_syntax_object_selector_state.snapshot.is_none()
            {
                let results = self.get_filtered_results_cached();
                if results.is_empty() {
                    // No matches - check if we should enter fallback mode
                    use crate::fallbacks::collect_fallbacks;
                    let fallbacks = collect_fallbacks(&text, self.scripts.as_slice());
                    if !fallbacks.is_empty() {
                        self.main_menu_fallback_state.replace_items(fallbacks);
                    }
                }
            }
        }

        // Single final preflight rebuild for immediate input changes. This must
        // stay after fallback state updates so submit diagnostics/preflight see
        // the final filter, selection, and fallback model.
        self.rebuild_main_window_preflight_if_needed();
        if self.filter_change_can_affect_window_size() {
            self.update_window_size_deferred(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn handle_script_list_printable_simulate_key(
        &mut self,
        key_char: Option<&str>,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }
        if modifiers.platform || modifiers.alt || modifiers.control {
            return false;
        }
        if self.menu_syntax_form_input_active && self.menu_syntax_capture_form_owns_input() {
            return false;
        }
        let Some(ch) = key_char else {
            return false;
        };
        if ch.is_empty() || ch.chars().count() != 1 {
            return false;
        }

        let mut next = self.filter_text.clone();
        next.push_str(ch);
        self.set_filter_text_immediate(next, window, cx);
        true
    }

    /// Write the given filter text into the current view's `filter` field
    /// when `current_view` is one of the shared-input builtin subviews.
    ///
    /// Returns `true` when a subview was handled — callers should skip any
    /// ScriptList-only bookkeeping (fallback mode, ranker, etc.) in that case.
    /// Returns `false` for `ScriptList`, `FileSearchView` (dedicated routing
    /// via `restart_file_search_stream_for_query`), and non-filter views.
    pub(crate) fn write_filter_to_current_subview(&mut self, text: &str) -> bool {
        match &mut self.current_view {
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::DesignGalleryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::FooterGalleryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::ThemeChooserView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::ProfileSearchView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::ProcessManagerView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::FlowUxView {
                filter,
                selected_index,
                ..
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::SettingsView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                }
                true
            }
            AppView::SearchAiPresetsView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                }
                true
            }
            AppView::FavoritesBrowseView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    self.builtin_row_stack_scroll_handle.scroll_to_item(0);
                }
                true
            }
            AppView::CurrentAppCommandsView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::DictationHistoryView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::TipsView {
                filter,
                selected_index,
                ..
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::NotesBrowseView {
                filter,
                selected_index,
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::EmojiPickerView {
                filter,
                selected_index,
                ..
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            AppView::MigrateV1View {
                filter,
                selected_index,
                ..
            } => {
                Self::sync_builtin_query_state(filter, selected_index, text);
                true
            }
            // Flow sessions: the main input holds a message draft, not a
            // query. Consume the write so main-menu filter logic never runs.
            AppView::FlowSessionView { .. } => true,
            _ => false,
        }
    }

    pub(crate) fn clear_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Today → main-menu `@context` round trip: Escape (the only
        // user-facing path into clear_filter while the launcher hosts the
        // pending search) cancels back to Today instead of stranding the
        // user on an emptied launcher filter.
        if matches!(self.current_view, AppView::ScriptList)
            && self.try_cancel_day_page_context_round_trip(window, cx)
        {
            return;
        }
        self.cancel_history_filter_render_pending_if_obsolete("");
        self.set_filter_text_immediate(String::new(), window, cx);
    }

    pub(crate) fn script_list_escape_should_clear_visible_filter(&self, cx: &App) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }

        if !self.gpui_input_state.read(cx).value().is_empty() {
            return true;
        }

        // Multiline menu-syntax forms render canonical text through a compact
        // single-line view instead of the raw GPUI input state.
        !self.filter_text.is_empty()
            && self
                .filter_text
                .chars()
                .any(|character| matches!(character, '\n' | '\r'))
            && (self
                .menu_syntax_mode
                .capture_composer_owns_input_for(&self.filter_text)
                || self
                    .menu_syntax_mode
                    .command_owns_input_for(&self.filter_text)
                || self.menu_syntax_capture_form_owns_input_for(&self.filter_text))
    }

    pub(crate) fn menu_syntax_filter_only_escape_should_clear(&self) -> bool {
        menu_syntax_filter_only_escape_should_clear(&self.filter_text, &self.menu_syntax_mode)
    }

    pub(crate) fn clear_hidden_script_list_filter_before_escape_close(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }
        if self.script_list_escape_should_clear_visible_filter(cx) {
            return;
        }
        if self.filter_text.is_empty()
            && self.computed_filter_text.is_empty()
            && !self.pending_filter_sync
        {
            return;
        }

        self.set_filter_text_immediate(String::new(), window, cx);
    }

    // ── Spine row acceptance ────────────────────────────────────────────

    /// Accept the currently selected Spine projection row (Enter / click).
    /// Returns `true` if the action was handled.
    pub(crate) fn accept_spine_projection_row(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.spine_projection_owns_main_list() {
            return false;
        }
        // Rich subsearch rows (files, clipboard, notes, scripts, history,
        // calendar, …) need interception: resolve them into compact
        // `@source:label` tokens + alias-registered context instead of
        // executing default launcher behavior (file-open, script-run,
        // note-open) while the user is mid-prompt.
        if let Some(outcome) = self.selected_spine_rich_subsearch_outcome() {
            if let Some((token, part)) = outcome.alias {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "spine_subsearch_alias_registered",
                    token = %token,
                );
                self.spine_mention_aliases.insert(token, part);
            }
            return self.apply_spine_list_action(outcome.action, window, cx);
        }
        let Some(row) = self.selected_spine_projection_row() else {
            tracing::debug!(
                target: "script_kit::spine",
                event = "accept_spine_projection_row_no_selection",
                selected_index = self.selected_index,
            );
            return false;
        };
        let action = row.action.clone();
        tracing::info!(
            target: "script_kit::spine",
            event = "accept_spine_projection_row",
            row_id = %row.id,
            row_title = %row.title,
            selected_index = self.selected_index,
        );
        self.apply_spine_list_action(action, window, cx)
    }

    fn selected_spine_rich_subsearch_outcome(
        &mut self,
    ) -> Option<crate::spine::attach::SpineAttachOutcome> {
        let projection = self.spine_projection.as_ref()?;
        let crate::spine::SpineSegmentKind::ContextMention {
            context_type,
            sub_query,
        } = &projection.active_segment_kind
        else {
            return None;
        };
        let (source, _) = crate::spine::catalog_subsearch::parse_context_subsearch(
            context_type,
            sub_query.as_deref(),
        )?;
        let segment_index = projection.active_segment_index;
        let segment_byte_range = self
            .spine_parse
            .segments
            .get(segment_index)
            .map(|seg| seg.byte_range.clone())?;

        let (grouped, flat) = self.get_grouped_results_cached();
        let result_idx = match grouped.get(self.selected_index)? {
            GroupedListItem::Item(idx) => *idx,
            _ => return None,
        };
        let result = flat.get(result_idx)?;

        let mut outcome = crate::spine::attach::attach_outcome_for_result(
            source,
            result,
            segment_index,
            segment_byte_range,
        )?;
        // File tokens are deduplicated against the live alias registry so a
        // second README.md gets `@file:README.md-2` instead of silently
        // overwriting the first alias.
        if let crate::spine::SpineListAction::ResolveSegment {
            replacement,
            resolution_id,
            resolution_source,
            ..
        } = &mut outcome.action
        {
            if resolution_source.as_ref() == "file" {
                if let Some(path) = resolution_id.as_ref().strip_prefix("file/") {
                    *replacement = self.unique_spine_file_mention_token(path).into();
                }
            }
        }
        Some(outcome)
    }

    /// Canonical compact spine token for a selected file: `@file:` plus the
    /// escaped basename. Both the inline subsearch accept and the file-search
    /// portal accept must produce the same token so the alias registry and
    /// the prompt plan resolve it identically.
    pub(crate) fn spine_file_mention_token(path: &str) -> String {
        crate::spine::attach::spine_file_mention_token(path)
    }

    /// `spine_file_mention_token`, deduplicated against the live alias
    /// registry: two different files sharing a basename get distinct tokens
    /// (`@file:README.md`, `@file:README.md-2`) so the second attach does
    /// not silently overwrite the first alias.
    pub(crate) fn unique_spine_file_mention_token(&self, path: &str) -> String {
        let base = Self::spine_file_mention_token(path);
        let collides = |token: &str| {
            matches!(
                self.spine_mention_aliases.get(token),
                Some(crate::ai::message_parts::AiContextPart::FilePath {
                    path: existing,
                    ..
                }) if existing != path
            )
        };
        if !collides(&base) {
            return base;
        }
        for n in 2..100 {
            let candidate = format!("{base}-{n}");
            if !collides(&candidate) {
                return candidate;
            }
        }
        base
    }

    /// Register the alias that maps a compact spine `@file:` token back to its
    /// full-path context part for prompt-plan resolution and atomic delete.
    pub(crate) fn register_spine_file_mention_alias(&mut self, token: String, path: String) {
        let label = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_string();
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_file_mention_alias_registered",
            token = %token,
        );
        self.spine_mention_aliases.insert(
            token,
            crate::ai::message_parts::AiContextPart::FilePath { path, label },
        );
    }

    /// Register the alias that maps a compact spine `@clipboard:` token back
    /// to its clipboard entry content. Parity with `@file:` tokens: the
    /// registered token gets full-token accent highlighting, atomic delete,
    /// and prompt-plan resolution (previously `@clipboard:<id>` lost its
    /// resolution on reparse and submitted as an unknown-context warning).
    pub(crate) fn register_spine_clipboard_mention_alias(
        &mut self,
        token: String,
        id: String,
        label: String,
    ) {
        let text = crate::clipboard_history::get_entry_content(&id).unwrap_or_default();
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_clipboard_mention_alias_registered",
            token = %token,
            bytes = text.len(),
        );
        self.spine_mention_aliases.insert(
            token,
            crate::ai::message_parts::AiContextPart::TextBlock {
                label,
                source: format!("spine:clipboard:{id}"),
                text,
                mime_type: None,
            },
        );
    }

    /// Token-atomic delete parity with the Agent Chat composer: when `next`
    /// is `previous` with exactly one character deleted from inside an
    /// alias-registered mention token, return `previous` with the whole token
    /// (plus one trailing space) removed. Only registered tokens qualify, so
    /// in-progress `@file:query` subsearch typing keeps per-character editing.
    pub(crate) fn spine_mention_atomic_delete_fixup(
        &self,
        previous: &str,
        next: &str,
    ) -> Option<String> {
        if self.spine_mention_aliases.is_empty() {
            return None;
        }
        let deleted_char_index = single_char_deletion_index(previous, next)?;
        let span = crate::ai::context_mentions::inline_token_spans(previous)
            .into_iter()
            .find(|span| {
                deleted_char_index >= span.range.start
                    && deleted_char_index < span.range.end
                    && self.spine_mention_aliases.contains_key(&span.token)
            })?;

        let chars: Vec<char> = previous.chars().collect();
        let mut end = span.range.end;
        if chars.get(end) == Some(&' ') {
            end += 1;
        }
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_mention_deleted_atomically",
            token = %span.token,
        );
        let mut out = String::with_capacity(previous.len());
        out.extend(chars[..span.range.start].iter());
        out.extend(chars[end..].iter());
        Some(out)
    }

    /// Return the `SpineListRow` at the current `selected_index`, if any.
    pub(crate) fn selected_spine_projection_row(&mut self) -> Option<crate::spine::SpineListRow> {
        let (grouped, flat) = self.get_grouped_results_cached();
        let item = grouped.get(self.selected_index)?;
        match item {
            GroupedListItem::Item(result_idx) => {
                if let Some(crate::scripts::SearchResult::SpineProjection(row)) =
                    flat.get(*result_idx)
                {
                    if row.is_selectable {
                        Some(row.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Dispatch a `SpineListAction` from a selected row.
    pub(crate) fn apply_spine_list_action(
        &mut self,
        action: crate::spine::SpineListAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::spine::SpineListAction;
        match action {
            SpineListAction::InsertSegmentText {
                segment_index,
                segment_byte_range,
                text,
                trailing_space,
            } => {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_insert_segment",
                    segment_index,
                    text = %text,
                    trailing_space,
                );
                self.replace_active_segment_text(
                    segment_index,
                    segment_byte_range,
                    text.as_ref(),
                    trailing_space,
                    window,
                    cx,
                )
            }
            SpineListAction::ResolveSegment {
                segment_index,
                segment_byte_range,
                replacement,
                resolution_id,
                resolution_label,
                resolution_source,
                trailing_space,
            } => {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_resolve_segment",
                    segment_index,
                    replacement = %replacement,
                    resolution_id = %resolution_id,
                    resolution_label = %resolution_label,
                    resolution_source = %resolution_source,
                    trailing_space,
                );
                if resolution_source.as_ref() == "file" {
                    if let Some(path) = resolution_id.as_ref().strip_prefix("file/") {
                        self.register_spine_file_mention_alias(
                            replacement.as_ref().to_string(),
                            path.to_string(),
                        );
                    }
                }
                if resolution_source.as_ref() == "clipboard" {
                    if let Some(id) = resolution_id.as_ref().strip_prefix("clipboard/") {
                        self.register_spine_clipboard_mention_alias(
                            replacement.as_ref().to_string(),
                            id.to_string(),
                            resolution_label.as_ref().to_string(),
                        );
                    }
                }
                if resolution_source.as_ref() == "cwd" {
                    let path = std::path::PathBuf::from(resolution_id.as_ref());
                    self.spine_cwd = Some(path);
                    self.spine_cwd_label = Some(resolution_label.as_ref().to_string());
                    self.spine_cwd_revision = self.spine_cwd_revision.wrapping_add(1);
                    self.persist_spine_cwd();
                    self.prewarm_agent_chat_for_spine_cwd(cx);
                    self.invalidate_grouped_cache();
                    // CWD becomes a footer chip — strip the segment text from
                    // the input bar so the user sees a clean prompt builder.
                    self.replace_active_segment_text(
                        segment_index,
                        segment_byte_range,
                        "",
                        false,
                        window,
                        cx,
                    )
                } else {
                    // Today → main-menu round trip: the resolved token goes
                    // back into the originating Day Page line instead of the
                    // launcher filter (see day_page_context_round_trip.rs).
                    if self.has_day_page_context_round_trip_pending() {
                        let token = replacement.as_ref().trim();
                        let alias = self.spine_mention_aliases.get(token).cloned();
                        return self.try_complete_day_page_context_round_trip_with_alias(
                            token, alias, window, cx,
                        );
                    }
                    // A9 decision (2026-06-09): picking a style when the
                    // style segment is the whole input is a single-keystroke
                    // "rewrite selected text" — auto-submit the prompt plan
                    // (style sugar adds `@selection` + `/rewrite`).
                    let style_auto_submit = resolution_source.as_ref() == "style"
                        && crate::spine::prompt_plan::spine_parse_is_style_only(&self.spine_parse);
                    let applied = self.replace_active_segment_text(
                        segment_index,
                        segment_byte_range,
                        replacement.as_ref(),
                        trailing_space,
                        window,
                        cx,
                    );
                    if applied && style_auto_submit {
                        tracing::info!(
                            target: "script_kit::spine",
                            event = "spine_style_only_auto_submit",
                            replacement = %replacement,
                        );
                        self.try_submit_spine_prompt_plan_from_enter(cx);
                    }
                    applied
                }
            }
            SpineListAction::OpenModeExit { sigil, rest } => {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_open_mode_exit",
                    sigil = %sigil,
                    rest = %rest,
                );
                match sigil {
                    '~' => {
                        self.open_file_search_view(
                            rest.to_string(),
                            FileSearchPresentation::Mini,
                            cx,
                        );
                        true
                    }
                    '!' => {
                        self.open_quick_terminal(None, cx);
                        true
                    }
                    '?' => {
                        if self.has_actions() {
                            self.toggle_actions(cx, window);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            SpineListAction::OpenFileSearchPortal {
                segment_index,
                segment_byte_range,
                query,
            } => {
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_open_file_search_portal",
                    segment_index,
                    query = %query,
                );
                self.open_spine_file_search_attachment_portal(
                    segment_byte_range,
                    query.to_string(),
                    cx,
                );
                true
            }
            SpineListAction::Noop => false,
        }
    }

    /// Replace the text of the active Spine segment in the filter input,
    /// optionally appending a trailing space, and reposition the cursor.
    pub(crate) fn replace_active_segment_text(
        &mut self,
        segment_index: usize,
        segment_byte_range: std::ops::Range<usize>,
        replacement: &str,
        trailing_space: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let current = self.filter_text.clone();

        // Validate byte range against current filter text.
        if !self.valid_filter_byte_range(&current, &segment_byte_range) {
            tracing::debug!(
                target: "script_kit::spine",
                event = "replace_segment_invalid_byte_range",
                range_start = segment_byte_range.start,
                range_end = segment_byte_range.end,
                filter_len = current.len(),
            );
            return false;
        }

        let Some(current_segment) = self.spine_parse.segments.get(segment_index) else {
            tracing::debug!(
                target: "script_kit::spine",
                event = "replace_segment_index_out_of_bounds",
                segment_index,
                segment_count = self.spine_parse.segments.len(),
            );
            return false;
        };

        if current_segment.byte_range != segment_byte_range {
            tracing::debug!(
                target: "script_kit::spine",
                event = "replace_segment_stale_range",
                segment_index,
                expected = ?current_segment.byte_range,
                got = ?segment_byte_range,
            );
            return false;
        }

        let prefix = &current[..segment_byte_range.start];
        let suffix = &current[segment_byte_range.end..];
        let add_space = trailing_space
            && !replacement.ends_with(char::is_whitespace)
            && !suffix.starts_with(char::is_whitespace);
        let space = if add_space { " " } else { "" };
        let new_text = format!("{prefix}{replacement}{space}{suffix}");
        let cursor = prefix.len() + replacement.len() + space.len();

        tracing::info!(
            target: "script_kit::spine",
            event = "replace_active_segment_text",
            segment_index,
            old_range = ?segment_byte_range,
            replacement,
            trailing_space,
            new_text_len = new_text.len(),
            cursor,
        );

        self.set_filter_text_and_cursor_immediate(new_text, cursor, window, cx);
        true
    }

    /// Set filter text and cursor position in one shot, then reparse spine.
    pub(crate) fn set_filter_text_and_cursor_immediate(
        &mut self,
        text: String,
        cursor_position: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Use existing set_filter_text_immediate to sync filter + input state.
        self.set_filter_text_immediate(text.clone(), window, cx);

        // Now reposition the cursor (set_filter_text_immediate places it at end).
        let cursor = self.clamp_filter_cursor_to_char_boundary(&text, cursor_position);
        self.suppress_filter_events = true;
        self.gpui_input_state.update(cx, |state, cx| {
            state.set_selection(cursor, cursor, window, cx);
        });
        self.suppress_filter_events = false;

        // Reparse spine at the new cursor position before
        // invalidation/reconciliation so sigil list state sees the correct
        // projection.
        if self.spine_enabled {
            self.set_spine_parse_from_filter_and_cursor(&text, cursor);
            self.maybe_start_spine_file_subsearch_for_current_projection(cx);
        }

        self.main_menu_fallback_state.clear();
        self.invalidate_grouped_cache();
        self.reconcile_script_list_after_filter_change("spine_segment_replace", cx);

        cx.notify();
    }

    /// Check if a byte range is valid for the given filter text.
    fn valid_filter_byte_range(&self, text: &str, range: &std::ops::Range<usize>) -> bool {
        range.start <= range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end)
    }

    /// Clamp a cursor position to the nearest char boundary.
    fn clamp_filter_cursor_to_char_boundary(&self, text: &str, pos: usize) -> usize {
        let clamped = pos.min(text.len());
        // Walk backwards to the nearest char boundary if needed.
        let mut p = clamped;
        while p > 0 && !text.is_char_boundary(p) {
            p -= 1;
        }
        p
    }

    pub(crate) fn sync_filter_input_if_needed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Sync placeholder if pending
        if let Some(placeholder) = self.pending_placeholder.take() {
            self.gpui_input_state.update(cx, |state, cx| {
                state.set_placeholder(placeholder, window, cx);
            });
        }

        if !self.pending_filter_sync {
            return;
        }

        let desired = self.filter_text.clone();
        let current = self.gpui_input_state.read(cx).value().to_string();
        if current == desired {
            self.pending_filter_sync = false;
            return;
        }

        self.suppress_filter_events = true;
        self.gpui_input_state.update(cx, |state, cx| {
            state.set_value(desired.clone(), window, cx);
            // Ensure cursor is at end with no selection after programmatic set_value
            let len = desired.len();
            state.set_selection(len, len, window, cx);
        });
        self.suppress_filter_events = false;
        self.pending_filter_sync = false;
    }

    pub(crate) fn refresh_runtime_copy_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pending_placeholder =
            Some(crate::dev_style_tool::runtime_overrides::effective_main_input_placeholder());
        self.sync_filter_input_if_needed(window, cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{menu_syntax_filter_only_escape_should_clear, *};

    fn should_clear(raw: &str) -> bool {
        let mode = crate::menu_syntax::MenuSyntaxMode::from_input(raw);
        menu_syntax_filter_only_escape_should_clear(raw, &mode)
    }

    #[test]
    fn filter_only_escape_clears_head_picker_states() {
        assert!(should_clear(":"));
        assert!(should_clear(":t"));
        assert!(should_clear(":ty"));
        assert!(should_clear(":type:"));
    }

    #[test]
    fn filter_only_escape_clears_qualifier_only_states() {
        assert!(should_clear("type:"));
        assert!(should_clear("type:s"));
        assert!(should_clear("kind:"));
        assert!(should_clear("type:script"));
    }

    #[test]
    fn filter_only_escape_preserves_composed_queries() {
        assert!(!should_clear("type:script git"));
        assert!(!should_clear("plain search"));
        assert!(!should_clear(""));
    }

    #[gpui::test]
    fn query_change_discards_root_file_handoff_selection(cx: &mut gpui::TestAppContext) {
        let app = main_menu_selection_test_app(cx);
        app.update(cx, |app, cx| {
            let old_query = "zzlauncherhandoffprobe";
            let new_query = "zzlauncherhandoffprobe reset";
            let handoff_key = "fallback/root-file-search-handoff/global";

            app.scripts = vec![main_menu_selection_test_script(
                "zzlauncherhandoffprobe reset first",
            )];
            app.scriptlets.clear();
            app.skills.clear();
            app.apps.clear();
            app.computed_filter_text = old_query.to_string();
            app.filter_text = old_query.to_string();
            app.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::from_input(old_query);
            app.root_search.root_file_search_mode =
                Some(crate::file_search::RootFileSectionMode::GlobalQuery);
            app.root_search.root_file_search_query = old_query.to_string();
            app.root_search.root_file_search_loading = true;
            app.root_search.root_file_provider_loading = true;
            app.invalidate_grouped_cache();
            app.get_grouped_results_cached();

            app.selected_index = app
                .main_menu_result_caches
                .grouped_index_for_stable_selection_key(handoff_key)
                .expect("old query should include the Search Files handoff");
            app.mark_main_menu_selection_user_moved();

            app.filter_text = new_query.to_string();
            app.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::from_input(new_query);
            app.apply_filter_compute_now(new_query.to_string(), cx);

            let selected_key = selected_main_menu_stable_key(app);
            let first_key = first_selectable_main_menu_stable_key(app);
            assert_eq!(app.computed_filter_text, new_query);
            assert!(!app.main_menu_selection_user_moved);
            assert_eq!(selected_key, first_key);
            assert_ne!(selected_key.as_deref(), Some(handoff_key));
            assert!(app
                .main_menu_result_caches
                .search_result_for_grouped_item(app.selected_index)
                .is_some());
        });
    }
}

/// Character index of the single deleted char when `next` equals `previous`
/// with exactly one character removed; `None` for any other edit shape.
fn single_char_deletion_index(previous: &str, next: &str) -> Option<usize> {
    let prev: Vec<char> = previous.chars().collect();
    let nxt: Vec<char> = next.chars().collect();
    if prev.len() != nxt.len() + 1 {
        return None;
    }
    let mut idx = 0;
    while idx < nxt.len() && prev[idx] == nxt[idx] {
        idx += 1;
    }
    (prev[idx + 1..] == nxt[idx..]).then_some(idx)
}

#[cfg(test)]
mod spine_mention_atomic_delete_tests {
    use super::single_char_deletion_index;

    #[test]
    fn detects_single_char_deletion() {
        assert_eq!(
            single_char_deletion_index("@file:demo.rs ", "@file:demo.r "),
            Some(12)
        );
        assert_eq!(single_char_deletion_index("abc", "ac"), Some(1));
        assert_eq!(single_char_deletion_index("abc", "abc"), None);
        assert_eq!(single_char_deletion_index("abc", "a"), None);
        assert_eq!(single_char_deletion_index("abc", "abd"), None);
    }

    #[test]
    fn deletion_within_repeated_chars_reports_end_of_equal_run() {
        // Deleting either `a` of "aab" yields "ab"; the scanner attributes the
        // deletion to the position after the shared prefix, which is the same
        // token span either way.
        assert_eq!(single_char_deletion_index("aab", "ab"), Some(1));
    }
}

#[cfg(test)]
mod filter_list_structure_tests {
    use super::filter_change_flips_list_structure;

    #[test]
    fn empty_to_sigil_flips_both_directions() {
        // The reported flash: type "@" (default sections must not linger),
        // then delete it (the `Use "@" with...` fallback must not linger).
        assert!(filter_change_flips_list_structure("", "@"));
        assert!(filter_change_flips_list_structure("@", ""));
    }

    #[test]
    fn empty_to_plain_text_flips() {
        assert!(filter_change_flips_list_structure("", "a"));
        assert!(filter_change_flips_list_structure("a", ""));
    }

    #[test]
    fn typing_within_one_family_defers() {
        assert!(!filter_change_flips_list_structure("a", "ab"));
        assert!(!filter_change_flips_list_structure("ab", "a"));
        assert!(!filter_change_flips_list_structure("@", "@f"));
        assert!(!filter_change_flips_list_structure(";todo", ";todo x"));
    }

    #[test]
    fn sigil_head_change_flips() {
        assert!(filter_change_flips_list_structure("@", "/"));
        assert!(filter_change_flips_list_structure(";", ":"));
        assert!(filter_change_flips_list_structure("a", "@a"));
    }

    #[test]
    fn qualifier_head_ownership_flips() {
        // "has" is plain fuzzy text; "has:" is a menu-syntax head that owns
        // the main list, so the transition must apply synchronously.
        assert!(filter_change_flips_list_structure("has", "has:"));
        assert!(filter_change_flips_list_structure("has:", "has"));
    }
}
