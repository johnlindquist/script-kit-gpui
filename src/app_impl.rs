impl ScriptListApp {
    fn new(config: config::Config, cx: &mut Context<Self>) -> Self {
        // PERF: Measure script loading time
        let load_start = std::time::Instant::now();
        let scripts = scripts::read_scripts();
        let scripts_elapsed = load_start.elapsed();

        let scriptlets_start = std::time::Instant::now();
        let scriptlets = scripts::read_scriptlets();
        let scriptlets_elapsed = scriptlets_start.elapsed();

        let theme = theme::load_theme();
        // Config is now passed in from main() to avoid duplicate load (~100-300ms savings)

        // Load frecency data for recently-used script tracking
        let frecency_config = config.get_frecency();
        let mut frecency_store = FrecencyStore::with_config(&frecency_config);
        frecency_store.load().ok(); // Ignore errors - starts fresh if file doesn't exist

        // Load built-in entries based on config
        let builtin_entries = builtins::get_builtin_entries(&config.get_builtins());

        // Apps are loaded in the background to avoid blocking startup
        // Start with empty list, will be populated asynchronously
        let apps = Vec::new();

        let total_elapsed = load_start.elapsed();
        logging::log("PERF", &format!(
            "Startup loading: {:.2}ms total ({} scripts in {:.2}ms, {} scriptlets in {:.2}ms, apps loading in background)",
            total_elapsed.as_secs_f64() * 1000.0,
            scripts.len(),
            scripts_elapsed.as_secs_f64() * 1000.0,
            scriptlets.len(),
            scriptlets_elapsed.as_secs_f64() * 1000.0
        ));
        logging::log(
            "APP",
            &format!("Loaded {} scripts from ~/.kenv/scripts", scripts.len()),
        );
        logging::log(
            "APP",
            &format!(
                "Loaded {} scriptlets from ~/.kenv/scriptlets/scriptlets.md",
                scriptlets.len()
            ),
        );
        logging::log(
            "APP",
            &format!("Loaded {} built-in features", builtin_entries.len()),
        );
        logging::log("APP", "Applications loading in background...");
        logging::log("APP", "Loaded theme with system appearance detection");
        logging::log(
            "APP",
            &format!(
                "Loaded config: hotkey={:?}+{}, bun_path={:?}",
                config.hotkey.modifiers, config.hotkey.key, config.bun_path
            ),
        );

        // Load apps in background thread to avoid blocking startup
        let app_launcher_enabled = config.get_builtins().app_launcher;
        if app_launcher_enabled {
            // Use a channel to send loaded apps back to main thread
            let (tx, rx) =
                std::sync::mpsc::channel::<(Vec<app_launcher::AppInfo>, std::time::Duration)>();

            // Spawn background thread for app scanning
            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let apps = app_launcher::scan_applications().clone();
                let elapsed = start.elapsed();
                let _ = tx.send((apps, elapsed));
            });

            // Poll for results using a spawned task
            cx.spawn(async move |this, cx| {
                // Poll the channel periodically
                loop {
                    Timer::after(std::time::Duration::from_millis(50)).await;
                    match rx.try_recv() {
                        Ok((apps, elapsed)) => {
                            let app_count = apps.len();
                            let _ = cx.update(|cx| {
                                this.update(cx, |app, cx| {
                                    app.apps = apps;
                                    // Invalidate caches since apps changed
                                    app.filter_cache_key = String::from("\0_APPS_LOADED_\0");
                                    app.grouped_cache_key = String::from("\0_APPS_LOADED_\0");
                                    logging::log(
                                        "APP",
                                        &format!(
                                            "Background app loading complete: {} apps in {:.2}ms",
                                            app_count,
                                            elapsed.as_secs_f64() * 1000.0
                                        ),
                                    );
                                    cx.notify();
                                })
                            });
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => continue,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    }
                }
            })
            .detach();
        }
        logging::log("UI", "Script Kit logo SVG loaded for header rendering");

        // Start cursor blink timer - updates all inputs that track cursor visibility
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(530)).await;
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        // Skip cursor blink when window is hidden or no input is focused
                        if !WINDOW_VISIBLE.load(Ordering::SeqCst)
                            || app.focused_input == FocusedInput::None
                        {
                            return;
                        }

                        app.cursor_visible = !app.cursor_visible;
                        // Also update ActionsDialog cursor if it exists
                        if let Some(ref dialog) = app.actions_dialog {
                            dialog.update(cx, |d, _cx| {
                                d.set_cursor_visible(app.cursor_visible);
                            });
                        }
                        cx.notify();
                    })
                });
            }
        })
        .detach();

        let mut app = ScriptListApp {
            scripts,
            scriptlets,
            builtin_entries,
            apps,
            selected_index: 0,
            filter_text: String::new(),
            last_output: None,
            focus_handle: cx.focus_handle(),
            show_logs: false,
            theme,
            config,
            // Scroll activity tracking: start with scrollbar hidden
            is_scrolling: false,
            last_scroll_time: None,
            current_view: AppView::ScriptList,
            script_session: Arc::new(ParkingMutex::new(None)),
            arg_input_text: String::new(),
            arg_selected_index: 0,
            prompt_receiver: None,
            response_sender: None,
            // Variable-height list state for main menu (section headers at 24px, items at 48px)
            // Start with 0 items, will be reset when grouped_items changes
            // .measure_all() ensures all items are measured upfront for correct scroll height
            main_list_state: ListState::new(0, ListAlignment::Top, px(100.)).measure_all(),
            list_scroll_handle: UniformListScrollHandle::new(),
            arg_list_scroll_handle: UniformListScrollHandle::new(),
            clipboard_list_scroll_handle: UniformListScrollHandle::new(),
            window_list_scroll_handle: UniformListScrollHandle::new(),
            design_gallery_scroll_handle: UniformListScrollHandle::new(),
            show_actions_popup: false,
            actions_dialog: None,
            cursor_visible: true,
            focused_input: FocusedInput::MainFilter,
            current_script_pid: None,
            // P1: Initialize filter cache
            cached_filtered_results: Vec::new(),
            filter_cache_key: String::from("\0_UNINITIALIZED_\0"), // Sentinel value to force initial compute
            // P1: Initialize grouped results cache (Arc for cheap clone)
            cached_grouped_items: Arc::from([]),
            cached_grouped_flat_results: Arc::from([]),
            grouped_cache_key: String::from("\0_UNINITIALIZED_\0"), // Sentinel value to force initial compute
            // P3: Two-stage filter coalescing
            computed_filter_text: String::new(),
            filter_coalescer: FilterCoalescer::new(),
            // Scroll stabilization: start with no last scrolled index
            last_scrolled_index: None,
            // Preview cache: start empty, will populate on first render
            preview_cache_path: None,
            preview_cache_lines: Vec::new(),
            // Design system: start with default design
            current_design: DesignVariant::default(),
            // Toast manager: initialize for error notifications
            toast_manager: ToastManager::new(),
            // Clipboard image cache: decoded RenderImages for thumbnails/preview
            clipboard_image_cache: std::collections::HashMap::new(),
            // Frecency store for tracking script usage
            frecency_store,
            // Mouse hover tracking - starts as None (no item hovered)
            hovered_index: None,
            // P0-2: Initialize hover debounce timer
            last_hover_notify: std::time::Instant::now(),
            // Pending path action - starts as None (Arc<Mutex<>> for callback access)
            pending_path_action: Arc::new(Mutex::new(None)),
            // Signal to close path actions dialog
            close_path_actions: Arc::new(Mutex::new(false)),
            // Shared state: path actions dialog visibility (for toggle behavior)
            path_actions_showing: Arc::new(Mutex::new(false)),
            // Shared state: path actions search text (for header display)
            path_actions_search_text: Arc::new(Mutex::new(String::new())),
            // Pending path action result - action_id + path_info to execute
            pending_path_action_result: Arc::new(Mutex::new(None)),
            // Alias/shortcut registries - populated below
            alias_registry: std::collections::HashMap::new(),
            shortcut_registry: std::collections::HashMap::new(),
            // SDK actions - starts empty, populated by setActions() from scripts
            sdk_actions: None,
            action_shortcuts: std::collections::HashMap::new(),
            // Navigation coalescing for rapid arrow key events
            nav_coalescer: NavCoalescer::new(),
            // Scroll stabilization: track last scrolled index for each handle
            last_scrolled_main: None,
            last_scrolled_arg: None,
            last_scrolled_clipboard: None,
            last_scrolled_window: None,
            last_scrolled_design_gallery: None,
        };

        // Build initial alias/shortcut registries (conflicts logged, not shown via HUD on startup)
        let conflicts = app.rebuild_registries();
        if !conflicts.is_empty() {
            logging::log(
                "STARTUP",
                &format!(
                    "Found {} alias/shortcut conflicts on startup",
                    conflicts.len()
                ),
            );
        }

        app
    }

    /// Switch to a different design variant
    ///
    /// Cycle to the next design variant.
    /// Use Cmd+1 to cycle through all designs.
    fn cycle_design(&mut self, cx: &mut Context<Self>) {
        let old_design = self.current_design;
        let new_design = old_design.next();
        let all_designs = DesignVariant::all();
        let old_idx = all_designs
            .iter()
            .position(|&v| v == old_design)
            .unwrap_or(0);
        let new_idx = all_designs
            .iter()
            .position(|&v| v == new_design)
            .unwrap_or(0);

        logging::log(
            "DESIGN",
            &format!(
                "Cycling design: {} ({}) -> {} ({}) [total: {}]",
                old_design.name(),
                old_idx,
                new_design.name(),
                new_idx,
                all_designs.len()
            ),
        );
        logging::log(
            "DESIGN",
            &format!(
                "Design '{}': {}",
                new_design.name(),
                new_design.description()
            ),
        );

        self.current_design = new_design;
        logging::log(
            "DESIGN",
            &format!("self.current_design is now: {:?}", self.current_design),
        );
        cx.notify();
    }

    fn update_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = theme::load_theme();
        logging::log("APP", "Theme reloaded based on system appearance");
        cx.notify();
    }

    fn update_config(&mut self, cx: &mut Context<Self>) {
        self.config = config::load_config();
        clipboard_history::set_max_text_content_len(
            self.config.get_clipboard_history_max_text_length(),
        );
        logging::log(
            "APP",
            &format!("Config reloaded: padding={:?}", self.config.get_padding()),
        );
        cx.notify();
    }

    fn refresh_scripts(&mut self, cx: &mut Context<Self>) {
        self.scripts = scripts::read_scripts();
        self.scriptlets = scripts::read_scriptlets();
        self.selected_index = 0;
        self.last_scrolled_index = None;
        // Use main_list_state for variable-height list (not the legacy list_scroll_handle)
        self.main_list_state.scroll_to_reveal_item(0);
        self.last_scrolled_index = Some(0);
        self.invalidate_filter_cache();
        self.invalidate_grouped_cache();

        // Rebuild alias/shortcut registries and show HUD for any conflicts
        let conflicts = self.rebuild_registries();
        for conflict in conflicts {
            self.show_hud(conflict, Some(4000), cx); // 4s for conflict messages
        }

        logging::log(
            "APP",
            &format!(
                "Scripts refreshed: {} scripts, {} scriptlets loaded",
                self.scripts.len(),
                self.scriptlets.len()
            ),
        );
        cx.notify();
    }

    /// Get unified filtered results combining scripts and scriptlets
    /// P1: Now uses caching - invalidates only when filter_text changes
    fn filtered_results(&self) -> Vec<scripts::SearchResult> {
        // P1: Return cached results if filter hasn't changed
        if self.filter_text == self.filter_cache_key {
            logging::log_debug(
                "CACHE",
                &format!("Filter cache HIT for '{}'", self.filter_text),
            );
            return self.cached_filtered_results.clone();
        }

        // P1: Cache miss - need to recompute (will be done by get_filtered_results_mut)
        logging::log_debug(
            "CACHE",
            &format!(
                "Filter cache MISS - need recompute for '{}' (cached key: '{}')",
                self.filter_text, self.filter_cache_key
            ),
        );

        // PERF: Measure search time (only log when actually filtering)
        let search_start = std::time::Instant::now();
        let results =
            scripts::fuzzy_search_unified(&self.scripts, &self.scriptlets, &self.filter_text);
        let search_elapsed = search_start.elapsed();

        // Only log search performance when there's an active filter
        if !self.filter_text.is_empty() {
            logging::log(
                "PERF",
                &format!(
                    "Search '{}' took {:.2}ms ({} results from {} total)",
                    self.filter_text,
                    search_elapsed.as_secs_f64() * 1000.0,
                    results.len(),
                    self.scripts.len() + self.scriptlets.len()
                ),
            );
        }
        results
    }

    /// P1: Get filtered results with cache update (mutable version)
    /// Call this when you need to ensure cache is updated
    fn get_filtered_results_cached(&mut self) -> &Vec<scripts::SearchResult> {
        if self.filter_text != self.filter_cache_key {
            logging::log_debug(
                "CACHE",
                &format!("Filter cache MISS - recomputing for '{}'", self.filter_text),
            );
            let search_start = std::time::Instant::now();
            self.cached_filtered_results = scripts::fuzzy_search_unified_all(
                &self.scripts,
                &self.scriptlets,
                &self.builtin_entries,
                &self.apps,
                &self.filter_text,
            );
            self.filter_cache_key = self.filter_text.clone();
            let search_elapsed = search_start.elapsed();

            if !self.filter_text.is_empty() {
                logging::log(
                    "PERF",
                    &format!(
                        "Search '{}' took {:.2}ms ({} results from {} total)",
                        self.filter_text,
                        search_elapsed.as_secs_f64() * 1000.0,
                        self.cached_filtered_results.len(),
                        self.scripts.len()
                            + self.scriptlets.len()
                            + self.builtin_entries.len()
                            + self.apps.len()
                    ),
                );
            }
        } else {
            logging::log_debug(
                "CACHE",
                &format!("Filter cache HIT for '{}'", self.filter_text),
            );
        }
        &self.cached_filtered_results
    }

    /// P1: Invalidate filter cache (call when scripts/scriptlets change)
    #[allow(dead_code)]
    fn invalidate_filter_cache(&mut self) {
        logging::log_debug("CACHE", "Filter cache INVALIDATED");
        self.filter_cache_key = String::from("\0_INVALIDATED_\0");
    }

    /// P1: Get grouped results with caching - avoids recomputing 9+ times per keystroke
    ///
    /// This is the ONLY place that should call scripts::get_grouped_results().
    /// P3: Cache is keyed off computed_filter_text (not filter_text) for two-stage filtering.
    ///
    /// P1-Arc: Returns Arc clones for cheap sharing with render closures.
    fn get_grouped_results_cached(
        &mut self,
    ) -> (Arc<[GroupedListItem]>, Arc<[scripts::SearchResult]>) {
        // P3: Key off computed_filter_text for two-stage filtering
        if self.computed_filter_text == self.grouped_cache_key {
            logging::log_debug(
                "CACHE",
                &format!("Grouped cache HIT for '{}'", self.computed_filter_text),
            );
            return (
                self.cached_grouped_items.clone(),
                self.cached_grouped_flat_results.clone(),
            );
        }

        // Cache miss - need to recompute
        logging::log_debug(
            "CACHE",
            &format!(
                "Grouped cache MISS - recomputing for '{}'",
                self.computed_filter_text
            ),
        );

        let start = std::time::Instant::now();
        let max_recent_items = self.config.get_frecency().max_recent_items;
        let (grouped_items, flat_results) = get_grouped_results(
            &self.scripts,
            &self.scriptlets,
            &self.builtin_entries,
            &self.apps,
            &self.frecency_store,
            &self.computed_filter_text,
            max_recent_items,
        );
        let elapsed = start.elapsed();

        // P1-Arc: Convert to Arc<[T]> for cheap clone
        self.cached_grouped_items = grouped_items.into();
        self.cached_grouped_flat_results = flat_results.into();
        self.grouped_cache_key = self.computed_filter_text.clone();

        if !self.computed_filter_text.is_empty() {
            logging::log_debug(
                "CACHE",
                &format!(
                    "Grouped results computed in {:.2}ms for '{}' ({} items)",
                    elapsed.as_secs_f64() * 1000.0,
                    self.computed_filter_text,
                    self.cached_grouped_items.len()
                ),
            );
        }

        (
            self.cached_grouped_items.clone(),
            self.cached_grouped_flat_results.clone(),
        )
    }

    /// P1: Invalidate grouped results cache (call when scripts/scriptlets/apps change)
    fn invalidate_grouped_cache(&mut self) {
        logging::log_debug("CACHE", "Grouped cache INVALIDATED");
        self.grouped_cache_key = String::from("\0_INVALIDATED_\0");
        // Also reset computed_filter_text to force recompute
        self.computed_filter_text = String::from("\0_INVALIDATED_\0");
    }

    /// Get the currently selected search result, correctly mapping from grouped index.
    ///
    /// This function handles the mapping from `selected_index` (which is the visual
    /// position in the grouped list including section headers) to the actual
    /// `SearchResult` in the flat results array.
    ///
    /// Returns `None` if:
    /// - The selected index points to a section header (headers aren't selectable)
    /// - The selected index is out of bounds
    /// - No results exist
    fn get_selected_result(&mut self) -> Option<scripts::SearchResult> {
        let selected_index = self.selected_index;
        let (grouped_items, flat_results) = self.get_grouped_results_cached();

        match grouped_items.get(selected_index) {
            Some(GroupedListItem::Item(idx)) => flat_results.get(*idx).cloned(),
            _ => None,
        }
    }

    /// Get or update the preview cache for syntax-highlighted code lines.
    /// Only re-reads and re-highlights when the script path actually changes.
    /// Returns cached lines if path matches, otherwise updates cache and returns new lines.
    fn get_or_update_preview_cache(
        &mut self,
        script_path: &str,
        lang: &str,
    ) -> &[syntax::HighlightedLine] {
        // Check if cache is valid for this path
        if self.preview_cache_path.as_deref() == Some(script_path)
            && !self.preview_cache_lines.is_empty()
        {
            logging::log_debug("CACHE", &format!("Preview cache HIT for '{}'", script_path));
            return &self.preview_cache_lines;
        }

        // Cache miss - need to re-read and re-highlight
        logging::log_debug(
            "CACHE",
            &format!("Preview cache MISS - loading '{}'", script_path),
        );

        self.preview_cache_path = Some(script_path.to_string());
        self.preview_cache_lines = match std::fs::read_to_string(script_path) {
            Ok(content) => {
                // Only take first 15 lines for preview
                let preview: String = content.lines().take(15).collect::<Vec<_>>().join("\n");
                syntax::highlight_code_lines(&preview, lang)
            }
            Err(e) => {
                logging::log("ERROR", &format!("Failed to read preview: {}", e));
                Vec::new()
            }
        };

        &self.preview_cache_lines
    }

    /// Invalidate the preview cache (call when selection might change to different script)
    #[allow(dead_code)]
    fn invalidate_preview_cache(&mut self) {
        self.preview_cache_path = None;
        self.preview_cache_lines.clear();
    }

    #[allow(dead_code)]
    fn filtered_scripts(&self) -> Vec<scripts::Script> {
        if self.filter_text.is_empty() {
            self.scripts.clone()
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.scripts
                .iter()
                .filter(|s| s.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect()
        }
    }

    /// Find a script or scriptlet by alias (case-insensitive exact match)
    /// Uses O(1) registry lookup instead of O(n) iteration
    fn find_alias_match(&self, alias: &str) -> Option<AliasMatch> {
        let alias_lower = alias.to_lowercase();

        // O(1) lookup in registry
        if let Some(path) = self.alias_registry.get(&alias_lower) {
            // Find the script/scriptlet by path
            for script in &self.scripts {
                if script.path.to_string_lossy() == *path {
                    logging::log(
                        "ALIAS",
                        &format!("Found script match: '{}' -> '{}'", alias, script.name),
                    );
                    return Some(AliasMatch::Script(script.clone()));
                }
            }

            // Check scriptlets by file_path or name
            for scriptlet in &self.scriptlets {
                let scriptlet_path = scriptlet.file_path.as_ref().unwrap_or(&scriptlet.name);
                if scriptlet_path == path {
                    logging::log(
                        "ALIAS",
                        &format!("Found scriptlet match: '{}' -> '{}'", alias, scriptlet.name),
                    );
                    return Some(AliasMatch::Scriptlet(scriptlet.clone()));
                }
            }

            // Path in registry but not found in current scripts (stale entry)
            logging::log(
                "ALIAS",
                &format!(
                    "Stale registry entry: '{}' -> '{}' (not found)",
                    alias, path
                ),
            );
        }

        None
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        // Get grouped results to check for section headers (cached)
        let (grouped_items, _) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues with self mutation below
        let grouped_items = grouped_items.clone();

        // Find the first selectable (non-header) item index
        let first_selectable = grouped_items
            .iter()
            .position(|item| matches!(item, GroupedListItem::Item(_)));

        // If already at or before first selectable, can't go further up
        if let Some(first) = first_selectable {
            if self.selected_index <= first {
                // Already at the first selectable item, stay here
                return;
            }
        }

        if self.selected_index > 0 {
            let mut new_index = self.selected_index - 1;

            // Skip section headers when moving up
            while new_index > 0 {
                if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                    new_index -= 1;
                } else {
                    break;
                }
            }

            // Make sure we didn't land on a section header at index 0
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                // Stay at current position if we can't find a valid item
                return;
            }

            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("keyboard_up");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        // Get grouped results to check for section headers (cached)
        let (grouped_items, _) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues with self mutation below
        let grouped_items = grouped_items.clone();

        let item_count = grouped_items.len();

        // Find the last selectable (non-header) item index
        let last_selectable = grouped_items
            .iter()
            .rposition(|item| matches!(item, GroupedListItem::Item(_)));

        // If already at or after last selectable, can't go further down
        if let Some(last) = last_selectable {
            if self.selected_index >= last {
                // Already at the last selectable item, stay here
                return;
            }
        }

        if self.selected_index < item_count.saturating_sub(1) {
            let mut new_index = self.selected_index + 1;

            // Skip section headers when moving down
            while new_index < item_count.saturating_sub(1) {
                if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                    new_index += 1;
                } else {
                    break;
                }
            }

            // Make sure we didn't land on a section header at the end
            if let Some(GroupedListItem::SectionHeader(_)) = grouped_items.get(new_index) {
                // Stay at current position if we can't find a valid item
                return;
            }

            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("keyboard_down");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    /// Scroll stabilization helper: only call scroll_to_reveal_item if we haven't already scrolled to this index.
    /// This prevents scroll jitter from redundant scroll calls.
    ///
    /// NOTE: Uses main_list_state (ListState) for the variable-height list() component,
    /// not the legacy list_scroll_handle (UniformListScrollHandle).
    fn scroll_to_selected_if_needed(&mut self, _reason: &str) {
        let target = self.selected_index;

        // Check if we've already scrolled to this index
        if self.last_scrolled_index == Some(target) {
            return;
        }

        // Use perf guard for scroll timing
        let _scroll_perf = crate::perf::ScrollPerfGuard::new();

        // Perform the scroll using ListState for variable-height list
        // This scrolls the actual list() component used in render_script_list
        self.main_list_state.scroll_to_reveal_item(target);
        self.last_scrolled_index = Some(target);
    }

    /// Trigger scroll activity - shows the scrollbar and schedules fade-out
    ///
    /// This should be called whenever scroll-related activity occurs:
    /// - Keyboard up/down navigation
    /// - scroll_to_item calls
    /// - Mouse wheel scrolling (if tracked)
    fn trigger_scroll_activity(&mut self, cx: &mut Context<Self>) {
        self.is_scrolling = true;
        self.last_scroll_time = Some(std::time::Instant::now());

        // Schedule fade-out after 1000ms of inactivity
        cx.spawn(async move |this, cx| {
            Timer::after(std::time::Duration::from_millis(1000)).await;
            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    // Only hide if no new scroll activity occurred
                    if let Some(last_time) = app.last_scroll_time {
                        if last_time.elapsed() >= std::time::Duration::from_millis(1000) {
                            app.is_scrolling = false;
                            cx.notify();
                        }
                    }
                })
            });
        })
        .detach();

        cx.notify();
    }

    /// Apply a coalesced navigation delta in the given direction
    fn apply_nav_delta(&mut self, dir: NavDirection, delta: i32, cx: &mut Context<Self>) {
        let signed = match dir {
            NavDirection::Up => -delta,
            NavDirection::Down => delta,
        };
        self.move_selection_by(signed, cx);
    }

    /// Move selection by a signed delta (positive = down, negative = up)
    /// Used by NavCoalescer for batched movements
    fn move_selection_by(&mut self, delta: i32, cx: &mut Context<Self>) {
        let len = self.get_filtered_results_cached().len();
        if len == 0 {
            self.selected_index = 0;
            return;
        }
        let new_index = (self.selected_index as i32 + delta).clamp(0, (len as i32) - 1) as usize;
        if new_index != self.selected_index {
            self.selected_index = new_index;
            self.scroll_to_selected_if_needed("coalesced_nav");
            self.trigger_scroll_activity(cx);
            cx.notify();
        }
    }

    /// Ensure the navigation flush task is running. Spawns a background task
    /// that periodically flushes pending navigation deltas.
    fn ensure_nav_flush_task(&mut self, cx: &mut Context<Self>) {
        if self.nav_coalescer.flush_task_running {
            return;
        }
        self.nav_coalescer.flush_task_running = true;
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(NavCoalescer::WINDOW).await;
                let keep_running = cx
                    .update(|cx| {
                        this.update(cx, |this, cx| {
                            // Flush any pending navigation delta
                            if let Some((dir, delta)) = this.nav_coalescer.flush_pending() {
                                this.apply_nav_delta(dir, delta, cx);
                            }
                            // Check if we should keep running
                            let now = std::time::Instant::now();
                            let recently_active = now.duration_since(this.nav_coalescer.last_event)
                                < NavCoalescer::WINDOW;
                            if !recently_active && this.nav_coalescer.pending_delta == 0 {
                                // No recent activity and no pending delta - stop the task
                                this.nav_coalescer.flush_task_running = false;
                                this.nav_coalescer.reset();
                                false
                            } else {
                                true
                            }
                        })
                    })
                    .unwrap_or(Ok(false))
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        // Get grouped results to map from selected_index to actual result (cached)
        let (grouped_items, flat_results) = self.get_grouped_results_cached();
        // Clone to avoid borrow issues with self mutation below
        let grouped_items = grouped_items.clone();
        let flat_results = flat_results.clone();

        // Get the grouped item at selected_index and extract the result index
        let result_idx = match grouped_items.get(self.selected_index) {
            Some(GroupedListItem::Item(idx)) => Some(*idx),
            Some(GroupedListItem::SectionHeader(_)) => None, // Section headers are not selectable
            None => None,
        };

        if let Some(idx) = result_idx {
            if let Some(result) = flat_results.get(idx).cloned() {
                // Record frecency usage before executing
                let frecency_path = match &result {
                    scripts::SearchResult::Script(sm) => {
                        sm.script.path.to_string_lossy().to_string()
                    }
                    scripts::SearchResult::App(am) => am.app.path.to_string_lossy().to_string(),
                    scripts::SearchResult::BuiltIn(bm) => format!("builtin:{}", bm.entry.name),
                    scripts::SearchResult::Scriptlet(sm) => {
                        format!("scriptlet:{}", sm.scriptlet.name)
                    }
                    scripts::SearchResult::Window(wm) => {
                        format!("window:{}:{}", wm.window.app, wm.window.title)
                    }
                };
                self.frecency_store.record_use(&frecency_path);
                self.frecency_store.save().ok(); // Best-effort save
                self.invalidate_grouped_cache(); // Invalidate cache so next show reflects frecency

                match result {
                    scripts::SearchResult::Script(script_match) => {
                        logging::log(
                            "EXEC",
                            &format!("Executing script: {}", script_match.script.name),
                        );
                        self.execute_interactive(&script_match.script, cx);
                    }
                    scripts::SearchResult::Scriptlet(scriptlet_match) => {
                        logging::log(
                            "EXEC",
                            &format!("Executing scriptlet: {}", scriptlet_match.scriptlet.name),
                        );
                        self.execute_scriptlet(&scriptlet_match.scriptlet, cx);
                    }
                    scripts::SearchResult::BuiltIn(builtin_match) => {
                        logging::log(
                            "EXEC",
                            &format!("Executing built-in: {}", builtin_match.entry.name),
                        );
                        self.execute_builtin(&builtin_match.entry, cx);
                    }
                    scripts::SearchResult::App(app_match) => {
                        logging::log("EXEC", &format!("Launching app: {}", app_match.app.name));
                        self.execute_app(&app_match.app, cx);
                    }
                    scripts::SearchResult::Window(window_match) => {
                        logging::log(
                            "EXEC",
                            &format!("Focusing window: {}", window_match.window.title),
                        );
                        self.execute_window_focus(&window_match.window, cx);
                    }
                }
            }
        }
    }

    fn update_filter(
        &mut self,
        new_char: Option<char>,
        backspace: bool,
        clear: bool,
        cx: &mut Context<Self>,
    ) {
        // P3: Stage 1 - Update filter_text immediately (displayed in input)
        if clear {
            self.filter_text.clear();
            self.selected_index = 0;
            self.last_scrolled_index = None;
            // Use main_list_state for variable-height list (not the legacy list_scroll_handle)
            self.main_list_state.scroll_to_reveal_item(0);
            self.last_scrolled_index = Some(0);
            // P3: Clear also immediately updates computed text (no coalescing needed)
            self.computed_filter_text.clear();
        } else if backspace && !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.selected_index = 0;
            self.last_scrolled_index = None;
            // Use main_list_state for variable-height list (not the legacy list_scroll_handle)
            self.main_list_state.scroll_to_reveal_item(0);
            self.last_scrolled_index = Some(0);
        } else if let Some(ch) = new_char {
            self.filter_text.push(ch);
            self.selected_index = 0;
            self.last_scrolled_index = None;
            // Use main_list_state for variable-height list (not the legacy list_scroll_handle)
            self.main_list_state.scroll_to_reveal_item(0);
            self.last_scrolled_index = Some(0);
        }

        // P3: Notify immediately so input field updates (responsive typing)
        cx.notify();

        // P3: Stage 2 - Coalesce expensive search work with 16ms delay
        // Keep only the latest filter text while a timer is pending.
        if self.filter_coalescer.queue(self.filter_text.clone()) {
            cx.spawn(async move |this, cx| {
                // Wait 16ms for coalescing window (one frame at 60fps)
                Timer::after(std::time::Duration::from_millis(16)).await;

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        if let Some(latest) = app.filter_coalescer.take_latest() {
                            if app.computed_filter_text != latest {
                                app.computed_filter_text = latest;
                                // This will trigger cache recompute on next get_grouped_results_cached()
                                app.update_window_size();
                                cx.notify();
                            }
                        }
                    })
                });
            })
            .detach();
        }
    }

    fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        cx.notify();
    }

    /// Update window size based on current view and item count.
    /// This implements dynamic window resizing:
    /// - Script list: resize based on filtered results (including section headers)
    /// - Arg prompt: resize based on filtered choices
    /// - Div/Editor/Term: use full height
    fn update_window_size(&mut self) {
        let (view_type, item_count) = match &self.current_view {
            AppView::ScriptList => {
                // Get grouped results which includes section headers (cached)
                let (grouped_items, _) = self.get_grouped_results_cached();
                let count = grouped_items.len();
                (ViewType::ScriptList, count)
            }
            AppView::ArgPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    (ViewType::ArgPromptNoChoices, 0)
                } else {
                    (ViewType::ArgPromptWithChoices, filtered.len())
                }
            }
            AppView::DivPrompt { .. } => (ViewType::DivPrompt, 0),
            AppView::FormPrompt { .. } => (ViewType::DivPrompt, 0), // Use DivPrompt size for forms
            AppView::EditorPrompt { .. } => (ViewType::EditorPrompt, 0),
            AppView::SelectPrompt { .. } => (ViewType::ArgPromptWithChoices, 0),
            AppView::PathPrompt { .. } => (ViewType::DivPrompt, 0),
            AppView::EnvPrompt { .. } => (ViewType::ArgPromptNoChoices, 0), // Env prompt is a simple input
            AppView::DropPrompt { .. } => (ViewType::DivPrompt, 0), // Drop prompt uses div size for drop zone
            AppView::TemplatePrompt { .. } => (ViewType::DivPrompt, 0), // Template prompt uses div size
            AppView::TermPrompt { .. } => (ViewType::TermPrompt, 0),
            AppView::ActionsDialog => {
                // Actions dialog is an overlay, don't resize
                return;
            }
            // Clipboard history and app launcher use standard height (same as script list)
            AppView::ClipboardHistoryView {
                entries, filter, ..
            } => {
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .filter(|e| e.content.to_lowercase().contains(&filter_lower))
                        .count()
                };
                (ViewType::ScriptList, filtered_count)
            }
            AppView::AppLauncherView { apps, filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    apps.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    apps.iter()
                        .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                        .count()
                };
                (ViewType::ScriptList, filtered_count)
            }
            AppView::WindowSwitcherView {
                windows, filter, ..
            } => {
                let filtered_count = if filter.is_empty() {
                    windows.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    windows
                        .iter()
                        .filter(|w| {
                            w.title.to_lowercase().contains(&filter_lower)
                                || w.app.to_lowercase().contains(&filter_lower)
                        })
                        .count()
                };
                (ViewType::ScriptList, filtered_count)
            }
            AppView::DesignGalleryView { filter, .. } => {
                // Calculate total gallery items (separators + icons)
                let total_items = designs::separator_variations::SeparatorStyle::count()
                    + designs::icon_variations::total_icon_count();
                let filtered_count = if filter.is_empty() {
                    total_items
                } else {
                    // For now, return total - filtering can be added later
                    total_items
                };
                (ViewType::ScriptList, filtered_count)
            }
        };

        let target_height = height_for_view(view_type, item_count);
        resize_first_window_to_height(target_height);
    }

    fn set_prompt_input(&mut self, text: String, cx: &mut Context<Self>) {
        match &mut self.current_view {
            AppView::ArgPrompt { .. } => {
                self.arg_input_text = text;
                self.arg_selected_index = 0;
                self.arg_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.update_window_size();
                cx.notify();
            }
            AppView::PathPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
            }
            AppView::SelectPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
            }
            AppView::EnvPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
            }
            AppView::TemplatePrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
            }
            AppView::FormPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
            }
            _ => {}
        }
    }

    /// Helper to get filtered arg choices without cloning
    fn get_filtered_arg_choices<'a>(&self, choices: &'a [Choice]) -> Vec<&'a Choice> {
        if self.arg_input_text.is_empty() {
            choices.iter().collect()
        } else {
            let filter = self.arg_input_text.to_lowercase();
            choices
                .iter()
                .filter(|c| c.name.to_lowercase().contains(&filter))
                .collect()
        }
    }

    fn toggle_actions(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        logging::log("KEY", "Toggling actions popup");
        if self.show_actions_popup {
            // Close - return focus to main filter
            self.show_actions_popup = false;
            self.actions_dialog = None;
            self.focused_input = FocusedInput::MainFilter;
            window.focus(&self.focus_handle, cx);
            logging::log("FOCUS", "Actions closed, focus returned to MainFilter");
        } else {
            // Open - create dialog entity
            self.show_actions_popup = true;
            self.focused_input = FocusedInput::ActionsSearch;
            let script_info = self.get_focused_script_info();

            let theme_arc = std::sync::Arc::new(self.theme.clone());
            let dialog = cx.new(|cx| {
                let focus_handle = cx.focus_handle();
                ActionsDialog::with_script(
                    focus_handle,
                    std::sync::Arc::new(|_action_id| {}), // Callback handled separately
                    script_info,
                    theme_arc,
                )
            });

            // Hide the dialog's built-in search input since header already has search
            dialog.update(cx, |d, _| d.set_hide_search(true));

            // Focus the dialog's internal focus handle
            let dialog_focus_handle = dialog.read(cx).focus_handle.clone();
            self.actions_dialog = Some(dialog.clone());
            window.focus(&dialog_focus_handle, cx);
            logging::log("FOCUS", "Actions opened, focus moved to ActionsSearch");
        }
        cx.notify();
    }

    /// Toggle actions dialog for arg prompts with SDK-defined actions
    fn toggle_arg_actions(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        logging::log(
            "KEY",
            &format!(
                "toggle_arg_actions called: show_actions_popup={}, actions_dialog.is_some={}, sdk_actions.is_some={}",
                self.show_actions_popup,
                self.actions_dialog.is_some(),
                self.sdk_actions.is_some()
            ),
        );
        if self.show_actions_popup {
            // Close - return focus to arg prompt
            self.show_actions_popup = false;
            self.actions_dialog = None;
            self.focused_input = FocusedInput::ArgPrompt;
            window.focus(&self.focus_handle, cx);
            logging::log("FOCUS", "Arg actions closed, focus returned to ArgPrompt");
        } else {
            // Check if we have SDK actions
            if let Some(ref sdk_actions) = self.sdk_actions {
                logging::log("KEY", &format!("SDK actions count: {}", sdk_actions.len()));
                if !sdk_actions.is_empty() {
                    // Open - create dialog entity with SDK actions
                    self.show_actions_popup = true;
                    self.focused_input = FocusedInput::ActionsSearch;

                    let theme_arc = std::sync::Arc::new(self.theme.clone());
                    let sdk_actions_clone = sdk_actions.clone();
                    let dialog = cx.new(|cx| {
                        let focus_handle = cx.focus_handle();
                        let mut dialog = ActionsDialog::with_script(
                            focus_handle,
                            std::sync::Arc::new(|_action_id| {}), // Callback handled separately
                            None,                                 // No script info for arg prompts
                            theme_arc,
                        );
                        // Set SDK actions to replace built-in actions
                        dialog.set_sdk_actions(sdk_actions_clone);
                        dialog
                    });

                    // Hide the dialog's built-in search input since header already has search
                    dialog.update(cx, |d, _| d.set_hide_search(true));

                    // Focus the dialog's internal focus handle
                    let dialog_focus_handle = dialog.read(cx).focus_handle.clone();
                    self.actions_dialog = Some(dialog.clone());
                    window.focus(&dialog_focus_handle, cx);
                    logging::log(
                        "FOCUS",
                        &format!(
                            "Arg actions OPENED: show_actions_popup={}, actions_dialog.is_some={}",
                            self.show_actions_popup,
                            self.actions_dialog.is_some()
                        ),
                    );
                } else {
                    logging::log("KEY", "No SDK actions available to show (empty list)");
                }
            } else {
                logging::log("KEY", "No SDK actions defined for this arg prompt (None)");
            }
        }
        cx.notify();
    }

    /// Handle action selection from the actions dialog
    fn handle_action(&mut self, action_id: String, cx: &mut Context<Self>) {
        logging::log("UI", &format!("Action selected: {}", action_id));

        // Close the dialog and return to script list
        self.current_view = AppView::ScriptList;

        match action_id.as_str() {
            "create_script" => {
                logging::log("UI", "Create script action - opening scripts folder");
                // Open ~/.kenv/scripts/ in Finder for now (future: create script dialog)
                let scripts_dir = shellexpand::tilde("~/.kenv/scripts").to_string();
                std::thread::spawn(move || {
                    use std::process::Command;
                    match Command::new("open").arg(&scripts_dir).spawn() {
                        Ok(_) => {
                            logging::log("UI", &format!("Opened scripts folder: {}", scripts_dir))
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open scripts folder: {}", e))
                        }
                    }
                });
                self.last_output = Some(SharedString::from("Opened scripts folder"));
            }
            "run_script" => {
                logging::log("UI", "Run script action");
                self.execute_selected(cx);
            }
            "view_logs" => {
                logging::log("UI", "View logs action");
                self.toggle_logs(cx);
            }
            "reveal_in_finder" => {
                logging::log("UI", "Reveal in Finder action");
                if let Some(result) = self.get_selected_result() {
                    match result {
                        scripts::SearchResult::Script(script_match) => {
                            let path_str = script_match.script.path.to_string_lossy().to_string();
                            std::thread::spawn(move || {
                                use std::process::Command;
                                match Command::new("open").arg("-R").arg(&path_str).spawn() {
                                    Ok(_) => logging::log(
                                        "UI",
                                        &format!("Revealed in Finder: {}", path_str),
                                    ),
                                    Err(e) => logging::log(
                                        "ERROR",
                                        &format!("Failed to reveal in Finder: {}", e),
                                    ),
                                }
                            });
                            self.last_output = Some(SharedString::from("Revealed in Finder"));
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal scriptlets in Finder"));
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal built-in features"));
                        }
                        scripts::SearchResult::App(app_match) => {
                            let path_str = app_match.app.path.to_string_lossy().to_string();
                            std::thread::spawn(move || {
                                use std::process::Command;
                                match Command::new("open").arg("-R").arg(&path_str).spawn() {
                                    Ok(_) => logging::log(
                                        "UI",
                                        &format!("Revealed app in Finder: {}", path_str),
                                    ),
                                    Err(e) => logging::log(
                                        "ERROR",
                                        &format!("Failed to reveal app in Finder: {}", e),
                                    ),
                                }
                            });
                            self.last_output = Some(SharedString::from("Revealed app in Finder"));
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot reveal windows in Finder"));
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No item selected"));
                }
            }
            "copy_path" => {
                logging::log("UI", "Copy path action");
                if let Some(result) = self.get_selected_result() {
                    let path_opt = match result {
                        scripts::SearchResult::Script(script_match) => {
                            Some(script_match.script.path.to_string_lossy().to_string())
                        }
                        scripts::SearchResult::App(app_match) => {
                            Some(app_match.app.path.to_string_lossy().to_string())
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot copy scriptlet path"));
                            None
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot copy built-in path"));
                            None
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output = Some(SharedString::from("Cannot copy window path"));
                            None
                        }
                    };

                    if let Some(path_str) = path_opt {
                        // Use pbcopy on macOS for reliable clipboard access
                        #[cfg(target_os = "macos")]
                        {
                            use std::io::Write;
                            use std::process::{Command, Stdio};

                            match Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                                Ok(mut child) => {
                                    if let Some(ref mut stdin) = child.stdin {
                                        if stdin.write_all(path_str.as_bytes()).is_ok() {
                                            let _ = child.wait();
                                            logging::log(
                                                "UI",
                                                &format!("Copied path to clipboard: {}", path_str),
                                            );
                                            self.last_output = Some(SharedString::from(format!(
                                                "Copied: {}",
                                                path_str
                                            )));
                                        } else {
                                            logging::log(
                                                "ERROR",
                                                "Failed to write to pbcopy stdin",
                                            );
                                            self.last_output =
                                                Some(SharedString::from("Failed to copy path"));
                                        }
                                    }
                                }
                                Err(e) => {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to spawn pbcopy: {}", e),
                                    );
                                    self.last_output =
                                        Some(SharedString::from("Failed to copy path"));
                                }
                            }
                        }

                        // Fallback for non-macOS platforms
                        #[cfg(not(target_os = "macos"))]
                        {
                            use arboard::Clipboard;
                            match Clipboard::new() {
                                Ok(mut clipboard) => match clipboard.set_text(&path_str) {
                                    Ok(_) => {
                                        logging::log(
                                            "UI",
                                            &format!("Copied path to clipboard: {}", path_str),
                                        );
                                        self.last_output = Some(SharedString::from(format!(
                                            "Copied: {}",
                                            path_str
                                        )));
                                    }
                                    Err(e) => {
                                        logging::log(
                                            "ERROR",
                                            &format!("Failed to copy path: {}", e),
                                        );
                                        self.last_output =
                                            Some(SharedString::from("Failed to copy path"));
                                    }
                                },
                                Err(e) => {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to access clipboard: {}", e),
                                    );
                                    self.last_output =
                                        Some(SharedString::from("Failed to access clipboard"));
                                }
                            }
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No item selected"));
                }
            }
            "edit_script" => {
                logging::log("UI", "Edit script action");
                if let Some(result) = self.get_selected_result() {
                    match result {
                        scripts::SearchResult::Script(script_match) => {
                            self.edit_script(&script_match.script.path);
                        }
                        scripts::SearchResult::Scriptlet(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit scriptlets"));
                        }
                        scripts::SearchResult::BuiltIn(_) => {
                            self.last_output =
                                Some(SharedString::from("Cannot edit built-in features"));
                        }
                        scripts::SearchResult::App(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit applications"));
                        }
                        scripts::SearchResult::Window(_) => {
                            self.last_output = Some(SharedString::from("Cannot edit windows"));
                        }
                    }
                } else {
                    self.last_output = Some(SharedString::from("No script selected"));
                }
            }
            "reload_scripts" => {
                logging::log("UI", "Reload scripts action");
                self.refresh_scripts(cx);
                self.last_output = Some(SharedString::from("Scripts reloaded"));
            }
            "settings" => {
                logging::log("UI", "Settings action");
                self.last_output = Some(SharedString::from("Settings (TODO)"));
            }
            "quit" => {
                logging::log("UI", "Quit action");
                // Clean up processes and PID file before quitting
                PROCESS_MANAGER.kill_all_processes();
                PROCESS_MANAGER.remove_main_pid();
                cx.quit();
            }
            "__cancel__" => {
                logging::log("UI", "Actions dialog cancelled");
            }
            _ => {
                // Check if this is an SDK action with has_action=true
                if let Some(ref actions) = self.sdk_actions {
                    if let Some(action) = actions.iter().find(|a| a.name == action_id) {
                        if action.has_action {
                            // Send ActionTriggered back to SDK
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action with handler: '{}' (has_action=true), sending ActionTriggered",
                                    action_id
                                ),
                            );
                            if let Some(ref sender) = self.response_sender {
                                let msg = protocol::Message::action_triggered(
                                    action_id.clone(),
                                    action.value.clone(),
                                    self.arg_input_text.clone(),
                                );
                                if let Err(e) = sender.send(msg) {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to send ActionTriggered: {}", e),
                                    );
                                }
                            }
                        } else if let Some(ref value) = action.value {
                            // Submit value directly (has_action=false with value)
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action without handler: '{}' (has_action=false), submitting value: {:?}",
                                    action_id, value
                                ),
                            );
                            if let Some(ref sender) = self.response_sender {
                                let msg = protocol::Message::Submit {
                                    id: "action".to_string(),
                                    value: Some(value.clone()),
                                };
                                if let Err(e) = sender.send(msg) {
                                    logging::log("ERROR", &format!("Failed to send Submit: {}", e));
                                }
                            }
                        } else {
                            logging::log(
                                "ACTIONS",
                                &format!(
                                    "SDK action '{}' has no value and has_action=false",
                                    action_id
                                ),
                            );
                        }
                    } else {
                        logging::log("UI", &format!("Unknown action: {}", action_id));
                    }
                } else {
                    logging::log("UI", &format!("Unknown action: {}", action_id));
                }
            }
        }

        cx.notify();
    }

    /// Trigger an SDK action by name
    /// Returns true if the action was found and triggered
    fn trigger_action_by_name(&mut self, action_name: &str, cx: &mut Context<Self>) -> bool {
        if let Some(ref actions) = self.sdk_actions {
            if let Some(action) = actions.iter().find(|a| a.name == action_name) {
                logging::log(
                    "ACTIONS",
                    &format!(
                        "Triggering SDK action '{}' via shortcut (has_action={})",
                        action_name, action.has_action
                    ),
                );

                if action.has_action {
                    // Send ActionTriggered back to SDK
                    if let Some(ref sender) = self.response_sender {
                        let msg = protocol::Message::action_triggered(
                            action_name.to_string(),
                            action.value.clone(),
                            self.arg_input_text.clone(),
                        );
                        if let Err(e) = sender.send(msg) {
                            logging::log(
                                "ERROR",
                                &format!("Failed to send ActionTriggered: {}", e),
                            );
                        }
                    }
                } else if let Some(ref value) = action.value {
                    // Submit value directly
                    if let Some(ref sender) = self.response_sender {
                        let msg = protocol::Message::Submit {
                            id: "action".to_string(),
                            value: Some(value.clone()),
                        };
                        if let Err(e) = sender.send(msg) {
                            logging::log("ERROR", &format!("Failed to send Submit: {}", e));
                        }
                    }
                }

                cx.notify();
                return true;
            }
        }
        false
    }

    /// Edit a script in configured editor (config.editor > $EDITOR > "code")
    #[allow(dead_code)]
    fn edit_script(&mut self, path: &std::path::Path) {
        let editor = self.config.get_editor();
        logging::log(
            "UI",
            &format!("Opening script in editor '{}': {}", editor, path.display()),
        );
        let path_str = path.to_string_lossy().to_string();

        std::thread::spawn(move || {
            use std::process::Command;
            match Command::new(&editor).arg(&path_str).spawn() {
                Ok(_) => logging::log("UI", &format!("Successfully spawned editor: {}", editor)),
                Err(e) => logging::log(
                    "ERROR",
                    &format!("Failed to spawn editor '{}': {}", editor, e),
                ),
            }
        });
    }

    /// Execute a path action from the actions dialog
    /// Handles actions like copy_path, open_in_finder, open_in_editor, etc.
    fn execute_path_action(
        &mut self,
        action_id: &str,
        path_info: &PathInfo,
        path_prompt_entity: &Entity<PathPrompt>,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "UI",
            &format!(
                "Executing path action '{}' for: {} (is_dir={})",
                action_id, path_info.path, path_info.is_dir
            ),
        );

        match action_id {
            "select_file" | "open_directory" => {
                // For select/open, trigger submission through the path prompt
                // We need to trigger the submit callback with this path
                path_prompt_entity.update(cx, |prompt, cx| {
                    // Find the index of this path in filtered_entries and submit it
                    if let Some(idx) = prompt
                        .filtered_entries
                        .iter()
                        .position(|e| e.path == path_info.path)
                    {
                        prompt.selected_index = idx;
                    }
                    // For directories, navigate into them; for files, submit
                    if path_info.is_dir && action_id == "open_directory" {
                        prompt.navigate_to(&path_info.path, cx);
                    } else {
                        // Submit the selected path
                        let id = prompt.id.clone();
                        let path = path_info.path.clone();
                        (prompt.on_submit)(id, Some(path));
                    }
                });
            }
            "copy_path" => {
                // Copy full path to clipboard
                #[cfg(target_os = "macos")]
                {
                    use std::io::Write;
                    use std::process::{Command, Stdio};

                    match Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                        Ok(mut child) => {
                            if let Some(ref mut stdin) = child.stdin {
                                if stdin.write_all(path_info.path.as_bytes()).is_ok() {
                                    let _ = child.wait();
                                    logging::log(
                                        "UI",
                                        &format!("Copied path to clipboard: {}", path_info.path),
                                    );
                                    self.last_output = Some(SharedString::from(format!(
                                        "Copied: {}",
                                        path_info.path
                                    )));
                                } else {
                                    logging::log("ERROR", "Failed to write to pbcopy stdin");
                                    self.last_output =
                                        Some(SharedString::from("Failed to copy path"));
                                }
                            }
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to spawn pbcopy: {}", e));
                            self.last_output = Some(SharedString::from("Failed to copy path"));
                        }
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    use arboard::Clipboard;
                    match Clipboard::new() {
                        Ok(mut clipboard) => match clipboard.set_text(&path_info.path) {
                            Ok(_) => {
                                logging::log(
                                    "UI",
                                    &format!("Copied path to clipboard: {}", path_info.path),
                                );
                                self.last_output =
                                    Some(SharedString::from(format!("Copied: {}", path_info.path)));
                            }
                            Err(e) => {
                                logging::log("ERROR", &format!("Failed to copy path: {}", e));
                                self.last_output = Some(SharedString::from("Failed to copy path"));
                            }
                        },
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to access clipboard: {}", e));
                            self.last_output =
                                Some(SharedString::from("Failed to access clipboard"));
                        }
                    }
                }
            }
            "copy_filename" => {
                // Copy just the filename to clipboard
                #[cfg(target_os = "macos")]
                {
                    use std::io::Write;
                    use std::process::{Command, Stdio};

                    match Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
                        Ok(mut child) => {
                            if let Some(ref mut stdin) = child.stdin {
                                if stdin.write_all(path_info.name.as_bytes()).is_ok() {
                                    let _ = child.wait();
                                    logging::log(
                                        "UI",
                                        &format!(
                                            "Copied filename to clipboard: {}",
                                            path_info.name
                                        ),
                                    );
                                    self.last_output = Some(SharedString::from(format!(
                                        "Copied: {}",
                                        path_info.name
                                    )));
                                }
                            }
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to spawn pbcopy: {}", e));
                        }
                    }
                }
            }
            "open_in_finder" => {
                // Reveal in Finder (macOS)
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    let path_to_reveal = if path_info.is_dir {
                        path_info.path.clone()
                    } else {
                        // For files, reveal the containing folder with the file selected
                        path_info.path.clone()
                    };

                    match Command::new("open").args(["-R", &path_to_reveal]).spawn() {
                        Ok(_) => {
                            logging::log("UI", &format!("Revealed in Finder: {}", path_info.path));
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to reveal in Finder: {}", e));
                            self.last_output =
                                Some(SharedString::from("Failed to reveal in Finder"));
                        }
                    }
                }
            }
            "open_in_editor" => {
                // Open in configured editor
                let editor = self.config.get_editor();
                let path_str = path_info.path.clone();
                logging::log(
                    "UI",
                    &format!("Opening in editor '{}': {}", editor, path_str),
                );

                std::thread::spawn(move || {
                    use std::process::Command;
                    match Command::new(&editor).arg(&path_str).spawn() {
                        Ok(_) => logging::log("UI", &format!("Opened in editor: {}", path_str)),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open in editor: {}", e))
                        }
                    }
                });
            }
            "open_in_terminal" => {
                // Open terminal at this location
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    // Get the directory (if file, use parent directory)
                    let dir_path = if path_info.is_dir {
                        path_info.path.clone()
                    } else {
                        std::path::Path::new(&path_info.path)
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|| path_info.path.clone())
                    };

                    // Try iTerm first, fall back to Terminal.app
                    let script = format!(
                        r#"tell application "Terminal"
                            do script "cd '{}'"
                            activate
                        end tell"#,
                        dir_path
                    );

                    match Command::new("osascript").args(["-e", &script]).spawn() {
                        Ok(_) => {
                            logging::log("UI", &format!("Opened terminal at: {}", dir_path));
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open terminal: {}", e));
                            self.last_output = Some(SharedString::from("Failed to open terminal"));
                        }
                    }
                }
            }
            "move_to_trash" => {
                // Move to trash (macOS)
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    let path_str = path_info.path.clone();
                    let name = path_info.name.clone();

                    // Use AppleScript to move to trash (preserves undo capability)
                    let script = format!(
                        r#"tell application "Finder"
                            delete POSIX file "{}"
                        end tell"#,
                        path_str
                    );

                    match Command::new("osascript").args(["-e", &script]).spawn() {
                        Ok(mut child) => {
                            // Wait for completion and check result
                            match child.wait() {
                                Ok(status) if status.success() => {
                                    logging::log("UI", &format!("Moved to trash: {}", path_str));
                                    self.last_output = Some(SharedString::from(format!(
                                        "Moved to Trash: {}",
                                        name
                                    )));
                                    // Refresh the path prompt to show the file is gone
                                    path_prompt_entity.update(cx, |prompt, cx| {
                                        let current = prompt.current_path.clone();
                                        prompt.navigate_to(&current, cx);
                                    });
                                }
                                _ => {
                                    logging::log(
                                        "ERROR",
                                        &format!("Failed to move to trash: {}", path_str),
                                    );
                                    self.last_output =
                                        Some(SharedString::from("Failed to move to Trash"));
                                }
                            }
                        }
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to spawn trash command: {}", e));
                            self.last_output = Some(SharedString::from("Failed to move to Trash"));
                        }
                    }
                }
            }
            _ => {
                logging::log("UI", &format!("Unknown path action: {}", action_id));
            }
        }

        cx.notify();
    }

    /// Execute a script interactively (for scripts that use arg/div prompts)
    fn execute_interactive(&mut self, script: &scripts::Script, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!("Starting interactive execution: {}", script.name),
        );

        // Store script path for error reporting in reader thread
        let script_path_for_errors = script.path.to_string_lossy().to_string();

        match executor::execute_script_interactive(&script.path) {
            Ok(session) => {
                logging::log("EXEC", "Interactive session started successfully");

                // Store PID for explicit cleanup (belt-and-suspenders approach)
                let pid = session.pid();
                self.current_script_pid = Some(pid);
                logging::log("EXEC", &format!("Stored script PID {} for cleanup", pid));

                *self.script_session.lock() = Some(session);

                // Create async_channel for script thread to send prompt messages to UI (event-driven)
                // P1-6: Use bounded channel to prevent unbounded memory growth from slow UI
                // Capacity of 100 is generous (scripts rarely send > 10 messages/sec)
                let (tx, rx) = async_channel::bounded(100);
                let rx_for_listener = rx.clone();
                self.prompt_receiver = Some(rx);

                // Spawn event-driven listener for prompt messages (replaces 50ms polling)
                cx.spawn(async move |this, cx| {
                    logging::log("EXEC", "Prompt message listener started (event-driven)");

                    // Event-driven: recv().await yields until a message arrives
                    while let Ok(msg) = rx_for_listener.recv().await {
                        logging::log("EXEC", &format!("Prompt message received: {:?}", msg));
                        let _ = cx.update(|cx| {
                            this.update(cx, |app, cx| {
                                app.handle_prompt_message(msg, cx);
                            })
                        });
                    }

                    logging::log("EXEC", "Prompt message listener exiting (channel closed)");
                })
                .detach();

                // We need separate threads for reading and writing to avoid deadlock
                // The read thread blocks on receive_message(), so we can't check for responses in the same loop

                // Take ownership of the session and split it
                let session = self.script_session.lock().take().unwrap();
                let split = session.split();

                let mut stdin = split.stdin;
                let mut stdout_reader = split.stdout_reader;
                // Capture stderr for error reporting - we'll read it in real-time for debugging
                let stderr_handle = split.stderr;
                // CRITICAL: Keep process_handle and child alive - they kill the process on drop!
                // We move them into the reader thread so they live until the script exits.
                let _process_handle = split.process_handle;
                let mut _child = split.child;

                // Stderr reader thread - forwards script stderr to logs in real-time
                if let Some(stderr) = stderr_handle {
                    std::thread::spawn(move || {
                        use std::io::BufRead;
                        let reader = std::io::BufReader::new(stderr);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => logging::log("SCRIPT", &l),
                                Err(e) => {
                                    logging::log("SCRIPT", &format!("stderr read error: {}", e));
                                    break;
                                }
                            }
                        }
                        logging::log("SCRIPT", "stderr reader exiting");
                    });
                }

                // Now stderr_handle is consumed, we pass None to reader thread
                let stderr_handle: Option<std::process::ChildStderr> = None;

                // Channel for sending responses from UI to writer thread
                let (response_tx, response_rx) = mpsc::channel::<Message>();

                // Clone response_tx for the reader thread to handle direct responses
                // (e.g., getSelectedText, setSelectedText, checkAccessibility)
                let reader_response_tx = response_tx.clone();

                // Writer thread - handles sending responses to script
                std::thread::spawn(move || {
                    use std::io::Write;
                    use std::os::unix::io::AsRawFd;

                    // Log the stdin file descriptor for debugging
                    let fd = stdin.as_raw_fd();
                    logging::log("EXEC", &format!("Writer thread started, stdin fd={}", fd));

                    // Check if fd is a valid pipe
                    #[cfg(unix)]
                    {
                        let stat_result = unsafe {
                            let mut stat: libc::stat = std::mem::zeroed();
                            libc::fstat(fd, &mut stat)
                        };
                        if stat_result == 0 {
                            logging::log("EXEC", &format!("fd={} fstat succeeded", fd));
                        } else {
                            logging::log(
                                "EXEC",
                                &format!(
                                    "fd={} fstat FAILED: errno={}",
                                    fd,
                                    std::io::Error::last_os_error()
                                ),
                            );
                        }
                    }

                    loop {
                        match response_rx.recv() {
                            Ok(response) => {
                                let json = match protocol::serialize_message(&response) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to serialize: {}", e),
                                        );
                                        continue;
                                    }
                                };
                                logging::log(
                                    "EXEC",
                                    &format!("Writing to stdin fd={}: {}", fd, json),
                                );
                                let bytes = format!("{}\n", json);
                                let bytes_len = bytes.len();

                                // Check fd validity before write
                                let fcntl_result = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                                logging::log(
                                    "EXEC",
                                    &format!(
                                        "Pre-write fcntl(F_GETFD) on fd={}: {}",
                                        fd, fcntl_result
                                    ),
                                );

                                match stdin.write_all(bytes.as_bytes()) {
                                    Ok(()) => {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Write succeeded: {} bytes to fd={}",
                                                bytes_len, fd
                                            ),
                                        );
                                    }
                                    Err(e) => {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to write {} bytes: {} (kind={:?})",
                                                bytes_len,
                                                e,
                                                e.kind()
                                            ),
                                        );
                                        break;
                                    }
                                }
                                if let Err(e) = stdin.flush() {
                                    logging::log(
                                        "EXEC",
                                        &format!("Failed to flush fd={}: {}", fd, e),
                                    );
                                    break;
                                }
                                logging::log("EXEC", &format!("Flush succeeded for fd={}", fd));
                            }
                            Err(_) => {
                                logging::log("EXEC", "Response channel closed, writer exiting");
                                break;
                            }
                        }
                    }
                    logging::log("EXEC", "Writer thread exiting");
                });

                // Reader thread - handles receiving messages from script (blocking is OK here)
                // CRITICAL: Move _process_handle and _child into this thread to keep them alive!
                // When the reader thread exits, they'll be dropped and the process killed.
                let script_path_clone = script_path_for_errors.clone();
                std::thread::spawn(move || {
                    // These variables keep the process alive - they're dropped when the thread exits
                    let _keep_alive_handle = _process_handle;
                    let mut keep_alive_child = _child;
                    let mut stderr_for_errors = stderr_handle;
                    let script_path = script_path_clone;

                    loop {
                        // Use next_message_graceful_with_handler to skip non-JSON lines and report parse issues
                        match stdout_reader.next_message_graceful_with_handler(|issue| {
                            let should_report = matches!(
                                issue.kind,
                                protocol::ParseIssueKind::InvalidPayload
                                    | protocol::ParseIssueKind::UnknownType
                            );
                            if !should_report {
                                return;
                            }

                            let summary = match issue.kind {
                                protocol::ParseIssueKind::InvalidPayload => issue
                                    .message_type
                                    .as_deref()
                                    .map(|message_type| {
                                        format!(
                                            "Invalid '{}' message payload from script",
                                            message_type
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        "Invalid message payload from script".to_string()
                                    }),
                                protocol::ParseIssueKind::UnknownType => issue
                                    .message_type
                                    .as_deref()
                                    .map(|message_type| {
                                        format!(
                                            "Unknown '{}' message type from script",
                                            message_type
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        "Unknown message type from script".to_string()
                                    }),
                                _ => "Protocol message issue from script".to_string(),
                            };

                            let mut details_lines = Vec::new();
                            details_lines.push(format!("Script: {}", script_path));
                            if let Some(ref message_type) = issue.message_type {
                                details_lines.push(format!("Type: {}", message_type));
                            }
                            if let Some(ref error) = issue.error {
                                details_lines.push(format!("Error: {}", error));
                            }
                            if !issue.raw_preview.is_empty() {
                                details_lines.push(format!("Preview: {}", issue.raw_preview));
                            }
                            let details = Some(details_lines.join("\n"));

                            let severity = match issue.kind {
                                protocol::ParseIssueKind::InvalidPayload => ErrorSeverity::Error,
                                protocol::ParseIssueKind::UnknownType => ErrorSeverity::Warning,
                                _ => ErrorSeverity::Warning,
                            };

                            let correlation_id = issue.correlation_id.clone();
                            let prompt_msg = PromptMessage::ProtocolError {
                                correlation_id: issue.correlation_id,
                                summary,
                                details,
                                severity,
                                script_path: script_path.clone(),
                            };

                            if tx.send_blocking(prompt_msg).is_err() {
                                tracing::warn!(
                                    correlation_id = %correlation_id,
                                    script_path = %script_path,
                                    "Prompt channel closed, dropping protocol error"
                                );
                            }
                        }) {
                            Ok(Some(msg)) => {
                                logging::log("EXEC", &format!("Received message: {:?}", msg));

                                // First, try to handle selected text messages directly (no UI needed)
                                match executor::handle_selected_text_message(&msg) {
                                    executor::SelectedTextHandleResult::Handled(response) => {
                                        logging::log("EXEC", &format!("Handled selected text message, sending response: {:?}", response));
                                        if let Err(e) = reader_response_tx.send(response) {
                                            logging::log(
                                                "EXEC",
                                                &format!(
                                                    "Failed to send selected text response: {}",
                                                    e
                                                ),
                                            );
                                        }
                                        continue;
                                    }
                                    executor::SelectedTextHandleResult::NotHandled => {
                                        // Fall through to other message handling
                                    }
                                }

                                // Handle ClipboardHistory directly (no UI needed)
                                if let Message::ClipboardHistory {
                                    request_id,
                                    action,
                                    entry_id,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!("ClipboardHistory request: {:?}", action),
                                    );

                                    let response = match action {
                                        protocol::ClipboardHistoryAction::List => {
                                            let entries =
                                                clipboard_history::get_clipboard_history(100);
                                            let entry_data: Vec<protocol::ClipboardHistoryEntryData> = entries
                                                .into_iter()
                                                .map(|e| {
                                                    // Truncate large content to avoid pipe buffer issues
                                                    // Images are stored as base64 which can be huge
                                                    let content = match e.content_type {
                                                        clipboard_history::ContentType::Image => {
                                                            // For images, send a placeholder with metadata
                                                            format!("[image:{}]", e.id)
                                                        }
                                                        clipboard_history::ContentType::Text => {
                                                            // Truncate very long text entries
                                                            if e.content.len() > 1000 {
                                                                format!("{}...", &e.content[..1000])
                                                            } else {
                                                                e.content
                                                            }
                                                        }
                                                    };
                                                    protocol::ClipboardHistoryEntryData {
                                                        entry_id: e.id,
                                                        content,
                                                        content_type: match e.content_type {
                                                            clipboard_history::ContentType::Text => protocol::ClipboardEntryType::Text,
                                                            clipboard_history::ContentType::Image => protocol::ClipboardEntryType::Image,
                                                        },
                                                        timestamp: chrono::DateTime::from_timestamp(e.timestamp, 0)
                                                            .map(|dt| dt.to_rfc3339())
                                                            .unwrap_or_default(),
                                                        pinned: e.pinned,
                                                    }
                                                })
                                                .collect();
                                            Message::clipboard_history_list_response(
                                                request_id.clone(),
                                                entry_data,
                                            )
                                        }
                                        protocol::ClipboardHistoryAction::Pin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::pin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Unpin => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::unpin_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Remove => {
                                            if let Some(id) = entry_id {
                                                match clipboard_history::remove_entry(id) {
                                                    Ok(()) => Message::clipboard_history_success(
                                                        request_id.clone(),
                                                    ),
                                                    Err(e) => Message::clipboard_history_error(
                                                        request_id.clone(),
                                                        e.to_string(),
                                                    ),
                                                }
                                            } else {
                                                Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    "Missing entry_id".to_string(),
                                                )
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::Clear => {
                                            match clipboard_history::clear_history() {
                                                Ok(()) => Message::clipboard_history_success(
                                                    request_id.clone(),
                                                ),
                                                Err(e) => Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    e.to_string(),
                                                ),
                                            }
                                        }
                                        protocol::ClipboardHistoryAction::TrimOversize => {
                                            match clipboard_history::trim_oversize_text_entries() {
                                                Ok(_) => Message::clipboard_history_success(
                                                    request_id.clone(),
                                                ),
                                                Err(e) => Message::clipboard_history_error(
                                                    request_id.clone(),
                                                    e.to_string(),
                                                ),
                                            }
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send clipboard history response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle Clipboard read/write directly (no UI needed)
                                if let Message::Clipboard {
                                    id,
                                    action,
                                    format,
                                    content,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "Clipboard request: {:?} format: {:?}",
                                            action, format
                                        ),
                                    );

                                    // If no request ID, we can't send a response, so just handle and continue
                                    let req_id = match id {
                                        Some(rid) => rid.clone(),
                                        None => {
                                            // Handle clipboard operation without response
                                            if let protocol::ClipboardAction::Write = action {
                                                if let Some(text) = content {
                                                    use arboard::Clipboard;
                                                    if let Ok(mut clipboard) = Clipboard::new() {
                                                        let _ = clipboard.set_text(text.clone());
                                                    }
                                                }
                                            }
                                            continue;
                                        }
                                    };

                                    let response = match action {
                                        protocol::ClipboardAction::Read => {
                                            // Read from clipboard
                                            match format {
                                                Some(protocol::ClipboardFormat::Text) | None => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_text() {
                                                                Ok(text) => Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(text),
                                                                },
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read error: {}", e));
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(String::new()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log(
                                                                "EXEC",
                                                                &format!(
                                                                    "Clipboard init error: {}",
                                                                    e
                                                                ),
                                                            );
                                                            Message::Submit {
                                                                id: req_id,
                                                                value: Some(String::new()),
                                                            }
                                                        }
                                                    }
                                                }
                                                Some(protocol::ClipboardFormat::Image) => {
                                                    use arboard::Clipboard;
                                                    match Clipboard::new() {
                                                        Ok(mut clipboard) => {
                                                            match clipboard.get_image() {
                                                                Ok(img) => {
                                                                    // Convert image to base64
                                                                    use base64::Engine;
                                                                    let bytes = img.bytes.to_vec();
                                                                    let base64_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(base64_str),
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    logging::log("EXEC", &format!("Clipboard read image error: {}", e));
                                                                    Message::Submit {
                                                                        id: req_id,
                                                                        value: Some(String::new()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            logging::log(
                                                                "EXEC",
                                                                &format!(
                                                                    "Clipboard init error: {}",
                                                                    e
                                                                ),
                                                            );
                                                            Message::Submit {
                                                                id: req_id,
                                                                value: Some(String::new()),
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        protocol::ClipboardAction::Write => {
                                            // Write to clipboard
                                            use arboard::Clipboard;
                                            match Clipboard::new() {
                                                Ok(mut clipboard) => {
                                                    if let Some(text) = content {
                                                        match clipboard.set_text(text.clone()) {
                                                            Ok(()) => {
                                                                logging::log("EXEC", &format!("Clipboard write success: {} bytes", text.len()));
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some("ok".to_string()),
                                                                }
                                                            }
                                                            Err(e) => {
                                                                logging::log(
                                                                    "EXEC",
                                                                    &format!(
                                                                        "Clipboard write error: {}",
                                                                        e
                                                                    ),
                                                                );
                                                                Message::Submit {
                                                                    id: req_id,
                                                                    value: Some(String::new()),
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        logging::log(
                                                            "EXEC",
                                                            "Clipboard write: no content provided",
                                                        );
                                                        Message::Submit {
                                                            id: req_id,
                                                            value: Some(String::new()),
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    logging::log(
                                                        "EXEC",
                                                        &format!("Clipboard init error: {}", e),
                                                    );
                                                    Message::Submit {
                                                        id: req_id,
                                                        value: Some(String::new()),
                                                    }
                                                }
                                            }
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send clipboard response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle WindowList directly (no UI needed)
                                if let Message::WindowList { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("WindowList request: {}", request_id),
                                    );

                                    let response = match window_control::list_windows() {
                                        Ok(windows) => {
                                            let window_infos: Vec<protocol::SystemWindowInfo> =
                                                windows
                                                    .into_iter()
                                                    .map(|w| protocol::SystemWindowInfo {
                                                        window_id: w.id,
                                                        title: w.title,
                                                        app_name: w.app,
                                                        bounds: Some(
                                                            protocol::TargetWindowBounds {
                                                                x: w.bounds.x,
                                                                y: w.bounds.y,
                                                                width: w.bounds.width,
                                                                height: w.bounds.height,
                                                            },
                                                        ),
                                                        is_minimized: None,
                                                        is_active: None,
                                                    })
                                                    .collect();
                                            Message::window_list_result(
                                                request_id.clone(),
                                                window_infos,
                                            )
                                        }
                                        Err(e) => {
                                            logging::log(
                                                "EXEC",
                                                &format!("WindowList error: {}", e),
                                            );
                                            // Return empty list on error
                                            Message::window_list_result(request_id.clone(), vec![])
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send window list response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle WindowAction directly (no UI needed)
                                if let Message::WindowAction {
                                    request_id,
                                    action,
                                    window_id,
                                    bounds,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "WindowAction request: {:?} for window {:?}",
                                            action, window_id
                                        ),
                                    );

                                    let result = match action {
                                        protocol::WindowActionType::Focus => {
                                            if let Some(id) = window_id {
                                                window_control::focus_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Close => {
                                            if let Some(id) = window_id {
                                                window_control::close_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Minimize => {
                                            if let Some(id) = window_id {
                                                window_control::minimize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Maximize => {
                                            if let Some(id) = window_id {
                                                window_control::maximize_window(*id)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id"))
                                            }
                                        }
                                        protocol::WindowActionType::Resize => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::resize_window(
                                                    *id, b.width, b.height,
                                                )
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                        protocol::WindowActionType::Move => {
                                            if let (Some(id), Some(b)) = (window_id, bounds) {
                                                window_control::move_window(*id, b.x, b.y)
                                            } else {
                                                Err(anyhow::anyhow!("Missing window_id or bounds"))
                                            }
                                        }
                                    };

                                    let response = match result {
                                        Ok(()) => {
                                            Message::window_action_success(request_id.clone())
                                        }
                                        Err(e) => Message::window_action_error(
                                            request_id.clone(),
                                            e.to_string(),
                                        ),
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send window action response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle FileSearch directly (no UI needed)
                                if let Message::FileSearch {
                                    request_id,
                                    query,
                                    only_in,
                                } = &msg
                                {
                                    logging::log(
                                        "EXEC",
                                        &format!(
                                            "FileSearch request: query='{}', only_in={:?}",
                                            query, only_in
                                        ),
                                    );

                                    let results = file_search::search_files(
                                        query,
                                        only_in.as_deref(),
                                        file_search::DEFAULT_LIMIT,
                                    );
                                    let file_entries: Vec<protocol::FileSearchResultEntry> =
                                        results
                                            .into_iter()
                                            .map(|f| protocol::FileSearchResultEntry {
                                                path: f.path,
                                                name: f.name,
                                                is_directory: f.file_type
                                                    == file_search::FileType::Directory,
                                                size: Some(f.size),
                                                modified_at: chrono::DateTime::from_timestamp(
                                                    f.modified as i64,
                                                    0,
                                                )
                                                .map(|dt| dt.to_rfc3339()),
                                            })
                                            .collect();

                                    let response = Message::file_search_result(
                                        request_id.clone(),
                                        file_entries,
                                    );

                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!("Failed to send file search response: {}", e),
                                        );
                                    }
                                    continue;
                                }

                                // Handle GetWindowBounds directly (no UI needed)
                                if let Message::GetWindowBounds { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("GetWindowBounds request: {}", request_id),
                                    );

                                    #[cfg(target_os = "macos")]
                                    let bounds_json = {
                                        if let Some(window) = window_manager::get_main_window() {
                                            unsafe {
                                                // Get the window frame
                                                let frame: NSRect = msg_send![window, frame];

                                                // Get the PRIMARY screen's height for coordinate conversion
                                                // macOS uses bottom-left origin, we convert to top-left
                                                let screens: id =
                                                    msg_send![class!(NSScreen), screens];
                                                let main_screen: id =
                                                    msg_send![screens, firstObject];
                                                let main_screen_frame: NSRect =
                                                    msg_send![main_screen, frame];
                                                let primary_screen_height =
                                                    main_screen_frame.size.height;

                                                // Convert from bottom-left origin (macOS) to top-left origin
                                                let flipped_y = primary_screen_height
                                                    - frame.origin.y
                                                    - frame.size.height;

                                                logging::log("EXEC", &format!(
                                                    "Window bounds: x={:.0}, y={:.0}, width={:.0}, height={:.0}",
                                                    frame.origin.x, flipped_y, frame.size.width, frame.size.height
                                                ));

                                                // Create JSON string with bounds
                                                format!(
                                                    r#"{{"x":{},"y":{},"width":{},"height":{}}}"#,
                                                    frame.origin.x as f64,
                                                    flipped_y as f64,
                                                    frame.size.width as f64,
                                                    frame.size.height as f64
                                                )
                                            }
                                        } else {
                                            logging::log(
                                                "EXEC",
                                                "GetWindowBounds: Main window not registered",
                                            );
                                            r#"{"error":"Main window not found"}"#.to_string()
                                        }
                                    };

                                    #[cfg(not(target_os = "macos"))]
                                    let bounds_json =
                                        r#"{"error":"Not supported on this platform"}"#.to_string();

                                    let response = Message::Submit {
                                        id: request_id.clone(),
                                        value: Some(bounds_json),
                                    };
                                    logging::log(
                                        "EXEC",
                                        &format!("Sending window bounds response: {:?}", response),
                                    );
                                    if let Err(e) = reader_response_tx.send(response) {
                                        logging::log(
                                            "EXEC",
                                            &format!(
                                                "Failed to send window bounds response: {}",
                                                e
                                            ),
                                        );
                                    }
                                    continue;
                                }

                                // Handle GetState - needs UI state, forward to UI thread
                                if let Message::GetState { request_id } = &msg {
                                    logging::log(
                                        "EXEC",
                                        &format!("GetState request: {}", request_id),
                                    );
                                    let prompt_msg = PromptMessage::GetState {
                                        request_id: request_id.clone(),
                                    };
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log(
                                            "EXEC",
                                            "Prompt channel closed, reader exiting",
                                        );
                                        break;
                                    }
                                    continue;
                                }

                                // Handle CaptureScreenshot directly (no UI needed)
                                if let Message::CaptureScreenshot { request_id, hi_dpi } = &msg {
                                    let hi_dpi_mode = hi_dpi.unwrap_or(false);
                                    tracing::info!(request_id = %request_id, hi_dpi = hi_dpi_mode, "Capturing screenshot");

                                    let response = match capture_app_screenshot(hi_dpi_mode) {
                                        Ok((png_data, width, height)) => {
                                            use base64::Engine;
                                            let base64_data =
                                                base64::engine::general_purpose::STANDARD
                                                    .encode(&png_data);
                                            tracing::info!(
                                                request_id = %request_id,
                                                width = width,
                                                height = height,
                                                hi_dpi = hi_dpi_mode,
                                                data_len = base64_data.len(),
                                                "Screenshot captured successfully"
                                            );
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                base64_data,
                                                width,
                                                height,
                                            )
                                        }
                                        Err(e) => {
                                            tracing::error!(
                                                request_id = %request_id,
                                                error = %e,
                                                "Screenshot capture failed"
                                            );
                                            // Send empty result on error
                                            Message::screenshot_result(
                                                request_id.clone(),
                                                String::new(),
                                                0,
                                                0,
                                            )
                                        }
                                    };

                                    if let Err(e) = reader_response_tx.send(response) {
                                        tracing::error!(error = %e, "Failed to send screenshot response");
                                    }
                                    continue;
                                }

                                let prompt_msg = match msg {
                                    Message::Arg {
                                        id,
                                        placeholder,
                                        choices,
                                        actions,
                                    } => Some(PromptMessage::ShowArg {
                                        id,
                                        placeholder,
                                        choices,
                                        actions,
                                    }),
                                    Message::Div {
                                        id,
                                        html,
                                        container_classes,
                                        actions,
                                        placeholder,
                                        hint,
                                        footer,
                                        container_bg,
                                        container_padding,
                                        opacity,
                                    } => Some(PromptMessage::ShowDiv {
                                        id,
                                        html,
                                        container_classes,
                                        actions,
                                        placeholder,
                                        hint,
                                        footer,
                                        container_bg,
                                        container_padding,
                                        opacity,
                                    }),
                                    Message::Form { id, html, actions } => {
                                        Some(PromptMessage::ShowForm { id, html, actions })
                                    }
                                    Message::Term {
                                        id,
                                        command,
                                        actions,
                                    } => Some(PromptMessage::ShowTerm {
                                        id,
                                        command,
                                        actions,
                                    }),
                                    Message::Editor {
                                        id,
                                        content,
                                        language,
                                        template,
                                        actions,
                                        ..
                                    } => Some(PromptMessage::ShowEditor {
                                        id,
                                        content,
                                        language,
                                        template,
                                        actions,
                                    }),
                                    // New prompt types (scaffolding)
                                    Message::Path {
                                        id,
                                        start_path,
                                        hint,
                                    } => Some(PromptMessage::ShowPath {
                                        id,
                                        start_path,
                                        hint,
                                    }),
                                    Message::Env { id, key, secret } => {
                                        Some(PromptMessage::ShowEnv {
                                            id,
                                            key,
                                            prompt: None,
                                            secret: secret.unwrap_or(false),
                                        })
                                    }
                                    Message::Drop { id } => Some(PromptMessage::ShowDrop {
                                        id,
                                        placeholder: None,
                                        hint: None,
                                    }),
                                    Message::Template { id, template } => {
                                        Some(PromptMessage::ShowTemplate { id, template })
                                    }
                                    Message::Select {
                                        id,
                                        placeholder,
                                        choices,
                                        multiple,
                                    } => Some(PromptMessage::ShowSelect {
                                        id,
                                        placeholder: Some(placeholder),
                                        choices,
                                        multiple: multiple.unwrap_or(false),
                                    }),
                                    Message::Exit { .. } => Some(PromptMessage::ScriptExit),
                                    Message::ForceSubmit { value } => {
                                        Some(PromptMessage::ForceSubmit { value })
                                    }
                                    Message::Hide {} => Some(PromptMessage::HideWindow),
                                    Message::Browse { url } => {
                                        Some(PromptMessage::OpenBrowser { url })
                                    }
                                    Message::Hud { text, duration_ms } => {
                                        Some(PromptMessage::ShowHud { text, duration_ms })
                                    }
                                    Message::SetActions { actions } => {
                                        Some(PromptMessage::SetActions { actions })
                                    }
                                    Message::SetInput { text } => {
                                        Some(PromptMessage::SetInput { text })
                                    }
                                    other => {
                                        // Get the message type name for user feedback
                                        let msg_type = format!("{:?}", other);
                                        // Extract just the variant name (before any {})
                                        let type_name = msg_type
                                            .split('{')
                                            .next()
                                            .unwrap_or(&msg_type)
                                            .trim()
                                            .to_string();
                                        logging::log(
                                            "WARN",
                                            &format!("Unhandled message type: {}", type_name),
                                        );
                                        Some(PromptMessage::UnhandledMessage {
                                            message_type: type_name,
                                        })
                                    }
                                };

                                if let Some(prompt_msg) = prompt_msg {
                                    if tx.send_blocking(prompt_msg).is_err() {
                                        logging::log(
                                            "EXEC",
                                            "Prompt channel closed, reader exiting",
                                        );
                                        break;
                                    }
                                }
                            }
                            Ok(None) => {
                                logging::log("EXEC", "Script stdout closed (EOF)");

                                // Check if process exited with error
                                let exit_code = match keep_alive_child.try_wait() {
                                    Ok(Some(status)) => status.code(),
                                    Ok(None) => {
                                        // Process still running, wait for it
                                        match keep_alive_child.wait() {
                                            Ok(status) => status.code(),
                                            Err(_) => None,
                                        }
                                    }
                                    Err(_) => None,
                                };

                                logging::log("EXEC", &format!("Script exit code: {:?}", exit_code));

                                // If non-zero exit code, capture stderr and send error
                                if let Some(code) = exit_code {
                                    if code != 0 {
                                        // Read stderr if available
                                        let stderr_output =
                                            if let Some(mut stderr) = stderr_for_errors.take() {
                                                use std::io::Read;
                                                let mut stderr_str = String::new();
                                                if stderr.read_to_string(&mut stderr_str).is_ok()
                                                    && !stderr_str.is_empty()
                                                {
                                                    Some(stderr_str)
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            };

                                        if let Some(ref stderr_text) = stderr_output {
                                            logging::log(
                                                "EXEC",
                                                &format!(
                                                    "Captured stderr ({} bytes)",
                                                    stderr_text.len()
                                                ),
                                            );

                                            // Parse error info and generate suggestions
                                            let error_message =
                                                executor::extract_error_message(stderr_text);
                                            let stack_trace =
                                                executor::parse_stack_trace(stderr_text);
                                            let suggestions = executor::generate_suggestions(
                                                stderr_text,
                                                Some(code),
                                            );

                                            // Send script error message
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message,
                                                stderr_output: Some(stderr_text.clone()),
                                                exit_code: Some(code),
                                                stack_trace,
                                                script_path: script_path.clone(),
                                                suggestions,
                                            });
                                        } else {
                                            // No stderr, send generic error
                                            let _ = tx.send_blocking(PromptMessage::ScriptError {
                                                error_message: format!(
                                                    "Script exited with code {}",
                                                    code
                                                ),
                                                stderr_output: None,
                                                exit_code: Some(code),
                                                stack_trace: None,
                                                script_path: script_path.clone(),
                                                suggestions: vec![
                                                    "Check the script for errors".to_string()
                                                ],
                                            });
                                        }
                                    }
                                }

                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                            Err(e) => {
                                logging::log("EXEC", &format!("Error reading from script: {}", e));

                                // Try to read stderr for error details
                                let stderr_output =
                                    if let Some(mut stderr) = stderr_for_errors.take() {
                                        use std::io::Read;
                                        let mut stderr_str = String::new();
                                        if stderr.read_to_string(&mut stderr_str).is_ok()
                                            && !stderr_str.is_empty()
                                        {
                                            Some(stderr_str)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                if let Some(ref stderr_text) = stderr_output {
                                    let error_message =
                                        executor::extract_error_message(stderr_text);
                                    let stack_trace = executor::parse_stack_trace(stderr_text);
                                    let suggestions =
                                        executor::generate_suggestions(stderr_text, None);

                                    let _ = tx.send_blocking(PromptMessage::ScriptError {
                                        error_message,
                                        stderr_output: Some(stderr_text.clone()),
                                        exit_code: None,
                                        stack_trace,
                                        script_path: script_path.clone(),
                                        suggestions,
                                    });
                                }

                                let _ = tx.send_blocking(PromptMessage::ScriptExit);
                                break;
                            }
                        }
                    }
                    logging::log(
                        "EXEC",
                        "Reader thread exited, process handle will now be dropped",
                    );
                });

                // Store the response sender for the UI to use
                self.response_sender = Some(response_tx);
            }
            Err(e) => {
                logging::log(
                    "EXEC",
                    &format!("Failed to start interactive session: {}", e),
                );
                self.last_output = Some(SharedString::from(format!("✗ Error: {}", e)));
                cx.notify();
            }
        }
    }

    /// Execute a scriptlet (simple code snippet from .md file)
    fn execute_scriptlet(&mut self, scriptlet: &scripts::Scriptlet, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!(
                "Executing scriptlet: {} (tool: {})",
                scriptlet.name, scriptlet.tool
            ),
        );

        let tool = scriptlet.tool.to_lowercase();

        // TypeScript/Kit scriptlets need to run interactively (they may use SDK prompts)
        // These should be spawned like regular scripts, not run synchronously
        if matches!(tool.as_str(), "kit" | "ts" | "bun" | "deno" | "js") {
            logging::log(
                "EXEC",
                &format!(
                    "TypeScript scriptlet '{}' - running interactively",
                    scriptlet.name
                ),
            );

            // Write scriptlet content to a temp file
            let temp_dir = std::env::temp_dir();
            let temp_file = temp_dir.join(format!(
                "scriptlet-{}-{}.ts",
                scriptlet.name.to_lowercase().replace(' ', "-"),
                std::process::id()
            ));

            if let Err(e) = std::fs::write(&temp_file, &scriptlet.code) {
                logging::log(
                    "ERROR",
                    &format!("Failed to write temp scriptlet file: {}", e),
                );
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to write scriptlet: {}", e),
                        &self.theme,
                    )
                    .duration_ms(Some(5000)),
                );
                cx.notify();
                return;
            }

            // Create a Script struct and run it interactively
            let script = scripts::Script {
                name: scriptlet.name.clone(),
                description: scriptlet.description.clone(),
                path: temp_file,
                extension: "ts".to_string(),
                icon: None,
                alias: None,
                shortcut: None,
                typed_metadata: None,
                schema: None,
            };

            self.execute_interactive(&script, cx);
            return;
        }

        // For non-TypeScript tools (bash, python, etc.), run synchronously
        // These don't use the SDK and won't block waiting for input

        // Convert scripts::Scriptlet to scriptlets::Scriptlet for executor
        let exec_scriptlet = scriptlets::Scriptlet {
            name: scriptlet.name.clone(),
            command: scriptlet.command.clone().unwrap_or_else(|| {
                // Generate command slug from name if not present
                scriptlet.name.to_lowercase().replace(' ', "-")
            }),
            tool: scriptlet.tool.clone(),
            scriptlet_content: scriptlet.code.clone(),
            inputs: vec![], // TODO: Parse inputs from code if needed
            group: scriptlet.group.clone().unwrap_or_default(),
            preview: None,
            metadata: scriptlets::ScriptletMetadata {
                shortcut: scriptlet.shortcut.clone(),
                expand: scriptlet.expand.clone(),
                description: scriptlet.description.clone(),
                ..Default::default()
            },
            typed_metadata: None,
            schema: None,
            kenv: None,
            source_path: scriptlet.file_path.clone(),
        };

        // Execute with default options (no inputs for now)
        let options = executor::ScriptletExecOptions::default();

        match executor::run_scriptlet(&exec_scriptlet, options) {
            Ok(result) => {
                if result.success {
                    logging::log(
                        "EXEC",
                        &format!(
                            "Scriptlet '{}' succeeded: exit={}",
                            scriptlet.name, result.exit_code
                        ),
                    );

                    // Store output if any
                    if !result.stdout.is_empty() {
                        self.last_output = Some(SharedString::from(result.stdout.clone()));
                    }

                    // Hide window after successful execution
                    WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                    cx.hide();
                } else {
                    // Execution failed (non-zero exit code)
                    let error_msg = if !result.stderr.is_empty() {
                        result.stderr.clone()
                    } else {
                        format!("Exit code: {}", result.exit_code)
                    };

                    logging::log(
                        "ERROR",
                        &format!("Scriptlet '{}' failed: {}", scriptlet.name, error_msg),
                    );

                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("Scriptlet failed: {}", error_msg),
                            &self.theme,
                        )
                        .duration_ms(Some(5000)),
                    );
                    cx.notify();
                }
            }
            Err(e) => {
                logging::log(
                    "ERROR",
                    &format!("Failed to execute scriptlet '{}': {}", scriptlet.name, e),
                );

                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to execute: {}", e),
                        &self.theme,
                    )
                    .duration_ms(Some(5000)),
                );
                cx.notify();
            }
        }
    }

    /// Execute a script or scriptlet by its file path
    /// Used by global shortcuts to directly invoke scripts
    fn execute_script_by_path(&mut self, path: &str, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Executing script by path: {}", path));

        // Check if it's a scriptlet (contains #)
        if path.contains('#') {
            // It's a scriptlet path like "/path/to/file.md#command"
            if let Some(scriptlet) = self
                .scriptlets
                .iter()
                .find(|s| s.file_path.as_ref().map(|p| p == path).unwrap_or(false))
            {
                let scriptlet_clone = scriptlet.clone();
                self.execute_scriptlet(&scriptlet_clone, cx);
                return;
            }
            logging::log("ERROR", &format!("Scriptlet not found: {}", path));
            return;
        }

        // It's a regular script - find by path
        if let Some(script) = self
            .scripts
            .iter()
            .find(|s| s.path.to_string_lossy() == path)
        {
            let script_clone = script.clone();
            self.execute_interactive(&script_clone, cx);
            return;
        }

        // Not found in loaded scripts - try to execute directly as a file
        let script_path = std::path::PathBuf::from(path);
        if script_path.exists() {
            let name = script_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("script")
                .to_string();

            let script = scripts::Script {
                name,
                path: script_path.clone(),
                extension: script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string(),
                description: None,
                icon: None,
                alias: None,
                shortcut: None,
                typed_metadata: None,
                schema: None,
            };

            self.execute_interactive(&script, cx);
        } else {
            logging::log("ERROR", &format!("Script file not found: {}", path));
        }
    }

    /// Execute a built-in feature
    fn execute_builtin(&mut self, entry: &builtins::BuiltInEntry, cx: &mut Context<Self>) {
        logging::log(
            "EXEC",
            &format!("Executing built-in: {} (id: {})", entry.name, entry.id),
        );

        match &entry.feature {
            builtins::BuiltInFeature::ClipboardHistory => {
                logging::log("EXEC", "Opening Clipboard History");
                // Use cached entries for faster loading
                let entries = clipboard_history::get_cached_entries(100);
                logging::log(
                    "EXEC",
                    &format!("Loaded {} clipboard entries (cached)", entries.len()),
                );
                // Initial selected_index should be 0 (first entry)
                // Note: clipboard history uses a flat list without section headers
                self.current_view = AppView::ClipboardHistoryView {
                    entries,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for clipboard history view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::AppLauncher => {
                logging::log("EXEC", "Opening App Launcher");
                let apps = app_launcher::scan_applications().clone();
                logging::log("EXEC", &format!("Loaded {} applications", apps.len()));
                self.current_view = AppView::AppLauncherView {
                    apps,
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for app launcher view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
            builtins::BuiltInFeature::App(app_name) => {
                logging::log("EXEC", &format!("Launching app: {}", app_name));
                // Find and launch the specific application
                let apps = app_launcher::scan_applications();
                if let Some(app) = apps.iter().find(|a| a.name == *app_name) {
                    if let Err(e) = app_launcher::launch_application(app) {
                        logging::log("ERROR", &format!("Failed to launch {}: {}", app_name, e));
                        self.last_output = Some(SharedString::from(format!(
                            "Failed to launch: {}",
                            app_name
                        )));
                    } else {
                        logging::log("EXEC", &format!("Launched app: {}", app_name));
                        // Hide window after launching app and set reset flag
                        // so filter_text is cleared when window is shown again
                        WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                        NEEDS_RESET.store(true, Ordering::SeqCst);
                        cx.hide();
                    }
                } else {
                    logging::log("ERROR", &format!("App not found: {}", app_name));
                    self.last_output =
                        Some(SharedString::from(format!("App not found: {}", app_name)));
                }
                cx.notify();
            }
            builtins::BuiltInFeature::WindowSwitcher => {
                logging::log("EXEC", "Opening Window Switcher");
                // Load windows when view is opened (windows change frequently)
                match window_control::list_windows() {
                    Ok(windows) => {
                        logging::log("EXEC", &format!("Loaded {} windows", windows.len()));
                        self.current_view = AppView::WindowSwitcherView {
                            windows,
                            filter: String::new(),
                            selected_index: 0,
                        };
                        // Use standard height for window switcher view
                        defer_resize_to_view(ViewType::ScriptList, 0, cx);
                    }
                    Err(e) => {
                        logging::log("ERROR", &format!("Failed to list windows: {}", e));
                        self.toast_manager.push(
                            components::toast::Toast::error(
                                format!("Failed to list windows: {}", e),
                                &self.theme,
                            )
                            .duration_ms(Some(5000)),
                        );
                    }
                }
                cx.notify();
            }
            builtins::BuiltInFeature::DesignGallery => {
                logging::log("EXEC", "Opening Design Gallery");
                self.current_view = AppView::DesignGalleryView {
                    filter: String::new(),
                    selected_index: 0,
                };
                // Use standard height for design gallery view
                defer_resize_to_view(ViewType::ScriptList, 0, cx);
                cx.notify();
            }
        }
    }

    /// Execute an application directly from the main search results
    fn execute_app(&mut self, app: &app_launcher::AppInfo, cx: &mut Context<Self>) {
        logging::log("EXEC", &format!("Launching app from search: {}", app.name));

        if let Err(e) = app_launcher::launch_application(app) {
            logging::log("ERROR", &format!("Failed to launch {}: {}", app.name, e));
            self.last_output = Some(SharedString::from(format!(
                "Failed to launch: {}",
                app.name
            )));
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Launched app: {}", app.name));
            // Hide window after launching app and set reset flag
            // so filter_text is cleared when window is shown again
            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
            NEEDS_RESET.store(true, Ordering::SeqCst);
            cx.hide();
        }
    }

    /// Focus a window from the main search results
    fn execute_window_focus(
        &mut self,
        window: &window_control::WindowInfo,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "EXEC",
            &format!("Focusing window: {} - {}", window.app, window.title),
        );

        if let Err(e) = window_control::focus_window(window.id) {
            logging::log("ERROR", &format!("Failed to focus window: {}", e));
            self.toast_manager.push(
                components::toast::Toast::error(
                    format!("Failed to focus window: {}", e),
                    &self.theme,
                )
                .duration_ms(Some(5000)),
            );
            cx.notify();
        } else {
            logging::log("EXEC", &format!("Focused window: {}", window.title));
            // Hide Script Kit after focusing window and set reset flag
            // so filter_text is cleared when window is shown again
            WINDOW_VISIBLE.store(false, Ordering::SeqCst);
            NEEDS_RESET.store(true, Ordering::SeqCst);
            cx.hide();
        }
    }

    /// Handle a prompt message from the script
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        match msg {
            PromptMessage::ShowArg {
                id,
                placeholder,
                choices,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing arg prompt: {} with {} choices, {} actions",
                        id,
                        choices.len(),
                        actions.as_ref().map(|a| a.len()).unwrap_or(0)
                    ),
                );
                let choice_count = choices.len();

                // If actions were provided, store them in the SDK actions system
                // so they can be triggered via shortcuts and Cmd+K
                if let Some(ref action_list) = actions {
                    // Store SDK actions for trigger_action_by_name lookup
                    self.sdk_actions = Some(action_list.clone());

                    // Register keyboard shortcuts for SDK actions
                    self.action_shortcuts.clear();
                    for action in action_list {
                        if let Some(shortcut) = &action.shortcut {
                            self.action_shortcuts.insert(
                                shortcuts::normalize_shortcut(shortcut),
                                action.name.clone(),
                            );
                        }
                    }
                } else {
                    // Clear any previous SDK actions
                    self.sdk_actions = None;
                    self.action_shortcuts.clear();
                }

                self.current_view = AppView::ArgPrompt {
                    id,
                    placeholder,
                    choices,
                    actions,
                };
                self.arg_input_text.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                defer_resize_to_view(view_type, choice_count, cx);
                cx.notify();
            }
            PromptMessage::ShowDiv {
                id,
                html,
                container_classes,
                actions,
                placeholder: _placeholder, // TODO: render in header
                hint: _hint,               // TODO: render hint
                footer: _footer,           // TODO: render footer
                container_bg,
                container_padding,
                opacity,
            } => {
                logging::log("UI", &format!("Showing div prompt: {}", id));
                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for div prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send div response: {}", e));
                            }
                        }
                    });

                // Create focus handle for div prompt
                let div_focus_handle = cx.focus_handle();

                // Build container options from protocol message
                let container_options = ContainerOptions {
                    background: container_bg,
                    padding: container_padding.and_then(|v| {
                        if v.is_string() && v.as_str() == Some("none") {
                            Some(ContainerPadding::None)
                        } else if let Some(n) = v.as_f64() {
                            Some(ContainerPadding::Pixels(n as f32))
                        } else {
                            v.as_i64().map(|n| ContainerPadding::Pixels(n as f32))
                        }
                    }),
                    opacity,
                    container_classes,
                };

                // Create DivPrompt entity with proper HTML rendering
                let div_prompt = DivPrompt::with_options(
                    id.clone(),
                    html,
                    None, // tailwind param deprecated - use container_classes in options
                    div_focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    crate::designs::DesignVariant::Default,
                    container_options,
                );

                let entity = cx.new(|_| div_prompt);
                self.current_view = AppView::DivPrompt { id, entity };
                self.focused_input = FocusedInput::None; // DivPrompt has no text input
                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowForm { id, html, actions } => {
                logging::log("UI", &format!("Showing form prompt: {}", id));

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create form field colors from theme
                let colors = FormFieldColors::from_theme(&self.theme);

                // Create FormPromptState entity with parsed fields
                let form_state = FormPromptState::new(id.clone(), html, colors, cx);
                let field_count = form_state.fields.len();
                let entity = cx.new(|_| form_state);

                self.current_view = AppView::FormPrompt { id, entity };
                self.focused_input = FocusedInput::None; // FormPrompt has its own focus handling

                // Resize based on field count (more fields = taller window)
                let view_type = if field_count > 0 {
                    ViewType::ArgPromptWithChoices
                } else {
                    ViewType::DivPrompt
                };
                defer_resize_to_view(view_type, field_count, cx);
                cx.notify();
            }
            PromptMessage::ShowTerm {
                id,
                command,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!("Showing term prompt: {} (command: {:?})", id, command),
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for terminal
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send terminal response: {}", e),
                                );
                            }
                        }
                    });

                // Get the target height for terminal view
                let term_height = window_resize::layout::MAX_HEIGHT;

                // Create terminal with explicit height - GPUI entities don't inherit parent flex sizing
                match term_prompt::TermPrompt::with_height(
                    id.clone(),
                    command,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                    std::sync::Arc::new(self.config.clone()),
                    Some(term_height),
                ) {
                    Ok(term_prompt) => {
                        let entity = cx.new(|_| term_prompt);
                        self.current_view = AppView::TermPrompt { id, entity };
                        self.focused_input = FocusedInput::None; // Terminal handles its own cursor
                        defer_resize_to_view(ViewType::TermPrompt, 0, cx);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to create terminal");
                        logging::log("ERROR", &format!("Failed to create terminal: {}", e));
                    }
                }
            }
            PromptMessage::ShowEditor {
                id,
                content,
                language,
                template,
                actions,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing editor prompt: {} (language: {:?}, template: {})",
                        id,
                        language,
                        template.is_some()
                    ),
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for editor
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send editor response: {}", e),
                                );
                            }
                        }
                    });

                // CRITICAL: Create a SEPARATE focus handle for the editor.
                // Using the parent's focus handle causes keyboard event routing issues
                // because the parent checks is_focused() in its render and both parent
                // and child would be tracking the same handle.
                let editor_focus_handle = cx.focus_handle();

                // Get the target height for editor view
                let editor_height = window_resize::layout::MAX_HEIGHT;

                // Create editor: use with_template if template provided, otherwise with_height
                let editor_prompt = if let Some(template_str) = template {
                    EditorPrompt::with_template(
                        id.clone(),
                        template_str,
                        language.unwrap_or_else(|| "plaintext".to_string()),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::new(self.theme.clone()),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                } else {
                    EditorPrompt::with_height(
                        id.clone(),
                        content.unwrap_or_default(),
                        language.unwrap_or_else(|| "markdown".to_string()),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::new(self.theme.clone()),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                };

                let entity = cx.new(|_| editor_prompt);
                self.current_view = AppView::EditorPrompt {
                    id,
                    entity,
                    focus_handle: editor_focus_handle,
                };
                self.focused_input = FocusedInput::None; // Editor handles its own focus

                defer_resize_to_view(ViewType::EditorPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ScriptExit => {
                logging::log("VISIBILITY", "=== ScriptExit message received ===");
                let was_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                logging::log(
                    "VISIBILITY",
                    &format!("WINDOW_VISIBLE was: {}", was_visible),
                );

                // CRITICAL: Update visibility state so hotkey toggle works correctly
                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");

                self.reset_to_script_list(cx);
                logging::log("VISIBILITY", "reset_to_script_list() called");

                // Hide window when script completes - scripts only stay active while code is running
                cx.hide();
                logging::log(
                    "VISIBILITY",
                    "cx.hide() called - window hidden on script completion",
                );
            }
            PromptMessage::HideWindow => {
                logging::log("VISIBILITY", "=== HideWindow message received ===");
                let was_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                logging::log(
                    "VISIBILITY",
                    &format!("WINDOW_VISIBLE was: {}", was_visible),
                );

                // CRITICAL: Update visibility state so hotkey toggle works correctly
                WINDOW_VISIBLE.store(false, Ordering::SeqCst);
                logging::log("VISIBILITY", "WINDOW_VISIBLE set to: false");

                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                logging::log("VISIBILITY", "NEEDS_RESET set to: true");

                cx.hide();
                logging::log(
                    "VISIBILITY",
                    "cx.hide() called - window should now be hidden",
                );
            }
            PromptMessage::OpenBrowser { url } => {
                logging::log("UI", &format!("Opening browser: {}", url));
                #[cfg(target_os = "macos")]
                {
                    match std::process::Command::new("open").arg(&url).spawn() {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match std::process::Command::new("xdg-open").arg(&url).spawn() {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    match std::process::Command::new("cmd")
                        .args(["/C", "start", &url])
                        .spawn()
                    {
                        Ok(_) => logging::log(
                            "UI",
                            &format!("Successfully opened URL in browser: {}", url),
                        ),
                        Err(e) => {
                            logging::log("ERROR", &format!("Failed to open URL '{}': {}", url, e))
                        }
                    }
                }
            }
            PromptMessage::RunScript { path } => {
                logging::log("EXEC", &format!("RunScript command received: {}", path));

                // Create a Script struct from the path
                let script_path = std::path::PathBuf::from(&path);
                let script_name = script_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let extension = script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string();

                let script = scripts::Script {
                    name: script_name.clone(),
                    description: Some(format!("External script: {}", path)),
                    path: script_path,
                    extension,
                    icon: None,
                    alias: None,
                    shortcut: None,
                    typed_metadata: None,
                    schema: None,
                };

                logging::log("EXEC", &format!("Executing script: {}", script_name));
                self.execute_interactive(&script, cx);
            }
            PromptMessage::ScriptError {
                error_message,
                stderr_output,
                exit_code,
                stack_trace,
                script_path,
                suggestions,
            } => {
                logging::log(
                    "ERROR",
                    &format!(
                        "Script error received: {} (exit code: {:?})",
                        error_message, exit_code
                    ),
                );

                // Create error toast with expandable details
                // Use stderr_output if available, otherwise use stack_trace
                let details_text = stderr_output.clone().or_else(|| stack_trace.clone());
                let toast = Toast::error(error_message.clone(), &self.theme)
                    .details_opt(details_text.clone())
                    .duration_ms(Some(10000)); // 10 seconds for errors

                // Add copy button action if we have stderr/stack trace
                let toast = if let Some(ref trace) = details_text {
                    let trace_clone = trace.clone();
                    toast.action(ToastAction::new(
                        "Copy Error",
                        Box::new(move |_, _, _| {
                            // Copy to clipboard
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(trace_clone.clone());
                                logging::log("UI", "Error copied to clipboard");
                            }
                        }),
                    ))
                } else {
                    toast
                };

                // Log suggestions if present
                if !suggestions.is_empty() {
                    logging::log("ERROR", &format!("Suggestions: {:?}", suggestions));
                }

                // Push toast to manager
                let toast_id = self.toast_manager.push(toast);
                logging::log(
                    "UI",
                    &format!(
                        "Toast created for script error: {} (id: {})",
                        script_path, toast_id
                    ),
                );

                cx.notify();
            }
            PromptMessage::ProtocolError {
                correlation_id,
                summary,
                details,
                severity,
                script_path,
            } => {
                tracing::warn!(
                    correlation_id = %correlation_id,
                    script_path = %script_path,
                    summary = %summary,
                    "Protocol parse issue received"
                );

                let mut toast = Toast::from_severity(summary.clone(), severity, &self.theme)
                    .details_opt(details.clone())
                    .duration_ms(Some(8000));

                if let Some(ref detail_text) = details {
                    let detail_clone = detail_text.clone();
                    toast = toast.action(ToastAction::new(
                        "Copy Details",
                        Box::new(move |_, _, _| {
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(detail_clone.clone());
                            }
                        }),
                    ));
                }

                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::UnhandledMessage { message_type } => {
                logging::log(
                    "WARN",
                    &format!("Displaying unhandled message warning: {}", message_type),
                );

                let toast = Toast::warning(
                    format!("'{}' is not yet implemented", message_type),
                    &self.theme,
                )
                .duration_ms(Some(5000));

                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::GetState { request_id } => {
                logging::log(
                    "UI",
                    &format!("Collecting state for request: {}", request_id),
                );

                // Collect current UI state
                let (
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                ) = match &self.current_view {
                    AppView::ScriptList => {
                        let filtered_len = self.filtered_results().len();
                        let selected_value = if self.selected_index < filtered_len {
                            self.filtered_results()
                                .get(self.selected_index)
                                .map(|r| match r {
                                    scripts::SearchResult::Script(m) => m.script.name.clone(),
                                    scripts::SearchResult::Scriptlet(m) => m.scriptlet.name.clone(),
                                    scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
                                    scripts::SearchResult::App(m) => m.app.name.clone(),
                                    scripts::SearchResult::Window(m) => m.window.title.clone(),
                                })
                        } else {
                            None
                        };
                        (
                            "none".to_string(),
                            None,
                            None,
                            self.filter_text.clone(),
                            self.scripts.len()
                                + self.scriptlets.len()
                                + self.builtin_entries.len()
                                + self.apps.len(),
                            filtered_len,
                            self.selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ArgPrompt {
                        id,
                        placeholder,
                        choices,
                        actions: _,
                    } => {
                        let filtered = self.get_filtered_arg_choices(choices);
                        let selected_value = if self.arg_selected_index < filtered.len() {
                            filtered
                                .get(self.arg_selected_index)
                                .map(|c| c.value.clone())
                        } else {
                            None
                        };
                        (
                            "arg".to_string(),
                            Some(id.clone()),
                            Some(placeholder.clone()),
                            self.arg_input_text.clone(),
                            choices.len(),
                            filtered.len(),
                            self.arg_selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::DivPrompt { id, .. } => (
                        "div".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::FormPrompt { id, .. } => (
                        "form".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TermPrompt { id, .. } => (
                        "term".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EditorPrompt { id, .. } => (
                        "editor".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::SelectPrompt { id, .. } => (
                        "select".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::PathPrompt { id, .. } => (
                        "path".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EnvPrompt { id, .. } => (
                        "env".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::DropPrompt { id, .. } => (
                        "drop".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TemplatePrompt { id, .. } => (
                        "template".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::ActionsDialog => (
                        "actions".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::ClipboardHistoryView {
                        entries,
                        filter,
                        selected_index,
                    } => {
                        let filtered_count = if filter.is_empty() {
                            entries.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            entries
                                .iter()
                                .filter(|e| e.content.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "clipboardHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::AppLauncherView {
                        apps,
                        filter,
                        selected_index,
                    } => {
                        let filtered_count = if filter.is_empty() {
                            apps.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            apps.iter()
                                .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "appLauncher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            apps.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::WindowSwitcherView {
                        windows,
                        filter,
                        selected_index,
                    } => {
                        let filtered_count = if filter.is_empty() {
                            windows.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            windows
                                .iter()
                                .filter(|w| {
                                    w.title.to_lowercase().contains(&filter_lower)
                                        || w.app.to_lowercase().contains(&filter_lower)
                                })
                                .count()
                        };
                        (
                            "windowSwitcher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            windows.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::DesignGalleryView {
                        filter,
                        selected_index,
                    } => {
                        let total_items = designs::separator_variations::SeparatorStyle::count()
                            + designs::icon_variations::total_icon_count()
                            + 8
                            + 6; // headers
                        (
                            "designGallery".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total_items,
                            total_items,
                            *selected_index as i32,
                            None,
                        )
                    }
                };

                // Focus state: we use focused_input as a proxy since we don't have Window access here.
                // When window is visible and we're tracking an input, we're focused.
                let window_visible = WINDOW_VISIBLE.load(Ordering::SeqCst);
                let is_focused = window_visible && self.focused_input != FocusedInput::None;

                // Create the response
                let response = Message::state_result(
                    request_id.clone(),
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                    is_focused,
                    window_visible,
                );

                logging::log(
                    "UI",
                    &format!("Sending state result for request: {}", request_id),
                );

                // Send the response
                if let Some(ref sender) = self.response_sender {
                    if let Err(e) = sender.send(response) {
                        logging::log("ERROR", &format!("Failed to send state result: {}", e));
                    }
                } else {
                    logging::log("ERROR", "No response sender available for state result");
                }
            }
            PromptMessage::ForceSubmit { value } => {
                logging::log(
                    "UI",
                    &format!("ForceSubmit received with value: {:?}", value),
                );

                // Get the current prompt ID and submit the value
                let prompt_id = match &self.current_view {
                    AppView::ArgPrompt { id, .. } => Some(id.clone()),
                    AppView::DivPrompt { id, .. } => Some(id.clone()),
                    AppView::FormPrompt { id, .. } => Some(id.clone()),
                    AppView::TermPrompt { id, .. } => Some(id.clone()),
                    AppView::EditorPrompt { id, .. } => Some(id.clone()),
                    _ => None,
                };

                if let Some(id) = prompt_id {
                    // Convert serde_json::Value to String for submission
                    let value_str = match &value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    };

                    logging::log(
                        "UI",
                        &format!(
                            "ForceSubmit: submitting '{}' for prompt '{}'",
                            value_str, id
                        ),
                    );
                    self.submit_prompt_response(id, Some(value_str), cx);
                } else {
                    logging::log(
                        "WARN",
                        "ForceSubmit received but no active prompt to submit to",
                    );
                }
            }
            // ============================================================
            // NEW PROMPT TYPES (scaffolding - TODO: implement full UI)
            // ============================================================
            PromptMessage::ShowPath {
                id,
                start_path,
                hint,
            } => {
                logging::log(
                    "UI",
                    &format!(
                        "Showing path prompt: {} (start: {:?}, hint: {:?})",
                        id, start_path, hint
                    ),
                );

                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        logging::log(
                            "UI",
                            &format!(
                                "PathPrompt submit_callback called: id={}, value={:?}",
                                id, value
                            ),
                        );
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send path response: {}", e));
                            }
                        }
                    });

                // Clone the pending_path_action Arc for the callback
                let pending_path_action_clone = self.pending_path_action.clone();

                let show_actions_callback: std::sync::Arc<dyn Fn(PathInfo) + Send + Sync> =
                    std::sync::Arc::new(move |path_info| {
                        logging::log(
                            "UI",
                            &format!("Path actions requested for: {}", path_info.path),
                        );
                        if let Ok(mut guard) = pending_path_action_clone.lock() {
                            *guard = Some(path_info);
                        }
                    });

                // Clone the close_path_actions Arc for the close callback
                let close_path_actions_clone = self.close_path_actions.clone();

                let close_actions_callback: std::sync::Arc<dyn Fn() + Send + Sync> =
                    std::sync::Arc::new(move || {
                        logging::log("UI", "Path close actions callback triggered");
                        if let Ok(mut guard) = close_path_actions_clone.lock() {
                            *guard = true;
                        }
                    });

                // Clone the path_actions_showing and search_text Arcs for header display
                let path_actions_showing = self.path_actions_showing.clone();
                let path_actions_search_text = self.path_actions_search_text.clone();

                let focus_handle = cx.focus_handle();
                let path_prompt = PathPrompt::new(
                    id.clone(),
                    start_path,
                    hint,
                    focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                )
                .with_show_actions(show_actions_callback)
                .with_close_actions(close_actions_callback)
                .with_actions_showing(path_actions_showing)
                .with_actions_search_text(path_actions_search_text);

                let entity = cx.new(|_| path_prompt);
                self.current_view = AppView::PathPrompt {
                    id,
                    entity,
                    focus_handle,
                };
                self.focused_input = FocusedInput::None;

                // Clear any previous pending action and reset showing state
                if let Ok(mut guard) = self.pending_path_action.lock() {
                    *guard = None;
                }
                if let Ok(mut guard) = self.path_actions_showing.lock() {
                    *guard = false;
                }

                defer_resize_to_view(ViewType::ScriptList, 20, cx);
                cx.notify();
            }
            PromptMessage::ShowEnv {
                id,
                key,
                prompt,
                secret,
            } => {
                tracing::info!(id, key, ?prompt, secret, "ShowEnv received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowEnv prompt received: {} (key: {}, secret: {})",
                        id, key, secret
                    ),
                );

                // Create submit callback for env prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send env response: {}", e));
                            }
                        }
                    });

                // Create EnvPrompt entity
                let focus_handle = self.focus_handle.clone();
                let mut env_prompt = prompts::EnvPrompt::new(
                    id.clone(),
                    key,
                    prompt,
                    secret,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                // Check keyring first - if value exists, auto-submit without showing UI
                if env_prompt.check_keyring_and_auto_submit() {
                    logging::log("UI", "EnvPrompt: value found in keyring, auto-submitted");
                    // Don't switch view, the callback already submitted
                    cx.notify();
                    return;
                }

                let entity = cx.new(|_| env_prompt);
                self.current_view = AppView::EnvPrompt { id, entity };
                self.focused_input = FocusedInput::None; // EnvPrompt has its own focus handling

                defer_resize_to_view(ViewType::ArgPromptNoChoices, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowDrop {
                id,
                placeholder,
                hint,
            } => {
                tracing::info!(id, ?placeholder, ?hint, "ShowDrop received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowDrop prompt received: {} (placeholder: {:?})",
                        id, placeholder
                    ),
                );

                // Create submit callback for drop prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log("UI", &format!("Failed to send drop response: {}", e));
                            }
                        }
                    });

                // Create DropPrompt entity
                let focus_handle = self.focus_handle.clone();
                let drop_prompt = prompts::DropPrompt::new(
                    id.clone(),
                    placeholder,
                    hint,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                let entity = cx.new(|_| drop_prompt);
                self.current_view = AppView::DropPrompt { id, entity };
                self.focused_input = FocusedInput::None;

                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowTemplate { id, template } => {
                tracing::info!(id, template, "ShowTemplate received");
                logging::log(
                    "UI",
                    &format!(
                        "ShowTemplate prompt received: {} (template: {})",
                        id, template
                    ),
                );

                // Create submit callback for template prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send template response: {}", e),
                                );
                            }
                        }
                    });

                // Create TemplatePrompt entity
                let focus_handle = self.focus_handle.clone();
                let template_prompt = prompts::TemplatePrompt::new(
                    id.clone(),
                    template,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );

                let entity = cx.new(|_| template_prompt);
                self.current_view = AppView::TemplatePrompt { id, entity };
                self.focused_input = FocusedInput::None;

                defer_resize_to_view(ViewType::DivPrompt, 0, cx);
                cx.notify();
            }
            PromptMessage::ShowSelect {
                id,
                placeholder,
                choices,
                multiple,
            } => {
                tracing::info!(
                    id,
                    ?placeholder,
                    choice_count = choices.len(),
                    multiple,
                    "ShowSelect received"
                );
                logging::log(
                    "UI",
                    &format!(
                        "ShowSelect prompt received: {} ({} choices, multiple: {})",
                        id,
                        choices.len(),
                        multiple
                    ),
                );

                // Create submit callback for select prompt
                let response_sender = self.response_sender.clone();
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        if let Some(ref sender) = response_sender {
                            let response = Message::Submit { id, value };
                            if let Err(e) = sender.send(response) {
                                logging::log(
                                    "UI",
                                    &format!("Failed to send select response: {}", e),
                                );
                            }
                        }
                    });

                // Create SelectPrompt entity
                let choice_count = choices.len();
                let select_prompt = prompts::SelectPrompt::new(
                    id.clone(),
                    placeholder,
                    choices,
                    multiple,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::new(self.theme.clone()),
                );
                let entity = cx.new(|_| select_prompt);
                self.current_view = AppView::SelectPrompt { id, entity };
                self.focused_input = FocusedInput::None; // SelectPrompt has its own focus handling

                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                defer_resize_to_view(view_type, choice_count, cx);
                cx.notify();
            }
            PromptMessage::ShowHud { text, duration_ms } => {
                self.show_hud(text, duration_ms, cx);
            }
            PromptMessage::SetInput { text } => {
                self.set_prompt_input(text, cx);
            }
            PromptMessage::SetActions { actions } => {
                logging::log(
                    "ACTIONS",
                    &format!("Received setActions with {} actions", actions.len()),
                );

                // Store SDK actions for trigger_action_by_name lookup
                self.sdk_actions = Some(actions.clone());

                // Build action shortcuts map for keyboard handling
                self.action_shortcuts.clear();
                for action in &actions {
                    if let Some(ref shortcut) = action.shortcut {
                        let normalized = shortcuts::normalize_shortcut(shortcut);
                        logging::log(
                            "ACTIONS",
                            &format!(
                                "Registering action shortcut: '{}' -> '{}' (normalized: '{}')",
                                shortcut, action.name, normalized
                            ),
                        );
                        self.action_shortcuts
                            .insert(normalized, action.name.clone());
                    }
                }

                // Update ActionsDialog if it exists and is open
                if let Some(ref dialog) = self.actions_dialog {
                    dialog.update(cx, |d, _cx| {
                        d.set_sdk_actions(actions);
                    });
                }

                cx.notify();
            }
        }
    }

    /// Cancel the currently running script and clean up all state
    fn cancel_script_execution(&mut self, cx: &mut Context<Self>) {
        logging::log("EXEC", "=== Canceling script execution ===");

        // Send cancel message to script (Exit with cancel code)
        if let Some(ref sender) = self.response_sender {
            // Try to send Exit message to terminate the script cleanly
            let exit_msg = Message::Exit {
                code: Some(1), // Non-zero code indicates cancellation
                message: Some("Cancelled by user".to_string()),
            };
            match sender.send(exit_msg) {
                Ok(()) => logging::log("EXEC", "Sent Exit message to script"),
                Err(e) => logging::log(
                    "EXEC",
                    &format!("Failed to send Exit: {} (script may have exited)", e),
                ),
            }
        } else {
            logging::log("EXEC", "No response_sender - script may not be running");
        }

        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This ensures cleanup even if Drop doesn't fire properly
        if let Some(pid) = self.current_script_pid.take() {
            logging::log(
                "CLEANUP",
                &format!("Force-killing script process group {}", pid),
            );
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &format!("-{}", pid)])
                    .output();
            }
        }

        // Abort script session if it exists
        {
            let mut session_guard = self.script_session.lock();
            if let Some(_session) = session_guard.take() {
                logging::log("EXEC", "Cleared script session");
            }
        }

        // Reset to script list view
        self.reset_to_script_list(cx);
        logging::log("EXEC", "=== Script cancellation complete ===");
    }

    /// Show a HUD (heads-up display) overlay message
    ///
    /// This creates a separate floating window positioned at bottom-center of the
    /// screen containing the mouse cursor. The HUD is independent of the main
    /// Script Kit window and will remain visible even when the main window is hidden.
    ///
    /// Position: Bottom-center (85% down screen)
    /// Duration: 2000ms default, configurable
    /// Shape: Pill (40px tall, variable width)
    fn show_hud(&mut self, text: String, duration_ms: Option<u64>, cx: &mut Context<Self>) {
        // Delegate to the HUD manager which creates a separate floating window
        // This ensures the HUD is visible even when the main app window is hidden
        hud_manager::show_hud(text, duration_ms, cx);
    }

    /// Rebuild alias and shortcut registries from current scripts/scriptlets.
    /// Returns a list of conflict messages (if any) for HUD display.
    /// Conflict rule: first-registered wins - duplicates are blocked.
    fn rebuild_registries(&mut self) -> Vec<String> {
        let mut conflicts = Vec::new();
        self.alias_registry.clear();
        self.shortcut_registry.clear();

        // Register script aliases
        for script in &self.scripts {
            if let Some(ref alias) = script.alias {
                let alias_lower = alias.to_lowercase();
                if let Some(existing_path) = self.alias_registry.get(&alias_lower) {
                    conflicts.push(format!(
                        "Alias conflict: '{}' already used by {}",
                        alias,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Conflict: alias '{}' in {} blocked (already used by {})",
                            alias,
                            script.path.display(),
                            existing_path
                        ),
                    );
                } else {
                    self.alias_registry
                        .insert(alias_lower, script.path.to_string_lossy().to_string());
                }
            }
        }

        // Register scriptlet aliases
        for scriptlet in &self.scriptlets {
            if let Some(ref alias) = scriptlet.alias {
                let alias_lower = alias.to_lowercase();
                if let Some(existing_path) = self.alias_registry.get(&alias_lower) {
                    conflicts.push(format!(
                        "Alias conflict: '{}' already used by {}",
                        alias,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "ALIAS",
                        &format!(
                            "Conflict: alias '{}' in {} blocked (already used by {})",
                            alias, scriptlet.name, existing_path
                        ),
                    );
                } else {
                    let path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());
                    self.alias_registry.insert(alias_lower, path);
                }
            }

            // Register scriptlet shortcuts
            if let Some(ref shortcut) = scriptlet.shortcut {
                let shortcut_lower = shortcut.to_lowercase();
                if let Some(existing_path) = self.shortcut_registry.get(&shortcut_lower) {
                    conflicts.push(format!(
                        "Shortcut conflict: '{}' already used by {}",
                        shortcut,
                        std::path::Path::new(existing_path)
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| existing_path.clone())
                    ));
                    logging::log(
                        "SHORTCUT",
                        &format!(
                            "Conflict: shortcut '{}' in {} blocked (already used by {})",
                            shortcut, scriptlet.name, existing_path
                        ),
                    );
                } else {
                    let path = scriptlet
                        .file_path
                        .clone()
                        .unwrap_or_else(|| scriptlet.name.clone());
                    self.shortcut_registry.insert(shortcut_lower, path);
                }
            }
        }

        logging::log(
            "REGISTRY",
            &format!(
                "Rebuilt registries: {} aliases, {} shortcuts, {} conflicts",
                self.alias_registry.len(),
                self.shortcut_registry.len(),
                conflicts.len()
            ),
        );

        conflicts
    }

    /// Reset all state and return to the script list view.
    /// This clears all prompt state and resizes the window appropriately.
    fn reset_to_script_list(&mut self, cx: &mut Context<Self>) {
        let old_view = match &self.current_view {
            AppView::ScriptList => "ScriptList",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
            AppView::FormPrompt { .. } => "FormPrompt",
            AppView::TermPrompt { .. } => "TermPrompt",
            AppView::EditorPrompt { .. } => "EditorPrompt",
            AppView::SelectPrompt { .. } => "SelectPrompt",
            AppView::PathPrompt { .. } => "PathPrompt",
            AppView::EnvPrompt { .. } => "EnvPrompt",
            AppView::DropPrompt { .. } => "DropPrompt",
            AppView::TemplatePrompt { .. } => "TemplatePrompt",
            AppView::ClipboardHistoryView { .. } => "ClipboardHistoryView",
            AppView::AppLauncherView { .. } => "AppLauncherView",
            AppView::WindowSwitcherView { .. } => "WindowSwitcherView",
            AppView::DesignGalleryView { .. } => "DesignGalleryView",
        };

        let old_focused_input = self.focused_input;
        logging::log(
            "UI",
            &format!(
                "Resetting to script list (was: {}, focused_input: {:?})",
                old_view, old_focused_input
            ),
        );

        // Belt-and-suspenders: Force-kill the process group using stored PID
        // This runs BEFORE clearing channels to ensure cleanup even if Drop doesn't fire
        if let Some(pid) = self.current_script_pid.take() {
            logging::log(
                "CLEANUP",
                &format!("Force-killing script process group {} during reset", pid),
            );
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &format!("-{}", pid)])
                    .output();
            }
        }

        // Reset view
        self.current_view = AppView::ScriptList;

        // CRITICAL: Reset focused_input to MainFilter so the cursor appears
        // This was a bug where focused_input could remain as ArgPrompt/None after
        // script exit, causing the cursor to not show in the main filter.
        self.focused_input = FocusedInput::MainFilter;
        logging::log(
            "FOCUS",
            "Reset focused_input to MainFilter for cursor display",
        );

        // Clear arg prompt state
        self.arg_input_text.clear();
        self.arg_selected_index = 0;
        // P0: Reset arg scroll handle
        self.arg_list_scroll_handle
            .scroll_to_item(0, ScrollStrategy::Top);

        // Clear filter and selection state for fresh menu
        self.filter_text.clear();
        self.computed_filter_text.clear();
        self.filter_coalescer.reset();
        self.selected_index = 0;
        self.last_scrolled_index = None;
        // Use main_list_state for variable-height list (not the legacy list_scroll_handle)
        self.main_list_state.scroll_to_reveal_item(0);
        self.last_scrolled_index = Some(0);

        // Resize window for script list content
        let count = self.scripts.len() + self.scriptlets.len();
        resize_first_window_to_height(height_for_view(ViewType::ScriptList, count));

        // Clear output
        self.last_output = None;

        // Clear channels (they will be dropped, closing the connections)
        self.prompt_receiver = None;
        self.response_sender = None;

        // Clear script session (parking_lot mutex never poisons)
        *self.script_session.lock() = None;

        // Clear actions popup state (prevents stale actions dialog from persisting)
        self.show_actions_popup = false;
        self.actions_dialog = None;

        // Clear pending path action and close signal
        if let Ok(mut guard) = self.pending_path_action.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.close_path_actions.lock() {
            *guard = false;
        }

        logging::log(
            "UI",
            "State reset complete - view is now ScriptList (filter, selection, scroll cleared)",
        );
        cx.notify();
    }

    /// Check if we're currently in a prompt view (script is running)
    fn is_in_prompt(&self) -> bool {
        matches!(
            self.current_view,
            AppView::ArgPrompt { .. }
                | AppView::DivPrompt { .. }
                | AppView::FormPrompt { .. }
                | AppView::TermPrompt { .. }
                | AppView::EditorPrompt { .. }
                | AppView::ClipboardHistoryView { .. }
                | AppView::AppLauncherView { .. }
                | AppView::WindowSwitcherView { .. }
                | AppView::DesignGalleryView { .. }
        )
    }

    /// Submit a response to the current prompt
    fn submit_prompt_response(
        &mut self,
        id: String,
        value: Option<String>,
        _cx: &mut Context<Self>,
    ) {
        logging::log(
            "UI",
            &format!("Submitting response for {}: {:?}", id, value),
        );

        let response = Message::Submit { id, value };

        if let Some(ref sender) = self.response_sender {
            match sender.send(response) {
                Ok(()) => {
                    logging::log("UI", "Response queued for script");
                }
                Err(e) => {
                    logging::log("UI", &format!("Failed to queue response: {}", e));
                }
            }
        } else {
            logging::log("UI", "No response sender available");
        }

        // Return to waiting state (script will send next prompt or exit)
        // Don't change view here - wait for next message from script
    }

    /// Get filtered choices for arg prompt
    fn filtered_arg_choices(&self) -> Vec<(usize, &Choice)> {
        if let AppView::ArgPrompt { choices, .. } = &self.current_view {
            if self.arg_input_text.is_empty() {
                choices.iter().enumerate().collect()
            } else {
                let filter = self.arg_input_text.to_lowercase();
                choices
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                    .collect()
            }
        } else {
            vec![]
        }
    }

    /// P0: Get filtered choices as owned data for uniform_list closure
    fn get_filtered_arg_choices_owned(&self) -> Vec<(usize, Choice)> {
        if let AppView::ArgPrompt { choices, .. } = &self.current_view {
            if self.arg_input_text.is_empty() {
                choices
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (i, c.clone()))
                    .collect()
            } else {
                let filter = self.arg_input_text.to_lowercase();
                choices
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.name.to_lowercase().contains(&filter))
                    .map(|(i, c)| (i, c.clone()))
                    .collect()
            }
        } else {
            vec![]
        }
    }

    /// Convert hex color to rgba with opacity from theme
    fn hex_to_rgba_with_opacity(&self, hex: u32, opacity: f32) -> u32 {
        // Convert opacity (0.0-1.0) to alpha byte (0-255)
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u32;
        (hex << 8) | alpha
    }

    /// Create box shadows from theme configuration
    fn create_box_shadows(&self) -> Vec<BoxShadow> {
        let shadow_config = self.theme.get_drop_shadow();

        if !shadow_config.enabled {
            return vec![];
        }

        // Convert hex color to HSLA
        // For black (0x000000), we use h=0, s=0, l=0
        let r = ((shadow_config.color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((shadow_config.color >> 8) & 0xFF) as f32 / 255.0;
        let b = (shadow_config.color & 0xFF) as f32 / 255.0;

        // Simple RGB to HSL conversion for shadow color
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        let (h, s) = if max == min {
            (0.0, 0.0) // achromatic
        } else {
            let d = max - min;
            let s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h / 6.0, s)
        };

        vec![BoxShadow {
            color: hsla(h, s, l, shadow_config.opacity),
            offset: point(px(shadow_config.offset_x), px(shadow_config.offset_y)),
            blur_radius: px(shadow_config.blur_radius),
            spread_radius: px(shadow_config.spread_radius),
        }]
    }
}

