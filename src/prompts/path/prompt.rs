use super::*;

impl PathPrompt {
    pub(crate) fn dictation_input_revision(&self, _cx: &gpui::App) -> u64 {
        self.input_revision
    }

    fn advance_input_revision(&mut self) {
        assert!(
            self.input_revision < u64::MAX,
            "path input revision exhausted"
        );
        self.input_revision += 1;
    }

    pub(crate) fn set_selected_index(&mut self, index: usize) {
        if self.selected_index != index {
            self.advance_input_revision();
            self.selected_index = index;
        }
    }

    pub(crate) fn set_header_context(
        &mut self,
        context: crate::prompts::base::PromptHeaderContext,
    ) {
        self.header_context = Some(context);
    }

    pub fn new(
        id: String,
        start_path: Option<String>,
        hint: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> Self {
        let (current_path, start_path_kind, preselect_path) =
            Self::resolve_initial_path(start_path.as_deref());

        logging::log(
            "PROMPTS",
            &format!(
                "PathPrompt::new starting at path ending: {}",
                Self::display_path_tail(&current_path)
            ),
        );

        let path_prefix = Self::format_path_prefix(&current_path);

        // Load entries from current path
        let load_result = Self::load_entries(&current_path);
        let entries = load_result.entries;
        let filtered_entries = entries.clone();
        let render_rows = Arc::new(Self::build_render_rows(&filtered_entries));

        let mut prompt = PathPrompt {
            id,
            start_path,
            start_path_kind,
            hint,
            current_path,
            path_prefix,
            filter_text: String::new(),
            input_revision: 0,
            selected_index: 0,
            entries,
            load_status: load_result.status,
            filtered_entries,
            focus_handle,
            on_submit,
            theme,
            design_variant: DesignVariant::Default,
            list_scroll_handle: UniformListScrollHandle::new(),
            on_show_actions: None,
            on_close_actions: None,
            actions_showing: Arc::new(Mutex::new(false)),
            actions_search_text: Arc::new(Mutex::new(String::new())),
            cursor_visible: true,
            render_rows,
            header_context: None,
        };
        prompt.select_path_if_present(preselect_path.as_deref());
        prompt.rebuild_render_rows();
        prompt
    }

    /// Use the real filesystem browser, confined by the process-owned root.
    pub fn from_owned_directory(
        id: String,
        directory: std::path::PathBuf,
        hint: Option<String>,
        focus_handle: FocusHandle,
        on_submit: SubmitCallback,
        theme: Arc<theme::Theme>,
    ) -> anyhow::Result<Self> {
        let policy = crate::runtime_policy::owned_evaluation()
            .ok_or_else(|| anyhow::anyhow!("owned_path_policy_missing"))?;
        policy.require_owned_path(&directory)?;
        anyhow::ensure!(directory.is_dir(), "owned_path_not_directory");
        Ok(Self::new(
            id,
            Some(directory.to_string_lossy().into_owned()),
            hint,
            focus_handle,
            on_submit,
            theme,
        ))
    }

    fn check_path(path: &Path) -> Result<(), crate::runtime_policy::EffectRefusal> {
        if let Some(policy) = crate::runtime_policy::owned_evaluation() {
            policy.require_owned_path(path)?;
        }
        Ok(())
    }

    fn resolve_initial_path(start_path: Option<&str>) -> (String, String, Option<String>) {
        if crate::runtime_policy::is_owned_evaluation()
            && start_path.is_none_or(|path| Self::check_path(Path::new(path)).is_err())
        {
            return (
                start_path.unwrap_or_default().to_string(),
                "refused".to_string(),
                None,
            );
        }
        let Some(raw) = start_path else {
            if let Some(home) = dirs::home_dir() {
                return (
                    home.to_string_lossy().to_string(),
                    "defaultHome".to_string(),
                    None,
                );
            }
            return ("/".to_string(), "defaultRoot".to_string(), None);
        };

        let path = Path::new(raw);
        let symlink_meta = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let kind = match error.kind() {
                    std::io::ErrorKind::NotFound => "missing",
                    std::io::ErrorKind::PermissionDenied => "permissionDenied",
                    _ => "readError",
                };
                return (raw.to_string(), kind.to_string(), None);
            }
        };
        let is_symlink = symlink_meta.file_type().is_symlink();
        let target_meta = if is_symlink {
            std::fs::metadata(path).ok()
        } else {
            Some(symlink_meta)
        };
        let Some(target_meta) = target_meta else {
            return (raw.to_string(), "brokenSymlink".to_string(), None);
        };
        if target_meta.is_dir() {
            return (
                raw.to_string(),
                if is_symlink {
                    "symlinkDirectory"
                } else {
                    "directory"
                }
                .to_string(),
                None,
            );
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            return (
                parent.to_string_lossy().to_string(),
                if is_symlink { "symlinkFile" } else { "file" }.to_string(),
                Some(raw.to_string()),
            );
        }
        (raw.to_string(), "file".to_string(), None)
    }

    /// Build lightweight render-row data from filtered entries.
    fn build_render_rows(filtered: &[PathEntry]) -> Vec<PathEntryRenderRow> {
        filtered
            .iter()
            .map(|e| PathEntryRenderRow {
                name: gpui::SharedString::from(e.name.clone()),
                is_dir: e.is_dir,
                is_symlink: e.is_symlink,
            })
            .collect()
    }

    /// Rebuild the cached render rows from current filtered_entries.
    fn rebuild_render_rows(&mut self) {
        self.render_rows = Arc::new(Self::build_render_rows(&self.filtered_entries));
    }

    fn select_path_if_present(&mut self, target_path: Option<&str>) {
        let Some(target_path) = target_path else {
            return;
        };
        if let Some(index) = self
            .filtered_entries
            .iter()
            .position(|entry| entry.path == target_path)
        {
            self.set_selected_index(index);
        }
    }

    fn format_path_prefix(path: &str) -> String {
        format!("{}/", path.trim_end_matches('/'))
    }

    fn display_path_tail(path: &str) -> String {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(path)
            .to_string()
    }

    /// Set the callback for showing actions dialog
    pub fn with_show_actions(mut self, callback: ShowActionsCallback) -> Self {
        self.on_show_actions = Some(callback);
        self
    }

    /// Set the show actions callback (mutable version)
    pub fn set_show_actions(&mut self, callback: ShowActionsCallback) {
        self.on_show_actions = Some(callback);
    }

    /// Set the close actions callback (for toggle behavior)
    pub fn with_close_actions(mut self, callback: CloseActionsCallback) -> Self {
        self.on_close_actions = Some(callback);
        self
    }

    /// Set the shared actions_showing state (for toggle behavior)
    pub fn with_actions_showing(mut self, actions_showing: Arc<Mutex<bool>>) -> Self {
        self.actions_showing = actions_showing;
        self
    }

    /// Set the shared actions_search_text state (for header display)
    pub fn with_actions_search_text(mut self, actions_search_text: Arc<Mutex<String>>) -> Self {
        self.actions_search_text = actions_search_text;
        self
    }

    /// Load directory entries from a path with a stable status for receipts.
    pub(super) fn load_entries(dir_path: &str) -> PathLoadResult {
        let path = Path::new(dir_path);
        if let Err(error) = Self::check_path(path) {
            return PathLoadResult {
                entries: Vec::new(),
                status: PathLoadStatus::new(
                    PathLoadStatusKind::PermissionDenied,
                    error.to_string(),
                    0,
                    0,
                ),
            };
        }
        let metadata = match std::fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let kind = match error.kind() {
                    std::io::ErrorKind::NotFound => PathLoadStatusKind::Missing,
                    std::io::ErrorKind::PermissionDenied => PathLoadStatusKind::PermissionDenied,
                    _ => PathLoadStatusKind::ReadError,
                };
                return PathLoadResult {
                    entries: Vec::new(),
                    status: PathLoadStatus::new(kind, Self::load_status_message(kind, 0), 0, 0),
                };
            }
        };

        if !metadata.is_dir() {
            return PathLoadResult {
                entries: Vec::new(),
                status: PathLoadStatus::new(
                    PathLoadStatusKind::NotDirectory,
                    Self::load_status_message(PathLoadStatusKind::NotDirectory, 0),
                    0,
                    0,
                ),
            };
        }

        // No ".." entry - use left arrow to navigate to parent

        // Read directory entries
        let read_dir = match std::fs::read_dir(path) {
            Ok(read_dir) => read_dir,
            Err(error) => {
                let kind = match error.kind() {
                    std::io::ErrorKind::PermissionDenied => PathLoadStatusKind::PermissionDenied,
                    std::io::ErrorKind::NotFound => PathLoadStatusKind::Missing,
                    _ => PathLoadStatusKind::ReadError,
                };
                return PathLoadResult {
                    entries: Vec::new(),
                    status: PathLoadStatus::new(kind, Self::load_status_message(kind, 0), 0, 0),
                };
            }
        };

        let mut dirs: Vec<PathEntry> = Vec::new();
        let mut files: Vec<PathEntry> = Vec::new();
        let mut hidden_count = 0;
        let mut failed_entry_count = 0;

        for entry_result in read_dir {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(_) => {
                    failed_entry_count += 1;
                    continue;
                }
            };
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Hidden dotfiles are intentionally skipped by PathPrompt.
            if name.starts_with('.') {
                hidden_count += 1;
                continue;
            }

            let is_symlink = entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false);
            if Self::check_path(&entry_path).is_err() {
                failed_entry_count += 1;
                continue;
            }
            let is_dir = entry_path.is_dir();
            let path_entry = PathEntry {
                name,
                path: entry_path.to_string_lossy().to_string(),
                is_dir,
                is_symlink,
            };

            if is_dir {
                dirs.push(path_entry);
            } else {
                files.push(path_entry);
            }
        }

        // Sort alphabetically (case insensitive)
        dirs.sort_by_cached_key(|entry| entry.name.to_lowercase());
        files.sort_by_cached_key(|entry| entry.name.to_lowercase());

        // Add dirs first, then files
        let mut entries = Vec::with_capacity(dirs.len() + files.len());
        entries.extend(dirs);
        entries.extend(files);

        let kind = if entries.is_empty() {
            PathLoadStatusKind::Empty
        } else {
            PathLoadStatusKind::Ready
        };

        logging::log(
            "PROMPTS",
            &format!(
                "PathPrompt loaded {} entries from path ending: {}",
                entries.len(),
                Self::display_path_tail(dir_path)
            ),
        );
        PathLoadResult {
            entries,
            status: PathLoadStatus::new(
                kind,
                Self::load_status_message(kind, hidden_count),
                hidden_count,
                failed_entry_count,
            ),
        }
    }

    fn load_status_message(kind: PathLoadStatusKind, hidden_count: usize) -> String {
        match kind {
            PathLoadStatusKind::Ready => {
                if hidden_count > 0 {
                    "Hidden dotfiles are not shown.".to_string()
                } else {
                    "Ready.".to_string()
                }
            }
            PathLoadStatusKind::Empty => {
                if hidden_count > 0 {
                    "No visible files or folders.".to_string()
                } else {
                    "This folder is empty.".to_string()
                }
            }
            PathLoadStatusKind::FilteredEmpty => "No matching files or folders.".to_string(),
            PathLoadStatusKind::Missing => "Path not found.".to_string(),
            PathLoadStatusKind::NotDirectory => "Path is not a folder.".to_string(),
            PathLoadStatusKind::PermissionDenied => "Permission denied.".to_string(),
            PathLoadStatusKind::ReadError => "Unable to read this folder.".to_string(),
        }
    }

    /// Update filtered entries based on filter text
    pub(super) fn update_filtered(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered_entries = self.entries.clone();
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.filtered_entries = self
                .entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect();
        }

        // Reset selection to 0 if out of bounds
        if self.selected_index >= self.filtered_entries.len() {
            self.set_selected_index(0);
        }

        self.rebuild_render_rows();
    }

    /// Set the current filter text programmatically
    pub fn set_input(&mut self, text: String, cx: &mut Context<Self>) {
        if self.filter_text == text {
            return;
        }

        self.filter_text = text;
        self.advance_input_revision();
        self.update_filtered();
        self.set_selected_index(0);
        self.list_scroll_handle
            .scroll_to_item(0, gpui::ScrollStrategy::Top);
        cx.notify();
    }

    /// Navigate into a directory
    pub fn navigate_to(&mut self, path: &str, cx: &mut Context<Self>) {
        if let Err(error) = Self::check_path(Path::new(path)) {
            self.load_status = PathLoadStatus::new(
                PathLoadStatusKind::PermissionDenied,
                error.to_string(),
                0,
                0,
            );
            cx.notify();
            return;
        }
        if self.current_path != path || !self.filter_text.is_empty() {
            self.advance_input_revision();
        }
        self.current_path = path.to_string();
        self.path_prefix = Self::format_path_prefix(path);
        let load_result = Self::load_entries(path);
        self.entries = load_result.entries;
        self.load_status = load_result.status;
        self.filter_text.clear();
        self.filtered_entries = self.entries.clone();
        self.set_selected_index(0);
        self.rebuild_render_rows();
        cx.notify();
    }

    pub fn visible_status_kind(&self) -> PathLoadStatusKind {
        if !self.filter_text.is_empty() && self.filtered_entries.is_empty() {
            PathLoadStatusKind::FilteredEmpty
        } else {
            self.load_status.kind
        }
    }

    pub fn visible_status_message(&self) -> String {
        if self.visible_status_kind() == PathLoadStatusKind::FilteredEmpty {
            Self::load_status_message(PathLoadStatusKind::FilteredEmpty, 0)
        } else {
            self.load_status.message.clone()
        }
    }

    fn load_status_for_automation(&self) -> &'static str {
        if self.load_status.is_error() {
            "error"
        } else if self.entries.is_empty() {
            "empty"
        } else {
            "loaded"
        }
    }

    fn load_error_kind_for_automation(&self) -> Option<&'static str> {
        match self.load_status.kind {
            PathLoadStatusKind::Missing => Some("missing"),
            PathLoadStatusKind::NotDirectory => Some("notDirectory"),
            PathLoadStatusKind::PermissionDenied => Some("permissionDenied"),
            PathLoadStatusKind::ReadError => Some("readError"),
            _ => None,
        }
    }

    fn empty_kind_for_automation(&self) -> Option<&'static str> {
        if !self.filtered_entries.is_empty() {
            return None;
        }
        if !self.filter_text.is_empty() {
            return Some("noMatches");
        }
        if let Some(error_kind) = self.load_error_kind_for_automation() {
            return Some(error_kind);
        }
        if self.load_status.hidden_count > 0 {
            return Some("hiddenOnly");
        }
        Some("emptyDirectory")
    }

    pub fn automation_state(&self) -> serde_json::Value {
        let selected = self.filtered_entries.get(self.selected_index).map(|entry| {
            serde_json::json!({
                "name": entry.name.clone(),
                "path": entry.path.clone(),
                "isDir": entry.is_dir,
                "isSymlink": entry.is_symlink,
            })
        });
        let selected_path = selected
            .as_ref()
            .and_then(|value| value.get("path"))
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let status_kind = self.visible_status_kind();
        serde_json::json!({
            "currentPath": self.current_path.clone(),
            "currentDirectory": self.current_path.clone(),
            "requestedStartPath": self.start_path.clone(),
            "startPathKind": self.start_path_kind.clone(),
            "filter": self.filter_text.clone(),
            "loadStatus": self.load_status_for_automation(),
            "loadErrorKind": self.load_error_kind_for_automation(),
            "emptyKind": self.empty_kind_for_automation(),
            "statusMessage": self.visible_status_message(),
            "entryCount": self.entries.len(),
            "visibleEntryCount": self.filtered_entries.len(),
            "selectedIndex": if self.filtered_entries.is_empty() { -1 } else { self.selected_index as i32 },
            "selectedPath": selected_path,
            "selected": selected,
            "hiddenPolicy": self.load_status.hidden_policy.clone(),
            "hiddenEntriesOmitted": self.load_status.hidden_count,
            "entryErrorsOmitted": self.load_status.failed_entry_count,
            "symlinkPolicy": "followDirectoryTargets",
            "status": {
                "kind": status_kind.as_str(),
                "message": self.visible_status_message(),
                "isError": self.load_status.is_error(),
                "hiddenPolicy": self.load_status.hidden_policy.clone(),
                "hiddenCount": self.load_status.hidden_count,
                "failedEntryCount": self.load_status.failed_entry_count,
            }
        })
    }

    /// Show actions dialog for the selected entry
    /// Emits PathPromptEvent::ShowActions for parent to handle
    pub(super) fn show_actions(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            let path_info = PathInfo::new(entry.name.clone(), entry.path.clone(), entry.is_dir);
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt emitting ShowActions for path ending: {} (is_dir={})",
                    Self::display_path_tail(&path_info.path),
                    path_info.is_dir
                ),
            );
            // Emit event for parent to handle (GPUI pattern)
            cx.emit(PathPromptEvent::ShowActions(path_info.clone()));
            // Also call legacy callback if present (backwards compatibility)
            if let Some(ref callback) = self.on_show_actions {
                (callback)(path_info);
            }
            cx.notify();
        }
    }

    /// Close actions dialog (for toggle behavior)
    /// Emits PathPromptEvent::CloseActions for parent to handle
    pub(super) fn close_actions(&mut self, cx: &mut Context<Self>) {
        logging::log("PROMPTS", "PathPrompt emitting CloseActions");
        // Emit event for parent to handle (GPUI pattern)
        cx.emit(PathPromptEvent::CloseActions);
        // Also call legacy callback if present (backwards compatibility)
        if let Some(ref callback) = self.on_close_actions {
            (callback)();
        }
        cx.notify();
    }

    /// Toggle actions dialog - show if hidden, close if showing
    pub fn toggle_actions(&mut self, cx: &mut Context<Self>) {
        let is_showing = match self.actions_showing.lock() {
            Ok(guard) => *guard,
            Err(poison) => {
                tracing::error!("path_prompt_actions_showing_mutex_poisoned_in_toggle");
                *poison.into_inner()
            }
        };

        if is_showing {
            logging::log(
                "PROMPTS",
                "PathPrompt toggle: closing actions (was showing)",
            );
            self.close_actions(cx);
        } else {
            logging::log("PROMPTS", "PathPrompt toggle: showing actions (was hidden)");
            self.show_actions(cx);
        }
    }

    /// Submit the selected path - always submits, never navigates
    /// For files and directories: submit the path (script will handle it)
    /// Navigation into directories is handled by → and Tab keys
    pub(super) fn submit_selected(&mut self, cx: &mut Context<Self>) {
        let path = self
            .filtered_entries
            .get(self.selected_index)
            .map(|entry| entry.path.as_str())
            .unwrap_or(self.filter_text.as_str());
        if let Err(error) = Self::check_path(Path::new(path)) {
            self.load_status = PathLoadStatus::new(
                PathLoadStatusKind::PermissionDenied,
                error.to_string(),
                0,
                0,
            );
            cx.notify();
            return;
        }
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            // Always submit the path, whether it's a file or directory
            // The calling script or default handler will decide what to do with it
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt submitting path ending: {} (is_dir={})",
                    Self::display_path_tail(&entry.path),
                    entry.is_dir
                ),
            );
            (self.on_submit)(self.id.clone(), Some(entry.path.clone()));
        } else if !self.filter_text.is_empty() {
            // If no entry selected but filter has text, submit the filter as a path
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt submitting filter text as path ending: {}",
                    Self::display_path_tail(&self.filter_text)
                ),
            );
            (self.on_submit)(self.id.clone(), Some(self.filter_text.clone()));
        }
    }

    /// Handle Enter key - always submit the selected path
    /// The calling code (main.rs) will open it with system default via std::process::Command
    pub fn handle_enter(&mut self, cx: &mut Context<Self>) {
        // Always submit directly - no actions dialog on Enter
        // Actions are available via Cmd+K
        self.submit_selected(cx);
    }

    /// Cancel - submit None
    pub fn submit_cancel(&mut self) {
        logging::log(
            "PROMPTS",
            &format!(
                "PathPrompt submit_cancel called - submitting None for id: {}",
                self.id
            ),
        );
        (self.on_submit)(self.id.clone(), None);
    }

    /// Move selection up
    pub fn move_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.set_selected_index(self.selected_index - 1);
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, gpui::ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// Move selection down
    pub fn move_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < self.filtered_entries.len().saturating_sub(1) {
            self.set_selected_index(self.selected_index + 1);
            self.list_scroll_handle
                .scroll_to_item(self.selected_index, gpui::ScrollStrategy::Top);
            cx.notify();
        }
    }

    /// Handle character input
    pub(super) fn handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.filter_text.push(ch);
        self.advance_input_revision();
        self.update_filtered();
        cx.notify();
    }

    /// Handle backspace - if filter empty, go up one directory
    pub(super) fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.advance_input_revision();
            self.update_filtered();
            cx.notify();
        } else {
            // If filter is empty, navigate up one directory
            let path = Path::new(&self.current_path);
            if let Some(parent) = path.parent() {
                let parent_path = parent.to_string_lossy().to_string();
                self.navigate_to(&parent_path, cx);
            }
        }
    }

    /// Navigate to parent directory (left arrow / shift+tab)
    pub fn navigate_to_parent(&mut self, cx: &mut Context<Self>) {
        let path = Path::new(&self.current_path);
        if let Some(parent) = path.parent() {
            let parent_path = parent.to_string_lossy().to_string();
            logging::log(
                "PROMPTS",
                &format!(
                    "PathPrompt navigating to parent path ending: {}",
                    Self::display_path_tail(&parent_path)
                ),
            );
            self.navigate_to(&parent_path, cx);
        }
        // If at root, do nothing
    }

    /// Navigate into selected directory (right arrow / tab)
    pub fn navigate_into_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(entry) = self.filtered_entries.get(self.selected_index) {
            if entry.is_dir {
                let path = entry.path.clone();
                logging::log(
                    "PROMPTS",
                    &format!(
                        "PathPrompt navigating into path ending: {}",
                        Self::display_path_tail(&path)
                    ),
                );
                self.navigate_to(&path, cx);
            }
            // If selected entry is a file, do nothing
        }
    }

    /// Get the currently selected path info (for actions dialog)
    pub fn get_selected_path_info(&self) -> Option<PathInfo> {
        self.filtered_entries
            .get(self.selected_index)
            .map(|entry| PathInfo::new(entry.name.clone(), entry.path.clone(), entry.is_dir))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_entries_reports_missing_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing");

        let result = PathPrompt::load_entries(&missing.to_string_lossy());

        assert!(result.entries.is_empty());
        assert_eq!(result.status.kind, PathLoadStatusKind::Missing);
        assert_eq!(result.status.message, "Path not found.");
    }

    #[test]
    fn load_entries_reports_non_directory_start() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hello").expect("write file");

        let result = PathPrompt::load_entries(&file.to_string_lossy());

        assert!(result.entries.is_empty());
        assert_eq!(result.status.kind, PathLoadStatusKind::NotDirectory);
        assert_eq!(result.status.message, "Path is not a folder.");
    }

    #[test]
    fn load_entries_reports_empty_directory_and_hidden_policy() {
        let dir = tempfile::tempdir().expect("tempdir");

        let empty = PathPrompt::load_entries(&dir.path().to_string_lossy());
        assert!(empty.entries.is_empty());
        assert_eq!(empty.status.kind, PathLoadStatusKind::Empty);
        assert_eq!(empty.status.message, "This folder is empty.");

        std::fs::write(dir.path().join(".secret"), "hidden").expect("write hidden");
        let hidden_only = PathPrompt::load_entries(&dir.path().to_string_lossy());
        assert!(hidden_only.entries.is_empty());
        assert_eq!(hidden_only.status.kind, PathLoadStatusKind::Empty);
        assert_eq!(hidden_only.status.hidden_policy, "omitDotfiles");
        assert_eq!(hidden_only.status.hidden_count, 1);
        assert_eq!(hidden_only.status.message, "No visible files or folders.");
    }

    #[cfg(unix)]
    #[test]
    fn load_entries_marks_symlink_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target_dir = dir.path().join("target");
        let link_dir = dir.path().join("linked-dir");
        std::fs::create_dir(&target_dir).expect("create target");
        std::os::unix::fs::symlink(&target_dir, &link_dir).expect("symlink");

        let result = PathPrompt::load_entries(&dir.path().to_string_lossy());
        let link = result
            .entries
            .iter()
            .find(|entry| entry.name == "linked-dir")
            .expect("symlink entry");

        assert!(link.is_symlink);
        assert!(link.is_dir);
    }

    #[cfg(unix)]
    #[test]
    fn load_entries_reports_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).expect("create locked");
        let original_permissions = std::fs::metadata(&locked).expect("metadata").permissions();
        let mut locked_permissions = original_permissions.clone();
        locked_permissions.set_mode(0o000);
        std::fs::set_permissions(&locked, locked_permissions).expect("lock dir");

        let result = PathPrompt::load_entries(&locked.to_string_lossy());

        std::fs::set_permissions(&locked, original_permissions).expect("restore permissions");

        assert!(result.entries.is_empty());
        assert_eq!(result.status.kind, PathLoadStatusKind::PermissionDenied);
        assert_eq!(result.status.message, "Permission denied.");
    }
}

#[cfg(test)]
#[gpui::test]
fn path_revision_tracks_filter_and_selection_aba(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("cat-a"), "a").unwrap();
    std::fs::write(directory.path().join("cat-b"), "b").unwrap();
    let prompt = cx.new(|cx| {
        PathPrompt::new(
            "epoch".into(),
            Some(directory.path().to_string_lossy().into_owned()),
            None,
            cx.focus_handle(),
            Arc::new(|_, _| {}),
            Arc::new(theme::Theme::default()),
        )
    });
    prompt.update(cx, |prompt, cx| {
        let initial = prompt.dictation_input_revision(cx);
        prompt.set_input(String::new(), cx);
        prompt.set_selected_index(0);
        assert_eq!(prompt.dictation_input_revision(cx), initial);
        prompt.set_input("cat".into(), cx);
        let original = prompt.dictation_input_revision(cx);
        prompt.set_input("dog".into(), cx);
        let changed = prompt.dictation_input_revision(cx);
        assert!(changed > original);
        prompt.set_input("cat".into(), cx);
        assert!(prompt.dictation_input_revision(cx) > changed);
        let before_selection = prompt.dictation_input_revision(cx);
        prompt.set_selected_index(1);
        prompt.set_selected_index(0);
        assert!(prompt.dictation_input_revision(cx) > before_selection);
        let before_read = prompt.dictation_input_revision(cx);
        prompt.visible_status_kind();
        assert_eq!(prompt.dictation_input_revision(cx), before_read);
    });
}
