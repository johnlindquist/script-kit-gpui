#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalOpenUtilityAction {
    SdkCommandTerminal,
    QuickTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebcamOpenUtilityAction {
    OpenWebcamPrompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileSearchSelectionProjection {
    pub(crate) requested_display_index: usize,
    pub(crate) display_index: usize,
    pub(crate) result_index: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct FileSearchSelectionBinding {
    pub(crate) projection: Option<FileSearchSelectionProjection>,
    pub(crate) file: Option<crate::file_search::FileResult>,
}

impl TerminalOpenUtilityAction {
    fn creation_failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SdkCommandTerminal => format!("Failed to create terminal: {error}"),
            Self::QuickTerminal => format!("Failed to create quick terminal: {error}"),
        }
    }

    fn open_failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SdkCommandTerminal | Self::QuickTerminal => {
                format!("Failed to open terminal: {error}")
            }
        }
    }
}

impl WebcamOpenUtilityAction {
    fn start_failure_log(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::OpenWebcamPrompt => format!("Failed to start webcam: {error}"),
        }
    }
}

fn quick_terminal_cwd_sync_hook() -> String {
    format!(
        "__skgpui_cwd_sync() {{ printf '\\033]0;{}%s\\007' \"$PWD\"; }}\r\
if [ -n \"$ZSH_VERSION\" ]; then autoload -Uz add-zsh-hook 2>/dev/null; add-zsh-hook precmd __skgpui_cwd_sync 2>/dev/null; elif [ -n \"$BASH_VERSION\" ]; then PROMPT_COMMAND=\"__skgpui_cwd_sync${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\"; fi\r\
__skgpui_cwd_sync\r",
        crate::term_prompt::SCRIPT_KIT_CWD_TITLE_PREFIX
    )
}

fn recent_file_result_from_existing_path(path: &str) -> Option<crate::file_search::FileResult> {
    crate::file_search::file_result_from_existing_path(path)
        .filter(|file| file.file_type != crate::file_search::FileType::Directory)
}

impl ScriptListApp {
    pub(crate) fn sync_spine_cwd_from_quick_terminal(
        &mut self,
        cwd: std::path::PathBuf,
        cx: &mut Context<Self>,
    ) {
        if !cwd.is_dir() || self.spine_cwd.as_ref() == Some(&cwd) {
            return;
        }

        let label = crate::file_search::shorten_path(&cwd.to_string_lossy());
        self.spine_cwd = Some(cwd.clone());
        self.spine_cwd_label = Some(label.clone());
        self.spine_cwd_revision = self.spine_cwd_revision.wrapping_add(1);
        self.persist_spine_cwd();
        self.prewarm_agent_chat_for_spine_cwd(cx);
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "quick_terminal_cwd_synced_to_agent_chat",
            cwd = %cwd.display(),
            label = %label,
            revision = self.spine_cwd_revision,
        );
        cx.notify();
    }

    pub(crate) fn recent_file_results_from_frecency(
        &self,
        limit: usize,
    ) -> Vec<crate::file_search::FileResult> {
        let mut seen = std::collections::HashSet::new();
        let mut hydrated: Vec<_> = self
            .frecency_store
            .top_file_paths(limit.saturating_mul(3).max(limit))
            .into_iter()
            .filter_map(|(path, score)| {
                if !seen.insert(path.clone()) {
                    return None;
                }
                recent_file_result_from_existing_path(&path).map(|file| (file, score))
            })
            .collect();

        hydrated.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .partial_cmp(a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.modified.cmp(&a.modified))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path.cmp(&b.path))
        });

        hydrated
            .into_iter()
            .take(limit)
            .map(|(file, _)| file)
            .collect()
    }

    pub(crate) fn recent_directory_results_from_frecency(
        &self,
        limit: usize,
    ) -> Vec<crate::file_search::FileResult> {
        let mut seen = std::collections::HashSet::new();
        let mut hydrated: Vec<_> = self
            .frecency_store
            .top_directory_paths(limit.saturating_mul(3).max(limit))
            .into_iter()
            .filter_map(|(path, score)| {
                if !seen.insert(path.clone()) {
                    return None;
                }
                let meta = crate::file_search::get_file_metadata(&path)?;
                if meta.file_type != crate::file_search::FileType::Directory {
                    return None;
                }
                let file = crate::file_search::FileResult {
                    path: meta.path,
                    name: meta.name,
                    size: meta.size,
                    modified: meta.modified,
                    file_type: meta.file_type,
                };
                Some((file, score))
            })
            .collect();

        hydrated.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .partial_cmp(a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.modified.cmp(&a.modified))
                .then_with(|| a.name.cmp(&b.name))
                .then_with(|| a.path.cmp(&b.path))
        });

        hydrated
            .into_iter()
            .take(limit)
            .map(|(file, _)| file)
            .collect()
    }

    fn resolve_file_search_results_with(
        query: &str,
        parse_directory_path: impl Fn(&str) -> Option<crate::file_search::ParsedDirPath>,
        is_directory_path: impl Fn(&str) -> bool,
        expand_path: impl Fn(&str) -> Option<String>,
        list_directory: impl Fn(&str, usize, bool) -> Vec<crate::file_search::FileResult>,
        search_files: impl Fn(&str, Option<&str>, usize) -> Vec<crate::file_search::FileResult>,
    ) -> Vec<crate::file_search::FileResult> {
        // Try structured parse first — handles ~/dev/fin → list ~/dev/
        if let Some(parsed) = parse_directory_path(query) {
            tracing::info!(
                message = %format!(
                    "Detected browseable path, listing parent directory: {}",
                    parsed.directory
                ),
            );
            return list_directory(
                &parsed.directory,
                crate::file_search::DEFAULT_CACHE_LIMIT,
                parsed.show_hidden,
            );
        }

        if is_directory_path(query) {
            tracing::info!(message = %format!("Detected directory path, listing: {}", query),
            );

            let expanded = expand_path(query);
            let is_real_dir = expanded
                .as_deref()
                .map(|path| std::path::Path::new(path).is_dir())
                .unwrap_or(false);

            let directory_results =
                list_directory(query, crate::file_search::DEFAULT_CACHE_LIMIT, false);
            if directory_results.is_empty() && !is_real_dir {
                tracing::info!(message = %"Path mode not a real directory; falling back to Spotlight search",
                );
                return search_files(query, None, crate::file_search::DEFAULT_SEARCH_LIMIT);
            }

            return directory_results;
        }

        search_files(query, None, crate::file_search::DEFAULT_SEARCH_LIMIT)
    }

    pub(crate) fn resolve_file_search_results(query: &str) -> Vec<crate::file_search::FileResult> {
        Self::resolve_file_search_results_with(
            query,
            crate::file_search::parse_directory_path,
            crate::file_search::is_directory_path,
            crate::file_search::expand_path,
            crate::file_search::list_directory_with_options,
            crate::file_search::search_files,
        )
    }

    pub(crate) fn update_file_search_results(
        &mut self,
        results: Vec<crate::file_search::FileResult>,
    ) {
        let previous_cached_count = self.cached_file_results.len();
        self.cached_file_results = results;

        let parsed_directory = match &self.current_view {
            AppView::FileSearchView { query, .. } => {
                crate::file_search::parse_directory_path(query)
            }
            _ => None,
        };

        if let Some(parsed) = &parsed_directory {
            self.file_search_current_dir = Some(parsed.directory.clone());
            self.file_search_current_dir_show_hidden = parsed.show_hidden;
        } else {
            self.file_search_current_dir = None;
            self.file_search_current_dir_show_hidden = false;
        }

        let directory_sort_applied = parsed_directory.is_some();

        if directory_sort_applied {
            self.sort_directory_results();
        }

        self.file_search_display_indices.clear();
        self.recompute_file_search_display_indices();

        let first_display_rows: Vec<String> = self
            .file_search_display_indices
            .iter()
            .take(5)
            .filter_map(|&result_index| {
                self.cached_file_results
                    .get(result_index)
                    .map(|entry| entry.name.clone())
            })
            .collect();

        tracing::debug!(
            category = "FILE_SEARCH",
            event = "update_file_search_results",
            previous_cached_count,
            cached_count = self.cached_file_results.len(),
            display_count = self.file_search_display_indices.len(),
            directory_sort_applied,
            ?self.file_search_sort_mode,
            first_display_rows = ?first_display_rows,
            "Updated file-search results"
        );
    }

    /// Open a terminal with a specific command (for fallback "Run in Terminal")
    pub fn open_terminal_with_command(&mut self, command: String, cx: &mut Context<Self>) {
        let terminal_action = TerminalOpenUtilityAction::SdkCommandTerminal;
        // Log length only — command lines routinely carry credentials as args
        // (--password, tokens) and must not be persisted to the on-disk log.
        tracing::info!(command_len = command.len(), "Opening terminal with command");

        // Create submit callback that just closes on exit/escape
        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(move |_id: String, _value: Option<String>| {
                // Terminal exited - nothing special to do
            });

        // Get the target height for terminal view (subtract footer height)
        let term_height =
            window_resize::layout::MAX_HEIGHT - px(window_resize::layout::FOOTER_HEIGHT);

        // Create terminal with the specified command
        match term_prompt::TermPrompt::with_height(
            "fallback-terminal".to_string(),
            Some(command), // Run the specified command
            self.focus_handle.clone(),
            submit_callback,
            std::sync::Arc::clone(&self.theme),
            std::sync::Arc::new(self.config.clone()),
            Some(term_height),
        ) {
            Ok(term_prompt) => {
                let entity = cx.new(|_| term_prompt);
                self.current_view = AppView::QuickTerminalView { entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::TermPrompt);
                // DEFERRED RESIZE: Avoid RefCell borrow error by deferring window resize
                // to after the current GPUI update cycle completes. Synchronous Cocoa
                // setFrame: calls during render can trigger events that re-borrow GPUI state.
                cx.spawn(async move |_this, _cx| {
                    resize_to_view_sync(ViewType::TermPrompt, 0);
                })
                .detach();
                cx.notify();
            }
            Err(e) => {
                let failure_message = terminal_action.open_failure_message(&e);
                tracing::error!(message = %terminal_action.creation_failure_log(&e));
                self.toast_manager.push(
                    components::toast::Toast::error(failure_message, &self.theme)
                        .duration_ms(Some(TOAST_ERROR_MS)),
                );
                cx.notify();
            }
        }
    }

    // =========================================================================
    // File Search Implementation
    // =========================================================================
    //
    // BLOCKED: Requires the following changes to main.rs (not in worker reservations):
    //
    // 1. Add to AppView enum:
    //    ```rust
    //    /// Showing file search results (Spotlight/mdfind based)
    //    FileSearchView {
    //        query: String,
    //        selected_index: usize,
    //    },
    //    ```
    //
    // 2. Add to ScriptListApp struct:
    //    ```rust
    //    /// Cached file search results
    //    cached_file_results: Vec<file_search::FileResult>,
    //    /// Scroll handle for file search list
    //    file_search_scroll_handle: UniformListScrollHandle,
    //    ```
    //
    // 3. Add initialization in app_impl.rs ScriptListApp::new():
    //    ```rust
    //    cached_file_results: Vec::new(),
    //    file_search_scroll_handle: UniformListScrollHandle::new(),
    //    ```
    //
    // 4. Add render call in main.rs Render impl match arm:
    //    ```rust
    //    AppView::FileSearchView { query, selected_index } => {
    //        self.render_file_search(query.clone(), *selected_index, cx)
    //    }
    //    ```
    //
    // 5. Wire up in app_impl.rs execute_fallback():
    //    ```rust
    //    FallbackResult::SearchFiles { query } => {
    //        self.open_file_search(query, cx);
    //    }
    //    ```
    //
    // Once those are added, uncomment the method below.
    // =========================================================================

    /// Open file search with the given query
    ///
    /// This performs an mdfind-based file search and displays results in a Raycast-like UI.
    ///
    /// # Arguments
    /// * `query` - The search query (passed from the "Search Files" fallback action)
    ///
    /// # Usage
    /// Called when user selects "Search Files" fallback with a search term.
    /// Features:
    /// - Live search as user types (debounced)
    /// - File type icons (folder, document, image, audio, video, code, etc.)
    /// - File size and modified date display
    /// - Enter: Open the selected item in the default application
    /// - Cmd+Enter: Reveal in Finder
    pub fn open_file_search(&mut self, query: String, cx: &mut Context<Self>) {
        self.open_file_search_view(query, FileSearchPresentation::Full, cx);
    }

    /// Return the rendered display index for the first visible row matching `path`.
    pub(crate) fn file_search_display_index_for_path(&self, path: &str) -> Option<usize> {
        self.file_search_display_indices
            .iter()
            .position(|&result_index| {
                self.cached_file_results
                    .get(result_index)
                    .map(|entry| entry.path == path)
                    .unwrap_or(false)
            })
    }

    /// Projection from the user-facing File Search display row into the raw
    /// cached result row. This is the selection identity used by preview,
    /// actions, and automation state.
    pub(crate) fn resolve_file_search_selection_projection(
        display_indices: &[usize],
        result_count: usize,
        selected_index: usize,
    ) -> Option<FileSearchSelectionProjection> {
        if display_indices.is_empty() {
            return None;
        }
        let display_index = selected_index.min(display_indices.len().saturating_sub(1));
        let result_index = *display_indices.get(display_index)?;
        if result_index >= result_count {
            return None;
        }
        Some(FileSearchSelectionProjection {
            requested_display_index: selected_index,
            display_index,
            result_index,
        })
    }

    pub(crate) fn file_search_selection_binding(
        &self,
        selected_index: usize,
    ) -> FileSearchSelectionBinding {
        let projection = Self::resolve_file_search_selection_projection(
            &self.file_search_display_indices,
            self.cached_file_results.len(),
            selected_index,
        );
        let file = projection
            .and_then(|projection| self.cached_file_results.get(projection.result_index))
            .cloned();
        FileSearchSelectionBinding { projection, file }
    }

    /// Look up a file search result by its position in the rendered display list
    /// (after filtering, scoring, and directory-first sorting).
    pub(crate) fn file_search_result_at_display_index(
        &self,
        display_index: usize,
    ) -> Option<&crate::file_search::FileResult> {
        self.file_search_display_indices
            .get(display_index)
            .and_then(|&result_index| self.cached_file_results.get(result_index))
    }

    /// Clamp a display index to the currently rendered file search list.
    pub(crate) fn clamp_file_search_display_index(&self, selected_index: usize) -> Option<usize> {
        if self.file_search_display_indices.is_empty() {
            None
        } else {
            Some(selected_index.min(self.file_search_display_indices.len() - 1))
        }
    }

    /// Return the currently selected file-search entry in display order.
    pub(crate) fn selected_file_search_result(
        &self,
        selected_index: usize,
    ) -> Option<(usize, &crate::file_search::FileResult)> {
        let projection = self.file_search_selection_binding(selected_index).projection?;
        let entry = self.cached_file_results.get(projection.result_index)?;
        Some((projection.display_index, entry))
    }

    /// Number of rows in the precomputed file-search display list.
    pub(crate) fn file_search_display_len(&self) -> usize {
        self.file_search_display_indices.len()
    }

    /// Clamp the file-search selection to the visible row count, updating
    /// `selected_index` inside `AppView::FileSearchView` in place.
    /// Returns the clamped index, or `None` when the display list is empty.
    pub(crate) fn clamp_current_file_search_selection(&mut self) -> Option<usize> {
        let len = self.file_search_display_len();
        let AppView::FileSearchView { selected_index, .. } = &mut self.current_view else {
            return None;
        };
        if len == 0 {
            *selected_index = 0;
            None
        } else {
            let clamped = (*selected_index).min(len - 1);
            *selected_index = clamped;
            Some(clamped)
        }
    }

    /// Return the currently selected file-search entry (owned clone) after
    /// clamping the selection to the visible display list.
    pub(crate) fn selected_file_search_result_owned(
        &mut self,
    ) -> Option<(usize, crate::file_search::FileResult)> {
        let display_index = self.clamp_current_file_search_selection()?;
        let entry = self
            .file_search_result_at_display_index(display_index)?
            .clone();
        Some((display_index, entry))
    }

    /// Browse into the currently selected file-search directory.
    ///
    /// Returns false when the current selection is empty or is not a directory.
    pub(crate) fn navigate_file_search_into_selected_directory(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, file)) = self.selected_file_search_result_owned() else {
            return false;
        };

        if file.file_type != crate::file_search::FileType::Directory {
            return false;
        }

        let next_presentation = match &self.current_view {
            AppView::FileSearchView { presentation, .. } => *presentation,
            _ => FileSearchPresentation::Full,
        };
        let next_query = format!(
            "{}/",
            crate::file_search::shorten_path(&file.path).trim_end_matches('/')
        );

        tracing::info!(
            category = "FILE_SEARCH",
            event = "file_search_tab_enter_directory",
            path = %file.path,
            query = %next_query,
            ?next_presentation,
            "Tab navigated into selected file-search directory"
        );
        self.open_file_search_view_preserving_current_results(next_query, next_presentation, cx);
        true
    }

    /// Cancel any in-flight file-search work and advance the generation
    /// counter. Returns the new generation value.
    pub(crate) fn begin_file_search_session(&mut self) -> u64 {
        if let Some(cancel) = self.file_search_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.file_search_debounce_task = None;
        self.file_search_loading = false;
        self.file_search_frozen_filter = None;
        self.file_search_gen = self.file_search_gen.wrapping_add(1);
        self.file_search_gen
    }

    /// Row labels for file search in the exact order shown to the user.
    pub(crate) fn file_search_display_row_names(&self) -> Vec<String> {
        self.file_search_display_indices
            .iter()
            .filter_map(|&result_index| self.cached_file_results.get(result_index))
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// Sort cached directory results using the active file-search sort mode.
    ///
    /// This wrapper keeps directory-refresh callers aligned with
    /// `compare_file_search_results_for_mode`.
    pub fn sort_directory_results(&mut self) {
        tracing::info!(
            category = "FILE_SEARCH",
            event = "sort_directory_results_delegate",
            ?self.file_search_sort_mode,
            cached_count = self.cached_file_results.len(),
            "Delegating directory result sorting to apply_file_search_sort_mode"
        );
        self.apply_file_search_sort_mode();
    }

    /// Recompute file_search_display_indices based on current filter pattern
    ///
    /// This is called when:
    /// 1. Results change (new directory listing or search results)
    /// 2. Filter pattern changes (user types in existing directory)
    /// 3. Loading completes (final sort/rank)
    ///
    /// By computing this OUTSIDE of render, we ensure that animation tickers
    /// calling cx.notify() at 60fps don't re-run expensive Nucleo scoring.
    /// Re-sort display indices for a directory-browse view using the active
    /// `file_search_sort_mode`.  This ensures filtered directory views respect
    /// the user-selected sort instead of drifting to Nucleo match order.
    fn sort_file_search_display_indices_for_directory(&self, indices: &mut [usize]) {
        let mode = self.file_search_sort_mode;
        indices.sort_by(|a_idx, b_idx| {
            let a = self.cached_file_results.get(*a_idx);
            let b = self.cached_file_results.get(*b_idx);
            match (a, b) {
                (Some(a), Some(b)) => Self::compare_file_search_results_for_mode(mode, a, b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a_idx.cmp(b_idx),
            }
        });
        tracing::debug!(
            category = "FILE_SEARCH",
            event = "sort_directory_display_indices",
            ?mode,
            display_count = indices.len(),
            "Applied active sort mode to directory display indices"
        );
    }

    pub fn recompute_file_search_display_indices(&mut self) {
        let (filter_pattern, is_directory_query) =
            if let AppView::FileSearchView { ref query, .. } = self.current_view {
                let parsed = crate::file_search::parse_directory_path(query);
                let filter_pattern = if let Some(ref frozen) = self.file_search_frozen_filter {
                    frozen.clone()
                } else if let Some(ref parsed) = parsed {
                    parsed.filter.clone()
                } else if !query.is_empty() {
                    Some(query.clone())
                } else {
                    None
                };
                (filter_pattern, parsed.is_some())
            } else {
                (None, false)
            };

        let cached_count = self.cached_file_results.len();

        self.file_search_display_indices = if let Some(ref pattern) = filter_pattern {
            let mut indices: Vec<usize> = crate::file_search::filter_results_nucleo_simple(
                &self.cached_file_results,
                pattern,
            )
            .into_iter()
            .map(|(idx, _)| idx)
            .collect();
            if is_directory_query {
                self.sort_file_search_display_indices_for_directory(&mut indices);
            }
            tracing::debug!(
                event = "recompute_display_indices",
                pattern = %pattern,
                cached_count,
                display_count = indices.len(),
                is_directory_query,
                ?self.file_search_sort_mode
            );
            indices
        } else {
            let mut indices: Vec<usize> = (0..self.cached_file_results.len()).collect();
            if is_directory_query {
                self.sort_file_search_display_indices_for_directory(&mut indices);
            }
            tracing::debug!(
                event = "recompute_display_indices",
                mode = "no_filter",
                cached_count,
                display_count = indices.len(),
                is_directory_query,
                ?self.file_search_sort_mode
            );
            indices
        };
    }

    fn shell_quote_path_for_cd(path: &std::path::Path) -> String {
        let value = path.to_string_lossy();
        if value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '=' | '+'))
        {
            return value.to_string();
        }
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    /// Open the quick terminal, optionally changing to `cwd` before user input.
    pub(crate) fn open_quick_terminal(
        &mut self,
        cwd: Option<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        self.open_quick_terminal_with_startup_input(cwd, None, cx);
    }

    pub(crate) fn open_quick_terminal_with_command(
        &mut self,
        cwd: Option<std::path::PathBuf>,
        command: String,
        cx: &mut Context<Self>,
    ) {
        self.open_quick_terminal_with_startup_input(cwd, Some(command), cx);
    }

    fn open_quick_terminal_with_startup_input(
        &mut self,
        cwd: Option<std::path::PathBuf>,
        startup_command: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let terminal_action = TerminalOpenUtilityAction::QuickTerminal;
        tracing::info!(
            message = %"Opening Quick Terminal",
            cwd = cwd.as_ref().map(|path| path.display().to_string()).as_deref()
        );

        // Create submit callback that just closes on exit/escape
        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(move |_id: String, _value: Option<String>| {
                // Terminal exited - nothing special to do
            });

        // Get the target height for terminal view (clamped to mini main window
        // height so opening Quick Terminal does not grow the launcher panel).
        let term_height = window_resize::quick_terminal_content_height();

        let warm_terminal = self.take_quick_terminal_warm_pty(cx);
        let term_prompt_result = if let Some(handle) = warm_terminal {
            term_prompt::TermPrompt::with_existing_terminal(
                "quick-terminal".to_string(),
                handle,
                self.focus_handle.clone(),
                submit_callback,
                std::sync::Arc::clone(&self.theme),
                std::sync::Arc::new(self.config.clone()),
                Some(term_height),
            )
        } else {
            // Create terminal without a specific command (opens default shell)
            term_prompt::TermPrompt::with_height(
                "quick-terminal".to_string(),
                None,
                self.focus_handle.clone(),
                submit_callback,
                std::sync::Arc::clone(&self.theme),
                std::sync::Arc::new(self.config.clone()),
                Some(term_height),
            )
        };

        match term_prompt_result {
            Ok(term_prompt) => {
                let entity = cx.new(|_| term_prompt);
                cx.observe(&entity, |this, term, cx| {
                    if !matches!(this.current_view, AppView::QuickTerminalView { .. }) {
                        return;
                    }
                    let Some(cwd) = term.read(cx).synced_cwd_path() else {
                        return;
                    };
                    this.sync_spine_cwd_from_quick_terminal(cwd, cx);
                })
                .detach();

                {
                    let mut initial_input = quick_terminal_cwd_sync_hook();
                    if let Some(cwd) = cwd.as_ref() {
                        initial_input.push_str(&format!(
                            "cd {}\r",
                            Self::shell_quote_path_for_cd(cwd)
                        ));
                    }
                    if let Some(command) = startup_command.as_ref() {
                        initial_input.push_str(command.trim_end_matches(&['\r', '\n'][..]));
                        initial_input.push('\r');
                    }
                    entity.update(cx, |term, _cx| {
                        if !term.terminal.is_running() {
                            tracing::warn!(
                                event = "quick_terminal_startup_input_pty_dead",
                                cwd = cwd.as_ref().map(|path| path.display().to_string()).as_deref(),
                                has_command = startup_command.is_some(),
                                "Quick Terminal opened with startup input but PTY is not running"
                            );
                            return;
                        }
                        match term.terminal.input(initial_input.as_bytes()) {
                            Ok(()) => tracing::info!(
                                event = "quick_terminal_startup_input_sent",
                                cwd = cwd.as_ref().map(|path| path.display().to_string()).as_deref(),
                                has_command = startup_command.is_some(),
                                "Quick Terminal startup input written to PTY"
                            ),
                            Err(error) => tracing::warn!(
                                event = "quick_terminal_startup_input_write_failed",
                                cwd = cwd.as_ref().map(|path| path.display().to_string()).as_deref(),
                                has_command = startup_command.is_some(),
                                error = %error,
                                "Failed to write Quick Terminal startup input"
                            ),
                        }
                    });
                }
                self.current_view = AppView::QuickTerminalView { entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::TermPrompt);
                self.warm_quick_terminal_pty(cx);
                // Intentionally NOT calling resize_to_view_sync(TermPrompt) here — Quick
                // Terminal must keep the main window at its mini height instead of
                // expanding to layout::MAX_HEIGHT. SDK-spawned terminals
                // (open_terminal_with_command) still resize.
                cx.notify();
            }
            Err(e) => {
                let failure_message = terminal_action.open_failure_message(&e);
                tracing::error!(message = %terminal_action.creation_failure_log(&e));
                self.toast_manager.push(
                    components::toast::Toast::error(failure_message, &self.theme)
                        .duration_ms(Some(TOAST_ERROR_MS)),
                );
                cx.notify();
            }
        }
    }

    /// Open Claude Code in the quick terminal view
    fn open_claude_code_terminal(&mut self, cx: &mut Context<Self>) {
        tracing::info!(message = %"Opening Claude Code Terminal");

        let command = r#"zsh -lc 'mkdir -p "$HOME/.scriptkit/sessions" && cd "$HOME/.scriptkit/sessions" && exec claude'"#.to_string();
        self.open_terminal_with_command(command, cx);
    }

    /// Open the webcam prompt
    #[cfg(target_os = "macos")]
    fn open_webcam(&mut self, cx: &mut Context<Self>) {
        tracing::info!(message = %"Opening Webcam prompt");

        let focus_handle = self.focus_handle.clone();
        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(|_id: String, value: Option<String>| {
                if let Some(data) = value {
                    tracing::info!(message = %&format!("Webcam capture data: {} bytes", data.len()),
                    );
                }
            });

        let webcam_prompt = prompts::WebcamPrompt::new(
            "webcam".to_string(),
            focus_handle,
            submit_callback,
            std::sync::Arc::clone(&self.theme),
        );

        let entity = cx.new(|_| webcam_prompt);

        // Zero-copy camera capture via AVFoundation.
        // Camera frames arrive as CVPixelBuffer on a dispatch queue,
        // then we poll and pass them to gpui::surface() — no CPU conversion.
        let entity_weak = entity.downgrade();
        let webcam_action = WebcamOpenUtilityAction::OpenWebcamPrompt;

        let (frame_rx, capture_handle) = match crate::camera::start_capture(640) {
            Ok(pair) => pair,
            Err(err) => {
                tracing::error!(message = %webcam_action.start_failure_log(&err));
                // Still show the prompt with an error
                let entity_weak2 = entity.downgrade();
                let err_msg = match err {
                    crate::camera::WebcamStartError::PermissionDenied { .. } => {
                        "Camera access is denied. Enable Script Kit in System Settings → Privacy & Security → Camera".to_string()
                    }
                    _ => err.to_string(),
                };
                cx.spawn(async move |_this, cx| {
                    cx.update(|cx| {
                        if let Some(entity) = entity_weak2.upgrade() {
                            entity.update(cx, |prompt, cx| {
                                prompt.set_error(err_msg, cx);
                            });
                        }
                    });
                })
                .detach();

                self.current_view = AppView::WebcamView { entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::AppRoot);
                resize_to_view_sync(ViewType::DivPrompt, 0);
                cx.notify();
                return;
            }
        };

        // Store the capture handle in the prompt — when the prompt entity is
        // dropped, the handle drops too, stopping the camera and releasing resources.
        entity.update(cx, |prompt, _cx| {
            prompt.capture_handle = Some(capture_handle);
        });

        // Async poller: drain CVPixelBuffers from channel, push latest to prompt.
        // Exits when the channel disconnects (CaptureHandle dropped) or the entity is gone.
        cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;

                // Drain to latest frame, detect channel disconnect
                let mut latest = None;
                loop {
                    match frame_rx.try_recv() {
                        Ok(buf) => latest = Some(buf),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    }
                }

                let Some(buf) = latest else {
                    continue;
                };

                let result = cx.update(|cx| {
                    if let Some(entity) = entity_weak.upgrade() {
                        entity.update(cx, |prompt, cx| {
                            prompt.set_pixel_buffer(buf, cx);
                        });
                        true
                    } else {
                        false
                    }
                });

                if result {
                    continue;
                }
                break;
            }
        })
        .detach();

        self.current_view = AppView::WebcamView { entity };
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::AppRoot);

        resize_to_view_sync(ViewType::DivPrompt, 0);
        cx.notify();
    }

    /// Open the webcam prompt
    #[cfg(not(target_os = "macos"))]
    fn open_webcam(&mut self, cx: &mut Context<Self>) {
        tracing::info!(message = %"Opening Webcam prompt (unsupported platform)");

        let focus_handle = self.focus_handle.clone();
        let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
            std::sync::Arc::new(|_id: String, _value: Option<String>| {});

        let webcam_prompt = prompts::WebcamPrompt::new(
            "webcam".to_string(),
            focus_handle,
            submit_callback,
            std::sync::Arc::clone(&self.theme),
        );

        let entity = cx.new(|_| webcam_prompt);
        entity.update(cx, |prompt, cx| {
            prompt.set_error("Webcam capture is only supported on macOS".to_string(), cx);
        });

        self.show_error_toast("Webcam capture is only supported on macOS", cx);

        self.current_view = AppView::WebcamView { entity };
        self.focused_input = FocusedInput::None;
        self.pending_focus = Some(FocusTarget::AppRoot);
        resize_to_view_sync(ViewType::DivPrompt, 0);
        cx.notify();
    }
}

#[cfg(test)]
mod utility_views_file_search_tests {
    use super::*;
    use crate::file_search::{FileResult, FileType};

    fn test_file_result(name: &str) -> FileResult {
        FileResult {
            path: format!("/tmp/{}", name),
            name: name.to_string(),
            size: 0,
            modified: 0,
            file_type: FileType::File,
        }
    }

    #[test]
    fn recent_files_hydration_skips_existing_directories() {
        let directory = std::env::temp_dir().join(format!(
            "script-kit-recent-files-dir-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create temp directory");

        assert!(recent_file_result_from_existing_path(&directory.to_string_lossy()).is_none());

        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn test_resolve_file_search_results_with_falls_back_when_directory_path_is_not_real_directory()
    {
        let results = ScriptListApp::resolve_file_search_results_with(
            "~/missing-dir",
            |_| None, // parse_directory_path returns None
            |_| true,
            |_| Some("/definitely/not/a/real/dir".to_string()),
            |_, _, _| Vec::new(),
            |query, onlyin, limit| {
                assert_eq!(query, "~/missing-dir");
                assert!(onlyin.is_none());
                assert_eq!(limit, crate::file_search::DEFAULT_SEARCH_LIMIT);
                vec![test_file_result("fallback-result")]
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fallback-result");
    }

    #[test]
    fn test_resolve_file_search_results_with_uses_directory_listing_when_results_exist() {
        let results = ScriptListApp::resolve_file_search_results_with(
            "~/dir",
            |_| None, // parse_directory_path returns None
            |_| true,
            |_| Some("/definitely/not/a/real/dir".to_string()),
            |query, limit, show_hidden| {
                assert_eq!(query, "~/dir");
                assert_eq!(limit, crate::file_search::DEFAULT_CACHE_LIMIT);
                assert!(!show_hidden);
                vec![test_file_result("directory-result")]
            },
            |_, _, _| {
                panic!("search_files should not be called when directory listing returns results")
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "directory-result");
    }

    #[test]
    fn test_resolve_file_search_results_with_uses_search_for_non_directory_queries() {
        let results = ScriptListApp::resolve_file_search_results_with(
            "invoice",
            |_| None, // parse_directory_path returns None
            |_| false,
            |_| None,
            |_, _, _| panic!("list_directory should not be called for non-directory query"),
            |query, onlyin, limit| {
                assert_eq!(query, "invoice");
                assert!(onlyin.is_none());
                assert_eq!(limit, crate::file_search::DEFAULT_SEARCH_LIMIT);
                vec![test_file_result("search-result")]
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "search-result");
    }

    #[test]
    fn test_resolve_file_search_results_with_uses_parsed_directory_when_available() {
        use crate::file_search::ParsedDirPath;
        let results = ScriptListApp::resolve_file_search_results_with(
            "~/dev/fin",
            |_| {
                Some(ParsedDirPath {
                    directory: "~/dev/".to_string(),
                    filter: Some("fin".to_string()),
                    show_hidden: false,
                })
            },
            |_| true,
            |_| None,
            |query, limit, show_hidden| {
                assert_eq!(query, "~/dev/");
                assert_eq!(limit, crate::file_search::DEFAULT_CACHE_LIMIT);
                assert!(!show_hidden);
                vec![test_file_result("parsed-dir-result")]
            },
            |_, _, _| {
                panic!("search_files should not be called when parse_directory_path succeeds")
            },
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "parsed-dir-result");
    }

    #[test]
    fn file_search_selection_projection_maps_display_row_to_result_row() {
        let projection =
            ScriptListApp::resolve_file_search_selection_projection(&[2, 0], 3, 0).unwrap();

        assert_eq!(projection.requested_display_index, 0);
        assert_eq!(projection.display_index, 0);
        assert_eq!(projection.result_index, 2);
    }

    #[test]
    fn file_search_selection_projection_clamps_stale_selected_index() {
        let projection =
            ScriptListApp::resolve_file_search_selection_projection(&[4, 1], 5, 99).unwrap();

        assert_eq!(projection.requested_display_index, 99);
        assert_eq!(projection.display_index, 1);
        assert_eq!(projection.result_index, 1);
    }

    #[test]
    fn file_search_selection_projection_rejects_empty_or_invalid_projection() {
        assert!(ScriptListApp::resolve_file_search_selection_projection(&[], 3, 0).is_none());
        assert!(ScriptListApp::resolve_file_search_selection_projection(&[9], 3, 0).is_none());
    }
}
