use super::launch_filter_policy::{
    filter_change_flips_list_structure, menu_syntax_filter_only_escape_should_clear,
};
use super::*;

const FILTER_COMPUTE_DEFER: std::time::Duration = std::time::Duration::from_millis(16);

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

    fn root_search_scope_for_input(&self, raw: &str) -> String {
        let file_work = self.root_file_work_identity_for_input(raw);
        self.root_search_scope_with_file_work(
            raw,
            file_work.as_ref().map(|(_, scope, _)| scope.as_str()),
        )
    }

    fn root_search_scope_with_file_work(&self, raw: &str, file_scope: Option<&str>) -> String {
        let advanced = self.menu_syntax_mode.advanced_query_for(raw);
        let spine_owner = self
            .spine_projection
            .as_ref()
            .filter(|_| self.spine_projection_owns_main_list())
            .map(|projection| {
                let head = self
                    .spine_parse
                    .segments
                    .get(projection.active_segment_index)
                    .and_then(|segment| segment.raw.split_once(':').map(|(head, _)| head));
                (
                    projection.active_segment_index,
                    std::mem::discriminant(&projection.active_segment_kind),
                    head,
                )
            });
        use sha2::Digest;
        let scope = format!("sources={:?};predicates={:?};spine={:?};files={:?};cwd={:?};config={:?};object={};trigger={}",
            advanced.map(|query| &query.source_filters), advanced.map(|query| &query.predicates),
            spine_owner, file_scope, self.spine_cwd,
            self.config.unified_search, self.menu_syntax_object_selector_state.owns_main_list(),
            self.menu_syntax_trigger_picker_state.owns_main_list());
        format!("{:x}", sha2::Sha256::digest(scope.as_bytes()))
    }

    pub(super) fn main_menu_has_pending_source_publication(&self) -> bool {
        matches!(
            self.main_menu_result_caches.grouped_cache_key(),
            crate::MAIN_MENU_RESULT_CACHE_UNINITIALIZED_KEY
                | crate::MAIN_MENU_RESULT_CACHE_INVALIDATED_KEY
                | crate::MAIN_MENU_RESULT_CACHE_APPS_LOADED_KEY
        )
    }

    /// Accept intent before debounce without altering the displayed row snapshot.
    pub(crate) fn accept_root_search_input_intent(&mut self, raw: &str) -> bool {
        if !matches!(self.current_view, AppView::ScriptList) {
            return false;
        }
        let file_work = self.root_file_work_identity_for_input(raw);
        let compatible = file_work
            .as_ref()
            .map(|(query, scope, policy)| (query.as_str(), scope.as_str(), *policy));
        let scope =
            self.root_search_scope_with_file_work(raw, compatible.map(|(_, scope, _)| scope));
        if !self
            .root_search
            .accept_query_intent(raw, &scope, compatible)
        {
            return false;
        }
        // Retire a deliberate anchor with the raw query, before deferred row computation.
        // The old rows may remain painted, but no longer own the new query's selection.
        self.reset_main_menu_selection_intent();
        self.main_menu_result_caches.selection_cause = "query_pending";
        self.main_menu_pointer_press = None;
        self.invalidate_main_window_preflight();
        self.mark_main_data_changed();
        true
    }

    /// Deliberate input and route restoration resolve the current query before
    /// choosing or dispatching a subject. Retired timers cannot consume this work.
    pub(crate) fn flush_pending_main_menu_query(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }
        let value = self.filter_text.clone();
        self.set_menu_syntax_mode_from_filter(&value);
        if self.spine_enabled && self.spine_parse.input != value {
            self.set_spine_parse_from_filter_and_cursor(&value, value.len());
        }
        self.accept_root_search_input_intent(&value);
        self.apply_filter_compute_now(value, cx);
    }

    /// Compiled source-change admissions invalidate the actual producer, never
    /// replace grouped rows or reset the current selection/query intent.
    pub(crate) fn apply_owned_search_source_change(
        &mut self,
        source: &str,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            crate::runtime_policy::is_owned_evaluation(),
            "search_source_change_requires_owned_runtime"
        );
        use sk_protocol::command_contract::CommandSource;
        let value = self.computed_filter_text.clone();
        match source {
            "tabs" => {
                crate::browser_tabs::invalidate_owned_root_browser_tabs_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::BrowserTab);
                self.maybe_start_root_browser_tabs_refresh_for_query(&value, cx);
            }
            "history" => {
                crate::browser_history::invalidate_owned_root_browser_history_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::BrowserHistory);
                self.maybe_start_root_browser_history_refresh_for_query(&value, cx);
            }
            "notes" => {
                crate::notes::invalidate_owned_root_notes_search_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::Note);
                self.maybe_start_root_notes_refresh_for_query(&value, cx);
            }
            "todos" => {
                crate::menu_syntax::invalidate_owned_root_todos_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::Todo);
                self.maybe_start_root_todos_refresh_for_query(&value, cx);
            }
            "clipboard" => {
                crate::clipboard_history::invalidate_owned_root_clipboard_history_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::Clipboard);
                self.maybe_start_root_clipboard_history_refresh_for_query(&value, cx);
            }
            "dictation" => {
                crate::dictation::invalidate_owned_root_dictation_history_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::Dictation);
                self.maybe_start_root_dictation_history_refresh_for_query(&value, cx);
            }
            "conversations" => {
                crate::ai::agent_chat::ui::history::invalidate_owned_root_agent_chat_history_freshness()?;
                self.root_search
                    .invalidate_provider_request(CommandSource::Conversation);
                self.maybe_start_root_agent_chat_history_refresh_for_query(&value, cx);
            }
            "brain-lexical" => {
                self.root_search.invalidate_root_brain_lexical_freshness();
                self.refresh_root_brain_lexical_for_query(&value, true, cx);
            }
            "brain-semantic" => {
                self.root_search.invalidate_root_brain_semantic_freshness();
                self.maybe_start_root_brain_semantic_search(&value, cx);
            }
            "brain-inbox" => {
                self.root_search.invalidate_root_brain_inbox_freshness();
                self.refresh_root_brain_inbox_if_stale(true, cx);
            }
            "files" | "directory" => self.refresh_root_file_source(cx),
            "spine" => self.refresh_spine_file_source(cx),
            "windows" => self.refresh_root_windows_source(cx),
            "icons" => self.refresh_root_app_icons(cx),
            "scripts" => self.refresh_scripts(cx),
            "apps" => self.start_root_app_catalog(cx),
            "skills" => self.refresh_skills(cx),
            "validation" => self.refresh_root_validation(cx),
            "flow-roster" => self.refresh_root_flow_roster(cx),
            _ => anyhow::bail!("unknown_search_source_change"),
        }
        Ok(())
    }

    pub(crate) fn start_owned_search_catalogues(
        &mut self,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            crate::runtime_policy::is_owned_evaluation(),
            "search_catalogues_require_owned_runtime"
        );
        self.refresh_scripts(cx);
        self.refresh_skills(cx);
        self.start_root_app_catalog(cx);
        self.refresh_root_validation(cx);
        self.refresh_root_flow_roster(cx);
        Ok(())
    }

    /// Query owners commit inputs before invoking this explicit row publication.
    pub(crate) fn reconcile_script_list_after_filter_change(
        &mut self,
        reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList) {
            return;
        }
        match self.rebuild_main_menu_results_cache(cx) {
            Ok(true) => {}
            Ok(false) => return,
            Err(code) => {
                self.main_menu_result_caches.publication_error = Some(code);
                return;
            }
        }
        self.reset_main_menu_selection_intent();
        self.reconcile_main_menu_selection_intent();
        self.main_menu_result_caches.selection_cause = "query_reset";
        self.begin_list_viewport_scroll(
            crate::scrolling::list_interaction::ListViewportInputSource::Filter,
            cx,
        );
        self.sync_list_state_for_filter_replacement(MainListReplacementPolicy::ResetToTop);
        let sequence = self.finish_main_menu_publication(reason, None, cx);
        cx.notify_with_owned_cause("mainSearchPublication", sequence);
    }

    /// Capture committed interaction before accepted sources mutate, then publish once.
    pub(crate) fn commit_main_menu_results_refresh(
        &mut self,
        reason: &'static str,
        source: Option<(&'static str, u64)>,
        cx: &mut Context<Self>,
        apply: impl FnOnce(&mut Self, &mut Context<Self>) -> bool,
    ) -> bool {
        let interaction_before = self.main_menu_interaction_snapshot();
        if !apply(self, cx) {
            return false;
        }
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        if !matches!(self.current_view, AppView::ScriptList) || !self.root_search.query_is_current()
        {
            return false;
        }
        match self.rebuild_main_menu_results_cache(cx) {
            Ok(true) => {}
            Ok(false) => return false,
            Err(code) => {
                tracing::warn!(target: "script_kit::selection", event = "main_menu_publication_rejected", reason, code);
                self.main_menu_result_caches.publication_error = Some(code);
                self.mark_main_presentation_changed();
                cx.notify();
                return false;
            }
        }
        self.reconcile_script_list_after_results_refresh(reason, interaction_before, cx);
        let sequence = self.finish_main_menu_publication(reason, source, cx);
        cx.notify_with_owned_cause("mainSearchPublication", sequence);
        true
    }

    /// Selection is resolved against the new committed projection before ListState replacement.
    pub(crate) fn reconcile_script_list_after_results_refresh(
        &mut self,
        reason: &'static str,
        interaction_before: MainMenuInteractionSnapshot,
        cx: &mut Context<Self>,
    ) {
        let same_query =
            interaction_before.selection.query_stamp == self.main_menu_committed_query_stamp();
        self.main_menu_result_caches.selection_intent = if same_query {
            interaction_before.selection.intent
        } else {
            MainMenuSelectionIntent::AutomaticTop
        };
        self.reconcile_main_menu_selection_intent();
        let viewport = interaction_before.viewport;
        let automatic = matches!(
            self.main_menu_selection_intent(),
            MainMenuSelectionIntent::AutomaticTop
        );
        let viewport_policy = viewport.intent.refresh_policy(
            automatic,
            same_query,
            viewport.selected_was_within_safe_viewport,
        );
        let (policy, reveal) = match viewport_policy {
            crate::scrolling::list_interaction::MainMenuRefreshViewportPolicy::ResetToTop => {
                if !same_query {
                    self.main_menu_result_caches.viewport_intent =
                        MainMenuViewportIntent::FollowSelection;
                }
                (MainListReplacementPolicy::ResetToTop, false)
            }
            crate::scrolling::list_interaction::MainMenuRefreshViewportPolicy::Preserve {
                reveal_selection,
            } => (
                MainListReplacementPolicy::PreserveViewport(viewport),
                reveal_selection,
            ),
        };
        self.begin_list_viewport_scroll(
            crate::scrolling::list_interaction::ListViewportInputSource::Refresh,
            cx,
        );
        self.sync_list_state_for_filter_replacement(policy);
        if reveal {
            self.adjust_selected_item_above_footer_overlay(self.selected_index);
            self.schedule_main_list_selection_reveal_above_footer(reason, cx);
        }
    }

    fn finish_main_menu_publication(
        &mut self,
        reason: &'static str,
        source: Option<(&'static str, u64)>,
        cx: &mut Context<Self>,
    ) -> u64 {
        self.invalidate_preview_cache();
        self.invalidate_main_window_preflight();
        self.mark_main_data_changed();
        self.rebuild_main_window_preflight_if_needed();
        self.refresh_ghost_with_input(cx);
        #[expect(
            clippy::expect_used,
            reason = "Publication sequence exhaustion must fail before an identity can be reused."
        )]
        let sequence = self.main_menu_last_publication().map_or(1, |stamp| {
            stamp
                .sequence
                .checked_add(1)
                .expect("main search publication sequence exhausted")
        });
        self.main_menu_result_caches.last_publication = Some(MainMenuPublicationStamp {
            sequence,
            reason,
            source: source.map(|(source, _)| source),
            source_generation: source.map(|(_, generation)| generation),
            query: self.root_search.query_stamp(),
            result_revision: self.main_menu_result_revision(),
            selection_revision: self.main_menu_selection_revision(),
            viewport_revision: self.main_menu_viewport_revision(),
        });
        sequence
    }

    fn apply_filter_compute_now(&mut self, value: String, cx: &mut Context<Self>) {
        if value != self.filter_text {
            return;
        }
        if !matches!(self.current_view, AppView::ScriptList) {
            if self.computed_filter_text == value {
                return;
            }
            self.filter_coalescer.reset();
            self.computed_filter_text = value;
            self.mark_main_data_changed();
            if self.filter_change_can_affect_window_size() {
                self.update_window_size();
            }
            cx.notify();
            return;
        }
        self.accept_root_search_input_intent(&value);
        if self.computed_filter_text == value
            && self.root_search.query_is_current()
            && self.main_menu_committed_query_stamp() == self.root_search.computed_query_stamp()
        {
            if self.main_menu_has_pending_source_publication() {
                self.commit_main_menu_results_refresh("source_invalidation", None, cx, |_, _| true);
            }
            return;
        }
        let started = std::time::Instant::now();
        self.filter_coalescer.reset();
        if !self.root_search.commit_query_inputs(
            &value,
            self.menu_syntax_mode.clone(),
            self.spine_parse.clone(),
            self.spine_projection.clone(),
        ) {
            return;
        }
        self.computed_filter_text = value.clone();
        self.inline_calculator = crate::calculator::try_build(&value);
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();
        if crate::menu_syntax::active_filter_head_owns_main_list(&value) {
            self.main_menu_fallback_state.clear();
        }
        self.maybe_start_root_file_search(&value, cx);
        self.refresh_root_brain_lexical_for_query(&value, false, cx);
        self.maybe_start_root_brain_semantic_search(&value, cx);
        self.refresh_root_brain_inbox_if_stale(false, cx);
        self.maybe_start_root_notes_refresh_for_query(&value, cx);
        self.maybe_start_root_todos_refresh_for_query(&value, cx);
        self.maybe_start_root_windows_refresh_for_query(&value, cx);
        self.maybe_start_root_browser_tabs_refresh_for_query(&value, cx);
        self.maybe_start_root_browser_history_refresh_for_query(&value, cx);
        self.maybe_start_root_clipboard_history_refresh_for_query(&value, cx);
        self.maybe_start_root_dictation_history_refresh_for_query(&value, cx);
        self.maybe_start_root_agent_chat_history_refresh_for_query(&value, cx);
        self.maybe_start_spine_file_subsearch_for_current_projection(cx);
        self.reconcile_script_list_after_filter_change("query_commit", cx);
        if self.filter_change_can_affect_window_size() {
            self.update_window_size();
        }
        if logging::filter_perf_trace_enabled()
            || started.elapsed() >= std::time::Duration::from_millis(8)
        {
            tracing::debug!(target: "script_kit::filter", event = "query_committed",
                elapsed_ms = started.elapsed().as_secs_f64() * 1000.0, filter_len = value.len());
        }
    }

    pub(crate) fn queue_filter_compute(&mut self, value: String, cx: &mut Context<Self>) {
        if matches!(self.current_view, AppView::ScriptList) {
            self.accept_root_search_input_intent(&value);
            if self.computed_filter_text == value
                && self.root_search.query_is_current()
                && self.main_menu_has_pending_source_publication()
            {
                self.apply_filter_compute_now(value, cx);
                return;
            }
        }
        if self.computed_filter_text == value
            && (!matches!(self.current_view, AppView::ScriptList)
                || self.root_search.query_is_current())
        {
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
        let root_scope_changed = matches!(self.current_view, AppView::ScriptList)
            && self
                .root_search
                .computed_query_stamp()
                .is_none_or(|computed| {
                    let live = self.root_search.query_stamp();
                    computed.lifetime != live.lifetime
                        || computed.scope_revision != live.scope_revision
                });
        if root_scope_changed
            || filter_change_flips_list_structure(&self.computed_filter_text, &value)
        {
            self.filter_coalescer.reset();
            self.apply_filter_compute_now(value, cx);
            return;
        }

        let Some(ticket) = self.filter_coalescer.queue(value) else {
            tracing::debug!(
                target: "script_kit::filter",
                event = "queue_filter_compute_coalesced",
            );
            return;
        };

        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FILTER_COMPUTE_DEFER).await;

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    let Some(latest) = app.filter_coalescer.take_latest(ticket) else {
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
    pub(crate) fn set_filter_text_and_cursor_immediate(
        &mut self,
        text: String,
        cursor_position: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The filter input is single-line; GPUI's text shaper panics on
        // newlines (`vendor/gpui/src/text_system.rs:414`). Sanitize early so
        // pasted multi-line content cannot crash the app.
        let text = crate::components::text_input::normalize_single_line_text(text);

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
        let cursor = self.clamp_filter_cursor_to_char_boundary(&text, cursor_position);

        self.pending_menu_syntax_ai_proposal = None;

        if let AppView::AgentChatView { entity } = &self.current_view {
            self.suppress_filter_events = true;
            self.filter_text = text.clone();
            self.pending_programmatic_filter_echo = Some(text.clone());
            self.gpui_input_state.update(cx, |state, cx| {
                state.set_highlight_ranges_with_roles(Vec::new());
                state.set_value(text.clone(), window, cx);
                state.set_selection(cursor, cursor, window, cx);
            });
            self.suppress_filter_events = false;
            self.pending_filter_sync = false;
            entity.update(cx, |chat, cx| {
                chat.set_input(text.clone(), cx);
                chat.refresh_agent_chat_spine_from_composer(cx);
            });
            self.note_main_input_changed(cx);
            cx.notify();
            return;
        }

        let input_already_matches = self.gpui_input_state.read(cx).value() == text;
        let input_selection_matches =
            self.gpui_input_state.read(cx).selection() == (cursor..cursor);
        if matches!(self.current_view, AppView::ScriptList)
            && self.filter_text == text
            && self.computed_filter_text == text
            && self.root_search.query_is_current()
            && self.root_search.accepted_scope() == self.root_search_scope_for_input(&text)
            && input_already_matches
            && input_selection_matches
            && self.root_search.computed_spine_parse() == &self.spine_parse
            && self.root_search.computed_spine_projection() == self.spine_projection.as_ref()
            && !self.pending_filter_sync
            && !self.main_menu_has_pending_source_publication()
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
            if !input_already_matches {
                state.set_value(text.clone(), window, cx);
            }
            state.set_selection(cursor, cursor, window, cx);
        });
        // Change events are queued; bind input authority before the publication can paint.
        self.note_main_input_changed(cx);
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

        if !handled_by_subview && matches!(self.current_view, AppView::ScriptList) {
            if let Some(entry) = Self::special_entry_from_script_list_filter(&text) {
                if self.route_script_list_special_entry(entry, &text, window, cx) {
                    return;
                }
            }
            self.set_menu_syntax_mode_from_filter(&text);
            if self.spine_enabled {
                self.set_spine_parse_from_filter_and_cursor(&text, cursor);
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
            self.accept_root_search_input_intent(&text);
            let handler_form_owns_input = self.menu_syntax_capture_form_owns_input_for(&text);
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
            // Typed menu ownership suppresses unrelated launcher fallbacks.
            // Dispatch resolves only the committed canonical row projection.
            self.main_menu_fallback_state.clear();
        }

        if matches!(self.current_view, AppView::ScriptList) {
            self.accept_root_search_input_intent(&text);
            if self.root_search.query_is_current()
                && (self.root_search.computed_spine_parse() != &self.spine_parse
                    || self.root_search.computed_spine_projection()
                        != self.spine_projection.as_ref())
            {
                self.commit_main_menu_results_refresh(
                    "spine_cursor_projection",
                    None,
                    cx,
                    |app, _cx| {
                        app.root_search.commit_query_inputs(
                            &text,
                            app.menu_syntax_mode.clone(),
                            app.spine_parse.clone(),
                            app.spine_projection.clone(),
                        )
                    },
                );
            }
            self.apply_filter_compute_now(text.clone(), cx);
        } else {
            self.computed_filter_text = text.clone();
            self.filter_coalescer.reset();
        }

        // Preflight binds to the final committed input and row projection.
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
        enum UniformReset {
            ClipboardHistory,
            AppLauncher,
            WindowSwitcher,
            BrowserTabs,
            ProcessManager,
            CurrentAppCommands,
            Tips,
            InstalledKits,
        }

        let mut uniform_reset = None;
        let mut tracked_reset = false;
        let handled = match &mut self.current_view {
            AppView::ClipboardHistoryView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::ClipboardHistory);
                }
                true
            }
            AppView::AppLauncherView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::AppLauncher);
                }
                true
            }
            AppView::WindowSwitcherView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::WindowSwitcher);
                }
                true
            }
            AppView::BrowserTabsView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::BrowserTabs);
                }
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
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::CurrentAppCommands);
                }
                true
            }
            AppView::AgentChatHistoryView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    self.agent_chat_history_scroll_handle.scroll_to_item(0);
                    tracked_reset = true;
                }
                true
            }
            AppView::BrowserHistoryView {
                filter,
                selected_index,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    self.browser_history_scroll_handle.scroll_to_item(0);
                    tracked_reset = true;
                }
                true
            }
            AppView::DictationHistoryView {
                filter,
                selected_index,
                visible_limit,
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    *visible_limit = crate::dictation::DICTATION_HISTORY_PAGE_SIZE;
                    self.dictation_history_scroll_handle.scroll_to_item(0);
                    tracked_reset = true;
                }
                true
            }
            AppView::TipsView {
                filter,
                selected_index,
                ..
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::Tips);
                }
                true
            }
            AppView::InstalledKitsView {
                filter,
                selected_index,
                ..
            } => {
                if Self::sync_builtin_query_state(filter, selected_index, text) {
                    uniform_reset = Some(UniformReset::InstalledKits);
                }
                true
            }
            AppView::NotesBrowseView { search } => {
                if search.query != text {
                    search.refresh(text.to_string(), &crate::notes::notes_brain_days_dir());
                    self.notes_browse_scroll_handle.scroll_to_item(0);
                    tracked_reset = true;
                }
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
        };

        let reset_pointer_policy = uniform_reset.is_some() || tracked_reset;
        if let Some(reset) = uniform_reset {
            match reset {
                UniformReset::ClipboardHistory => {
                    self.clipboard_list_scroll_handle
                        .scroll_to_item(0, gpui::ScrollStrategy::Top);
                    if let AppView::ClipboardHistoryView { filter, .. } = &self.current_view {
                        self.focused_clipboard_entry_id = self
                            .clipboard_history_visible_rows(filter)
                            .first()
                            .map(|(_, entry)| entry.id.clone());
                    }
                }
                UniformReset::AppLauncher => self
                    .list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::WindowSwitcher => self
                    .window_list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::BrowserTabs => self
                    .browser_tabs_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::ProcessManager => self
                    .process_list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::CurrentAppCommands => self
                    .current_app_commands_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::Tips => self
                    .tips_list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
                UniformReset::InstalledKits => self
                    .list_scroll_handle
                    .scroll_to_item(0, gpui::ScrollStrategy::Top),
            }
        }
        if reset_pointer_policy {
            self.hovered_index = None;
            self.list_suppress_hover_until_pointer_move = true;
            self.last_list_interaction_source =
                crate::scrolling::list_interaction::ListViewportInputSource::Filter;
        }

        handled
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
        let inputs = self.root_search.computed_query_inputs();
        if inputs.spine_parse.input != inputs.raw
            || !self.spine_projection_owns_main_list_for(
                &inputs.spine_parse,
                inputs.spine_projection.as_ref(),
            )
        {
            return false;
        }
        let Some(ResolvedMainMenuSelection::SearchResult { row, .. }) =
            self.resolved_main_menu_selected_subject()
        else {
            return false;
        };
        if !row.eligibility.activatable {
            return false;
        }
        let observation = MainMenuDispatchObservation {
            query: self.root_search.query_stamp(),
            stable_key: row.stable_key.clone(),
            content_fingerprint: row.content_fingerprint.clone(),
            status: "dispatchRequested",
            reason: None,
        };
        // Rich subsearch rows (files, clipboard, notes, scripts, history,
        // calendar, …) need interception: resolve them into compact
        // `@source:label` tokens + alias-registered context instead of
        // executing default launcher behavior (file-open, script-run,
        // note-open) while the user is mid-prompt.
        if let Some(outcome) = self.selected_spine_rich_subsearch_outcome() {
            let handled = self.apply_spine_attachment_outcome(outcome, window, cx);
            self.set_main_menu_dispatch_observation(Some(MainMenuDispatchObservation {
                status: if handled { "completed" } else { "refused" },
                reason: (!handled).then_some("main_menu_spine_action_refused"),
                ..observation
            }));
            return handled;
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
        let safe_row_id = logging::log_private_user_value(row.id.as_ref());
        let safe_row_title = logging::log_private_user_value(row.title.as_ref());
        tracing::info!(
            target: "script_kit::spine",
            event = "accept_spine_projection_row",
            row_id_bytes = safe_row_id.raw_bytes,
            row_id_sha256 = %safe_row_id.sha256,
            row_title_bytes = safe_row_title.raw_bytes,
            row_title_sha256 = %safe_row_title.sha256,
            selected_index = self.selected_index,
        );
        let handled = self.apply_spine_list_action(action, window, cx);
        self.set_main_menu_dispatch_observation(Some(MainMenuDispatchObservation {
            status: if handled { "completed" } else { "refused" },
            reason: (!handled).then_some("main_menu_spine_action_refused"),
            ..observation
        }));
        handled
    }

    fn apply_spine_attachment_outcome(
        &mut self,
        outcome: crate::spine::attach::SpineAttachOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some((token, part)) = outcome.alias {
            let safe_token = logging::log_private_user_value(&token);
            tracing::info!(
                target: "script_kit::spine",
                event = "spine_subsearch_alias_registered",
                token_bytes = safe_token.raw_bytes,
                token_sha256 = %safe_token.sha256,
            );
            self.spine_mention_aliases.insert(token, part);
        }
        self.apply_spine_list_action(outcome.action, window, cx)
    }

    fn selected_spine_rich_subsearch_outcome(
        &mut self,
    ) -> Option<crate::spine::attach::SpineAttachOutcome> {
        let inputs = self.root_search.computed_query_inputs();
        let projection = inputs.spine_projection.as_ref()?;
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
        let segment_byte_range = inputs
            .spine_parse
            .segments
            .get(segment_index)
            .map(|seg| seg.byte_range.clone())?;

        let ResolvedMainMenuSelection::SearchResult { result, .. } =
            self.resolved_main_menu_selected_subject()?
        else {
            return None;
        };

        let mut outcome = crate::spine::attach::attach_outcome_for_result(
            source,
            result,
            segment_index,
            segment_byte_range,
        )?;
        // Every alias-bearing source is deduplicated against the live
        // registry so same-named files, scripts, scriptlets, skills, notes,
        // or history entries cannot overwrite another selected owner.
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
            } else if let Some((token, part)) = outcome.alias.as_mut() {
                let unique = crate::spine::attach::unique_context_attachment_token(
                    token,
                    part,
                    &self.spine_mention_aliases,
                );
                *token = unique.clone();
                *replacement = unique.into();
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
        let part = crate::ai::message_parts::AiContextPart::FilePath {
            path: path.to_string(),
            label: std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_string(),
        };
        crate::spine::attach::unique_context_attachment_token(
            &base,
            &part,
            &self.spine_mention_aliases,
        )
    }

    /// Register the alias that maps a compact spine `@file:` token back to its
    /// full-path context part for prompt-plan resolution and atomic delete.
    pub(crate) fn register_spine_file_mention_alias(&mut self, token: String, path: String) {
        let label = std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_string();
        let safe_token = logging::log_private_user_value(&token);
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_file_mention_alias_registered",
            token_bytes = safe_token.raw_bytes,
            token_sha256 = %safe_token.sha256,
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
        let safe_token = logging::log_private_user_value(&token);
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_clipboard_mention_alias_registered",
            token_bytes = safe_token.raw_bytes,
            token_sha256 = %safe_token.sha256,
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
        let safe_token = logging::log_private_user_value(&span.token);
        tracing::info!(
            target: "script_kit::spine",
            event = "spine_mention_deleted_atomically",
            token_bytes = safe_token.raw_bytes,
            token_sha256 = %safe_token.sha256,
        );
        let mut out = String::with_capacity(previous.len());
        out.extend(chars[..span.range.start].iter());
        out.extend(chars[end..].iter());
        Some(out)
    }

    /// Return the `SpineListRow` at the current `selected_index`, if any.
    pub(crate) fn selected_spine_projection_row(&mut self) -> Option<crate::spine::SpineListRow> {
        let ResolvedMainMenuSelection::SearchResult {
            result: scripts::SearchResult::SpineProjection(row),
            ..
        } = self.resolved_main_menu_selected_subject()?
        else {
            return None;
        };
        Some(row.clone())
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
                let safe_text = logging::log_private_user_value(text.as_ref());
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_insert_segment",
                    segment_index,
                    text_bytes = safe_text.raw_bytes,
                    text_sha256 = %safe_text.sha256,
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
                let safe_replacement = logging::log_private_user_value(replacement.as_ref());
                let safe_resolution_id = logging::log_private_user_value(resolution_id.as_ref());
                let safe_resolution_label =
                    logging::log_private_user_value(resolution_label.as_ref());
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_resolve_segment",
                    segment_index,
                    replacement_bytes = safe_replacement.raw_bytes,
                    replacement_sha256 = %safe_replacement.sha256,
                    resolution_id_bytes = safe_resolution_id.raw_bytes,
                    resolution_id_sha256 = %safe_resolution_id.sha256,
                    resolution_label_bytes = safe_resolution_label.raw_bytes,
                    resolution_label_sha256 = %safe_resolution_label.sha256,
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
                            replacement_bytes = safe_replacement.raw_bytes,
                            replacement_sha256 = %safe_replacement.sha256,
                        );
                        self.try_submit_spine_prompt_plan_from_enter(cx);
                    }
                    applied
                }
            }
            SpineListAction::OpenModeExit { sigil, rest } => {
                let safe_rest = logging::log_private_user_value(&rest);
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_open_mode_exit",
                    sigil = %sigil,
                    rest_bytes = safe_rest.raw_bytes,
                    rest_sha256 = %safe_rest.sha256,
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
                    '?' if self.has_actions() => {
                        self.toggle_actions(cx, window);
                        true
                    }
                    _ => false,
                }
            }
            SpineListAction::OpenFileSearchPortal {
                segment_index,
                segment_byte_range,
                query,
            } => {
                let safe_query = logging::log_private_user_value(&query);
                tracing::info!(
                    target: "script_kit::spine",
                    event = "apply_spine_action_open_file_search_portal",
                    segment_index,
                    query_bytes = safe_query.raw_bytes,
                    query_sha256 = %safe_query.sha256,
                );
                self.open_spine_file_search_attachment_portal(
                    segment_byte_range,
                    query.to_string(),
                    cx,
                );
                true
            }
            SpineListAction::AcceptMenuSyntaxTrigger { row_id } => {
                self.accept_menu_syntax_trigger_picker_row(row_id.as_ref(), Some(window), cx)
            }
            SpineListAction::AcceptMenuSyntaxObject { row_id } => {
                self.accept_menu_syntax_object_selector_row(row_id.as_ref(), Some(window), cx)
            }
            SpineListAction::AttachContextResult { source } => {
                let inputs = self.root_search.computed_query_inputs();
                if inputs.spine_parse.input != inputs.raw
                    || !self.spine_projection_owns_main_list_for(
                        &inputs.spine_parse,
                        inputs.spine_projection.as_ref(),
                    )
                {
                    return false;
                }
                let Some(ResolvedMainMenuSelection::SearchResult {
                    row,
                    result: scripts::SearchResult::SpineProjection(selected),
                }) = self.resolved_main_menu_selected_subject()
                else {
                    return false;
                };
                if !row.eligibility.activatable
                    || !matches!(&selected.action, SpineListAction::AttachContextResult { source: owner } if owner == &source)
                {
                    return false;
                }
                let Some(outcome) = self.selected_spine_rich_subsearch_outcome() else {
                    return false;
                };
                self.apply_spine_attachment_outcome(outcome, window, cx)
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
        let safe_replacement = logging::log_private_user_value(replacement);

        tracing::info!(
            target: "script_kit::spine",
            event = "replace_active_segment_text",
            segment_index,
            old_range = ?segment_byte_range,
            replacement_bytes = safe_replacement.raw_bytes,
            replacement_sha256 = %safe_replacement.sha256,
            trailing_space,
            new_text_len = new_text.len(),
            cursor,
        );

        self.set_filter_text_and_cursor_immediate(new_text, cursor, window, cx);
        true
    }

    /// Set filter text with the caret at the end through the cursor-aware owner.
    pub(crate) fn set_filter_text_immediate(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let cursor = text.len();
        self.set_filter_text_and_cursor_immediate(text, cursor, window, cx);
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

    fn commit_selection_test_query(
        app: &mut ScriptListApp,
        query: &str,
        cx: &mut Context<ScriptListApp>,
    ) {
        app.filter_text = query.to_string();
        app.flush_pending_main_menu_query(cx);
        assert!(app.root_search.query_is_current());
        assert_eq!(
            app.main_menu_committed_query_stamp(),
            app.root_search.computed_query_stamp()
        );
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
            commit_selection_test_query(app, old_query, cx);

            let handoff_index = app
                .main_menu_result_caches
                .grouped_index_for_stable_selection_key(handoff_key)
                .expect("old query should include the Search Files handoff");
            assert!(app.select_main_menu_row(handoff_index, MainMenuSelectionOrigin::Keyboard, cx));

            commit_selection_test_query(app, new_query, cx);

            let selected_key = selected_main_menu_stable_key(app);
            let first_key = first_selectable_main_menu_stable_key(app);
            assert_eq!(app.computed_filter_text, new_query);
            assert!(matches!(
                app.main_menu_selection_intent(),
                MainMenuSelectionIntent::AutomaticAnchor { .. }
            ));
            assert_eq!(selected_key, first_key);
            assert_eq!(app.main_list_state.logical_scroll_top().item_ix, 0);
            assert_eq!(
                app.main_list_state.logical_scroll_top().offset_in_item,
                gpui::px(0.0)
            );
            assert_eq!(
                app.last_list_interaction_source,
                crate::scrolling::list_interaction::ListViewportInputSource::Filter
            );
            assert_ne!(selected_key.as_deref(), Some(handoff_key));
            assert!(app
                .main_menu_result_caches
                .search_result_for_grouped_item(app.selected_index)
                .is_some());
        });
    }

    #[gpui::test]
    fn clearing_filter_selects_first_result_and_resets_viewport(cx: &mut gpui::TestAppContext) {
        let app = main_menu_selection_test_app(cx);
        app.update(cx, |app, cx| {
            app.scripts = vec![
                main_menu_selection_test_script("zzwp5clear alpha"),
                main_menu_selection_test_script("zzwp5clear beta"),
            ];
            app.scriptlets.clear();
            app.skills.clear();
            app.apps.clear();
            commit_selection_test_query(app, "zzwp5clear", cx);
            app.main_list_state.scroll_to(gpui::ListOffset {
                item_ix: 1,
                offset_in_item: gpui::px(9.0),
            });

            commit_selection_test_query(app, "", cx);

            assert_eq!(app.computed_filter_text, "");
            assert_eq!(
                selected_main_menu_stable_key(app),
                first_selectable_main_menu_stable_key(app)
            );
            let cleared_offset = app.main_list_state.logical_scroll_top();
            assert_eq!(cleared_offset.item_ix, 0);
            assert_eq!(cleared_offset.offset_in_item, gpui::px(0.0));
        });
    }

    #[gpui::test]
    fn committed_same_query_refresh_preserves_offscreen_selection_and_viewport_anchor(
        cx: &mut gpui::TestAppContext,
    ) {
        let app = main_menu_selection_test_app(cx);
        app.update(cx, |app, cx| {
            app.scripts = (0..6)
                .map(|ix| main_menu_selection_test_script(&format!("zzwp5refresh {ix}")))
                .collect();
            app.scriptlets.clear();
            app.skills.clear();
            app.apps.clear();
            commit_selection_test_query(app, "zzwp5refresh", cx);

            let first = app
                .main_menu_result_caches
                .first_selectable_index()
                .expect("refresh fixture has selectable rows");
            let last = app
                .main_menu_result_caches
                .last_selectable_index()
                .expect("refresh fixture has a trailing selectable row");
            assert!(app.select_main_menu_row(first, MainMenuSelectionOrigin::Keyboard, cx));
            app.begin_list_viewport_scroll(crate::scrolling::list_interaction::ListViewportInputSource::Wheel, cx);
            let selected_key_before = selected_main_menu_stable_key(app);
            app.main_list_state.scroll_to(gpui::ListOffset {
                item_ix: last,
                offset_in_item: gpui::px(9.5),
            });
            let interaction_before = app.main_menu_interaction_snapshot();
            let anchor_before = interaction_before
                .viewport
                .first_visible_keys
                .first()
                .cloned()
                .expect("lazy viewport captures a stable leading row");
            assert!(
                !interaction_before
                    .viewport
                    .selected_was_within_safe_viewport
            );

            assert!(app.commit_main_menu_results_refresh("test_same_query_refresh", None, cx, |app, _cx| {
                app.scripts.reverse();
                true
            }));

            let viewport_after = app.main_menu_viewport_snapshot();
            assert_eq!(selected_main_menu_stable_key(app), selected_key_before);
            assert!(matches!(app.main_menu_selection_intent(), MainMenuSelectionIntent::ExplicitAnchor { stable_key } if Some(stable_key) == selected_key_before.as_ref()));
            assert_eq!(
                viewport_after.first_visible_keys.first(),
                Some(&anchor_before)
            );
            assert_eq!(viewport_after.offset_in_item, gpui::px(9.5));
            assert!(!viewport_after.selected_was_within_safe_viewport);
            assert_eq!(
                app.last_list_interaction_source,
                crate::scrolling::list_interaction::ListViewportInputSource::Refresh
            );
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
