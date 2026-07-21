use super::*;
use crate::ui_foundation::{
    is_key_backspace, is_key_delete, is_key_down, is_key_enter, is_key_escape, is_key_k,
    is_key_tab, is_key_up,
};

pub(super) fn is_context_inspector_shortcut(key: &str, modifiers: &gpui::Modifiers) -> bool {
    modifiers.platform && modifiers.alt && !modifiers.shift && !modifiers.control && key == "i"
}

/// Cmd+Shift+A opens the context palette (keyboard-first attach flow).
pub(super) fn is_context_palette_shortcut(key: &str, modifiers: &gpui::Modifiers) -> bool {
    modifiers.platform && modifiers.shift && !modifiers.alt && !modifiers.control && key == "a"
}

pub(super) fn is_mini_history_shortcut(key: &str, modifiers: &gpui::Modifiers) -> bool {
    modifiers.platform && !modifiers.shift && !modifiers.alt && !modifiers.control && key == "j"
}

/// Returns true when the keystroke is the global focus-search shortcut (Cmd+Shift+F).
fn is_global_focus_search_shortcut(key: &str, platform: bool, shift: bool) -> bool {
    platform && shift && key == "f"
}

impl AiApp {
    pub(super) fn handle_root_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Hide mouse cursor on any keyboard interaction
        self.hide_mouse_cursor(cx);

        // Clear stale CommandBar `is_open` state if the detached actions
        // window was dismissed externally (focus loss, click outside) without
        // routing through close(). Without this, the open-branch routing at
        // lines ~288/331/627 would consume keys for an invisible bar and
        // Cmd+K reopen could short-circuit on a stale flag.
        self.command_bar.reconcile_open_state();
        self.new_chat_command_bar.reconcile_open_state();

        // Handle keyboard shortcuts
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        let no_system_modifiers = !modifiers.platform && !modifiers.alt && !modifiers.control;

        // Setup-card keyboard navigation when no providers are configured.
        // Skip while API key input is visible so Enter/typing route to the input.
        let in_setup_mode = self.available_models.is_empty() && !self.showing_api_key_input;

        if no_system_modifiers && in_setup_mode {
            match key {
                k if is_key_tab(k) => {
                    if modifiers.shift {
                        self.move_setup_button_focus(-1, cx);
                    } else {
                        self.move_setup_button_focus(1, cx);
                    }
                    window.activate_window();
                    cx.stop_propagation();
                    return;
                }
                k if is_key_up(k) => {
                    self.move_setup_button_focus(-1, cx);
                    window.activate_window();
                    cx.stop_propagation();
                    return;
                }
                k if is_key_down(k) => {
                    self.move_setup_button_focus(1, cx);
                    window.activate_window();
                    cx.stop_propagation();
                    return;
                }
                k if is_key_enter(k) => {
                    if self.setup_button_focus_index == 0 {
                        self.open_agent_chat_agents_catalog(window, cx);
                    }
                    window.activate_window();
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        // Cmd+Shift+F: always focus search — preempts all modal guards below.
        // This must run before command_bar / new_chat_command_bar / presets_dropdown
        // early-returns so the shortcut is never swallowed by an open popup.
        if modifiers.platform && modifiers.shift && key == "f" {
            super::telemetry::log_ai_shortcut_decision(
                "cmd_shift_f",
                self.window_mode,
                "handle_root_key_down",
                key,
                if self.window_mode.is_mini() {
                    "mini_show_overlay"
                } else {
                    "full_focus_search"
                },
                true,
                self.showing_mini_history_overlay,
                !self.search_query.is_empty(),
            );
            super::observability::emit_ai_ui_event(
                &super::observability::AiUiEvent {
                    kind: AiUiEventKind::ShortcutDecision,
                    action: if self.window_mode.is_mini() {
                        "mini_show_overlay"
                    } else {
                        "full_focus_search"
                    },
                    source: "handle_root_key_down",
                    window_mode: self.window_mode,
                    selected_chat_id: self.selected_chat_id.as_ref(),
                    overlay_visible: self.showing_mini_history_overlay,
                    search_active: !self.search_query.is_empty(),
                },
                None,
            );
            if self.window_mode.is_mini() {
                self.hide_all_dropdowns(cx);
                self.show_mini_history_overlay("shortcut_cmd_shift_f", window, cx);
            } else {
                if self.sidebar_collapsed {
                    self.sidebar_collapsed = false;
                }
                self.hide_all_dropdowns(cx);
                self.focus_search(window, cx);
            }
            cx.stop_propagation();
            return;
        }

        // Handle command bar navigation when it's open
        // This routes all relevant keys to the CommandBar
        // CRITICAL: Must stop propagation to prevent Input from consuming the keys
        if self.command_bar.is_open() {
            match key {
                k if is_key_up(k) => {
                    self.command_bar_select_prev(cx);
                    cx.stop_propagation(); // Prevent Input from handling
                    return;
                }
                k if is_key_down(k) => {
                    self.command_bar_select_next(cx);
                    cx.stop_propagation(); // Prevent Input from handling
                    return;
                }
                k if is_key_enter(k) => {
                    self.execute_command_bar_action(window, cx);
                    cx.stop_propagation(); // Prevent Input from handling
                    return;
                }
                k if is_key_escape(k) => {
                    self.hide_command_bar(cx);
                    cx.stop_propagation(); // Prevent further handling
                    return;
                }
                k if is_key_backspace(k) || is_key_delete(k) => {
                    self.command_bar_handle_backspace(cx);
                    cx.stop_propagation(); // Prevent Input from handling
                    return;
                }
                _ => {
                    // Handle printable characters for search (when no modifiers)
                    if !modifiers.platform && !modifiers.control && !modifiers.alt {
                        // Get the character from the keystroke
                        if let Some(ch) = key.chars().next() {
                            if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_'
                            {
                                self.command_bar_handle_char(ch, cx);
                                cx.stop_propagation(); // Prevent Input from handling
                                return;
                            }
                        }
                    }
                }
            }
            // Don't fall through to other handlers when command bar is open
            return;
        }

        // Handle presets dropdown navigation
        if self.showing_presets_dropdown {
            match key {
                k if is_key_up(k) => {
                    self.presets_select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_down(k) => {
                    self.presets_select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_enter(k) || is_key_tab(k) => {
                    self.confirm_presets_selection(window, cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_escape(k) => {
                    self.hide_presets_dropdown(cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        // Handle new chat dropdown navigation (Raycast-style CommandBar)
        if self.new_chat_command_bar.is_open() {
            match key {
                k if is_key_up(k) => {
                    self.new_chat_command_bar_select_prev(cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_down(k) => {
                    self.new_chat_command_bar_select_next(cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_enter(k) => {
                    self.execute_new_chat_action(window, cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_escape(k) => {
                    self.hide_new_chat_command_bar(cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {
                    // Let printable characters propagate to the search Input component.
                    // No cx.stop_propagation() so GPUI capture phase forwards to child.
                }
            }
            // Don't fall through to Cmd+J/K/N shortcuts when new chat bar is open
            return;
        }

        // Cmd+1-4: submit welcome suggestion cards (only when welcome screen is visible)
        if modifiers.platform
            && !modifiers.shift
            && !modifiers.alt
            && !modifiers.control
            && self.current_messages.is_empty()
            && !self.is_streaming
            && !self.available_models.is_empty()
        {
            let max_visible_suggestions = if self.window_mode.is_mini() {
                super::types::MINI_SUGGESTION_COUNT
            } else {
                super::types::FULL_SUGGESTION_COUNT
            };
            let idx = match key {
                "1" => Some(0),
                "2" => Some(1),
                "3" => Some(2),
                "4" => Some(3),
                _ => None,
            };
            if let Some(i) = idx.filter(|i| *i < max_visible_suggestions) {
                let suggestions = super::render_welcome::script_kit_welcome_suggestions();
                let Some((title, desc, _icon)) = suggestions.get(i) else {
                    return;
                };
                let prompt = format!("{} {}", title, desc);
                info!(
                    shortcut = i + 1,
                    prompt = %prompt,
                    "welcome_suggestion_shortcut"
                );
                self.set_composer_value(&prompt, window, cx);
                self.submit_message(window, cx);
                cx.stop_propagation();
                return;
            }
        }

        // platform modifier = Cmd on macOS, Ctrl on Windows/Linux
        if modifiers.platform {
            match key {
                k if self.window_mode.is_mini() && is_mini_history_shortcut(k, modifiers) => {
                    self.toggle_mini_history_overlay("shortcut_cmd_j", window, cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_context_inspector_shortcut(k, modifiers) => {
                    self.toggle_context_inspector(cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+Shift+A to open context palette (keyboard-first attach)
                k if is_context_palette_shortcut(k, modifiers) => {
                    self.hide_all_dropdowns(cx);
                    self.show_command_bar("shortcut_cmd_shift_a", window, cx);
                    cx.stop_propagation();
                    return;
                }
                // Cmd+K to toggle command bar (like Raycast)
                k if is_key_k(k) => {
                    if self.command_bar.is_open() {
                        self.hide_command_bar(cx);
                    } else {
                        self.hide_all_dropdowns(cx);
                        self.show_command_bar("shortcut_cmd_k", window, cx);
                    }
                    cx.stop_propagation();
                }
                // Cmd+N for new chat (with Shift for presets)
                "n" => {
                    if modifiers.shift {
                        // Cmd+Shift+N opens presets dropdown
                        self.hide_all_dropdowns(cx);
                        self.show_presets_dropdown(window, cx);
                    } else {
                        super::telemetry::log_ai_shortcut_decision(
                            "cmd_n",
                            self.window_mode,
                            "handle_root_key_down",
                            key,
                            "new_conversation",
                            true,
                            self.showing_mini_history_overlay,
                            !self.search_query.is_empty(),
                        );
                        super::observability::emit_ai_ui_event(
                            &super::observability::AiUiEvent {
                                kind: AiUiEventKind::ShortcutDecision,
                                action: "new_conversation",
                                source: "handle_root_key_down",
                                window_mode: self.window_mode,
                                selected_chat_id: self.selected_chat_id.as_ref(),
                                overlay_visible: self.showing_mini_history_overlay,
                                search_active: !self.search_query.is_empty(),
                            },
                            None,
                        );
                        self.new_conversation(window, cx);
                    }
                    cx.stop_propagation();
                }
                // Cmd+Shift+F handled above modal guards (preempt block)
                k if is_key_enter(k) => self.submit_message(window, cx),
                // Cmd+Shift+M toggles between Mini and Full mode
                "m" if modifiers.shift => {
                    self.toggle_window_mode(window, cx);
                    cx.stop_propagation();
                }
                // Cmd+\ to toggle sidebar (like Raycast) — no-op in mini mode
                "\\" | "backslash" => {
                    if !self.window_mode.is_mini() {
                        self.toggle_sidebar(cx);
                    }
                }
                // Cmd+B also toggles sidebar (common convention) — no-op in mini mode
                "b" => {
                    if !self.window_mode.is_mini() {
                        self.toggle_sidebar(cx);
                    }
                }
                // Cmd+V for paste - check for images first
                "v" => {
                    // Try to paste an image; if not found, let normal text paste happen
                    // We don't need to prevent the event since the Input handles text paste
                    self.handle_paste_for_image(cx);
                }
                // Cmd+L to focus input (standard shortcut)
                "l" => {
                    self.focus_input(window, cx);
                    cx.stop_propagation();
                }
                // Cmd+Shift+C to copy last assistant response
                "c" => {
                    if modifiers.shift {
                        self.copy_last_assistant_response(cx);
                        cx.stop_propagation();
                    }
                }
                // Cmd+[ to navigate to previous chat, Cmd+] to next chat
                "[" | "bracketleft" => {
                    self.navigate_chat(-1, window, cx);
                    cx.stop_propagation();
                }
                "]" | "bracketright" => {
                    self.navigate_chat(1, window, cx);
                    cx.stop_propagation();
                }
                // Cmd+Shift+Backspace to delete current chat
                k if is_key_backspace(k) || is_key_delete(k) => {
                    if modifiers.shift {
                        self.delete_current_chat(cx);
                        cx.stop_propagation();
                    }
                }
                // Cmd+Shift+E to export chat to clipboard as markdown
                "e" => {
                    if modifiers.shift {
                        self.export_chat_to_clipboard(cx);
                        cx.stop_propagation();
                    }
                }
                // Cmd+/ to toggle keyboard shortcuts overlay
                "/" | "slash" => {
                    self.toggle_shortcuts_overlay(cx);
                    cx.stop_propagation();
                }
                // Cmd+W closes the AI window (standard macOS pattern)
                "w" => {
                    cx.stop_propagation();
                    // Save bounds before closing
                    let wb = window.window_bounds();
                    crate::window_state::save_window_from_gpui(
                        super::window_api::window_role_for_mode(self.window_mode),
                        wb,
                    );
                    super::telemetry::log_ai_lifecycle(
                        "ai_window_close",
                        self.window_mode,
                        "cmd_w",
                        "closing",
                    );
                    // Clear global handle + state so reopen works correctly
                    super::window_api::cleanup_ai_window_globals();
                    crate::platform::dematerialize_then_remove_gpui_window(window, cx, "AI", "AI");
                }
                _ => {}
            }
        }

        // Mini history overlay key routing: Up/Down navigate, Enter selects, Esc dismisses.
        // This guard must precede the generic edit_last_user_message path below.
        if self.window_mode.is_mini() && self.showing_mini_history_overlay {
            let search_active = !self.search_query.is_empty();
            match key {
                // navigate_chat(1) moves toward newer / visually up
                k if is_key_up(k) => {
                    super::telemetry::log_ai_shortcut_decision(
                        "mini_history_overlay_key",
                        self.window_mode,
                        "handle_root_key_down",
                        key,
                        "overlay_up",
                        true,
                        true,
                        search_active,
                    );
                    self.navigate_chat_preserving_mini_overlay(1, window, cx);
                    cx.stop_propagation();
                    return;
                }
                // navigate_chat_preserving_mini_overlay(-1) moves toward older / visually down
                k if is_key_down(k) => {
                    super::telemetry::log_ai_shortcut_decision(
                        "mini_history_overlay_key",
                        self.window_mode,
                        "handle_root_key_down",
                        key,
                        "overlay_down",
                        true,
                        true,
                        search_active,
                    );
                    self.navigate_chat_preserving_mini_overlay(-1, window, cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_enter(k) => {
                    super::telemetry::log_ai_shortcut_decision(
                        "mini_history_overlay_key",
                        self.window_mode,
                        "handle_root_key_down",
                        key,
                        "overlay_enter",
                        true,
                        true,
                        search_active,
                    );
                    self.dismiss_mini_history_overlay("enter_key", window, cx);
                    cx.stop_propagation();
                    return;
                }
                k if is_key_escape(k) => {
                    super::telemetry::log_ai_shortcut_decision(
                        "mini_history_overlay_key",
                        self.window_mode,
                        "handle_root_key_down",
                        key,
                        "overlay_escape",
                        true,
                        true,
                        search_active,
                    );
                    self.dismiss_mini_history_overlay("escape_key", window, cx);
                    super::telemetry::log_ai_state(
                        "esc_dismiss_history_overlay",
                        "escape_key",
                        &self.debug_snapshot(),
                    );
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }

        // Escape closes shortcuts overlay
        if is_key_escape(key) && self.showing_shortcuts_overlay {
            self.showing_shortcuts_overlay = false;
            super::telemetry::log_ai_state(
                "esc_dismiss_shortcuts_overlay",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // Escape clears active search before falling through to close/stop handlers
        if is_key_escape(key) && !self.search_query.is_empty() {
            info!(
                previous_query = %self.search_query,
                "escape_clear_search"
            );
            self.search_query.clear();
            self.search_generation += 1;
            self.search_snippets.clear();
            self.search_matched_title.clear();
            self.chats = crate::ai::storage::get_all_chats().unwrap_or_default();
            // Clear the search input text
            self.search_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            // Return focus to chat input
            self.focus_input(window, cx);
            super::telemetry::log_ai_state(
                "esc_clear_search",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // ── Token-atomic inline mention deletion ──────────────
        // When backspace/delete lands inside or at the boundary of an inline
        // @mention token, remove the whole token plus one trailing space
        // instead of deleting a single character.
        if is_key_backspace(key) || is_key_delete(key) {
            let current_text = self.input_state.read(cx).value().to_string();
            let cursor_byte = self.input_state.read(cx).cursor();
            // Convert byte offset to char offset for mention functions.
            let cursor_char = current_text[..cursor_byte.min(current_text.len())]
                .chars()
                .count();

            if let Some((next_text, next_cursor_char)) =
                crate::ai::context_mentions::remove_inline_mention_at_cursor(
                    &current_text,
                    cursor_char,
                    is_key_delete(key),
                )
            {
                tracing::info!(
                    target: "ai",
                    event = "ai_inline_mention_deleted_atomically",
                    key = %key,
                    cursor_char,
                    next_cursor_char,
                );

                // Convert char offset back to byte offset for InputState.
                let next_cursor_byte = next_text
                    .char_indices()
                    .nth(next_cursor_char)
                    .map(|(byte, _)| byte)
                    .unwrap_or(next_text.len());

                self.input_state.update(cx, |state, cx| {
                    state.set_value(next_text, window, cx);
                    state.set_selection(next_cursor_byte, next_cursor_byte, window, cx);
                });
                self.sync_inline_mentions(cx);
                cx.stop_propagation();
                return;
            }
        }

        // Up arrow in empty input: edit last user message
        if is_key_up(key) && self.input_state.read(cx).value().is_empty() && !self.is_streaming {
            self.edit_last_user_message(window, cx);
            cx.stop_propagation();
            return;
        }

        // Escape cancels editing mode
        if is_key_escape(key) && self.editing_message_id.is_some() {
            self.editing_message_id = None;
            self.clear_composer(window, cx);
            super::telemetry::log_ai_state(
                "esc_cancel_editing",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.stop_propagation();
            return;
        }

        // Escape cancels rename
        if is_key_escape(key) && self.renaming_chat_id.is_some() {
            self.cancel_rename(cx);
            super::telemetry::log_ai_state(
                "esc_cancel_rename",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.stop_propagation();
            return;
        }

        // Escape stops streaming if active
        if is_key_escape(key) && self.is_streaming {
            self.stop_streaming(cx);
            super::telemetry::log_ai_state(
                "esc_stop_streaming",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.stop_propagation();
            return;
        }

        // Escape closes API key input (back to setup card)
        if is_key_escape(key) && self.showing_api_key_input {
            self.hide_api_key_input(window, cx);
            super::telemetry::log_ai_state(
                "esc_dismiss_api_key_input",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.stop_propagation();
            return;
        }

        // Escape closes any open dropdown (defense-in-depth — early handlers above
        // catch each dropdown individually, but this guard ensures none slip through)
        if is_key_escape(key)
            && (self.command_bar.is_open()
                || self.new_chat_command_bar.is_open()
                || self.showing_presets_dropdown)
        {
            self.hide_all_dropdowns(cx);
            super::telemetry::log_ai_state(
                "esc_dismiss_dropdown",
                "escape_key",
                &self.debug_snapshot(),
            );
            cx.stop_propagation();
            return;
        }

        // Final Esc: close AI window and return to main menu
        if is_key_escape(key) {
            super::telemetry::log_ai_state(
                "esc_return_to_main",
                "escape_key",
                &self.debug_snapshot(),
            );
            let wb = window.window_bounds();
            crate::window_state::save_window_from_gpui(
                super::window_api::window_role_for_mode(self.window_mode),
                wb,
            );
            super::telemetry::log_ai_lifecycle(
                "ai_window_close",
                self.window_mode,
                "escape_key",
                "return_to_main",
            );
            // Clear global handle + state so reopen works correctly
            super::window_api::cleanup_ai_window_globals();
            crate::platform::dematerialize_then_remove_gpui_window(window, cx, "AI", "AI");
            // Show the main window so user lands back on the main menu
            crate::platform::show_main_window_without_activation();
            cx.stop_propagation();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_mini_history_shortcut;

    #[test]
    fn test_mini_history_shortcut_requires_cmd_j_only() {
        let enabled = is_mini_history_shortcut(
            "j",
            &gpui::Modifiers {
                platform: true,
                ..Default::default()
            },
        );
        assert!(enabled, "Cmd+J should toggle mini history");

        let wrong_key = is_mini_history_shortcut(
            "k",
            &gpui::Modifiers {
                platform: true,
                ..Default::default()
            },
        );
        assert!(!wrong_key, "Cmd+K must not match the mini history shortcut");

        let extra_shift = is_mini_history_shortcut(
            "j",
            &gpui::Modifiers {
                platform: true,
                shift: true,
                ..Default::default()
            },
        );
        assert!(
            !extra_shift,
            "Cmd+Shift+J must not match the dedicated mini history shortcut"
        );
    }
}

#[cfg(test)]
mod key_routing_tests {
    use super::is_global_focus_search_shortcut;

    #[test]
    fn cmd_shift_f_is_classified_as_global_search_shortcut() {
        assert!(is_global_focus_search_shortcut("f", true, true));
    }

    #[test]
    fn cmd_f_without_shift_is_not_search_shortcut() {
        assert!(!is_global_focus_search_shortcut("f", true, false));
    }

    #[test]
    fn wrong_key_is_not_search_shortcut() {
        assert!(!is_global_focus_search_shortcut("k", true, true));
        assert!(!is_global_focus_search_shortcut("s", true, true));
    }

    #[test]
    fn shift_f_without_platform_is_not_search_shortcut() {
        assert!(!is_global_focus_search_shortcut("f", false, true));
    }
}
