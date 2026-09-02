use super::*;
use crate::ui_foundation::{
    is_key_backspace, is_key_delete, is_key_down, is_key_enter, is_key_escape, is_key_tab,
    is_key_up,
};

#[inline]
fn is_plain_platform_cmd_w(event: &KeyDownEvent) -> bool {
    let key = event.keystroke.key.as_str();
    let modifiers = &event.keystroke.modifiers;
    modifiers.platform
        && !modifiers.shift
        && !modifiers.alt
        && !modifiers.control
        && key.eq_ignore_ascii_case("w")
}

#[inline]
fn is_key_backtick(key: &str) -> bool {
    key == "`" || key.eq_ignore_ascii_case("backtick")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NotesGhostAcceptMode {
    Word,
    Full,
}

impl NotesApp {
    /// Single escape "dismiss ladder" for the Notes window.
    ///
    /// Owns the order in which Escape dismisses transient chrome: detached
    /// CommandBar popups (Cmd+K actions, Cmd+P switcher) → embedded Agent
    /// Chat popups/streaming/surface → ghost autocomplete → search bar →
    /// focus mode → trash view. Returns `(action, handled)`.
    ///
    /// Both the live `handle_key_down` Escape branch and the DevTools
    /// automation route (`handle_notes_ghost_key_for_automation`) call this,
    /// so the two paths cannot drift. Window close (the final live Escape
    /// behavior) intentionally stays in `handle_key_down`.
    pub(super) fn escape_dismiss_ladder(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (&'static str, bool) {
        if self.command_bar.is_open() {
            self.close_actions_panel(window, cx);
            return ("closeActionsPanel", true);
        }
        if self.note_switcher.is_open() {
            self.close_browse_panel(window, cx);
            return ("closeBrowsePanel", true);
        }
        if self.notes_spine_input(cx).is_some() {
            self.reset_notes_spine_navigation(cx);
            return ("dismissNotesSpine", true);
        }
        if self.kit_resource_preview.is_some() {
            self.close_kit_resource_preview(window, cx);
            return ("closeKitResourcePreview", true);
        }
        if self.dismiss_notes_ghost(cx) {
            return ("dismissNotesGhost", true);
        }
        if self.show_search {
            self.toggle_search(window, cx);
            return ("closeSearch", true);
        }
        if self.focus_mode {
            self.toggle_focus_mode(cx);
            return ("exitFocusMode", true);
        }
        if self.view_mode == NotesViewMode::Trash {
            self.set_view_mode(NotesViewMode::AllNotes, window, cx);
            return ("exitTrash", true);
        }
        ("noNotesEscapeAction", false)
    }

    pub(super) fn dismiss_notes_ghost(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(prediction) = self.notes_ghost_prediction.take() else {
            return false;
        };
        self.notes_ghost_last_action = Some(NotesGhostActionReceipt::dismissed(&prediction));
        self.sync_notes_ghost_inline_completion(cx);
        cx.notify();
        true
    }

    pub(super) fn try_accept_notes_ghost(
        &mut self,
        mode: NotesGhostAcceptMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(prediction) = self.notes_ghost_prediction.clone() else {
            return false;
        };
        if !prediction.accepts_tab || prediction.generation != self.notes_ghost_generation {
            return false;
        }

        // The editor clears its native inline completion on escape / mouse /
        // selection changes (the rendering channel for this ghost). If it is
        // gone, the user dismissed the ghost — never accept invisible text.
        if !self.editor_state.read(cx).has_inline_completion() {
            self.notes_ghost_prediction = None;
            self.notes_ghost_last_action = Some(NotesGhostActionReceipt::dismissed(&prediction));
            return false;
        }

        let (value, selection) = {
            let editor = self.editor_state.read(cx);
            (editor.value().to_string(), editor.selection())
        };

        let Some(line) = crate::notes::ghost::current_line_prefix(&value, selection.clone()) else {
            self.notes_ghost_prediction = None;
            self.notes_ghost_last_action = Some(NotesGhostActionReceipt::stale(&prediction));
            self.sync_notes_ghost_inline_completion(cx);
            return false;
        };
        if line.text != prediction.query_prefix {
            self.notes_ghost_prediction = None;
            self.notes_ghost_last_action = Some(NotesGhostActionReceipt::stale(&prediction));
            self.sync_notes_ghost_inline_completion(cx);
            return false;
        }

        let cursor = selection.start.min(value.len());
        if !value.is_char_boundary(cursor) {
            self.notes_ghost_prediction = None;
            self.notes_ghost_last_action = Some(NotesGhostActionReceipt::stale(&prediction));
            self.sync_notes_ghost_inline_completion(cx);
            return false;
        }

        let accepted_suffix = match mode {
            NotesGhostAcceptMode::Word => {
                crate::notes::ghost::first_word_acceptance_suffix(&prediction.suffix).to_string()
            }
            NotesGhostAcceptMode::Full => prediction.suffix.clone(),
        };
        if accepted_suffix.is_empty() {
            self.notes_ghost_prediction = None;
            self.notes_ghost_last_action = Some(NotesGhostActionReceipt::stale(&prediction));
            self.sync_notes_ghost_inline_completion(cx);
            return false;
        }

        let next_value = format!(
            "{}{}{}",
            &value[..cursor],
            accepted_suffix.as_str(),
            &value[cursor..]
        );
        let next_cursor = cursor + accepted_suffix.len();
        self.editor_state.update(cx, |state, cx| {
            state.set_value_preserving_scroll(next_value, next_cursor, window, cx);
        });

        self.notes_ghost_last_action = Some(match mode {
            NotesGhostAcceptMode::Word => {
                NotesGhostActionReceipt::accepted_word(&prediction, &accepted_suffix)
            }
            NotesGhostAcceptMode::Full => NotesGhostActionReceipt::accepted_full(&prediction),
        });
        // Accepted brain-grounded hints reinforce the memories that produced
        // them (fire-and-forget; never blocks the editor input path).
        if prediction.source_kind == crate::notes::ghost::NotesGhostSourceKind::Brain {
            crate::brain::record_ghost_accept_signals(&prediction.query_prefix, &accepted_suffix);
        }
        self.notes_ghost_prediction = None;
        self.on_editor_change(window, cx);
        true
    }

    fn close_notes_window_from_top_level_cmd_w(
        &mut self,
        reason: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::keyboard",
            event = "top_level_cmd_w_close_notes_window",
            reason,
            focus_surface = ?self.current_focus_surface(),
            show_search = self.show_search,
            focus_mode = self.focus_mode,
            has_active_dialog = window.has_active_dialog(cx),
        );

        if !self.save_before_window_close(window, cx) {
            cx.stop_propagation();
            return;
        }

        self.command_bar.close_app(cx);
        self.note_switcher.close_app(cx);

        self.maybe_save_stable_bounds_for_exit(window);

        window.close_all_dialogs(cx);
        let _ = self.entry_reveal.prepare_for_window_exit();
        self.lock_native_resize_for_exit(window);
        super::window_ops::close_current_notes_window(window, cx);
        cx.stop_propagation();
    }

    pub(super) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hide_mouse_cursor(cx);

        if is_plain_platform_cmd_w(event) {
            self.close_notes_window_from_top_level_cmd_w("notes_top_level_cmd_w", window, cx);
            return;
        }

        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        // Reconcile detached CommandBar windows: if they were dismissed
        // externally (focus loss, click outside) without routing through
        // close_actions_panel / close_browse_panel, the in-memory `is_open`
        // flag would otherwise stick true and swallow every keystroke at the
        // popup-first branches below — making Cmd+P / Cmd+K appear dead.
        let command_bar_was_stale = self.command_bar.reconcile_open_state();
        let note_switcher_was_stale = self.note_switcher.reconcile_open_state();
        if command_bar_was_stale || note_switcher_was_stale {
            // Detached action windows are visual-only; restore focus to the
            // Notes root so the next Cmd+P / Cmd+K is routable. Avoid forcing
            // editor focus so Notes-hosted Agent Chat keeps its surface.
            self.focus_handle.focus(window, cx);
            cx.notify();
        }

        if window.has_active_dialog(cx) {
            // The dialog component registers Enter→Confirm and Escape→Cancel
            // keybindings in the "Dialog" key context.  However, the Notes
            // window uses `capture_key_down` which runs *after* GPUI action
            // dispatch.  If the dialog's focus handle is not yet in the
            // rendered dispatch tree (e.g. first frame after opening) or if
            // macOS routes the key through the text input system before GPUI
            // sees it, the built-in keybinding never fires.
            //
            // Dispatching the actions explicitly here ensures Enter/Escape
            // always work while a dialog is open, regardless of focus state.
            if !is_key_enter(key) && !is_key_escape(key) {
                tracing::info!(
                    event = "notes_dialog_key_guard",
                    key = %key,
                    platform = modifiers.platform,
                    shift = modifiers.shift,
                    control = modifiers.control,
                    alt = modifiers.alt,
                    "notes_dialog_key_guard"
                );
            }

            if is_key_enter(key) && !modifiers.platform && !modifiers.control {
                window.dispatch_action(
                    Box::new(gpui_component::actions::Confirm { secondary: false }),
                    cx,
                );
                cx.stop_propagation();
                return;
            }
            if is_key_escape(key) {
                window.dispatch_action(Box::new(gpui_component::actions::Cancel), cx);
                cx.stop_propagation();
                return;
            }
            if is_key_tab(key) && !modifiers.platform && !modifiers.control && !modifiers.alt {
                if modifiers.shift {
                    window.focus_prev_in_dialog(cx);
                } else {
                    window.focus_next_in_dialog(cx);
                }
                cx.stop_propagation();
                return;
            }
            cx.propagate();
            return;
        }

        if self.command_bar.is_open() {
            tracing::info!(
                event = "notes_popup_key_routed",
                popup = "command_bar",
                key = %key,
                platform = modifiers.platform,
                alt = modifiers.alt,
            );
            match key {
                key if is_key_escape(key) => {
                    self.close_actions_panel(window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_up(key) => {
                    self.command_bar.select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_down(key) => {
                    self.command_bar.select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_enter(key) => {
                    if let Some(action_id) = self.command_bar.execute_selected_action(cx) {
                        self.execute_action(&action_id, window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                key if (key.eq_ignore_ascii_case("left")
                    || key.eq_ignore_ascii_case("arrowleft"))
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt =>
                {
                    self.command_bar
                        .move_search_cursor(false, modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if (key.eq_ignore_ascii_case("right")
                    || key.eq_ignore_ascii_case("arrowright"))
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt =>
                {
                    self.command_bar
                        .move_search_cursor(true, modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if (is_key_backspace(key) || is_key_delete(key)) && !modifiers.platform => {
                    if modifiers.alt {
                        self.command_bar.handle_backspace_word(window, cx);
                    } else {
                        self.command_bar.handle_backspace(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => {
                    if !modifiers.platform && !modifiers.control && !modifiers.alt {
                        // Full printable charset via the produced character,
                        // matching the main search input; fall back to
                        // single-char key names for synthetic events.
                        if let Some(ch) = crate::ui_foundation::printable_char(
                            event.keystroke.key_char.as_deref(),
                        ) {
                            self.command_bar.handle_char(ch, window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(ch) = key.chars().next() {
                            if ch.len_utf8() != key.len() {
                                cx.stop_propagation();
                                return;
                            }
                            let ch = ch.to_ascii_lowercase();
                            if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_'
                            {
                                self.command_bar.handle_char(ch, window, cx);
                                cx.stop_propagation();
                                return;
                            }
                        }
                    }
                    if modifiers.platform && key.eq_ignore_ascii_case("k") {
                        self.close_actions_panel(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    // Row shortcuts (⌘D Duplicate, ⇧⌘⌫ Delete Note, …) must
                    // work whichever window AppKit made key: the detached
                    // popup matches these in ActionsWindow::on_key_down, and
                    // this branch mirrors it for when the Notes window keeps
                    // key focus.
                    let matched_action_id = self.command_bar.dialog().and_then(|dialog| {
                        let d = dialog.read(cx);
                        crate::actions::matching_filtered_action_id_for_keystroke(
                            &d.actions,
                            &d.filtered_actions,
                            key,
                            modifiers,
                        )
                    });
                    if let Some(action_id) = matched_action_id {
                        self.execute_action(&action_id, window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if modifiers.platform
                        && !modifiers.shift
                        && !modifiers.control
                        && !modifiers.alt
                        && key.eq_ignore_ascii_case("v")
                    {
                        self.command_bar.handle_paste(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                }
            }
            return;
        }

        if self.note_switcher.is_open() {
            tracing::info!(
                event = "notes_popup_key_routed",
                popup = "note_switcher",
                key = %key,
                platform = modifiers.platform,
                alt = modifiers.alt,
            );
            match key {
                key if is_key_escape(key) => {
                    self.close_browse_panel(window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_up(key) => {
                    self.note_switcher.select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_down(key) => {
                    self.note_switcher.select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                key if is_key_enter(key) => {
                    if let Some(action_id) = self.note_switcher.execute_selected_action(cx) {
                        self.execute_note_switcher_action(&action_id, window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                key if (key.eq_ignore_ascii_case("left")
                    || key.eq_ignore_ascii_case("arrowleft"))
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt =>
                {
                    self.note_switcher
                        .move_search_cursor(false, modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if (key.eq_ignore_ascii_case("right")
                    || key.eq_ignore_ascii_case("arrowright"))
                    && !modifiers.platform
                    && !modifiers.control
                    && !modifiers.alt =>
                {
                    self.note_switcher
                        .move_search_cursor(true, modifiers.shift, window, cx);
                    cx.stop_propagation();
                    return;
                }
                key if (is_key_backspace(key) || is_key_delete(key)) && !modifiers.platform => {
                    if modifiers.alt {
                        self.note_switcher.handle_backspace_word(window, cx);
                    } else {
                        self.note_switcher.handle_backspace(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => {
                    if !modifiers.platform && !modifiers.control && !modifiers.alt {
                        // Full printable charset via the produced character,
                        // matching the main search input; fall back to
                        // single-char key names for synthetic events.
                        if let Some(ch) = crate::ui_foundation::printable_char(
                            event.keystroke.key_char.as_deref(),
                        ) {
                            self.note_switcher.handle_char(ch, window, cx);
                            cx.stop_propagation();
                            return;
                        }
                        if let Some(ch) = key.chars().next() {
                            if ch.len_utf8() != key.len() {
                                cx.stop_propagation();
                                return;
                            }
                            let ch = ch.to_ascii_lowercase();
                            if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_'
                            {
                                self.note_switcher.handle_char(ch, window, cx);
                                cx.stop_propagation();
                                return;
                            }
                        }
                    }
                    if modifiers.platform && key.eq_ignore_ascii_case("p") {
                        self.close_browse_panel(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                    if modifiers.platform
                        && !modifiers.shift
                        && !modifiers.control
                        && !modifiers.alt
                        && key.eq_ignore_ascii_case("v")
                    {
                        self.note_switcher.handle_paste(window, cx);
                        cx.stop_propagation();
                        return;
                    }
                }
            }
            return;
        }

        // Kit resource preview keyboard contract: the preview replaces the
        // editor, so its power-user keys must win before any editor handling.
        // ⌘C copies the resource URI, ↵ opens the editable source note (when
        // one exists). Escape stays with the shared dismiss ladder below.
        // These mirror the clickable footer hints rendered by the shared
        // preview component.
        if self.kit_resource_preview.is_some() && !is_key_escape(key) {
            if modifiers.platform
                && !modifiers.shift
                && !modifiers.control
                && !modifiers.alt
                && key.eq_ignore_ascii_case("c")
            {
                self.copy_kit_resource_preview_uri(cx);
                cx.stop_propagation();
                return;
            }
            if is_key_enter(key)
                && !modifiers.platform
                && !modifiers.shift
                && !modifiers.control
                && !modifiers.alt
            {
                // Swallow Enter even without a source note so it cannot leak
                // into the hidden editor behind the preview.
                self.open_kit_resource_preview_source(window, cx);
                cx.stop_propagation();
                return;
            }
        }

        if is_key_escape(key) {
            cx.stop_propagation();
            // Shared dismiss ladder (popups → ghost → search → focus → trash).
            let (_action, handled) = self.escape_dismiss_ladder(window, cx);
            if handled {
                return;
            }
            // Escape close must have the same data-loss profile as Cmd+W:
            // the autosave debounce (SAVE_DEBOUNCE_MS) cannot outrun
            // remove_window(), so save synchronously first.
            if !self.save_before_window_close(window, cx) {
                return;
            }
            self.maybe_save_stable_bounds_for_exit(window);
            window.close_all_dialogs(cx);
            let _ = self.entry_reveal.prepare_for_window_exit();
            self.lock_native_resize_for_exit(window);
            super::window_ops::close_current_notes_window(window, cx);
            return;
        }

        if let Some(descriptor) =
            crate::notes::notes_action_for_keystroke(self.notes_action_context(), key, modifiers)
        {
            self.handle_action(descriptor.action, window, cx);
            cx.stop_propagation();
            return;
        }

        if is_key_backtick(key) && !modifiers.platform && !modifiers.control && !modifiers.alt {
            if self.try_accept_notes_ghost(NotesGhostAcceptMode::Full, window, cx) {
                cx.stop_propagation();
                return;
            }
            cx.propagate();
            return;
        }

        if !modifiers.platform && !modifiers.control && !modifiers.alt {
            if is_key_up(key) && self.move_notes_spine_selection(-1, cx) {
                cx.stop_propagation();
                return;
            }
            if is_key_down(key) && self.move_notes_spine_selection(1, cx) {
                cx.stop_propagation();
                return;
            }
            if is_key_enter(key) && self.accept_notes_spine_selection(window, cx) {
                cx.stop_propagation();
                return;
            }
        }

        if is_key_tab(key) && !modifiers.platform && !modifiers.control && !modifiers.alt {
            if !modifiers.shift && self.accept_notes_spine_selection(window, cx) {
                cx.stop_propagation();
                return;
            }
            if !modifiers.shift
                && self.try_accept_notes_ghost(NotesGhostAcceptMode::Word, window, cx)
            {
                cx.stop_propagation();
                return;
            }
            if modifiers.shift {
                self.outdent_line(window, cx);
            } else {
                self.indent_at_cursor(window, cx);
            }
            cx.stop_propagation();
            return;
        }

        if modifiers.alt && !modifiers.platform {
            match key {
                key if is_key_up(key) => {
                    if modifiers.shift {
                        self.duplicate_line(false, window, cx);
                    } else {
                        self.move_line_up(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                key if is_key_down(key) => {
                    if modifiers.shift {
                        self.duplicate_line(true, window, cx);
                    } else {
                        self.move_line_down(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        if modifiers.control && modifiers.shift && key.eq_ignore_ascii_case("k") {
            self.delete_current_line(window, cx);
            cx.stop_propagation();
            return;
        }

        if modifiers.platform {
            match key {
                // Cmd+Shift+Enter: follow the [[wiki link]] under the cursor.
                key if is_key_enter(key)
                    && modifiers.shift
                    && !modifiers.control
                    && !modifiers.alt =>
                {
                    if self.follow_wiki_link_at_cursor(window, cx) {
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("k") => {
                    if self.command_bar.is_open() {
                        self.close_actions_panel(window, cx);
                    } else {
                        self.open_actions_panel(window, cx);
                    }
                    cx.stop_propagation();
                }
                key if modifiers.shift && key.eq_ignore_ascii_case("o") => {
                    if self.open_focused_note_mention_portal(window, cx) {
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("f") && modifiers.shift => {
                    self.toggle_search(window, cx);
                    cx.stop_propagation();
                }
                key if key.eq_ignore_ascii_case("n") && modifiers.shift => {
                    self.create_note_from_clipboard(window, cx);
                    cx.stop_propagation();
                }
                key if key.eq_ignore_ascii_case("w") && !modifiers.shift => {
                    self.close_notes_window_from_top_level_cmd_w("notes_cmd_w_close", window, cx);
                }
                "." => {
                    if modifiers.shift {
                        self.toggle_blockquote(window, cx);
                    } else if self.activate_deeplink_under_cursor(window, cx) {
                        // Handled by deeplink activation. If no link is under
                        // the cursor, preserve the long-standing focus-mode
                        // binding below.
                    } else {
                        self.toggle_focus_mode(cx);
                    }
                    cx.stop_propagation();
                }
                key if key.eq_ignore_ascii_case("x") => {
                    if modifiers.shift {
                        self.insert_formatting("~~", "~~", window, cx);
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("l") && !modifiers.shift => {
                    self.select_current_line(window, cx);
                    cx.stop_propagation();
                }
                "-" => {
                    if modifiers.shift {
                        self.insert_horizontal_rule(window, cx);
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("h") => {
                    if modifiers.shift {
                        self.cycle_heading(window, cx);
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("v") => {
                    if self.try_smart_paste(window, cx) {
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("e") && !modifiers.shift => {
                    self.insert_formatting("`", "`", window, cx);
                    cx.stop_propagation();
                }
                key if key.eq_ignore_ascii_case("j") => {
                    self.join_lines(window, cx);
                    cx.stop_propagation();
                }
                key if key.eq_ignore_ascii_case("u") => {
                    if modifiers.shift {
                        self.transform_case(window, cx);
                        cx.stop_propagation();
                    }
                }
                key if key.eq_ignore_ascii_case("b") => {
                    self.insert_formatting("**", "**", window, cx)
                }
                key if key.eq_ignore_ascii_case("i") => {
                    if modifiers.shift {
                        self.toggle_pin_current_note(cx);
                    } else {
                        self.insert_formatting("_", "_", window, cx);
                    }
                }
                key if is_key_up(key) => {
                    let editor_is_focused = self
                        .editor_state
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window);
                    if !editor_is_focused {
                        if modifiers.shift {
                            self.select_first_note(window, cx);
                        } else {
                            self.select_prev_note(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }
                key if is_key_down(key) => {
                    let editor_is_focused = self
                        .editor_state
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window);
                    if !editor_is_focused {
                        if modifiers.shift {
                            self.select_last_note(window, cx);
                        } else {
                            self.select_next_note(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }
                "7" if modifiers.shift => {
                    self.toggle_numbered_list(window, cx);
                    cx.stop_propagation();
                }
                "8" if modifiers.shift => {
                    self.toggle_bullet_list(window, cx);
                    cx.stop_propagation();
                }
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" if !modifiers.shift => {
                    if let Ok(num) = key.parse::<usize>() {
                        self.select_pinned_note_by_index(num - 1, window, cx);
                        cx.stop_propagation();
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod dialog_modal_guard_tests {
    use std::fs;

    fn normalize_ws(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn notes_dialog_guard_precedes_tab_indentation_logic() {
        let source = fs::read_to_string("src/notes/window/keyboard.rs")
            .expect("Failed to read src/notes/window/keyboard.rs");
        let normalized = normalize_ws(&source);

        let dialog_guard = normalized
            .find("if window.has_active_dialog(cx) {")
            .expect("Notes should defer key handling when a dialog is active");
        let tab_handler = normalized
            .find(
                "if is_key_tab(key) && !modifiers.platform && !modifiers.control && !modifiers.alt {",
            )
            .expect("Notes should retain editor tab indentation logic");

        assert!(
            dialog_guard < tab_handler,
            "Dialog guard must run before Notes consumes Tab for indentation"
        );
    }

    #[test]
    fn test_notes_keyboard_logs_when_active_dialog_intercepts_keys() {
        const KEYBOARD_SOURCE: &str = include_str!("keyboard.rs");
        assert!(
            KEYBOARD_SOURCE.contains("event = \"notes_dialog_key_guard\""),
            "Notes keyboard should log when an active dialog is intercepting keys"
        );
    }
}
