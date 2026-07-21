use super::*;
use crate::ui_foundation::{
    is_key_backspace, is_key_delete, is_key_down, is_key_enter, is_key_escape, is_key_tab,
    is_key_up,
};

impl AiApp {
    pub(super) fn show_command_bar(
        &mut self,
        source: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.showing_mini_history_overlay {
            self.dismiss_mini_history_overlay("show_command_bar", window, cx);
        }

        tracing::info!(
            target: "ai",
            category = "AI_UI",
            event = "command_bar_open",
            window_mode = ?self.window_mode,
            source,
            "Command bar opened"
        );
        // Open the command bar (CommandBar handles window creation internally)
        self.command_bar.open(window, cx);

        // CRITICAL: Focus main focus_handle so keyboard events route to us
        // The ActionsWindow is a visual-only popup - it does NOT take keyboard focus.
        // macOS popup windows often don't receive keyboard events properly.
        // This also unfocuses the Input component which would otherwise consume arrow keys.
        self.focus_handle.focus(window, cx);

        // Request command bar focus on next render for keyboard routing
        // This ensures the focus persists even if something else tries to steal it
        self.needs_command_bar_focus = true;

        // Log focus state for debugging - check both main handle AND input's focus state
        let main_focused = self.focus_handle.is_focused(window);
        let input_focused = self
            .input_state
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        tracing::debug!(target: "ai", main_focused, input_focused, "show_command_bar focus state (input should be false for arrow keys)");

        cx.notify();
    }

    /// Hide the command bar (closes the vibrancy window) and refocus the input
    pub(super) fn hide_command_bar(&mut self, cx: &mut Context<Self>) {
        tracing::info!(
            target: "ai",
            category = "AI_UI",
            event = "command_bar_close",
            window_mode = ?self.window_mode,
            "Command bar closed"
        );
        self.command_bar.close(cx);
        // Refocus the chat input after closing the command bar
        self.request_focus(cx);
    }

    /// Handle character input in command bar
    pub(super) fn command_bar_handle_char(&mut self, ch: char, cx: &mut Context<Self>) {
        self.command_bar.handle_char(ch, cx);
    }

    /// Handle backspace in command bar
    pub(super) fn command_bar_handle_backspace(&mut self, cx: &mut Context<Self>) {
        self.command_bar.handle_backspace(cx);
    }

    /// Move selection up in command bar
    pub(super) fn command_bar_select_prev(&mut self, cx: &mut Context<Self>) {
        self.command_bar.select_prev(cx);
    }

    /// Move selection down in command bar
    pub(super) fn command_bar_select_next(&mut self, cx: &mut Context<Self>) {
        self.command_bar.select_next(cx);
    }

    /// Execute the selected command bar action
    pub(super) fn execute_command_bar_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(action_id) = self.command_bar.execute_selected_action(cx) {
            tracing::info!(target: "ai", action = %action_id, "execute_command_bar_action: executing action");
            self.execute_action(&action_id, window, cx);
            // Refocus the chat input after action execution
            // execute_selected_action closes the command bar but doesn't restore focus
            tracing::info!(target: "ai", "execute_command_bar_action: restoring focus to chat input");
            self.request_focus(cx);
        }
    }

    // === New Chat Command Bar Methods ===
    // Raycast-style dropdown in the titlebar using CommandBar component

    /// Toggle the new chat command bar dropdown
    pub(super) fn toggle_new_chat_command_bar(
        &mut self,
        source: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.new_chat_command_bar.is_open() {
            self.hide_new_chat_command_bar(cx);
        } else {
            self.show_new_chat_command_bar(source, window, cx);
        }
    }

    /// Show the new chat command bar with dynamically built actions
    pub(super) fn show_new_chat_command_bar(
        &mut self,
        source: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use crate::actions::{
            get_new_chat_actions, NewChatModelInfo, NewChatPresetInfo, WindowPosition,
        };

        if self.showing_mini_history_overlay {
            self.dismiss_mini_history_overlay("show_new_chat_command_bar", window, cx);
        }

        // Build last used settings from recent chats
        let last_used: Vec<NewChatModelInfo> = self
            .last_used_settings
            .iter()
            .map(|s| NewChatModelInfo {
                model_id: s.model_id.clone(),
                display_name: s.display_name.clone(),
                provider: s.provider.clone(),
                provider_display_name: s.provider_display_name.clone(),
            })
            .collect();

        // Build presets list
        let presets: Vec<NewChatPresetInfo> = self
            .presets
            .iter()
            .map(|p| {
                NewChatPresetInfo {
                    id: p.id.to_string(),
                    name: p.name.to_string(),
                    icon: p.icon, // Use the preset's icon
                }
            })
            .collect();

        // Build models list
        let models: Vec<NewChatModelInfo> = self
            .available_models
            .iter()
            .map(|m| {
                let provider_display = match m.provider.as_str() {
                    "anthropic" => "Anthropic",
                    "openai" => "OpenAI",
                    "google" => "Google",
                    "groq" => "Groq",
                    "openrouter" => "OpenRouter",
                    "vercel" => "Vercel",
                    _ => &m.provider,
                }
                .to_string();
                NewChatModelInfo {
                    model_id: m.id.clone(),
                    display_name: m.display_name.clone(),
                    provider: m.provider.clone(),
                    provider_display_name: provider_display,
                }
            })
            .collect();

        // Build actions and update the command bar
        let actions = get_new_chat_actions(&last_used, &presets, &models);
        self.new_chat_command_bar.set_actions(actions, cx);

        tracing::info!(
            target: "ai",
            category = "AI_UI",
            event = "new_chat_menu_open",
            window_mode = ?self.window_mode,
            source,
            last_used_count = last_used.len(),
            preset_count = presets.len(),
            model_count = models.len(),
            "New chat menu opened"
        );

        // Open at top-right position (below titlebar)
        self.new_chat_command_bar
            .open_at_position(window, cx, WindowPosition::TopRight);

        // Focus main handle for keyboard routing
        self.focus_handle.focus(window, cx);

        // Also hide other dropdowns
        self.hide_presets_dropdown(cx);

        cx.notify();
    }

    /// Hide the new chat command bar
    pub(super) fn hide_new_chat_command_bar(&mut self, cx: &mut Context<Self>) {
        self.new_chat_command_bar.close(cx);
        self.request_focus(cx);
    }

    /// Execute the selected new chat action
    pub(super) fn execute_new_chat_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(action_id) = self.new_chat_command_bar.execute_selected_action(cx) {
            self.handle_new_chat_action(&action_id, window, cx);
        }
    }

    /// Handle action from the new chat dropdown
    pub(super) fn handle_new_chat_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action_id.starts_with("last_used_") {
            // Parse index from action ID
            if let Some(idx_str) = action_id.strip_prefix("last_used_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(setting) = self.last_used_settings.get(idx) {
                        let model_id = setting.model_id.clone();
                        let provider = setting.provider.clone();
                        self.create_chat_with_model(&model_id, &provider, window, cx);
                    }
                }
            }
        } else if action_id.starts_with("preset_") {
            // Parse preset ID
            if let Some(preset_id) = action_id.strip_prefix("preset_") {
                if let Some(idx) = self.presets.iter().position(|p| p.id == preset_id) {
                    self.presets_selected_index = idx;
                    self.create_chat_with_preset(window, cx);
                }
            }
        } else if action_id.starts_with("model_") {
            // Parse model index
            if let Some(idx_str) = action_id.strip_prefix("model_") {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    if let Some(model) = self.available_models.get(idx) {
                        let model_id = model.id.clone();
                        let provider = model.provider.clone();
                        self.create_chat_with_model(&model_id, &provider, window, cx);
                    }
                }
            }
        } else {
            tracing::warn!(
                target: "ai",
                category = "AI_UI",
                event = "new_chat_action_unresolved",
                window_mode = ?self.window_mode,
                action_id,
                last_used_count = self.last_used_settings.len(),
                preset_count = self.presets.len(),
                model_count = self.available_models.len(),
                "New chat action could not be resolved"
            );
        }
    }

    /// Move selection up in new chat dropdown
    pub(super) fn new_chat_command_bar_select_prev(&mut self, cx: &mut Context<Self>) {
        self.new_chat_command_bar.select_prev(cx);
    }

    /// Move selection down in new chat dropdown
    pub(super) fn new_chat_command_bar_select_next(&mut self, cx: &mut Context<Self>) {
        self.new_chat_command_bar.select_next(cx);
    }

    /// Execute an action by ID
    pub(super) fn execute_action(
        &mut self,
        action_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(target: "ai", action = %action_id, "execute_action: dispatching");

        // Context attachment actions — resolved via the canonical contract
        if let Some(kind) =
            crate::ai::context_contract::ContextAttachmentKind::from_action_id(action_id)
        {
            self.add_context_part(kind.part(), cx);
            return;
        }

        if crate::ai::context_contract::is_clear_context_action(action_id) {
            self.clear_context_parts(cx);
            return;
        }

        let action_id = action_id.strip_prefix("chat:").unwrap_or(action_id);

        match action_id {
            "copy_response" => self.copy_last_response(cx),
            "copy_chat" => self.copy_entire_chat(cx),
            "copy_last_code" => self.copy_last_code_block(cx),
            "submit" => self.submit_message(window, cx),
            "new_chat" => {
                self.new_conversation(window, cx);
            }
            "delete_chat" => {
                self.delete_selected_chat(cx);
            }
            "add_file" => self.open_file_picker(cx),
            "add_image" => self.open_image_picker(cx),
            "paste_image" => self.paste_image_from_clipboard(cx),
            "capture_screen_area" => {
                self.capture_screen_area_attachment(cx);
            }
            "change_model" => {
                self.cycle_model(cx);
            }
            "inspect_context" => {
                self.toggle_context_inspector(cx);
            }
            "export_markdown" => {
                self.export_chat_to_clipboard(cx);
            }
            "toggle_window_mode" => {
                self.toggle_window_mode(window, cx);
            }
            _ => {
                tracing::warn!(action = action_id, "Unknown action");
            }
        }
    }

    /// Handle a simulated key press (for testing via stdin)
    pub(super) fn handle_simulated_key(
        &mut self,
        key: &str,
        modifiers: &[KeyModifier],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Mirror the physical handler: clear any stale CommandBar `is_open`
        // state so simulated Cmd+K / Cmd+P can reopen after an external
        // dismissal of the detached actions window.
        self.command_bar.reconcile_open_state();
        self.new_chat_command_bar.reconcile_open_state();

        let has_cmd = modifiers.contains(&KeyModifier::Cmd);
        let key_lower = key.to_lowercase();

        tracing::debug!(target: "ai", key = %key_lower, ?modifiers, command_bar_open = self.command_bar.is_open(), "SimulateKey received");

        // Handle Cmd+K to toggle command bar
        if has_cmd && key_lower == "k" {
            tracing::debug!(target: "ai", "SimulateKey: Cmd+K - toggling command bar");
            if self.command_bar.is_open() {
                self.hide_command_bar(cx);
            } else {
                self.hide_all_dropdowns(cx);
                self.show_command_bar("simulated_cmd_k", window, cx);
            }
            return;
        }

        // Handle Cmd+J to toggle mini history overlay
        if has_cmd && key_lower == "j" && self.window_mode.is_mini() {
            tracing::debug!(target: "ai", "SimulateKey: Cmd+J - toggling mini history overlay");
            self.toggle_mini_history_overlay("simulated_cmd_j", window, cx);
            return;
        }

        // Handle Cmd+Shift shortcuts
        let has_shift = modifiers.contains(&KeyModifier::Shift);
        if has_cmd && has_shift {
            match key_lower.as_str() {
                "m" => {
                    tracing::debug!(target: "ai", "SimulateKey: Cmd+Shift+M - toggling window mode");
                    self.toggle_window_mode(window, cx);
                    return;
                }
                "f" => {
                    tracing::debug!(target: "ai", "SimulateKey: Cmd+Shift+F - focusing search");
                    if self.window_mode.is_mini() {
                        self.hide_all_dropdowns(cx);
                        self.show_mini_history_overlay("simulated_cmd_shift_f", window, cx);
                    } else {
                        if self.sidebar_collapsed {
                            self.sidebar_collapsed = false;
                        }
                        self.hide_all_dropdowns(cx);
                        self.focus_search(window, cx);
                    }
                    return;
                }
                "n" => {
                    tracing::debug!(target: "ai", "SimulateKey: Cmd+Shift+N - presets dropdown");
                    self.hide_all_dropdowns(cx);
                    self.show_presets_dropdown(window, cx);
                    return;
                }
                _ => {}
            }
        }

        // Handle Cmd+N for new chat
        if has_cmd && key_lower == "n" {
            tracing::debug!(target: "ai", "SimulateKey: Cmd+N - new conversation");
            super::observability::emit_ai_ui_event(
                &super::observability::AiUiEvent {
                    kind: super::types::AiUiEventKind::ShortcutDecision,
                    action: "new_conversation",
                    source: "handle_simulated_key",
                    window_mode: self.window_mode,
                    selected_chat_id: self.selected_chat_id.as_ref(),
                    overlay_visible: self.showing_mini_history_overlay,
                    search_active: !self.search_query.is_empty(),
                },
                None,
            );
            self.new_conversation(window, cx);
            return;
        }

        // Handle command bar navigation when it's open
        if self.command_bar.is_open() {
            match key_lower.as_str() {
                k if is_key_up(k) => {
                    tracing::debug!(target: "ai", "SimulateKey: Up in command bar");
                    self.command_bar_select_prev(cx);
                }
                k if is_key_down(k) => {
                    tracing::debug!(target: "ai", "SimulateKey: Down in command bar");
                    self.command_bar_select_next(cx);
                }
                k if is_key_enter(k) => {
                    tracing::debug!(target: "ai", "SimulateKey: Enter in command bar");
                    self.execute_command_bar_action(window, cx);
                }
                k if is_key_escape(k) => {
                    tracing::debug!(target: "ai", "SimulateKey: Escape - closing command bar");
                    self.hide_command_bar(cx);
                }
                k if is_key_backspace(k) || is_key_delete(k) => {
                    tracing::debug!(target: "ai", "SimulateKey: Backspace in command bar");
                    self.command_bar_handle_backspace(cx);
                }
                _ => {
                    // Handle printable characters for search
                    if let Some(ch) = key_lower.chars().next() {
                        if ch.is_alphanumeric() || ch.is_whitespace() || ch == '-' || ch == '_' {
                            tracing::debug!(target: "ai", char = %ch, "SimulateKey: Typing in command bar search");
                            self.command_bar_handle_char(ch, cx);
                        }
                    }
                }
            }
            return;
        }

        // Handle presets dropdown navigation
        if self.showing_presets_dropdown {
            match key_lower.as_str() {
                k if is_key_up(k) => self.presets_select_prev(cx),
                k if is_key_down(k) => self.presets_select_next(cx),
                k if is_key_enter(k) => self.create_chat_with_preset(window, cx),
                k if is_key_escape(k) => self.hide_presets_dropdown(cx),
                _ => {}
            }
            return;
        }

        // Handle mini history overlay navigation when visible
        if self.window_mode.is_mini() && self.showing_mini_history_overlay {
            match key_lower.as_str() {
                k if is_key_up(k) => {
                    self.navigate_chat_preserving_mini_overlay(1, window, cx);
                    return;
                }
                k if is_key_down(k) => {
                    self.navigate_chat_preserving_mini_overlay(-1, window, cx);
                    return;
                }
                k if is_key_enter(k) => {
                    self.dismiss_mini_history_overlay("simulated_enter", window, cx);
                    return;
                }
                k if is_key_escape(k) => {
                    self.dismiss_mini_history_overlay("simulated_escape", window, cx);
                    return;
                }
                _ => {}
            }
        }

        // Handle setup mode navigation (when no providers configured)
        let in_setup_mode = self.available_models.is_empty() && !self.showing_api_key_input;
        if in_setup_mode {
            tracing::debug!(target: "ai", key = %key_lower, focus_index = self.setup_button_focus_index, "SimulateKey in setup mode");
            let has_shift = modifiers.contains(&KeyModifier::Shift);
            match key_lower.as_str() {
                k if is_key_tab(k) => {
                    if has_shift {
                        self.move_setup_button_focus(-1, cx);
                    } else {
                        self.move_setup_button_focus(1, cx);
                    }
                    return;
                }
                k if is_key_up(k) => {
                    self.move_setup_button_focus(-1, cx);
                    return;
                }
                k if is_key_down(k) => {
                    self.move_setup_button_focus(1, cx);
                    return;
                }
                k if is_key_enter(k) => {
                    if self.setup_button_focus_index == 0 {
                        self.open_agent_chat_agents_catalog(window, cx);
                    }
                    return;
                }
                _ => {}
            }
        }

        // Handle API key input escape
        if self.showing_api_key_input && key_lower == "escape" {
            self.hide_api_key_input(window, cx);
            return;
        }

        // Esc chain — mirrors handle_root_key_down guard sequence so SimulateKey
        // produces the same single-step-at-a-time behavior as a real keypress.
        if is_key_escape(&key_lower) {
            // 1. Mini history overlay
            if self.window_mode.is_mini() && self.showing_mini_history_overlay {
                self.dismiss_mini_history_overlay("simulated_escape", window, cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - dismissed mini history overlay");
                return;
            }
            // 2. Shortcuts overlay
            if self.showing_shortcuts_overlay {
                self.showing_shortcuts_overlay = false;
                tracing::info!(target: "ai", "SimulateKey: Escape - dismissed shortcuts overlay");
                cx.notify();
                return;
            }
            // 3. Active search
            if !self.search_query.is_empty() {
                self.search_query.clear();
                self.search_generation += 1;
                self.search_snippets.clear();
                self.search_matched_title.clear();
                self.chats = crate::ai::storage::get_all_chats().unwrap_or_default();
                self.search_state.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                });
                self.focus_input(window, cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - cleared search");
                cx.notify();
                return;
            }
            // 4. Editing mode
            if self.editing_message_id.is_some() {
                self.editing_message_id = None;
                self.clear_composer(window, cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - cancelled edit");
                return;
            }
            // 5. Rename
            if self.renaming_chat_id.is_some() {
                self.cancel_rename(cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - cancelled rename");
                return;
            }
            // 6. Streaming
            if self.is_streaming {
                self.stop_streaming(cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - stopped streaming");
                return;
            }
            // 7. New chat command bar
            if self.new_chat_command_bar.is_open() {
                self.hide_new_chat_command_bar(cx);
                tracing::info!(target: "ai", "SimulateKey: Escape - closed new chat command bar");
                return;
            }
            // 8. Final mini close
            if self.window_mode.is_mini() {
                let wb = window.window_bounds();
                crate::window_state::save_window_from_gpui(
                    super::window_api::window_role_for_mode(self.window_mode),
                    wb,
                );
                super::telemetry::log_ai_lifecycle(
                    "ai_window_close",
                    self.window_mode,
                    "simulated_escape",
                    "closing",
                );
                super::window_api::cleanup_ai_window_globals();
                crate::platform::dematerialize_then_remove_gpui_window(window, cx, "AI", "AI");
                tracing::info!(target: "ai", "SimulateKey: Escape - closed mini window");
                return;
            }
            // Full mode: Esc with nothing to dismiss is a no-op
            tracing::debug!(target: "ai", "SimulateKey: Escape - nothing to dismiss");
            return;
        }

        tracing::debug!(target: "ai", key = %key_lower, "SimulateKey: Unhandled key in AI window");
    }

    /// Copy the last AI response to clipboard
    pub(super) fn copy_last_response(&self, cx: &mut Context<Self>) {
        // Find the last assistant message
        if let Some(last_response) = self
            .current_messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::Assistant)
        {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                last_response.content.clone(),
            ));
            info!("Copied last response to clipboard");
        }
    }

    /// Copy the entire chat to clipboard
    pub(super) fn copy_entire_chat(&self, cx: &mut Context<Self>) {
        let chat_text: String = self
            .current_messages
            .iter()
            .map(|m| {
                let role = if m.role == MessageRole::User {
                    "You"
                } else {
                    "AI"
                };
                format!("**{}**: {}\n\n", role, m.content)
            })
            .collect();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(chat_text));
        info!("Copied entire chat to clipboard");
    }

    /// Copy the last code block from AI response
    pub(super) fn copy_last_code_block(&self, cx: &mut Context<Self>) {
        // Find the last assistant message with a code block
        for msg in self.current_messages.iter().rev() {
            if msg.role == MessageRole::Assistant {
                // Simple regex-like search for code blocks
                if let Some(start) = msg.content.find("```") {
                    let after_start = &msg.content[start + 3..];
                    // Find the end of the language identifier (newline)
                    if let Some(lang_end) = after_start.find('\n') {
                        let code_start = &after_start[lang_end + 1..];
                        if let Some(end) = code_start.find("```") {
                            let code = &code_start[..end];
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                code.to_string(),
                            ));
                            info!("Copied last code block to clipboard");
                            return;
                        }
                    }
                }
            }
        }
        info!("No code block found to copy");
    }

    /// Paste image from clipboard as attachment.
    ///
    /// Delegates to `handle_paste_for_image` which reads clipboard via arboard,
    /// encodes any image as base64 PNG, and sets it as the pending image attachment.
    pub(super) fn paste_image_from_clipboard(&mut self, cx: &mut Context<Self>) {
        info!(
            action = "paste_image_from_clipboard",
            "Checking clipboard for image data"
        );
        if self.handle_paste_for_image(cx) {
            info!(
                action = "paste_image_from_clipboard_success",
                "Image pasted from clipboard"
            );
        } else {
            info!(
                action = "paste_image_from_clipboard_none",
                "No image found in clipboard"
            );
        }
    }
}

#[cfg(test)]
mod mini_history_overlay_key_routing_tests {
    use std::fs;

    #[test]
    fn overlay_up_and_down_route_through_preserving_navigation() {
        let source = fs::read_to_string("src/ai/window/command_bar.rs")
            .expect("Failed to read src/ai/window/command_bar.rs");

        let overlay_start = source
            .find("if self.window_mode.is_mini() && self.showing_mini_history_overlay {")
            .expect("mini history overlay key routing block not found");
        let overlay_block = &source[overlay_start..];

        let up_branch_start = overlay_block
            .find("k if is_key_up(k) => {")
            .expect("up branch missing");
        let down_branch_start = overlay_block
            .find("k if is_key_down(k) => {")
            .expect("down branch missing");
        let enter_branch_start = overlay_block
            .find("k if is_key_enter(k) => {")
            .expect("enter branch missing");

        let up_call_pos = overlay_block[up_branch_start..down_branch_start]
            .find("self.navigate_chat_preserving_mini_overlay(1, window, cx);")
            .expect("up branch must call preserving navigation");
        let down_call_pos = overlay_block[down_branch_start..enter_branch_start]
            .find("self.navigate_chat_preserving_mini_overlay(-1, window, cx);")
            .expect("down branch must call preserving navigation");

        assert!(up_call_pos < (down_branch_start - up_branch_start));
        assert!(down_call_pos < (enter_branch_start - down_branch_start));
    }
}
