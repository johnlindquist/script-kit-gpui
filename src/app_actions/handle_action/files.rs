// File-related action handlers for handle_action dispatch.
//
// Contains: reveal_in_finder, copy_path, copy_deeplink, file search actions
// (open_file, open_directory, quick_look, open_with, show_info, attach_to_ai),
// copy_filename, __cancel__.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchHandlerAction {
    Open,
    QuickLook,
    OpenWith,
    ShowInfo,
    AttachToAi,
}

impl FileSearchHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "open_file" | "open_directory" => Some(Self::Open),
            "quick_look" => Some(Self::QuickLook),
            "open_with" => Some(Self::OpenWith),
            "show_info" => Some(Self::ShowInfo),
            "attach_to_ai" => Some(Self::AttachToAi),
            _ => None,
        }
    }

    fn success_hud(self, action_id: &str) -> Option<&'static str> {
        match self {
            Self::Open | Self::QuickLook | Self::OpenWith | Self::ShowInfo => {
                file_search_action_success_hud(action_id)
            }
            Self::AttachToAi => None,
        }
    }

    fn error_prefix(self, action_id: &str) -> &'static str {
        match self {
            Self::Open | Self::QuickLook | Self::OpenWith | Self::ShowInfo => {
                file_search_action_error_hud_prefix(action_id)
                    .unwrap_or("Failed to complete action")
            }
            Self::AttachToAi => "Failed to attach",
        }
    }

    fn hides_main_after_success(self) -> bool {
        matches!(self, Self::Open)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchSortHandlerAction {
    NameAsc,
    NameDesc,
    ModifiedDesc,
    ModifiedAsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchCurrentDirectoryAction {
    Refresh,
    Reveal,
    CopyPath,
    OpenQuickTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchEditorHandlerAction {
    OpenInEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchRenameHandlerAction {
    RenamePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchMoveHandlerAction {
    MovePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchDuplicateHandlerAction {
    DuplicatePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchTrashHandlerAction {
    MoveToTrash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchFilenameCopyHandlerAction {
    CopyFilename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchPathCopyHandlerAction {
    CopyPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchDeeplinkCopyHandlerAction {
    CopyDeeplink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchRevealHandlerAction {
    RevealInFinder,
}

impl FileSearchSortHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "sort_name_asc" => Some(Self::NameAsc),
            "sort_name_desc" => Some(Self::NameDesc),
            "sort_modified_desc" => Some(Self::ModifiedDesc),
            "sort_modified_asc" => Some(Self::ModifiedAsc),
            _ => None,
        }
    }

    fn mode(self) -> crate::actions::FileSearchSortMode {
        match self {
            Self::NameAsc => crate::actions::FileSearchSortMode::NameAsc,
            Self::NameDesc => crate::actions::FileSearchSortMode::NameDesc,
            Self::ModifiedDesc => crate::actions::FileSearchSortMode::ModifiedDesc,
            Self::ModifiedAsc => crate::actions::FileSearchSortMode::ModifiedAsc,
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::NameAsc => "Sorted by Name (A\u{2192}Z)",
            Self::NameDesc => "Sorted by Name (Z\u{2192}A)",
            Self::ModifiedDesc => "Sorted by Modified (Newest)",
            Self::ModifiedAsc => "Sorted by Modified (Oldest)",
        }
    }
}

impl FileSearchCurrentDirectoryAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "refresh_directory" => Some(Self::Refresh),
            "reveal_current_directory" => Some(Self::Reveal),
            "copy_current_directory_path" => Some(Self::CopyPath),
            "open_current_directory_in_quick_terminal" => Some(Self::OpenQuickTerminal),
            _ => None,
        }
    }

    fn missing_directory_message(self) -> &'static str {
        match self {
            Self::Refresh => "No current directory to refresh",
            Self::Reveal => "No current directory to reveal",
            Self::CopyPath => "No current directory to copy",
            Self::OpenQuickTerminal => "No current directory to open",
        }
    }

    fn success_hud(self, dir: &str) -> Option<String> {
        match self {
            Self::Refresh => Some("Refreshed Directory".to_string()),
            Self::Reveal => Some("Opened in Finder".to_string()),
            Self::CopyPath => Some(format!("Copied: {dir}")),
            Self::OpenQuickTerminal => None,
        }
    }

    fn error_prefix(self) -> &'static str {
        match self {
            Self::Reveal => "Failed to reveal current directory",
            Self::OpenQuickTerminal => "Failed to open current directory in Quick Terminal",
            Self::Refresh | Self::CopyPath => "Failed to use current directory",
        }
    }
}

impl FileSearchEditorHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "open_in_editor" => Some(Self::OpenInEditor),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::OpenInEditor => "No file selected",
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::OpenInEditor => "Opened in Editor",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::OpenInEditor => format!("Failed to open in editor: {error}"),
        }
    }
}

impl FileSearchRenameHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "rename_path" => Some(Self::RenamePath),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::RenamePath => "No file selected",
        }
    }

    fn success_hud(self, new_name: &str) -> String {
        match self {
            Self::RenamePath => format!("Renamed to {new_name}"),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::RenamePath => format!("Failed to rename: {error}"),
        }
    }
}

impl FileSearchMoveHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "move_path" => Some(Self::MovePath),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::MovePath => "No file selected",
        }
    }

    fn success_hud(self, destination_dir: &str) -> String {
        match self {
            Self::MovePath => {
                format!("Moved to {}", crate::file_search::shorten_path(destination_dir))
            }
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::MovePath => format!("Failed to move: {error}"),
        }
    }
}

impl FileSearchDuplicateHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "duplicate_path" => Some(Self::DuplicatePath),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::DuplicatePath => "No file selected",
        }
    }

    fn success_hud(self, name: &str) -> String {
        match self {
            Self::DuplicatePath => format!("Duplicated {name}"),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::DuplicatePath => format!("Failed to duplicate: {error}"),
        }
    }
}

impl FileSearchTrashHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "move_to_trash" => Some(Self::MoveToTrash),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::MoveToTrash => "No file selected",
        }
    }

    fn confirm_title(self) -> &'static str {
        match self {
            Self::MoveToTrash => "Move to Trash",
        }
    }

    fn confirm_message(self, name: &str) -> String {
        match self {
            Self::MoveToTrash => format!("Move \"{name}\" to Trash?"),
        }
    }

    fn confirm_button(self) -> &'static str {
        match self {
            Self::MoveToTrash => "Move to Trash",
        }
    }

    fn confirmation_failure_message(self) -> &'static str {
        match self {
            Self::MoveToTrash => "Failed to open confirmation dialog",
        }
    }

    fn success_hud(self, name: &str) -> String {
        match self {
            Self::MoveToTrash => format!("Moved to Trash: {name}"),
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::MoveToTrash => format!("Failed to move to Trash: {error}"),
        }
    }
}

impl FileSearchFilenameCopyHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "copy_filename" => Some(Self::CopyFilename),
            _ => None,
        }
    }

    fn selection_required_message(self) -> &'static str {
        match self {
            Self::CopyFilename => "No file selected",
        }
    }

    fn copied_hud(self, name: &str) -> String {
        match self {
            Self::CopyFilename => format!("Copied filename: {name}"),
        }
    }
}

impl FileSearchPathCopyHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "copy_path" => Some(Self::CopyPath),
            _ => None,
        }
    }

    fn copied_hud(self, path: &str) -> String {
        match self {
            Self::CopyPath => format!("Copied: {path}"),
        }
    }
}

impl FileSearchDeeplinkCopyHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "copy_deeplink" => Some(Self::CopyDeeplink),
            _ => None,
        }
    }

    fn share_hud(self, title: &str) -> String {
        match self {
            Self::CopyDeeplink => format!("Copied share link for {title}"),
        }
    }

    fn deeplink_hud(self, deeplink_url: &str) -> String {
        match self {
            Self::CopyDeeplink => format!("Copied: {deeplink_url}"),
        }
    }

    fn share_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::CopyDeeplink => format!("Failed to build share link: {error}"),
        }
    }
}

impl FileSearchRevealHandlerAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "reveal_in_finder" => Some(Self::RevealInFinder),
            _ => None,
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::RevealInFinder => "Opened in Finder",
        }
    }

    fn unsupported_message(self) -> gpui::SharedString {
        match self {
            Self::RevealInFinder => {
                gpui::SharedString::from("Cannot reveal this item type in Finder")
            }
        }
    }
}

impl ScriptListApp {
    fn deeplink_for_result(result: &scripts::SearchResult) -> String {
        result
            .launcher_command_id()
            .and_then(|command_id| crate::config::command_id_to_deeplink(&command_id).ok())
            .unwrap_or_else(|| {
                let deeplink_name = crate::actions::to_deeplink_name(result.name());
                format!("scriptkit://run/{}", deeplink_name)
            })
    }

    /// Resolve the target path for a file action.
    ///
    /// Priority: `file_search_actions_path` (consumed) > selected SearchResult.
    /// The `extractor` callback is used for SearchResult-based path extraction so
    /// callers can choose `extract_path_for_reveal` vs `extract_path_for_copy`.
    fn resolve_file_action_path<F>(
        &mut self,
        extractor: F,
    ) -> Result<std::path::PathBuf, Option<gpui::SharedString>>
    where
        F: FnOnce(
            Option<&scripts::SearchResult>,
        )
            -> Result<std::path::PathBuf, crate::action_helpers::PathExtractionError>,
    {
        // file_search_actions_path takes priority (consumed on use)
        if let Some(path) = self.file_search_actions_path.take() {
            return Ok(std::path::PathBuf::from(path));
        }
        // Fall back to main menu selected result via the shared extractor
        let selected = self.get_selected_result();
        extractor(selected.as_ref()).map_err(|e| Some(e.message()))
    }

    /// Extract (path, is_dir, name) from the actions-path or the selected file search result.
    fn resolve_file_search_path_info(&self) -> Option<(String, bool, String)> {
        if let Some(ref path) = self.file_search_actions_path {
            let p = std::path::Path::new(path);
            let is_dir = p.is_dir();
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            return Some((path.clone(), is_dir, name));
        }
        let AppView::FileSearchView { selected_index, .. } = &self.current_view else {
            return None;
        };
        let binding = self.file_search_selection_binding(*selected_index);
        let entry = binding.file.as_ref()?;
        let is_dir = matches!(entry.file_type, crate::file_search::FileType::Directory);
        Some((entry.path.clone(), is_dir, entry.name.clone()))
    }

    /// Build a `FileResult` from live filesystem metadata.
    fn build_file_result_from_metadata(path: &str) -> Option<crate::file_search::FileResult> {
        let meta = crate::file_search::get_file_metadata(path)?;
        tracing::info!(
            category = "FILE_SEARCH",
            event = "build_file_result_from_metadata",
            path = %meta.path,
            ?meta.file_type,
            modified = meta.modified,
            size = meta.size,
            "Built file-search result from live metadata"
        );
        Some(crate::file_search::FileResult {
            path: meta.path,
            name: meta.name,
            size: meta.size,
            modified: meta.modified,
            file_type: meta.file_type,
        })
    }

    /// Return the absolute directory path when the current view is a
    /// directory-browse (not a global search).
    fn current_file_search_directory_abs(&self) -> Option<String> {
        let AppView::FileSearchView { query, .. } = &self.current_view else {
            return None;
        };
        let parsed = crate::file_search::parse_directory_path(query)?;
        crate::file_search::expand_path(parsed.directory.trim_end_matches('/'))
            .map(|dir| crate::file_search::ensure_trailing_slash(&dir))
    }

    /// Absolute parent directory of `path`, with trailing slash.
    fn parent_directory_abs(path: &str) -> Option<String> {
        std::path::Path::new(path)
            .parent()
            .and_then(|parent| parent.to_str())
            .map(crate::file_search::ensure_trailing_slash)
    }

    /// After a mutation (trash, rename, move, etc.), patch the cached directory
    /// listing in place when possible and fall back to a full refresh for global
    /// search.
    fn refresh_file_search_after_mutation(
        &mut self,
        old_path: &str,
        preferred_path: Option<&str>,
        previous_display_index: usize,
        cx: &mut Context<Self>,
    ) {
        let AppView::FileSearchView { presentation, .. } = &self.current_view else {
            return;
        };
        let presentation_value = *presentation;

        let current_dir = self.current_file_search_directory_abs();
        let old_dir = Self::parent_directory_abs(old_path);
        let new_dir = preferred_path.and_then(Self::parent_directory_abs);

        // We can patch in place when we are browsing a concrete directory and
        // the mutation touches that directory (source or destination).
        let can_patch_in_place = current_dir.is_some()
            && (old_dir.as_ref() == current_dir.as_ref()
                || new_dir.as_ref() == current_dir.as_ref());

        if can_patch_in_place {
            // Remove the old entry from the cache.
            self.cached_file_results
                .retain(|entry| entry.path != old_path);

            // If the item was renamed/moved into the current directory, add it.
            if let Some(new_path) = preferred_path {
                if new_dir.as_ref() == current_dir.as_ref() {
                    if let Some(updated) = Self::build_file_result_from_metadata(new_path) {
                        self.cached_file_results.push(updated);
                    }
                }
            }

            self.sort_directory_results();
            self.recompute_file_search_display_indices();
        } else {
            // Global search or cross-directory — full refresh.
            let AppView::FileSearchView { query, .. } = &self.current_view else {
                return;
            };
            let query_value = query.clone();
            let results = Self::resolve_file_search_results(&query_value);
            self.update_file_search_results(results);
        }

        let next_index = preferred_path
            .and_then(|path| self.file_search_display_index_for_path(path))
            .or_else(|| {
                let len = self.file_search_display_len();
                (len > 0).then_some(previous_display_index.min(len.saturating_sub(1)))
            });

        if let AppView::FileSearchView {
            ref mut selected_index,
            ..
        } = self.current_view
        {
            *selected_index = next_index.unwrap_or(0);
        }

        Self::resize_file_search_window_for_presentation(
            presentation_value,
            self.file_search_display_indices.len(),
        );
        if let Some(index) = next_index {
            self.file_search_scroll_handle
                .scroll_to_item(index, gpui::ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Clear the pending file-search action target so the next verb acts on
    /// the current selection, not a stale path from a cancelled/failed action.
    fn clear_file_search_action_target(&mut self) {
        self.file_search_actions_path = None;
    }

    /// Restore keyboard focus to the file-search input after an async
    /// file verb (rename, move, trash, copy-name) completes.
    ///
    /// Routes through the focus coordinator so popup-close and post-verb
    /// restore follow the same path, then syncs to legacy fields.
    fn restore_file_search_input_focus(&mut self, cx: &mut Context<Self>) {
        if matches!(self.current_view, AppView::FileSearchView { .. }) {
            self.focus_coordinator
                .request(crate::focus_coordinator::FocusRequest::main_filter());
            self.sync_coordinator_to_legacy();
            cx.notify();
        }
    }

    /// After an insertion (duplicate), patch the cached directory listing
    /// in place when possible and fall back to a full refresh for global search.
    fn refresh_file_search_after_insert(
        &mut self,
        preferred_path: &str,
        previous_display_index: usize,
        cx: &mut Context<Self>,
    ) {
        let AppView::FileSearchView { presentation, .. } = &self.current_view else {
            return;
        };
        let presentation_value = *presentation;

        let current_dir = self.current_file_search_directory_abs();
        let new_dir = Self::parent_directory_abs(preferred_path);

        if current_dir.is_some() && new_dir.as_ref() == current_dir.as_ref() {
            if let Some(new_entry) = Self::build_file_result_from_metadata(preferred_path) {
                self.cached_file_results.push(new_entry);
                self.sort_directory_results();
                self.recompute_file_search_display_indices();
            }
        } else {
            let AppView::FileSearchView { query, .. } = &self.current_view else {
                return;
            };
            let query_value = query.clone();
            let results = Self::resolve_file_search_results(&query_value);
            self.update_file_search_results(results);
        }

        let next_index = self
            .file_search_display_index_for_path(preferred_path)
            .or_else(|| {
                let len = self.file_search_display_len();
                (len > 0).then_some(previous_display_index.min(len.saturating_sub(1)))
            });

        if let AppView::FileSearchView {
            ref mut selected_index,
            ..
        } = self.current_view
        {
            *selected_index = next_index.unwrap_or(0);
        }

        Self::resize_file_search_window_for_presentation(
            presentation_value,
            self.file_search_display_indices.len(),
        );
        if let Some(index) = next_index {
            self.file_search_scroll_handle
                .scroll_to_item(index, gpui::ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Handle file-related actions. Returns `true` if handled.
    fn handle_file_action(
        &mut self,
        action_id: &str,
        dctx: &DispatchContext,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let trace_id = &dctx.trace_id;
        let action_id = action_id.strip_prefix("file:").unwrap_or(action_id);
        match action_id {
            "reveal_in_finder" => {
                let Some(reveal_action) = FileSearchRevealHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "reveal in Finder action");
                let path_result =
                    self.resolve_file_action_path(crate::action_helpers::extract_path_for_reveal);

                if let Ok(path) = path_result {
                    let reveal_result_rx =
                        self.reveal_in_finder_with_feedback_async(&path, trace_id);
                    let trace_id = trace_id.to_string();
                    let start = std::time::Instant::now();
                    cx.spawn(async move |this, cx| {
                        let Ok(reveal_result) = reveal_result_rx.recv().await else {
                            return;
                        };

                        let _ = this.update(cx, |this, cx| match reveal_result {
                            Ok(()) => {
                                tracing::info!(
                                    trace_id = %trace_id,
                                    status = "completed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    "Async action completed: reveal_in_finder"
                                );
                                this.show_hud(
                                    reveal_action.success_hud().to_string(),
                                    Some(HUD_SHORT_MS),
                                    cx,
                                );
                                this.hide_main_and_reset(cx);
                            }
                            Err(message) => {
                                tracing::error!(
                                    trace_id = %trace_id,
                                    status = "failed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    error = %message,
                                    "Async action failed: reveal_in_finder"
                                );
                                this.show_error_toast_with_code(
                                    message,
                                    Some(crate::action_helpers::ERROR_REVEAL_FAILED),
                                    cx,
                                );
                            }
                        });
                    })
                    .detach();
                } else {
                    let msg = path_result
                        .err()
                        .flatten()
                        .unwrap_or_else(|| reveal_action.unsupported_message());
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        msg.to_string(),
                    );
                }
                DispatchOutcome::success()
            }
            "copy_path" => {
                let Some(copy_path_action) =
                    FileSearchPathCopyHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "copy path action");
                let path_result =
                    self.resolve_file_action_path(crate::action_helpers::extract_path_for_copy);

                match path_result {
                    Ok(path) => {
                        let path_str = path.to_string_lossy().to_string();
                        tracing::info!(category = "UI", path = %path_str, "copying path to clipboard");
                        self.copy_to_clipboard_with_feedback(
                            &path_str,
                            copy_path_action.copied_hud(&path_str),
                            true,
                            cx,
                        );
                    }
                    Err(msg) => {
                        let error_msg = msg.map(|m| m.to_string()).unwrap_or_else(|| {
                            selection_required_message_for_action(action_id).to_string()
                        });
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            error_msg,
                        );
                    }
                }
                DispatchOutcome::success()
            }
            "open_in_quick_terminal" => {
                tracing::info!(category = "UI", "open in Quick Terminal action");
                let path_result = self.resolve_file_action_path(
                    crate::action_helpers::extract_path_for_quick_terminal,
                );

                match path_result {
                    Ok(path) => match crate::action_helpers::resolve_quick_terminal_cwd(&path) {
                        Ok(cwd) => {
                            self.open_quick_terminal(Some(cwd), cx);
                            DispatchOutcome::success()
                        }
                        Err(message) => DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            message,
                        ),
                    },
                    Err(msg) => {
                        let error_msg = msg.map(|m| m.to_string()).unwrap_or_else(|| {
                            selection_required_message_for_action(action_id).to_string()
                        });
                        DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            error_msg,
                        )
                    }
                }
            }
            "copy_deeplink" => {
                let Some(deeplink_action) =
                    FileSearchDeeplinkCopyHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "copy deeplink action");
                if let Some(result) = self.get_selected_result() {
                    if crate::script_sharing::is_shareable_result(&result) {
                        match crate::script_sharing::bundle_from_search_result(&result).and_then(
                            |bundle| {
                                crate::script_sharing::encode_share_bundle(&bundle)
                                    .map(|uri| (bundle, uri))
                            },
                        ) {
                            Ok((bundle, share_uri)) => {
                                crate::script_sharing::mark_recently_exported_share(&share_uri);
                                tracing::info!(
                                    category = "UI",
                                    share_uri = %share_uri,
                                    title = %bundle.title,
                                    "copying share uri to clipboard"
                                );
                                self.copy_to_clipboard_with_feedback(
                                    &share_uri,
                                    deeplink_action.share_hud(&bundle.title),
                                    true,
                                    cx,
                                );
                            }
                            Err(error) => {
                                return DispatchOutcome::error(
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    deeplink_action.share_failure_message(error),
                                );
                            }
                        }
                    } else {
                        let deeplink_url = Self::deeplink_for_result(&result);

                        tracing::info!(category = "UI", deeplink = %deeplink_url, "copying deeplink to clipboard");
                        self.copy_to_clipboard_with_feedback(
                            &deeplink_url,
                            deeplink_action.deeplink_hud(&deeplink_url),
                            true,
                            cx,
                        );
                    }
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        selection_required_message_for_action(action_id),
                    );
                }
                DispatchOutcome::success()
            }
            "__cancel__" => {
                tracing::info!(category = "UI", "actions dialog cancelled");
                self.clear_file_search_action_target();
                DispatchOutcome::success()
            }
            // File search specific actions
            "open_file" | "open_directory" | "quick_look" | "open_with" | "show_info"
            | "attach_to_ai" => {
                let Some(file_action) = FileSearchHandlerAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                if let Some(path) = self.file_search_actions_path.clone() {
                    tracing::info!(category = "UI", action = action_id, path = %path, "file action");

                    let result: Result<(), String> = match file_action {
                        FileSearchHandlerAction::Open => crate::file_search::open_file(&path),
                        FileSearchHandlerAction::QuickLook => crate::file_search::quick_look(&path),
                        FileSearchHandlerAction::OpenWith => crate::file_search::open_with(&path),
                        FileSearchHandlerAction::ShowInfo => crate::file_search::show_info(&path),
                        FileSearchHandlerAction::AttachToAi => {
                            self.open_agent_chat_after_main_hide(
                                action_id,
                                &dctx.trace_id,
                                DeferredAgentChatAction::AddAttachment { path: path.clone() },
                                cx,
                            );

                            Ok(())
                        }
                    };

                    match result {
                        Ok(()) => {
                            if let Some(message) = file_action.success_hud(action_id) {
                                self.show_hud(message.to_string(), Some(HUD_SHORT_MS), cx);
                            }
                            self.clear_file_search_action_target();
                            if file_action.hides_main_after_success() {
                                self.hide_main_and_reset(cx);
                            }
                        }
                        Err(e) => {
                            tracing::error!(action = action_id, path = %path, error = %e, "file search action failed");
                            let prefix = file_action.error_prefix(action_id);
                            self.clear_file_search_action_target();
                            return DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                format!("{}: {}", prefix, e),
                            );
                        }
                    }
                }
                DispatchOutcome::success()
            }
            "open_in_editor" => {
                let Some(editor_action) = FileSearchEditorHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((path, _, _)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        editor_action.selection_required_message(),
                    );
                };
                let path_buf = std::path::PathBuf::from(&path);
                match crate::script_creation::open_in_editor(&path_buf, &self.config) {
                    Ok(()) => {
                        self.clear_file_search_action_target();
                        self.show_hud(
                            editor_action.success_hud().to_string(),
                            Some(HUD_SHORT_MS),
                            cx,
                        );
                        self.hide_main_and_reset(cx);
                    }
                    Err(e) => {
                        self.clear_file_search_action_target();
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            editor_action.failure_message(e),
                        );
                    }
                }
                DispatchOutcome::success()
            }
            "rename_path" => {
                let Some(rename_action) = FileSearchRenameHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((path, _, _name)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        rename_action.selection_required_message(),
                    );
                };
                let previous_display_index = match &self.current_view {
                    AppView::FileSearchView { selected_index, .. } => *selected_index,
                    _ => 0,
                };

                cx.spawn(async move |this, cx| {
                    let new_name = match crate::file_search::prompt_rename_target_name(&path) {
                        Ok(Some(value)) => value,
                        Ok(None) => {
                            let _ = this.update(cx, |this, cx| {
                                this.clear_file_search_action_target();
                                this.restore_file_search_input_focus(cx);
                            });
                            return;
                        }
                        Err(e) => {
                            let _ = this.update(cx, |this, cx| {
                                this.clear_file_search_action_target();
                                this.show_error_toast(rename_action.failure_message(e), cx);
                                this.restore_file_search_input_focus(cx);
                            });
                            return;
                        }
                    };

                    let _ = this.update(cx, |this, cx| {
                        match crate::file_search::rename_path(&path, &new_name) {
                            Ok(new_path) => {
                                this.clear_file_search_action_target();
                                this.show_hud(
                                    rename_action.success_hud(&new_name),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                                this.refresh_file_search_after_mutation(
                                    &path,
                                    Some(&new_path),
                                    previous_display_index,
                                    cx,
                                );
                                this.restore_file_search_input_focus(cx);
                            }
                            Err(e) => {
                                this.file_search_actions_path = None;
                                this.show_error_toast(rename_action.failure_message(e), cx);
                                this.restore_file_search_input_focus(cx);
                            }
                        }
                    });
                })
                .detach();

                DispatchOutcome::success()
            }
            "move_path" => {
                let Some(move_action) = FileSearchMoveHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((path, is_dir, _name)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        move_action.selection_required_message(),
                    );
                };
                let previous_display_index = match &self.current_view {
                    AppView::FileSearchView { selected_index, .. } => *selected_index,
                    _ => 0,
                };

                cx.spawn(async move |this, cx| {
                    let destination_dir =
                        match crate::file_search::prompt_move_destination_dir(&path, is_dir) {
                            Ok(Some(value)) => value,
                            Ok(None) => {
                                let _ = this.update(cx, |this, cx| {
                                    this.clear_file_search_action_target();
                                    this.restore_file_search_input_focus(cx);
                                });
                                return;
                            }
                            Err(e) => {
                                let _ = this.update(cx, |this, cx| {
                                    this.clear_file_search_action_target();
                                    this.show_error_toast(move_action.failure_message(e), cx);
                                    this.restore_file_search_input_focus(cx);
                                });
                                return;
                            }
                        };

                    let _ = this.update(cx, |this, cx| {
                        match crate::file_search::move_path(&path, &destination_dir) {
                            Ok(new_path) => {
                                this.clear_file_search_action_target();
                                this.show_hud(
                                    move_action.success_hud(&destination_dir),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                                this.refresh_file_search_after_mutation(
                                    &path,
                                    Some(&new_path),
                                    previous_display_index,
                                    cx,
                                );
                                this.restore_file_search_input_focus(cx);
                            }
                            Err(e) => {
                                this.clear_file_search_action_target();
                                this.show_error_toast(move_action.failure_message(e), cx);
                                this.restore_file_search_input_focus(cx);
                            }
                        }
                    });
                })
                .detach();

                DispatchOutcome::success()
            }
            "move_to_trash" => {
                let Some(trash_action) = FileSearchTrashHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((path, _, name)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        trash_action.selection_required_message(),
                    );
                };
                let previous_display_index = match &self.current_view {
                    AppView::FileSearchView { selected_index, .. } => *selected_index,
                    _ => 0,
                };
                let trace_id = trace_id.to_string();
                let start = std::time::Instant::now();

                cx.spawn(async move |this, cx| {
                    let confirm_options = crate::confirm::ParentConfirmOptions::destructive(
                        trash_action.confirm_title(),
                        trash_action.confirm_message(&name),
                        trash_action.confirm_button(),
                    );

                    match crate::confirm::confirm_with_parent_dialog(cx, confirm_options, &trace_id)
                        .await
                    {
                        Ok(true) => {}
                        Ok(false) => {
                            tracing::info!(
                                trace_id = %trace_id,
                                status = "cancelled",
                                duration_ms = start.elapsed().as_millis() as u64,
                                "Async action cancelled: move_to_trash"
                            );
                            let _ = this.update(cx, |this, cx| {
                                this.clear_file_search_action_target();
                                this.restore_file_search_input_focus(cx);
                            });
                            return;
                        }
                        Err(e) => {
                            let _ = this.update(cx, |this, cx| {
                                tracing::error!(
                                    trace_id = %trace_id,
                                    status = "failed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    error = %e,
                                    "failed to open confirmation modal"
                                );
                                this.clear_file_search_action_target();
                                this.show_error_toast_with_code(
                                    trash_action.confirmation_failure_message(),
                                    Some(crate::action_helpers::ERROR_MODAL_FAILED),
                                    cx,
                                );
                                this.restore_file_search_input_focus(cx);
                            });
                            return;
                        }
                    }

                    let _ = this.update(cx, |this, cx| {
                        match crate::file_search::move_to_trash(&path) {
                            Ok(()) => {
                                tracing::info!(
                                    trace_id = %trace_id,
                                    status = "completed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    path = %path,
                                    "file moved to trash"
                                );
                                this.clear_file_search_action_target();
                                this.show_hud(
                                    trash_action.success_hud(&name),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                                this.refresh_file_search_after_mutation(
                                    &path,
                                    None,
                                    previous_display_index,
                                    cx,
                                );
                                this.restore_file_search_input_focus(cx);
                            }
                            Err(e) => {
                                tracing::error!(
                                    trace_id = %trace_id,
                                    status = "failed",
                                    duration_ms = start.elapsed().as_millis() as u64,
                                    error = %e,
                                    path = %path,
                                    "failed to move to trash"
                                );
                                this.clear_file_search_action_target();
                                this.show_error_toast(
                                    trash_action.failure_message(e),
                                    cx,
                                );
                                this.restore_file_search_input_focus(cx);
                            }
                        }
                    });
                })
                .detach();

                DispatchOutcome::success()
            }
            "duplicate_path" => {
                let Some(duplicate_action) =
                    FileSearchDuplicateHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((path, _, name)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        duplicate_action.selection_required_message(),
                    );
                };
                let previous_display_index = match &self.current_view {
                    AppView::FileSearchView { selected_index, .. } => *selected_index,
                    _ => 0,
                };
                match crate::file_search::duplicate_path(&path) {
                    Ok(new_path) => {
                        self.clear_file_search_action_target();
                        self.show_hud(
                            duplicate_action.success_hud(&name),
                            Some(HUD_MEDIUM_MS),
                            cx,
                        );
                        self.refresh_file_search_after_insert(
                            &new_path,
                            previous_display_index,
                            cx,
                        );
                        self.restore_file_search_input_focus(cx);
                    }
                    Err(e) => {
                        self.clear_file_search_action_target();
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            duplicate_action.failure_message(e),
                        );
                    }
                }
                DispatchOutcome::success()
            }
            "copy_filename" => {
                let Some(copy_filename_action) =
                    FileSearchFilenameCopyHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some((_path, _is_dir, name)) = self.resolve_file_search_path_info() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        copy_filename_action.selection_required_message(),
                    );
                };
                tracing::info!(category = "UI", filename = %name, "copy filename");
                self.clear_file_search_action_target();
                self.copy_to_clipboard_with_feedback(
                    &name,
                    copy_filename_action.copied_hud(&name),
                    true,
                    cx,
                );
                self.restore_file_search_input_focus(cx);
                DispatchOutcome::success()
            }
            // ── Current-directory actions ────────────────────────────────
            "sort_name_asc" | "sort_name_desc" | "sort_modified_desc" | "sort_modified_asc" => {
                let Some(sort_action) = FileSearchSortHandlerAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let preferred_selected_path = self.current_file_search_selected_path();
                let mode = sort_action.mode();
                tracing::info!(
                    category = "FILE_SEARCH",
                    event = "sort_action_selected",
                    action = action_id,
                    ?mode,
                    selected_path = preferred_selected_path.as_deref().unwrap_or(""),
                    cached_count = self.cached_file_results.len(),
                    "Applying file-search sort action"
                );
                self.file_search_sort_mode = mode;
                self.apply_file_search_sort_mode();
                self.recompute_file_search_display_indices();
                self.restore_file_search_selection_after_results_change(
                    preferred_selected_path.as_deref(),
                );
                // Scroll the preserved selection back into view after resort.
                if let AppView::FileSearchView { selected_index, .. } = &self.current_view {
                    self.file_search_scroll_handle
                        .scroll_to_item(*selected_index, gpui::ScrollStrategy::Nearest);
                }
                self.show_hud(
                    sort_action.success_hud().to_string(),
                    Some(HUD_SHORT_MS),
                    cx,
                );
                self.restore_file_search_input_focus(cx);
                cx.notify();
                DispatchOutcome::success()
            }
            "refresh_directory"
            | "reveal_current_directory"
            | "copy_current_directory_path"
            | "open_current_directory_in_quick_terminal" => {
                let Some(directory_action) =
                    FileSearchCurrentDirectoryAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                let Some(dir) = self.current_file_search_directory_abs() else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        directory_action.missing_directory_message(),
                    );
                };

                match directory_action {
                    FileSearchCurrentDirectoryAction::Refresh => {
                        let (query, presentation) = if let AppView::FileSearchView {
                            query,
                            presentation,
                            ..
                        } = &self.current_view
                        {
                            (query.clone(), *presentation)
                        } else {
                            (format!("{dir}/"), FileSearchPresentation::Mini)
                        };
                        let frozen_filter = crate::file_search::parse_directory_path(&query)
                            .map(|parsed| parsed.filter)
                            .unwrap_or(None);
                        self.restart_file_search_stream_for_query(
                            query,
                            presentation,
                            Some(frozen_filter),
                            true,
                            cx,
                        );
                        if let Some(message) = directory_action.success_hud(&dir) {
                            self.show_hud(message, Some(HUD_SHORT_MS), cx);
                        }
                        self.restore_file_search_input_focus(cx);
                        DispatchOutcome::success()
                    }
                    FileSearchCurrentDirectoryAction::Reveal => {
                        match crate::file_search::reveal_in_finder(&dir) {
                            Ok(()) => {
                                if let Some(message) = directory_action.success_hud(&dir) {
                                    self.show_hud(message, Some(HUD_SHORT_MS), cx);
                                }
                                DispatchOutcome::success()
                            }
                            Err(e) => DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                format!("{}: {e}", directory_action.error_prefix()),
                            ),
                        }
                    }
                    FileSearchCurrentDirectoryAction::CopyPath => {
                        let message = directory_action
                            .success_hud(&dir)
                            .unwrap_or_else(|| format!("Copied: {dir}"));
                        self.copy_to_clipboard_with_feedback(&dir, message, true, cx);
                        DispatchOutcome::success()
                    }
                    FileSearchCurrentDirectoryAction::OpenQuickTerminal => {
                        match crate::action_helpers::resolve_quick_terminal_cwd(
                            std::path::Path::new(&dir),
                        ) {
                            Ok(cwd) => {
                                self.open_quick_terminal(Some(cwd), cx);
                                DispatchOutcome::success()
                            }
                            Err(e) => DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                format!("{}: {e}", directory_action.error_prefix()),
                            ),
                        }
                    }
                }
            }
            _ => DispatchOutcome::not_handled(),
        }
    }

    /// Compare two file-search results according to the given sort mode.
    ///
    /// Name sorts group directories first. Modified-time sorts compare files
    /// and directories together so the newest or oldest item wins.
    /// This is the single source of truth for file-search ordering.
    fn compare_file_search_results_for_mode(
        mode: crate::actions::FileSearchSortMode,
        a: &crate::file_search::FileResult,
        b: &crate::file_search::FileResult,
    ) -> std::cmp::Ordering {
        match mode {
            crate::actions::FileSearchSortMode::NameAsc => {
                let a_is_dir = matches!(a.file_type, crate::file_search::FileType::Directory);
                let b_is_dir = matches!(b.file_type, crate::file_search::FileType::Directory);
                match (a_is_dir, b_is_dir) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
            crate::actions::FileSearchSortMode::NameDesc => {
                let a_is_dir = matches!(a.file_type, crate::file_search::FileType::Directory);
                let b_is_dir = matches!(b.file_type, crate::file_search::FileType::Directory);
                match (a_is_dir, b_is_dir) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
                b.name.to_lowercase().cmp(&a.name.to_lowercase())
            }
            crate::actions::FileSearchSortMode::ModifiedDesc => b
                .modified
                .cmp(&a.modified)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            crate::actions::FileSearchSortMode::ModifiedAsc => a
                .modified
                .cmp(&b.modified)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        }
    }

    /// Apply the current sort mode to cached file results.
    fn apply_file_search_sort_mode(&mut self) {
        let mode = self.file_search_sort_mode;
        self.cached_file_results
            .sort_by(|a, b| Self::compare_file_search_results_for_mode(mode, a, b));

        let first_rows: Vec<String> = self
            .cached_file_results
            .iter()
            .take(5)
            .map(|entry| entry.name.clone())
            .collect();

        tracing::info!(
            category = "FILE_SEARCH",
            event = "apply_file_search_sort_mode",
            ?mode,
            cached_count = self.cached_file_results.len(),
            first_rows = ?first_rows,
            "Applied file-search sort mode to cached results"
        );
    }
}

#[cfg(test)]
mod files_action_tests {
    use super::*;

    #[test]
    fn deeplink_for_config_backed_rows_uses_command_namespace() {
        let builtin = scripts::SearchResult::BuiltIn(scripts::BuiltInMatch {
            entry: crate::builtins::BuiltInEntry {
                id: "builtin/clipboard-history".to_string(),
                name: "Clipboard History".to_string(),
                description: "Browse clipboard history".to_string(),
                keywords: vec![],
                feature: crate::builtins::BuiltInFeature::ClipboardHistory,
                icon: None,
                group: crate::builtins::BuiltInGroup::Core,
            },
            score: 1,
            match_evidence: None,
        });
        let app = scripts::SearchResult::App(scripts::AppMatch {
            app: crate::app_launcher::AppInfo {
                name: "Safari".to_string(),
                path: std::path::PathBuf::from("/Applications/Safari.app"),
                bundle_id: Some("com.apple.Safari".to_string()),
                icon: None,
            },
            score: 1,
            match_evidence: None,
        });

        assert_eq!(
            ScriptListApp::deeplink_for_result(&builtin),
            "scriptkit://commands/builtin/clipboard-history"
        );
        assert_eq!(
            ScriptListApp::deeplink_for_result(&app),
            "scriptkit://commands/app/com.apple.Safari"
        );
    }
}
