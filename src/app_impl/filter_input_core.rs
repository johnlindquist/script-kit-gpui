use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScriptListSpecialEntry {
    FileSearchMini { query: String },
    QuickTerminal,
    ActionsHelp,
    DayPage,
}

impl ScriptListApp {
    /// Transient first-character launch triggers should not persist when the
    /// user returns to the ScriptList surface.
    pub(crate) fn is_transient_script_list_trigger(new_text: &str) -> bool {
        matches!(new_text, "~" | "!" | "?" | " ")
    }

    /// Parse `raw` through the menu-syntax classifier and store the result in
    /// [`ScriptListApp::menu_syntax_mode`]. Called from every input-change
    /// boundary (per-keystroke `handle_filter_input_change`, programmatic
    /// `set_filter_text_immediate`) so result grouping and execution see a
    /// snapshot tied to the current raw input instead of racing the filter
    /// coalescer's `computed_filter_text` field.
    pub(crate) fn set_menu_syntax_mode_from_filter(&mut self, raw: &str) {
        if self.spine_enabled && (raw.starts_with('>') || raw.starts_with('!')) {
            self.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::default();
            self.menu_syntax_object_selector_state = Default::default();
            self.menu_syntax_trigger_picker_state = Default::default();
            return;
        }
        let capture_targets =
            crate::menu_syntax::registered_capture_targets_from_scripts(&self.scripts);
        self.menu_syntax_mode = crate::menu_syntax::MenuSyntaxMode::from_input_with_capture_targets(
            raw,
            &capture_targets,
        );
    }

    pub(crate) fn current_view_uses_shared_filter_input(&self) -> bool {
        matches!(
            self.current_view,
            AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::BrowserTabsView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::FileSearchView { .. }
                | AppView::ProfileSearchView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::FlowUxView { .. }
                // Flow sessions compose in the shared MAIN input: the text is
                // a message draft, not a query, but the input plumbing (change
                // events, actions-open preservation, resync) is the same.
                | AppView::FlowSessionView { .. }
                | AppView::SettingsView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::AgentChatHistoryView { .. }
                | AppView::BrowserHistoryView { .. }
                | AppView::DictationHistoryView { .. }
                | AppView::NotesBrowseView { .. }
                | AppView::TipsView { .. }
        )
    }

    pub(crate) fn source_filter_mode_blocks_input_history_recall(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
            && self
                .menu_syntax_mode
                .advanced_query_for(&self.filter_text)
                .is_some_and(|query| query.has_source_filters())
    }

    pub(crate) fn single_line_filter_input_contains_newline(new_text: &str) -> bool {
        new_text.contains('\n') || new_text.contains('\r')
    }

    pub(crate) fn sync_builtin_query_state(
        query: &mut String,
        selected_index: &mut usize,
        new_text: &str,
    ) -> bool {
        if query == new_text {
            return false;
        }

        *query = new_text.to_string();
        *selected_index = 0;
        true
    }

    pub(crate) fn clear_builtin_query_state(query: &mut String, selected_index: &mut usize) {
        query.clear();
        *selected_index = 0;
    }

    /// Returns `true` when the typed text should hand off from ScriptList into
    /// mini file search (the `~` trigger).
    pub(crate) fn should_enter_file_search_from_script_list(new_text: &str) -> bool {
        new_text == "~" || new_text.starts_with("~/")
    }

    /// Normalises the mini file-search query: bare `~` becomes `~/` so the
    /// directory listing starts immediately.
    pub(crate) fn normalize_mini_file_search_query(new_text: &str) -> String {
        if new_text == "~" {
            "~/".to_string()
        } else {
            new_text.to_string()
        }
    }

    /// Classify narrow, first-character ScriptList handoffs into dedicated
    /// surfaces so regular search queries are not hijacked.
    pub(crate) fn special_entry_from_script_list_filter(
        new_text: &str,
    ) -> Option<ScriptListSpecialEntry> {
        if Self::should_enter_file_search_from_script_list(new_text) {
            return Some(ScriptListSpecialEntry::FileSearchMini {
                query: Self::normalize_mini_file_search_query(new_text),
            });
        }

        match new_text {
            "!" => Some(ScriptListSpecialEntry::QuickTerminal),
            ">" => None,
            "?" => Some(ScriptListSpecialEntry::ActionsHelp),
            " " => Some(ScriptListSpecialEntry::DayPage),
            _ => None,
        }
    }

    /// Choose the correct resize path for file search based on presentation mode.
    pub(crate) fn resize_file_search_window_for_presentation(
        presentation: FileSearchPresentation,
        result_count: usize,
    ) {
        match presentation {
            FileSearchPresentation::Mini => {
                crate::window_resize::resize_to_file_search_window_sync(result_count);
            }
            FileSearchPresentation::Full => resize_to_view_sync(ViewType::MainWindow, 0),
        }
    }

    /// Resize only when the mini explorer's row count change should actually
    /// affect window height.  Full presentation is fixed-size and should not
    /// re-enter synchronous resize work on every stream batch.
    pub(crate) fn resize_file_search_window_after_results_change(
        presentation: FileSearchPresentation,
        result_count: usize,
        is_first_batch: bool,
        is_done: bool,
    ) {
        match presentation {
            FileSearchPresentation::Mini if is_first_batch || is_done => {
                crate::window_resize::resize_to_file_search_window_sync(result_count);
            }
            FileSearchPresentation::Full if is_first_batch => {
                resize_to_view_sync(ViewType::MainWindow, 0);
            }
            _ => {} // skip intermediate batch resizes
        }
    }

    // ── Spine projection helpers ──────────────────────────────────────

    /// Reparse the Spine model from the current filter text and cursor position.
    pub(crate) fn set_spine_parse_from_filter_and_cursor(&mut self, raw: &str, cursor: usize) {
        self.spine_parse = crate::spine::parse_spine(raw);
        self.spine_projection = Some(crate::spine::project_cursor(&self.spine_parse, cursor));
        // The Down/click arming of an empty `@source:` colon mode only
        // survives while that exact empty colon mode persists; typing,
        // deleting back, or switching sources re-suppresses the selection.
        if self.spine_empty_subsearch_armed_for.is_some()
            && self.spine_empty_subsearch_armed_for != self.active_empty_spine_subsearch_source()
        {
            self.spine_empty_subsearch_armed_for = None;
        }
    }

    /// The active `@source:` subsearch when its sub-query is empty (the
    /// colon-mode landing state), else `None`.
    fn active_empty_spine_subsearch_source(
        &self,
    ) -> Option<crate::spine::catalog_subsearch::ContextSubsearchSource> {
        self.active_spine_context_subsearch()
            .filter(|(_, query)| query.trim().is_empty())
            .map(|(source, _)| source)
    }

    /// Whether the main list renders UNARMED (no selected row): the empty
    /// `@source:` colon mode shows recents, but the first concrete row must
    /// not be Enter-able until the user explicitly chooses with Down or a
    /// click. Replaces the old selectable "Type to search" guard row.
    pub(crate) fn spine_empty_subsearch_selection_suppressed(&self) -> bool {
        match self.active_empty_spine_subsearch_source() {
            Some(source) => self.spine_empty_subsearch_armed_for != Some(source),
            None => false,
        }
    }

    /// Explicit choose gesture (Down or click) in an empty colon mode: arm
    /// the selection so it renders and Enter works.
    pub(crate) fn arm_spine_empty_subsearch_selection(&mut self) {
        if let Some(source) = self.active_empty_spine_subsearch_source() {
            self.spine_empty_subsearch_armed_for = Some(source);
        }
    }

    /// Whether the Spine projection currently owns the main list (i.e., a sigil
    /// segment is active and should replace the normal unified search results).
    pub(crate) fn spine_projection_owns_main_list(&self) -> bool {
        self.spine_projection_owns_main_list_for(&self.spine_parse, self.spine_projection.as_ref())
    }

    pub(crate) fn spine_projection_owns_main_list_for(
        &self,
        parse: &crate::spine::SpineParse,
        projection: Option<&crate::spine::SpineCursorProjection>,
    ) -> bool {
        if !self.spine_enabled {
            return false;
        }
        if matches!(self.current_view, AppView::DayPage { .. }) {
            return false;
        }
        match projection {
            Some(proj) => {
                // Committed postfix captures (`todo; …`) are owned by the
                // menu-syntax capture composer (the restored form
                // experience). The spine owns capture target *selection*
                // (the `;to` prefix spelling) but classifies the committed
                // postfix form only for highlighting and submit blocking.
                if matches!(
                    proj.active_segment_kind,
                    crate::spine::SpineSegmentKind::Capture { .. }
                ) {
                    let raw = parse
                        .segments
                        .get(proj.active_segment_index)
                        .map(|segment| segment.raw.as_str())
                        .unwrap_or("");
                    if !raw.starts_with(';') {
                        return false;
                    }
                }
                // `-` flow search is an Agent Chat composer feature. The
                // main menu already lists flows as primary rows, so a
                // leading dash stays plain query text here instead of
                // swapping the list for the spine Flows section.
                if matches!(
                    proj.active_segment_kind,
                    crate::spine::SpineSegmentKind::Flow { .. }
                ) {
                    return false;
                }
                if let crate::spine::SpineSegmentKind::ListFilter { query } =
                    &proj.active_segment_kind
                {
                    if parse
                        .segments
                        .get(proj.active_segment_index)
                        .is_some_and(|segment| {
                            crate::menu_syntax::list_filter_segment_is_terminal(
                                &parse.input,
                                segment.byte_range.clone(),
                                query,
                            )
                        })
                    {
                        return false;
                    }
                }
                !matches!(
                    proj.active_segment_kind,
                    crate::spine::SpineSegmentKind::FreeText
                )
            }
            None => false,
        }
    }

    /// Shared helper that opens file search in the given presentation mode.
    /// Used by both the builtin "Search Files" entry (Full) and the `~`
    /// trigger from ScriptList (Mini).
    ///
    /// Directory mini-entry from ScriptList seeds its first rows before
    /// switching surfaces; other opens paint loading state and stream results
    /// asynchronously via `restart_file_search_stream_for_query`.
    pub(crate) fn open_file_search_view(
        &mut self,
        query: String,
        presentation: FileSearchPresentation,
        cx: &mut Context<Self>,
    ) {
        self.open_file_search_view_with_result_transition(query, presentation, false, cx);
    }

    /// Browse within file search without blanking the current directory view
    /// before the next directory stream has produced its first batch.
    pub(crate) fn open_file_search_view_preserving_current_results(
        &mut self,
        query: String,
        presentation: FileSearchPresentation,
        cx: &mut Context<Self>,
    ) {
        self.open_file_search_view_with_result_transition(query, presentation, true, cx);
    }

    fn seed_file_search_directory_results_for_first_paint(
        &mut self,
        query: &str,
    ) -> std::io::Result<bool> {
        let Some(parsed) = crate::file_search::parse_directory_path(query) else {
            return Ok(false);
        };

        let results = crate::file_search::list_directory_with_options(
            &parsed.directory,
            crate::file_search::DEFAULT_CACHE_LIMIT,
            parsed.show_hidden,
        )?;

        if results.is_empty() {
            tracing::info!(
                category = "FILE_SEARCH",
                query,
                directory = %parsed.directory,
                "File-search first-paint seed found no directory rows"
            );
            return Ok(false);
        }

        self.cached_file_results = results;
        self.file_search_current_dir = Some(parsed.directory.clone());
        self.file_search_current_dir_show_hidden = parsed.show_hidden;
        self.file_search_frozen_filter = None;
        self.apply_file_search_sort_mode();
        self.recompute_file_search_display_indices();
        self.restore_file_search_selection_after_results_change(None);

        tracing::info!(
            category = "FILE_SEARCH",
            query,
            directory = %parsed.directory,
            cached_count = self.cached_file_results.len(),
            display_count = self.file_search_display_indices.len(),
            "Seeded file-search directory rows before first paint"
        );

        Ok(true)
    }

    fn seed_file_search_default_results_for_first_paint(&mut self) -> std::io::Result<bool> {
        let mut results = self
            .recent_file_results_from_frecency(crate::file_search::ROOT_FILE_RECENT_HYDRATE_LIMIT);
        let source = if results.is_empty() {
            results = crate::file_search::list_directory_with_options(
                "~/",
                crate::file_search::DEFAULT_CACHE_LIMIT,
                false,
            )?;
            "home_directory"
        } else {
            "frecency_recent_files"
        };

        if results.is_empty() {
            tracing::info!(
                category = "FILE_SEARCH",
                "Default file-search first-paint seed found no recent or home-directory rows"
            );
            return Ok(false);
        }

        self.cached_file_results = results;
        self.file_search_current_dir = None;
        self.file_search_current_dir_show_hidden = false;
        self.file_search_frozen_filter = None;
        self.recompute_file_search_display_indices();
        self.restore_file_search_selection_after_results_change(None);
        self.file_search_loading = false;

        tracing::info!(
            category = "FILE_SEARCH",
            source,
            cached_count = self.cached_file_results.len(),
            display_count = self.file_search_display_indices.len(),
            "Seeded default file-search rows before first paint"
        );

        Ok(true)
    }

    fn open_file_search_view_with_result_transition(
        &mut self,
        query: String,
        presentation: FileSearchPresentation,
        preserve_current_results_until_first_batch: bool,
        cx: &mut Context<Self>,
    ) {
        // Preserve sort mode when already inside file search (e.g. browsing
        // into a subdirectory) — only reset on fresh entry from outside.
        let preserve_sort_mode = matches!(self.current_view, AppView::FileSearchView { .. });

        tracing::info!(
            category = "FILE_SEARCH",
            %query,
            ?presentation,
            preserve_sort_mode,
            preserve_current_results_until_first_batch,
            "Opening file search view"
        );

        self.filter_text = query.clone();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search files...".to_string());
        self.gpui_input_state.update(cx, |state, _cx| {
            state.clear_highlight_ranges();
        });

        self.current_view = AppView::FileSearchView {
            query: query.clone(),
            selected_index: 0,
            presentation,
        };
        self.note_main_route_changed();
        self.rekey_main_automation_surface_from_current_view();
        self.hovered_index = None;
        // opened_from_main_menu intentionally NOT set here — the entry point
        // owns launch origin (chaos-20 F1: protocol/tray opens must CLOSE on
        // Esc, not return to the launcher; see trigger_builtin_dispatch.rs
        // intent comment and open_builtin_filterable_view's doc comment).

        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;

        if !preserve_sort_mode {
            self.file_search_sort_mode = crate::actions::FileSearchSortMode::default();
        }

        // Any non-preserving open into a concrete directory — the Mini `~`
        // browse, the Full `~` open, the Full cwd picker opened via Tab
        // (query "~/"), and the Backspace reseed to "/" — paints that
        // directory's rows synchronously on the first frame instead of flashing
        // a loading skeleton while the async stream task spawns. The
        // synchronous listing is sub-millisecond for typical directories
        // (~0.5 ms for a 100-entry home dir); the stream still runs afterward
        // to reconcile. In-view subdirectory browsing/filtering uses
        // `open_file_search_view_preserving_current_results`
        // (preserve_current_results_until_first_batch = true), which keeps the
        // previous rows visible and is intentionally left on the async path.
        let fresh_directory_entry = !preserve_current_results_until_first_batch;
        let seeded_initial_results = if self.main_services.is_production() && fresh_directory_entry
        {
            match self.seed_file_search_directory_results_for_first_paint(&query) {
                Ok(seeded) => seeded,
                Err(error) => {
                    let _ = self.begin_file_search_session();
                    self.file_search_loading = false;
                    self.show_error_toast(format!("File search failed: {error}"), cx);
                    return;
                }
            }
        } else {
            false
        };
        let seed_fresh_full_default = !preserve_current_results_until_first_batch
            && !preserve_sort_mode
            && presentation == FileSearchPresentation::Full
            && query.trim().is_empty();
        let seeded_default_results =
            if self.main_services.is_production() && seed_fresh_full_default {
                match self.seed_file_search_default_results_for_first_paint() {
                    Ok(seeded) => seeded,
                    Err(error) => {
                        let _ = self.begin_file_search_session();
                        self.file_search_loading = false;
                        self.show_error_toast(format!("File search failed: {error}"), cx);
                        return;
                    }
                }
            } else {
                false
            };
        let seeded_initial_results = seeded_initial_results || seeded_default_results;

        if !preserve_current_results_until_first_batch && !seeded_initial_results {
            self.cached_file_results.clear();
            self.file_search_display_indices.clear();
            self.file_search_current_dir = None;
            self.file_search_current_dir_show_hidden = false;
        }
        self.file_search_frozen_filter = None;
        self.file_search_selection_mode = FileSearchSelectionMode::AutoFirst;

        // Fresh opens paint their empty/loading state immediately. Internal
        // directory browsing keeps the previous rows and size until the next
        // stream's first batch replaces them.
        if !preserve_current_results_until_first_batch {
            Self::resize_file_search_window_for_presentation(
                presentation,
                self.file_search_display_indices.len(),
            );
        }

        if seeded_default_results {
            let _ = self.begin_file_search_session();
            cx.notify();
            return;
        }

        let preserve_stream_results =
            preserve_current_results_until_first_batch || seeded_initial_results;
        self.restart_file_search_stream_for_query(
            query,
            presentation,
            None,
            preserve_stream_results,
            cx,
        );
    }

    pub(crate) fn accept_ghost_prediction(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        let Some(accepted_suffix) = self
            .ghost_prediction
            .as_ref()
            .filter(|prediction| prediction.accepts_tab())
            .map(|prediction| {
                crate::scripts::search::ghost::first_word_acceptance_suffix(
                    &prediction.ghost_suffix,
                )
                .to_string()
            })
        else {
            return false;
        };
        if accepted_suffix.is_empty() {
            return false;
        }
        let accepted = self.gpui_input_state.update(cx, |state, window_cx| {
            state.clear_inline_completion(window_cx);
            state.insert(accepted_suffix.clone(), window, window_cx);
            true
        });
        if accepted {
            self.ghost_prediction = None;
            cx.notify();
        }
        accepted
    }

    pub(crate) fn accept_full_ghost_prediction(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> bool {
        if self.ghost_prediction.is_none() {
            return false;
        }
        if self
            .ghost_prediction
            .as_ref()
            .is_some_and(|prediction| !prediction.accepts_tab())
        {
            return false;
        }
        let accepted = self.gpui_input_state.update(cx, |state, window_cx| {
            state.accept_inline_completion(window, window_cx)
        });
        if accepted {
            self.ghost_prediction = None;
            cx.notify();
        }
        accepted
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[test]
    fn test_current_view_uses_shared_filter_input_includes_script_list_and_builtin_views() {
        let source = fs::read_to_string("src/app_impl/filter_input_core.rs")
            .expect("Failed to read src/app_impl/filter_input_core.rs");
        let required_views = [
            "AppView::ScriptList",
            "AppView::ClipboardHistoryView",
            "AppView::EmojiPickerView",
            "AppView::AppLauncherView",
            "AppView::WindowSwitcherView",
            "AppView::BrowserTabsView",
            "AppView::ThemeChooserView",
            "AppView::FileSearchView",
            "AppView::SettingsView",
            "AppView::BrowserHistoryView",
            // Flow sessions compose in the shared MAIN input; dropping this
            // arm makes the composer draft silently vanish (2026-07-10).
            "AppView::FlowSessionView",
        ];

        for view in required_views {
            assert!(
                source.contains(view),
                "current_view_uses_shared_filter_input must include {}",
                view
            );
        }
    }

    #[test]
    fn accept_ghost_prediction_respects_hint_only_predictions() {
        let source = fs::read_to_string("src/app_impl/filter_input_core.rs")
            .expect("Failed to read src/app_impl/filter_input_core.rs");
        let accept_pos = source
            .find("pub(crate) fn accept_ghost_prediction")
            .expect("accept_ghost_prediction should exist");
        let body = &source[accept_pos..(accept_pos + 900).min(source.len())];
        assert!(
            body.contains("accepts_tab()")
                && body.contains("first_word_acceptance_suffix")
                && body.contains("state.insert("),
            "Tab ghost acceptance must gate on accepts_tab and insert only the first accepted suffix"
        );
        assert!(
            body.find("accepts_tab()") < body.find("state.insert("),
            "hint-only ghost predictions must be rejected before inserting accepted ghost text"
        );

        let full_accept_pos = source
            .find("pub(crate) fn accept_full_ghost_prediction")
            .expect("accept_full_ghost_prediction should exist");
        let full_body = &source[full_accept_pos..(full_accept_pos + 900).min(source.len())];
        assert!(
            full_body.contains("accepts_tab()") && full_body.contains("accept_inline_completion"),
            "Backquote ghost acceptance must gate on accepts_tab before accepting the full inline completion"
        );
        assert!(
            full_body.find("accepts_tab()") < full_body.find("accept_inline_completion"),
            "hint-only ghost predictions must be rejected before full inline completion acceptance"
        );
    }

    #[test]
    fn test_should_enter_file_search_from_script_list() {
        use super::ScriptListApp;
        assert!(ScriptListApp::should_enter_file_search_from_script_list(
            "~"
        ));
        assert!(ScriptListApp::should_enter_file_search_from_script_list(
            "~/src"
        ));
        assert!(!ScriptListApp::should_enter_file_search_from_script_list(
            "foo"
        ));
        assert!(!ScriptListApp::should_enter_file_search_from_script_list(
            "/tmp"
        ));
        assert!(!ScriptListApp::should_enter_file_search_from_script_list(
            ""
        ));
    }

    #[test]
    fn test_normalize_mini_file_search_query() {
        use super::ScriptListApp;
        assert_eq!(ScriptListApp::normalize_mini_file_search_query("~"), "~/");
        assert_eq!(
            ScriptListApp::normalize_mini_file_search_query("~/src"),
            "~/src"
        );
    }

    #[test]
    fn test_special_entry_from_script_list_filter() {
        use super::{ScriptListApp, ScriptListSpecialEntry};

        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("~"),
            Some(ScriptListSpecialEntry::FileSearchMini {
                query: "~/".to_string()
            })
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("~/src"),
            Some(ScriptListSpecialEntry::FileSearchMini {
                query: "~/src".to_string()
            })
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("/"),
            None,
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("@"),
            None,
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("|"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("!"),
            Some(ScriptListSpecialEntry::QuickTerminal)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("!dep"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(">"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(">deploy"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("?"),
            Some(ScriptListSpecialEntry::ActionsHelp)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(" "),
            Some(ScriptListSpecialEntry::DayPage)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(","),
            None
        );
        // A space-prefixed query (only reachable via paste or programmatic
        // setFilter, since the bare space swaps surfaces on the first keystroke)
        // must stay normal search text.
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(" todo"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("/tmp"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("@browser"),
            None
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("foo"),
            None
        );
    }

    #[test]
    fn test_is_transient_script_list_trigger() {
        use super::ScriptListApp;

        for trigger in ["~", "!", "?", " "] {
            assert!(
                ScriptListApp::is_transient_script_list_trigger(trigger),
                "expected '{trigger}' to be transient"
            );
        }

        for query in [
            ">", ">deploy", "!dep", ",", ",todo", " todo", "/", "@", "|", ".", ";", "~/src",
            "@browser", "/tmp", "foo", "",
        ] {
            assert!(
                !ScriptListApp::is_transient_script_list_trigger(query),
                "expected '{query}' to remain a real query"
            );
        }
    }

    #[test]
    fn test_power_syntax_prefixes_do_not_route_to_special_surfaces() {
        use super::ScriptListApp;

        for query in [
            ":type:script shortcut:true",
            ":",
            ";todo Renew passport tomorrow",
            "+",
            "+xyz unknown target",
            "todo: Renew passport tomorrow",
            "cal: Lunch next friday",
            ">deploy -- prod",
            "!dep",
            "#finance search",
        ] {
            assert_eq!(
                ScriptListApp::special_entry_from_script_list_filter(query),
                None,
                "power-user syntax '{query}' must not route through special_entry_from_script_list_filter"
            );
        }
    }

    #[test]
    fn test_power_syntax_prefixes_are_not_transient_triggers() {
        use super::ScriptListApp;

        for prefix in [":", "+", "#"] {
            assert!(
                !ScriptListApp::is_transient_script_list_trigger(prefix),
                "power-user prefix '{prefix}' must not be classified as a transient trigger"
            );
        }
        assert!(
            !ScriptListApp::is_transient_script_list_trigger("!dep"),
            "bang-prefixed query text must not be classified as a transient trigger"
        );
    }

    #[test]
    fn test_single_line_filter_input_contains_newline_detects_enter_leaks() {
        use super::ScriptListApp;

        assert!(!ScriptListApp::single_line_filter_input_contains_newline(
            "do in"
        ));
        assert!(ScriptListApp::single_line_filter_input_contains_newline(
            "do in\n"
        ));
        assert!(ScriptListApp::single_line_filter_input_contains_newline(
            "do in\r"
        ));
        assert!(ScriptListApp::single_line_filter_input_contains_newline(
            "do in\r\n"
        ));
    }
}
