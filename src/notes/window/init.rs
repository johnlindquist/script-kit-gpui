use super::*;

use crate::components::notes_editor::{NotesEditor, NotesEditorLayout, NotesEditorMarkdownConfig};

impl NotesApp {
    /// Create a new NotesApp
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instance_id =
            NOTES_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        CURRENT_NOTES_INSTANCE.store(instance_id, std::sync::atomic::Ordering::SeqCst);

        // Initialize storage
        if let Err(e) = storage::init_notes_db() {
            let safe_error = crate::logging::log_private_user_value(&e.to_string());
            tracing::error!(
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
                "Failed to initialize notes database"
            );
        }

        // Auto-prune trash entries older than 30 days
        match storage::prune_old_deleted_notes(30) {
            Ok(pruned) if pruned > 0 => {
                info!(
                    pruned_count = pruned,
                    "Auto-pruned old trash notes (>30 days)"
                );
            }
            Err(e) => {
                let safe_error = crate::logging::log_private_user_value(&e.to_string());
                tracing::error!(
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                    "Failed to auto-prune trash"
                );
            }
            _ => {}
        }

        // Load notes from storage
        let mut notes = storage::get_all_notes().unwrap_or_default();
        let deleted_notes = storage::get_deleted_notes().unwrap_or_default();

        // First launch: create a welcome note if no notes exist
        if notes.is_empty() && deleted_notes.is_empty() {
            let welcome = Note::with_content(Self::welcome_note_content());
            if let Err(e) = storage::save_note(&welcome) {
                let safe_error = crate::logging::log_private_user_value(&e.to_string());
                tracing::error!(
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                    "Failed to create welcome note"
                );
            } else {
                notes.push(welcome);
                info!("Created welcome note for first launch");
            }
        }

        let selected_note_id = notes.first().map(|n| n.id);

        // Get initial content if we have a selected note
        let initial_content = selected_note_id
            .and_then(|id| notes.iter().find(|n| n.id == id))
            .map(|n| n.content.clone())
            .unwrap_or_default();

        // Calculate initial line count for auto-resize (before moving content)
        let initial_line_count = initial_content.lines().count().max(1);

        let metrics = style::adopted_metrics();
        let (editor_state, notes_editor) = NotesEditor::new_markdown_pair(
            window,
            cx,
            NotesEditorMarkdownConfig::new(initial_content)
                .layout(NotesEditorLayout::new(
                    metrics.editor_padding_x,
                    metrics.editor_padding_y,
                ))
                .rows(20),
        );

        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search notes..."));

        let focus_handle = cx.focus_handle();

        // Subscribe to editor changes - passes window for auto-resize
        let editor_sub = cx.subscribe_in(&editor_state, window, {
            move |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.on_editor_change(window, cx),
                InputEvent::SelectionChange => {
                    this.notes_editor
                        .update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
                }
                _ => {}
            }
        });

        // Deeplink hover state lives on the InputState and changes via plain
        // cx.notify (no InputEvent), so the window must observe the editor
        // state or the hover hint chip would never appear/disappear live.
        let editor_hover_observation = cx.observe(&editor_state, |_, _, cx| cx.notify());

        // Subscribe to search changes
        let search_sub = cx.subscribe_in(&search_state, window, {
            move |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_search_change(window, cx);
                }
            }
        });

        // Get initial window height to use as minimum
        let initial_height: f32 = window.bounds().size.height.into();

        info!(
            note_count = notes.len(),
            initial_height = initial_height,
            "Notes app initialized"
        );

        // Log the adopted Notes window style for storybook parity verification.
        let notes_style = style::adopted_style();
        tracing::info!(
            target: "notes",
            event = "notes_window_style_applied",
            titlebar_height = notes_style.titlebar_height,
            footer_height = notes_style.footer_height,
            editor_padding_x = notes_style.editor_padding_x,
            editor_padding_y = notes_style.editor_padding_y,
            chrome_opacity = notes_style.chrome_opacity,
            "Applied Notes window style"
        );

        // Seed the popup from the same canonical regular/day corpus used by
        // Today and Notes Browse. `open_browse_panel` refreshes this state on
        // every open, so startup errors cannot later masquerade as empty data.
        let note_search_state = crate::notes::search_model::load_note_search_state(
            "",
            &crate::notes::notes_brain_days_dir(),
            1,
            None,
        );
        let note_switcher_actions = crate::actions::get_canonical_note_search_actions(
            &note_search_state,
            selected_note_id.map(crate::notes::search_model::NoteSearchDocumentId::Note),
        );

        Self {
            notes,
            deleted_notes,
            view_mode: NotesViewMode::AllNotes,
            selected_note_id,
            notes_editor,
            editor_state,
            spine_runtime: Default::default(),
            search_state,
            search_query: String::new(),
            titlebar_hovered: false,
            window_hovered: false,
            mouse_cursor_hidden: false,
            force_hovered: false,
            show_format_toolbar: false,
            show_search: false,
            preview_enabled: false,
            last_line_count: initial_line_count,
            initial_height,
            auto_sizing_enabled: true,          // Auto-sizing ON by default
            last_window_height: initial_height, // Track for manual resize detection
            autosize_generation: 0,
            last_autosize_transition: None,
            last_bottom_resize_receipt: None,
            native_resize_phase: resize::NotesNativeResizePhase::EntryLocked,
            last_automation_synced_bounds: None,
            focus_handle,
            _subscriptions: vec![editor_sub, editor_hover_observation, search_sub],
            last_deeplink_hover_hint: None,
            // Initialize CommandBar with notes-specific actions
            command_bar: CommandBar::new(
                get_notes_command_bar_actions(&NotesInfo {
                    has_selection: selected_note_id.is_some(),
                    is_trash_view: false,
                    auto_sizing_enabled: true,
                }),
                CommandBarConfig::notes_style(),
                std::sync::Arc::new(theme::get_cached_theme()),
            ),
            // Initialize note switcher CommandBar (Cmd+P) with note list
            note_switcher: CommandBar::new(
                note_switcher_actions,
                CommandBarConfig::notes_recent_style(),
                std::sync::Arc::new(theme::get_cached_theme()),
            ),
            active_day_binding: None,
            pending_day_editor_reconcile: None,
            has_unsaved_changes: false,
            instance_id,
            entry_reveal: NotesEntryReveal::new(instance_id),
            last_save_time: None,
            last_persisted_bounds: None,
            last_bounds_save: Instant::now(),
            theme_rev_seen: crate::theme::service::theme_revision(),
            history_back: Vec::new(),
            history_forward: Vec::new(),
            navigating_history: false,
            focus_mode: false,
            sort_mode: NotesSortMode::default(),
            last_save_confirmed: None,
            action_feedback: None,
            last_notes_action_id: None,
            notes_action_execution_generation: 0,
            pending_focus_surface: None,
            focus_transition_generation: 0,
            focus_transition_log: Vec::new(),
            notes_ghost_prediction: None,
            notes_ghost_generation: 0,
            notes_ghost_last_action: None,
            notes_ghost_llm_generation: 0,
            notes_ghost_llm_cancel: None,
            notes_ghost_llm_cache: std::collections::VecDeque::new(),
            last_ai_handoff: None,
            ai_handoff_generation: 0,
            mention_portal_edit: None,
            kit_resource_preview: None,
        }
    }

    /// Debounce interval for saves (in milliseconds)
    const SAVE_DEBOUNCE_MS: u64 = 300;

    /// Debounce interval for bounds persistence (in milliseconds)
    const BOUNDS_DEBOUNCE_MS: u64 = 250;

    /// Update cached theme-derived values if theme revision has changed.
    ///
    /// This is called during render to detect theme hot-reloads.
    /// NOTE: Box shadows were removed for vibrancy compatibility.
    pub(super) fn maybe_update_theme_cache(&mut self) {
        let current_rev = crate::theme::service::theme_revision();
        if self.theme_rev_seen != current_rev {
            self.theme_rev_seen = current_rev;
            // Box shadows disabled for vibrancy - no cached values to update
        }
    }

    /// Persist window bounds if they've changed (debounced).
    ///
    /// This ensures bounds are saved even when the window is closed via traffic light
    /// (red close button) which doesn't go through our close handlers.
    /// Bounds may only persist while the interlock is `Enabled`: the
    /// calibrated entry morph (compression/rebound) and exit fade own the
    /// frame in the locked phases, and persisting one of THOSE intermediate
    /// frames would restore a morph-sized window after a rapid close or
    /// crash. The close paths save the stable frame explicitly before the
    /// exit begins.
    fn bounds_are_persistable(&self) -> bool {
        self.native_resize_phase == resize::NotesNativeResizePhase::Enabled
            && NOTES_EXIT_TICKET
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .is_none()
    }

    /// Explicit close-path save of the stable pre-exit frame. Skipped while
    /// the interlock is locked: a close during the calibrated entry morph
    /// would otherwise persist a transient 106%/98.5% morph frame (runtime
    /// probe notes-live-resize, morph clause, 2026-07-26). The last stable
    /// save simply remains authoritative.
    pub(super) fn maybe_save_stable_bounds_for_exit(&self, window: &gpui::Window) {
        if !self.bounds_are_persistable() {
            tracing::info!(
                target: "notes",
                event = "notes_exit_bounds_save_skipped",
                phase = self.native_resize_phase.as_str(),
                "Notes close-path bounds save skipped — frame is morph-owned"
            );
            return;
        }
        crate::window_state::save_window_from_gpui(
            crate::window_state::WindowRole::Notes,
            window.window_bounds(),
        );
    }

    pub(super) fn maybe_persist_bounds(&mut self, window: &gpui::Window) {
        if !self.bounds_are_persistable() {
            return;
        }

        let wb = window.window_bounds();

        // Skip if bounds haven't changed
        if self.last_persisted_bounds.as_ref() == Some(&wb) {
            return;
        }

        // Debounce to avoid too-frequent saves
        if self.last_bounds_save.elapsed()
            < std::time::Duration::from_millis(Self::BOUNDS_DEBOUNCE_MS)
        {
            return;
        }

        // Save bounds
        self.last_persisted_bounds = Some(wb);
        self.last_bounds_save = Instant::now();
        crate::window_state::save_window_from_gpui(crate::window_state::WindowRole::Notes, wb);
    }

    /// Save the current note if it has unsaved changes.
    /// Returns `true` when the note was saved (or was already clean),
    /// `false` when the save failed or the note was not in the current list.
    pub(super) fn save_current_note(&mut self) -> bool {
        if !self.has_unsaved_changes {
            return true;
        }

        if let Some(day) = self.active_day_binding.as_mut() {
            // Serialize against the background appenders (`;todo`, clipboard
            // sediment, dictation) that also mutate this day file, and use an
            // atomic write (fsync + rename). If an external writer appended
            // lines since we last read disk, MERGE them instead of blindly
            // overwriting — otherwise a capture that landed during the autosave
            // debounce is silently lost (the whole reason `with_brain_write_lock`
            // exists). Mirrors the main-window DayPageView save path.
            use crate::brain::substrate::io;
            let path = day.path.clone();
            let editor_content = day.content.clone();
            let base = day.base_disk_content.clone();

            let written = io::with_brain_write_lock(|| -> anyhow::Result<(String, bool)> {
                let disk_now = io::read_private_document_if_present(&path)?.unwrap_or_default();
                if disk_now == base {
                    io::atomic_write(&path, &editor_content)?;
                    return Ok((editor_content.clone(), false));
                }
                if let Some(suffix) = day_external_append_suffix(&disk_now, &base) {
                    let merged =
                        day_merge_editor_with_external_appends(&editor_content, suffix, &disk_now);
                    io::atomic_write(&path, &merged)?;
                    return Ok((merged, true));
                }
                // Preserve the conflicting disk version through the same
                // private, collision-safe owner used by the main Day Page.
                let conflict_path = io::preserve_private_conflict_copy(&path, &disk_now)?;
                let safe_path = crate::logging::log_private_user_value(&path.display().to_string());
                let safe_conflict =
                    crate::logging::log_private_user_value(&conflict_path.display().to_string());
                tracing::warn!(
                    target: "script_kit::notes",
                    path_bytes = safe_path.raw_bytes,
                    path_sha256 = %safe_path.sha256,
                    conflict_bytes = safe_conflict.raw_bytes,
                    conflict_sha256 = %safe_conflict.sha256,
                    "day note diverged on disk; preserved the conflicting private document"
                );
                io::atomic_write(&path, &editor_content)?;
                Ok((editor_content.clone(), false))
            });

            match written {
                Ok((content, merged)) => {
                    day.content = content.clone();
                    day.base_disk_content = content.clone();
                    if merged {
                        // External captures were folded in — reconcile them into
                        // the editor on the next render tick so they're visible
                        // and the next edit doesn't overwrite them.
                        self.pending_day_editor_reconcile = Some(content);
                    }
                    debug!(date = %day.date, merged, "Day note saved from Notes window");
                    self.has_unsaved_changes = false;
                    self.last_save_time = Some(Instant::now());
                    self.last_save_confirmed = Some(Instant::now());
                    return true;
                }
                Err(e) => {
                    let safe_error = crate::logging::log_private_user_value(&e.to_string());
                    let safe_path =
                        crate::logging::log_private_user_value(&path.display().to_string());
                    tracing::error!(
                        error_bytes = safe_error.raw_bytes,
                        error_sha256 = %safe_error.sha256,
                        date = %day.date,
                        path_bytes = safe_path.raw_bytes,
                        path_sha256 = %safe_path.sha256,
                        "Failed to save day note from Notes window"
                    );
                    return false;
                }
            }
        }

        let Some(id) = self.selected_note_id else {
            return true;
        };

        let Some(note) = self.notes.iter().find(|n| n.id == id) else {
            let safe_query = crate::logging::log_private_user_value(&self.search_query);
            tracing::warn!(
                note_id = %id,
                search_query_bytes = safe_query.raw_bytes,
                search_query_sha256 = %safe_query.sha256,
                notes_len = self.notes.len(),
                "Skipping note save because the selected note is not present in the current notes list"
            );
            return false;
        };

        if let Err(e) = storage::save_note(note) {
            let safe_error = crate::logging::log_private_user_value(&e.to_string());
            tracing::error!(
                error_bytes = safe_error.raw_bytes,
                error_sha256 = %safe_error.sha256,
                note_id = %id,
                "Failed to save note"
            );
            return false;
        }

        debug!(note_id = %id, "Note saved (debounced)");
        self.has_unsaved_changes = false;
        self.last_save_time = Some(Instant::now());
        self.last_save_confirmed = Some(Instant::now());
        true
    }

    /// Check if we should save now (debounce check)
    pub(super) fn should_save_now(&self) -> bool {
        if !self.has_unsaved_changes {
            return false;
        }

        match self.last_save_time {
            None => true,
            Some(last_save) => last_save.elapsed() >= Duration::from_millis(Self::SAVE_DEBOUNCE_MS),
        }
    }

    /// Handle editor content changes with auto-resize
    pub(crate) fn on_editor_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.notes_editor
            .update(cx, |editor, cx| editor.sync_markdown_link_highlights(cx));
        let previous_content = self
            .active_day_binding
            .as_ref()
            .map(|day| day.content.clone())
            .or_else(|| {
                self.selected_note_id.and_then(|id| {
                    self.notes
                        .iter()
                        .find(|note| note.id == id)
                        .map(|note| note.content.clone())
                })
            })
            .unwrap_or_default();
        let content = self.editor_state.read(cx).value();
        let mut content_string = content.to_string();
        if let Some((fixed, cursor)) =
            crate::components::notes_editor::spine::mention_atomic_delete_fixup(
                &previous_content,
                &content_string,
                &self.spine_runtime.mention_aliases,
            )
        {
            self.editor_state.update(cx, |state, cx| {
                state.set_value_preserving_scroll(fixed.clone(), cursor, window, cx);
            });
            content_string = fixed;
        }
        self.spine_runtime
            .prune_mention_aliases_for_content(&content_string);
        self.spine_runtime.clear_transient_cache();

        // Auto-create a note if user is typing with no note selected
        // This prevents data loss when users start typing immediately
        if let Some(day) = self.active_day_binding.as_mut() {
            day.content = content_string.clone();
            self.has_unsaved_changes = true;
            let new_line_count = self.editor_display_line_count(&content_string, cx);
            if new_line_count != self.last_line_count {
                self.last_line_count = new_line_count;
                self.update_window_height(window, new_line_count, cx);
            }
            self.recompute_notes_ghost(cx);
            cx.notify();
            return;
        }

        if self.selected_note_id.is_none() && !content_string.is_empty() {
            info!("Auto-creating note from unselected editor content");
            let note = Note::with_content(content_string.clone());
            let id = note.id;

            // Save to storage
            if let Err(e) = storage::save_note(&note) {
                let safe_error = crate::logging::log_private_user_value(&e.to_string());
                tracing::error!(
                    error_bytes = safe_error.raw_bytes,
                    error_sha256 = %safe_error.sha256,
                    "Failed to create auto-generated note"
                );
                return;
            }

            // Add to cache and select it
            self.notes.insert(0, note);
            self.selected_note_id = Some(id);
            self.recompute_notes_ghost(cx);
            cx.notify();
            return;
        }

        if let Some(id) = self.selected_note_id {
            // Update the note in our cache (in-memory only)
            let content_updated = if let Some(note) = self.notes.iter_mut().find(|n| n.id == id) {
                note.set_content(content_string.clone());
                // Mark as dirty - actual save is debounced
                self.has_unsaved_changes = true;
                true
            } else {
                false
            };

            if content_updated {
                // Auto-resize: adjust window height based on content
                let new_line_count = self.editor_display_line_count(&content_string, cx);
                if new_line_count != self.last_line_count {
                    self.last_line_count = new_line_count;
                    self.update_window_height(window, new_line_count, cx);
                }
            }

            self.recompute_notes_ghost(cx);
            cx.notify();
        }
    }

    pub(super) fn recompute_notes_ghost(&mut self, cx: &mut Context<Self>) {
        self.notes_ghost_generation = self.notes_ghost_generation.wrapping_add(1).max(1);

        if self.preview_enabled
            || self.view_mode == NotesViewMode::Trash
            || self.show_search
            || self.command_bar.is_open()
            || self.note_switcher.is_open()
            || self.notes_spine_input(cx).is_some()
        {
            self.notes_ghost_prediction = None;
            self.cancel_notes_ghost_llm();
            self.sync_notes_ghost_inline_completion(cx);
            return;
        }

        let (editor_text, selection) = {
            let editor = self.editor_state.read(cx);
            (editor.value().to_string(), editor.selection())
        };

        let clipboard_texts = self.collect_notes_ghost_clipboard_texts();
        self.notes_ghost_prediction = crate::notes::ghost::compute_notes_ghost_prediction(
            crate::notes::ghost::NotesGhostInput {
                editor_text: &editor_text,
                selection: selection.clone(),
                selected_note_id: self.selected_note_id,
                notes: &self.notes,
                clipboard_texts: &clipboard_texts,
                generation: self.notes_ghost_generation,
            },
        );

        if self.notes_ghost_prediction.is_some() {
            // Deterministic candidates win; abort any in-flight LLM hint.
            self.cancel_notes_ghost_llm();
        } else {
            self.maybe_start_notes_ghost_llm(&editor_text, selection, cx);
        }
        self.sync_notes_ghost_inline_completion(cx);
    }

    /// Mirror `notes_ghost_prediction` into the editor's native inline
    /// completion so the ghost suffix is shaped inside the editor's own text
    /// layout — exact caret/baseline alignment, like VS Code — instead of an
    /// absolutely positioned overlay that re-derives padding and line metrics
    /// by hand. Must be called after every `notes_ghost_prediction` mutation.
    pub(super) fn sync_notes_ghost_inline_completion(&mut self, cx: &mut Context<Self>) {
        let suffix = self
            .notes_ghost_prediction
            .as_ref()
            .map(|prediction| prediction.suffix.clone());
        self.editor_state.update(cx, |state, cx| match suffix {
            Some(suffix) => state.set_inline_completion_text(suffix, cx),
            None => {
                if state.has_inline_completion() {
                    state.clear_inline_completion(cx);
                }
            }
        });
    }

    /// Best-effort cancel of the in-flight LLM ghost request and a generation
    /// bump so late results are dropped on arrival.
    fn cancel_notes_ghost_llm(&mut self) {
        if let Some(cancel) = self.notes_ghost_llm_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.notes_ghost_llm_generation = self.notes_ghost_llm_generation.wrapping_add(1).max(1);
    }

    /// Debounced on-device LLM ghost side-channel for the Notes editor.
    ///
    /// Mirrors the launcher's `maybe_start_ghost_llm_prediction` discipline:
    /// debounce, recall brain context, generate on the local-llm actor thread,
    /// sanitize, and apply only while the editor line prefix is unchanged and
    /// no deterministic prediction appeared in the meantime.
    fn maybe_start_notes_ghost_llm(
        &mut self,
        editor_text: &str,
        selection: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) {
        let line = crate::notes::ghost::current_line_prefix(editor_text, selection.clone());
        let Some(line) = line else {
            self.cancel_notes_ghost_llm();
            return;
        };
        if !crate::notes::ghost_llm::line_prefix_is_eligible(&line.text) {
            self.cancel_notes_ghost_llm();
            return;
        }

        // Cache hit: serve instantly under the current accept generation.
        if let Some(suffix) = self.cached_notes_ghost_llm_suffix(&line.text) {
            self.cancel_notes_ghost_llm();
            self.notes_ghost_prediction = Some(crate::notes::ghost_llm::prediction_from_suffix(
                &line.text,
                suffix,
                self.notes_ghost_generation,
            ));
            return;
        }

        self.cancel_notes_ghost_llm();
        let generation = self.notes_ghost_llm_generation;
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.notes_ghost_llm_cancel = Some(cancel.clone());

        let line_prefix = line.text;
        let note_title = self
            .selected_note_id
            .and_then(|id| self.notes.iter().find(|note| note.id == id))
            .map(|note| note.title.clone())
            .unwrap_or_default();
        let excerpt = crate::notes::ghost_llm::excerpt_around_cursor(editor_text, selection.start);
        let note_id = self.selected_note_id;

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(
                    crate::notes::ghost_llm::NOTES_GHOST_LLM_DEBOUNCE_MS,
                ))
                .await;
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }

            let line_for_model = line_prefix.clone();
            let title_for_model = note_title.clone();
            let cancel_for_model = cancel.clone();
            // Brain recall + on-device GGUF generation — no network. Runs on
            // the background executor / local-llm actor thread.
            let result = cx
                .background_executor()
                .spawn(async move {
                    let config = crate::config::load_config();
                    // No model on disk yet? Kick the first-run download and
                    // bail; deterministic ghost keeps working meanwhile.
                    crate::ai::local_llm::ensure_ghost_model_in_background(&config);
                    let recall_query = if title_for_model.trim().is_empty() {
                        line_for_model.clone()
                    } else {
                        format!("{title_for_model} {line_for_model}")
                    };
                    let brain_block = crate::brain::recall_context_block(&recall_query)
                        .ok()
                        .flatten();
                    let prompt = crate::notes::ghost_llm::build_notes_ghost_prompt(
                        &line_for_model,
                        &title_for_model,
                        &excerpt,
                        brain_block.as_deref(),
                    );
                    let generate = |cancel| {
                        crate::ai::local_llm::generate_ghost_completion(
                            &config,
                            crate::ai::local_llm::LocalGhostRequest {
                                prompt: crate::ai::local_llm::GhostPromptSpec::NotesContinuation {
                                    prompt: prompt.clone(),
                                },
                                cancel,
                            },
                        )
                        .map(|response| response.raw_completion)
                    };
                    let mut result = generate(cancel_for_model.clone());
                    // Empty/unsafe completions sanitize to None and the user
                    // sees nothing; sampling is stochastic, so one retry often
                    // recovers a usable suffix. Never retry after cancel.
                    let sanitized_empty = matches!(
                        &result,
                        Ok(raw)
                            if crate::notes::ghost_llm::sanitize_notes_llm_suffix(
                                raw,
                                &line_for_model
                            )
                            .is_none()
                    );
                    if sanitized_empty
                        && !cancel_for_model.load(std::sync::atomic::Ordering::Relaxed)
                    {
                        result = generate(cancel_for_model.clone());
                    }
                    result
                })
                .await;

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    if app.notes_ghost_llm_generation != generation {
                        return;
                    }
                    app.notes_ghost_llm_cancel = None;
                    let raw_response = match result {
                        Ok(raw) => raw,
                        Err(error) => {
                            // Silent fallback: no model / cancelled / backend
                            // unavailable all keep the deterministic behavior.
                            let safe_error =
                                crate::logging::log_private_user_value(&format!("{error:#}"));
                            tracing::debug!(
                                target: "script_kit::ghost_text",
                                error_bytes = safe_error.raw_bytes,
                                error_sha256 = %safe_error.sha256,
                                "notes ghost llm generation failed; keeping deterministic ghost"
                            );
                            return;
                        }
                    };
                    // Re-validate the editor against the request snapshot: a
                    // deterministic prediction that appeared meanwhile wins,
                    // and the line prefix must be unchanged.
                    let (value, selection) = {
                        let editor = app.editor_state.read(cx);
                        (editor.value().to_string(), editor.selection())
                    };
                    let current_line = crate::notes::ghost::current_line_prefix(&value, selection);
                    if !crate::notes::ghost_llm::should_apply_llm_result(
                        app.notes_ghost_prediction.is_some(),
                        &line_prefix,
                        current_line.as_ref().map(|line| line.text.as_str()),
                    ) {
                        return;
                    }
                    let Some(prediction) = crate::notes::ghost_llm::llm_prediction_from_response(
                        &line_prefix,
                        &raw_response,
                        app.notes_ghost_generation,
                    ) else {
                        return;
                    };
                    app.cache_notes_ghost_llm_suffix(note_id, &line_prefix, &prediction.suffix);
                    app.notes_ghost_prediction = Some(prediction);
                    app.sync_notes_ghost_inline_completion(cx);
                    cx.notify();
                })
            });
        })
        .detach();
    }

    /// Fresh cached suffix for the selected note + line prefix, if any.
    fn cached_notes_ghost_llm_suffix(&mut self, line_prefix: &str) -> Option<String> {
        self.notes_ghost_llm_cache.retain(|entry| {
            entry.inserted_at.elapsed() <= crate::notes::ghost_llm::NOTES_GHOST_LLM_CACHE_TTL
        });
        let note_id = self.selected_note_id;
        self.notes_ghost_llm_cache
            .iter()
            .find(|entry| entry.note_id == note_id && entry.line_prefix == line_prefix)
            .map(|entry| entry.suffix.clone())
    }

    fn cache_notes_ghost_llm_suffix(
        &mut self,
        note_id: Option<NoteId>,
        line_prefix: &str,
        suffix: &str,
    ) {
        self.notes_ghost_llm_cache.retain(|entry| {
            !(entry.note_id == note_id && entry.line_prefix == line_prefix)
                && entry.inserted_at.elapsed() <= crate::notes::ghost_llm::NOTES_GHOST_LLM_CACHE_TTL
        });
        self.notes_ghost_llm_cache
            .push_front(super::NotesGhostLlmCacheEntry {
                note_id,
                line_prefix: line_prefix.to_string(),
                suffix: suffix.to_string(),
                inserted_at: Instant::now(),
            });
        while self.notes_ghost_llm_cache.len()
            > crate::notes::ghost_llm::NOTES_GHOST_LLM_CACHE_LIMIT
        {
            self.notes_ghost_llm_cache.pop_back();
        }
    }

    fn collect_notes_ghost_clipboard_texts(
        &self,
    ) -> Vec<crate::notes::ghost::NotesGhostClipboardText> {
        crate::clipboard_history::get_clipboard_history_meta(20, 0)
            .into_iter()
            .filter(|entry| {
                matches!(
                    entry.content_type,
                    crate::clipboard_history::ContentType::Text
                        | crate::clipboard_history::ContentType::Link
                )
            })
            .filter_map(|entry| crate::clipboard_history::get_entry_content(&entry.id))
            .map(|text| crate::notes::ghost::NotesGhostClipboardText { text })
            .collect()
    }

    pub(crate) fn set_editor_text_for_automation(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor_state.update(cx, |state, inner_cx| {
            state.set_value(text.clone(), window, inner_cx);
            state.set_selection(text.len(), text.len(), window, inner_cx);
        });
        self.on_editor_change(window, cx);

        let new_line_count = self.editor_display_line_count(&text, cx);
        if new_line_count != self.last_line_count {
            self.last_line_count = new_line_count;
            self.update_window_height(window, new_line_count, cx);
        }
    }

    /// Visual line count for auto-resize: the editor's soft-wrapped display
    /// lines (kept in sync by the input's text wrapper on every edit), so a
    /// long wrapped paragraph grows the window like multiple lines do.
    /// Falls back to logical lines before the editor's first layout.
    fn editor_display_line_count(&self, content: &str, cx: &Context<Self>) -> usize {
        let wrapped = self.editor_state.read(cx).soft_wrapped_lines_len();
        if wrapped > 0 {
            wrapped
        } else {
            content.lines().count()
        }
        .max(1)
    }

    /// Update window height based on content line count
    /// Raycast-style: window grows AND shrinks to fit content when auto_sizing_enabled
    /// IMPORTANT: Window never shrinks below initial_height (the height at window creation)
    pub(super) fn resolve_auto_resize_height(
        total_height: f32,
        min_height: f32,
        max_height: f32,
    ) -> f32 {
        let min_height = if min_height.is_finite() && min_height > 0.0 {
            min_height
        } else {
            0.0
        };
        let max_height = if max_height.is_finite() && max_height > 0.0 {
            max_height.max(min_height)
        } else {
            min_height
        };
        let total_height = if total_height.is_finite() {
            total_height
        } else {
            min_height
        };

        total_height.clamp(min_height, max_height)
    }

    /// Desired auto-size height for `line_count` editor lines. Shared by the
    /// resize path and `automation_state` so the two cannot drift.
    ///
    /// Routes through the single Notes layout resolver
    /// (`layout::resolve_notes_autosize`), whose footer term is the resolved
    /// footer action row: ZERO while the row is absent (the Notes footer rail
    /// was removed — Notes renders no footer), and the shared footer-chrome
    /// row height if the row ever returns. No independent footer scalar
    /// participates in this equation.
    pub(super) fn autosize_desired_height(
        metrics: &style::NotesLayoutMetrics,
        line_count: usize,
    ) -> f32 {
        // Desired height is independent of the restored floor; 0.0 keeps the
        // resolver input honest about "no restored minimum considered here".
        super::layout::resolve_notes_autosize(super::layout::notes_autosize_input(
            metrics, line_count, 0.0,
        ))
        .desired_height
    }

    pub(super) fn update_window_height(
        &mut self,
        window: &mut Window,
        line_count: usize,
        _cx: &mut Context<Self>,
    ) {
        // Use initial_height as minimum - never shrink below starting size
        let min_height = self.initial_height;

        // Calculate desired height from the same metrics used by render/layout receipts.
        let metrics = style::adopted_metrics();
        let total_height = Self::autosize_desired_height(&metrics, line_count);
        let clamped_height = Self::resolve_auto_resize_height(
            total_height,
            min_height,
            metrics.auto_resize_max_height,
        );

        // Get current bounds and update height
        let current_bounds = window.bounds();
        let old_height: f32 = current_bounds.size.height.into();
        let old_width: f32 = current_bounds.size.width.into();
        let mut applied = false;
        let mut skipped_reason = None;

        self.autosize_generation = self.autosize_generation.wrapping_add(1);

        // Skip if auto-sizing is disabled (user manually resized)
        if !self.auto_sizing_enabled {
            skipped_reason = Some("disabled");
        } else if (clamped_height - old_height).abs() <= metrics.auto_resize_threshold {
            skipped_reason = Some("below-threshold");
        } else {
            let new_size = size(current_bounds.size.width, px(clamped_height));

            debug!(
                old_height = old_height,
                new_height = clamped_height,
                min_height = min_height,
                line_count = line_count,
                auto_sizing = self.auto_sizing_enabled,
                "Auto-resize: adjusting window height"
            );

            let transition = NotesAutosizeTransition {
                generation: self.autosize_generation,
                cause: "editor-input",
                before_height: old_height,
                after_height: clamped_height,
                before_width: old_width,
                after_width: old_width,
                line_count,
                desired_height: total_height,
                clamped_height,
                applied: true,
                skipped_reason: None,
                recorded_at: Instant::now(),
            };
            self.last_autosize_transition = Some(transition);

            // Record the app-owned target before resizing. `window.resize`
            // can cause another render pass while AppKit is still between the
            // old and new bounds; `detect_manual_resize` treats either bound
            // as app-owned while this fresh transition is active.
            self.last_window_height = clamped_height;
            window.resize(new_size);
            crate::windows::upsert_automation_window(crate::protocol::AutomationWindowInfo {
                id: "notes".to_string(),
                kind: crate::protocol::AutomationWindowKind::Notes,
                title: Some("Notes".to_string()),
                focused: true,
                visible: true,
                semantic_surface: Some("notes".to_string()),
                bounds: Some(crate::protocol::AutomationWindowBounds {
                    x: f32::from(current_bounds.origin.x) as f64,
                    y: f32::from(current_bounds.origin.y) as f64,
                    width: f32::from(new_size.width) as f64,
                    height: f32::from(new_size.height) as f64,
                }),
                parent_window_id: None,
                parent_kind: None,
                pid: Some(std::process::id()),
                generation: None,
            });
            applied = true;
        }

        if !applied {
            self.last_autosize_transition = Some(NotesAutosizeTransition {
                generation: self.autosize_generation,
                cause: "editor-input",
                before_height: old_height,
                after_height: old_height,
                before_width: old_width,
                after_width: old_width,
                line_count,
                desired_height: total_height,
                clamped_height,
                applied,
                skipped_reason,
                recorded_at: Instant::now(),
            });
        }
    }

    /// Enable auto-sizing (called from actions panel)
    pub fn enable_auto_sizing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_sizing_enabled = true;
        // Re-calculate and apply the correct height
        let line_count = self.last_line_count;
        self.update_window_height(window, line_count, cx);
        info!("Auto-sizing enabled");
        cx.notify();
    }

    pub fn toggle_auto_sizing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.auto_sizing_enabled {
            self.auto_sizing_enabled = false;
            info!("Auto-sizing disabled");
            cx.notify();
        } else {
            self.enable_auto_sizing(window, cx);
        }
    }

    /// Reset the Notes window to its default size and position — the
    /// top-right corner of the display the mouse cursor is on. The Notes
    /// counterpart of the main window's "Reset Window Positions" settings
    /// command, so a window dragged off-screen is always recoverable from
    /// the actions menu.
    pub fn reset_window_position_to_default(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bounds = super::window_ops::default_notes_window_bounds();
        let x: f64 = bounds.origin.x.into();
        let y: f64 = bounds.origin.y.into();
        let width: f64 = bounds.size.width.into();
        let height: f64 = bounds.size.height.into();

        // GPUI's Window::resize is size-only; reposition through the platform
        // layer (setFrame:) like the AI window-mode restore does.
        let mut moved = false;
        if let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
            if let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() {
                crate::platform::move_window_by_view(appkit.ns_view, x, y, width, height);
                moved = true;
            }
        }
        if !moved {
            window.resize(bounds.size);
        }

        // The height change is ours — don't let detect_manual_resize read it
        // as a user resize and silently disable auto-sizing.
        self.last_window_height = f32::from(bounds.size.height);

        crate::window_state::save_window_bounds(
            crate::window_state::WindowRole::Notes,
            crate::window_state::PersistedWindowBounds {
                mode: crate::window_state::PersistedWindowMode::Windowed,
                x,
                y,
                width,
                height,
            },
        );
        crate::windows::set_automation_bounds(
            "notes",
            Some(crate::protocol::AutomationWindowBounds {
                x,
                y,
                width,
                height,
            }),
        );

        tracing::info!(
            target: "notes",
            event = "notes_window_position_reset",
            x,
            y,
            width,
            height,
            moved,
        );
        self.show_action_feedback("Window position reset", false);
        cx.notify();
    }

    /// Check if user manually resized the window and disable auto-sizing if so
    pub(super) fn detect_manual_resize(&mut self, window: &Window) {
        if !self.auto_sizing_enabled {
            return; // Already disabled
        }

        let current_height: f32 = window.bounds().size.height.into();
        let recent_app_resize = self
            .last_autosize_transition
            .as_ref()
            .and_then(|transition| {
                if !transition.applied
                    || transition.cause != "editor-input"
                    || transition.recorded_at.elapsed() > Duration::from_millis(750)
                {
                    return None;
                }
                Some((transition.before_height, transition.after_height))
            });
        if let Some((before_height, after_height)) = recent_app_resize {
            if (current_height - before_height).abs() <= MANUAL_RESIZE_THRESHOLD
                || (current_height - after_height).abs() <= MANUAL_RESIZE_THRESHOLD
            {
                if (current_height - after_height).abs() <= MANUAL_RESIZE_THRESHOLD {
                    self.last_window_height = after_height;
                }
                return;
            }
        }

        // If height differs significantly from what we set, user resized manually
        if (current_height - self.last_window_height).abs() > MANUAL_RESIZE_THRESHOLD {
            self.auto_sizing_enabled = false;
            self.last_window_height = current_height;
            debug!(
                current_height = current_height,
                last_height = self.last_window_height,
                "Manual resize detected - auto-sizing disabled"
            );
        }
    }
}

/// Return the suffix that an external writer appended to a day file since we
/// last saw `base`, or `None` if the disk content isn't `base` plus an append.
/// Mirrors the DayPageView merge so a `;todo`/clipboard/dictation capture landing
/// during the autosave debounce is preserved rather than clobbered.
fn day_external_append_suffix<'a>(disk_now: &'a str, base: &str) -> Option<&'a str> {
    let suffix = disk_now.strip_prefix(base.trim_end())?;
    if suffix.trim().is_empty() {
        return None;
    }
    Some(suffix)
}

/// Combine the editor buffer with external appends detected on disk, preserving
/// a single joining newline and the trailing newline if the disk file had one.
fn day_merge_editor_with_external_appends(content: &str, suffix: &str, disk_now: &str) -> String {
    let mut merged = content.trim_end().to_string();
    merged.push('\n');
    merged.push_str(suffix.trim_start_matches('\n'));
    if disk_now.ends_with('\n') && !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged
}

#[cfg(test)]
mod day_save_merge_tests {
    use super::{day_external_append_suffix, day_merge_editor_with_external_appends};

    #[test]
    fn detects_external_append_and_ignores_pure_edit() {
        let base = "# Today\n\n- first\n";
        assert_eq!(
            day_external_append_suffix("# Today\n\n- first\n- captured\n", base),
            Some("\n- captured\n")
        );
        // No external append: disk equals base -> None.
        assert_eq!(day_external_append_suffix(base, base), None);
    }

    #[test]
    fn merge_preserves_editor_edits_and_external_capture() {
        // The editor changed the first line WHILE a capture appended a line.
        let editor = "# Today edited\n\n- first";
        let disk = "# Today\n\n- first\n- captured\n";
        let base = "# Today\n\n- first\n";
        let suffix = day_external_append_suffix(disk, base).expect("append detected");
        let merged = day_merge_editor_with_external_appends(editor, suffix, disk);
        // Both the editor's edit and the captured line survive.
        assert!(merged.contains("# Today edited"));
        assert!(merged.contains("- captured"));
        assert!(merged.ends_with('\n'));
    }
}
