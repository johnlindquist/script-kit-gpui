use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScriptListSpecialEntry {
    FileSearchMini { query: String },
    AcpSlashPicker,
    AcpMentionPicker,
    QuickTerminal,
    ActionsHelp,
}

impl ScriptListApp {
    /// Transient first-character launch triggers should not persist when the
    /// user returns to the ScriptList surface.
    pub(crate) fn is_transient_script_list_trigger(new_text: &str) -> bool {
        matches!(new_text, "~" | "/" | "@" | ">" | "?")
    }

    pub(crate) fn current_view_uses_shared_filter_input(&self) -> bool {
        matches!(
            self.current_view,
            AppView::ScriptList
                | AppView::ClipboardHistoryView { .. }
                | AppView::EmojiPickerView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::DesignGalleryView { .. }
                | AppView::ThemeChooserView { .. }
                | AppView::FileSearchView { .. }
                | AppView::ProcessManagerView { .. }
                | AppView::SettingsView { .. }
                | AppView::CurrentAppCommandsView { .. }
                | AppView::SearchAiPresetsView { .. }
                | AppView::AcpHistoryView { .. }
                | AppView::NotesBrowseView { .. }
        )
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
            "/" => Some(ScriptListSpecialEntry::AcpSlashPicker),
            "@" => Some(ScriptListSpecialEntry::AcpMentionPicker),
            ">" => Some(ScriptListSpecialEntry::QuickTerminal),
            "?" => Some(ScriptListSpecialEntry::ActionsHelp),
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
                crate::window_resize::resize_to_mini_file_search_window_sync(result_count);
            }
            FileSearchPresentation::Full => resize_to_view_sync(ViewType::ScriptList, 0),
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
                crate::window_resize::resize_to_mini_file_search_window_sync(result_count);
            }
            FileSearchPresentation::Full if is_first_batch => {
                resize_to_view_sync(ViewType::ScriptList, 0);
            }
            _ => {} // skip intermediate batch resizes
        }
    }

    /// Shared helper that opens file search in the given presentation mode.
    /// Used by both the builtin "Search Files" entry (Full) and the `~`
    /// trigger from ScriptList (Mini).
    ///
    /// The view paints immediately with an empty/loading state; results
    /// stream in asynchronously via `restart_file_search_stream_for_query`.
    pub(crate) fn open_file_search_view(
        &mut self,
        query: String,
        presentation: FileSearchPresentation,
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
            "Opening file search view"
        );

        self.filter_text = query.clone();
        self.pending_filter_sync = true;
        self.pending_placeholder = Some("Search files...".to_string());

        self.current_view = AppView::FileSearchView {
            query: query.clone(),
            selected_index: 0,
            presentation,
        };
        self.hovered_index = None;
        self.opened_from_main_menu = true;

        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;

        self.cached_file_results.clear();
        self.file_search_display_indices.clear();
        self.file_search_current_dir = None;
        self.file_search_frozen_filter = None;
        self.file_search_selection_mode = FileSearchSelectionMode::AutoFirst;
        if !preserve_sort_mode {
            self.file_search_sort_mode = crate::actions::FileSearchSortMode::default();
        }

        // Full view still needs its split-view resize immediately.
        // Mini opens small and grows only as results arrive.
        Self::resize_file_search_window_for_presentation(presentation, 0);

        self.restart_file_search_stream_for_query(query, presentation, None, false, cx);
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
            "AppView::DesignGalleryView",
            "AppView::ThemeChooserView",
            "AppView::FileSearchView",
            "AppView::SettingsView",
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
            Some(ScriptListSpecialEntry::AcpSlashPicker)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("@"),
            Some(ScriptListSpecialEntry::AcpMentionPicker)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter(">"),
            Some(ScriptListSpecialEntry::QuickTerminal)
        );
        assert_eq!(
            ScriptListApp::special_entry_from_script_list_filter("?"),
            Some(ScriptListSpecialEntry::ActionsHelp)
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

        for trigger in ["~", "/", "@", ">", "?"] {
            assert!(
                ScriptListApp::is_transient_script_list_trigger(trigger),
                "expected '{trigger}' to be treated as a transient ScriptList trigger"
            );
        }

        for query in ["~/src", "@browser", "/tmp", "foo", ""] {
            assert!(
                !ScriptListApp::is_transient_script_list_trigger(query),
                "expected '{query}' to remain a real query"
            );
        }
    }
}
