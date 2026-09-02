use super::*;
use crate::ui::chrome::{alpha_from_opacity, DIVIDER_HEIGHT, DIVIDER_OPACITY};

impl ChatPrompt {
    fn footer_status_text(&self) -> gpui::SharedString {
        let mut parts: Vec<String> = Vec::new();

        if let Some(status) = &self.command_status {
            parts.push(status.clone());
        }
        if self.is_streaming() {
            parts.push("Streaming".to_string());
        } else if self.script_generation_mode {
            parts.push("Script mode".to_string());
        }

        parts.push(
            self.model
                .as_ref()
                .map(|model| model.to_string())
                .unwrap_or_else(|| "Select Model".to_string()),
        );
        parts.push("Shift+Enter newline".to_string());

        if let Some(status) = &self.script_generation_status {
            parts.push(status.to_string());
        }

        gpui::SharedString::from(parts.join(" · "))
    }

    pub(crate) fn conversation_command_bindings(
        &self,
    ) -> Vec<
        crate::components::conversation_actions::BoundConversationCommand<
            crate::components::conversation_actions::ChatPromptConversationCommand,
        >,
    > {
        use crate::components::conversation_actions::{
            ChatPromptConversationCommandFacts, ConversationCommandId,
        };
        if self.is_transcript_only() {
            return Vec::new();
        }
        let dismiss_command = self
            .dismiss_binding
            .as_ref()
            .map(|binding| match binding.route {
                ChatPromptDismissRoute::Back => ConversationCommandId::Back,
                ChatPromptDismissRoute::Close => ConversationCommandId::Close,
            });
        let active_work = self
            .dismiss_binding
            .as_ref()
            .map(|binding| binding.active_work)
            .unwrap_or_default();
        crate::components::conversation_actions::chat_prompt_conversation_commands_for_facts(
            ChatPromptConversationCommandFacts {
                response_in_progress: self.is_streaming(),
                composer_has_text: !self.input.text().trim().is_empty(),
                has_response: crate::components::conversation_actions::resolve_latest_copyable_assistant_response(
                    self.messages
                        .iter()
                        .filter(|message| !message.is_user())
                        .map(|message| message.get_content()),
                )
                .is_some(),
                submit_installed: !self.needs_setup
                    && !self.loading_providers
                    && (self.on_submit.is_some() || self.has_builtin_ai()),
                stop_installed: self.on_stop.is_some()
                    || (self.has_builtin_ai() && self.builtin_cancel_signal.is_some()),
                retry_available: self.latest_retryable_assistant_id().is_some(),
                dismiss_command,
                active_work,
            },
        )
    }

    fn conversation_footer_hints(&self) -> Vec<gpui::SharedString> {
        use crate::components::conversation_actions::ChatPromptConversationCommand;
        let commands = self.conversation_command_bindings();
        let Some(primary) = commands.iter().find(|command| {
            matches!(
                command.handler,
                ChatPromptConversationCommand::Send | ChatPromptConversationCommand::Stop
            )
        }) else {
            tracing::error!("ChatPrompt has no routable Send or Stop command for its footer");
            return Vec::new();
        };
        let Some(shortcut) = primary.descriptor.shortcut else {
            tracing::error!("ChatPrompt primary footer command does not define a shortcut");
            return Vec::new();
        };
        crate::components::universal_prompt_hints_with_primary_key_label(
            shortcut,
            primary.descriptor.label,
        )
    }

    fn render_mini_hint_strip(&self) -> impl IntoElement {
        let hints = self.conversation_footer_hints();
        crate::components::emit_prompt_hint_audit("prompts::chat::mini", &hints);
        crate::components::render_simple_hint_strip(hints, None)
    }

    fn render_gpui_footer(&self) -> AnyElement {
        if self.mini_mode() {
            return self.render_mini_hint_strip().into_any_element();
        }

        let hints = self.conversation_footer_hints();
        let helper_text = self.footer_status_text();

        tracing::info!(
            target: "script_kit::prompt_chrome",
            surface = "prompts::chat",
            footer_mode = "hint_strip",
            helper_text = %helper_text,
            is_streaming = self.is_streaming(),
            script_generation_mode = self.script_generation_mode,
            "chat_footer_built"
        );

        crate::components::emit_prompt_hint_audit("prompts::chat", &hints);

        crate::components::render_simple_hint_strip(
            hints,
            Some(crate::components::render_hint_strip_leading_text(
                helper_text,
                self.theme.colors.text.primary,
            )),
        )
        .into_any_element()
    }

    fn render_footer(&self, _cx: &mut Context<Self>) -> AnyElement {
        crate::components::prompt_layout_shell::render_main_window_footer_slot_for_prompt_surface(
            "chat_prompt",
            || self.render_gpui_footer(),
        )
    }
}

impl Focusable for ChatPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let published = crate::theme::get_theme_snapshot();
        if self.theme_revision_seen != published.revision {
            self.theme_revision_seen = published.revision;
            self.theme = published.theme.clone();
            self.prompt_colors = self.theme.colors.prompt_colors();
        }
        // Resolve the chrome + key-handler + LIFECYCLE ownership plan as the
        // FIRST operation, so a transcript-only host (flow session) suppresses
        // ALL local chrome, key handlers, AND lifecycle work in every body
        // state — it is the single lifecycle/key owner. (C-R2 bug this fixes:
        // focus/cursor-blink/pending-submit/initial-response ran BEFORE the
        // plan resolved, so a hidden transcript-only host could auto-submit,
        // steal focus, or start a blink task; setup/loading also used to
        // compose the local header even under an external host.)
        let plan =
            resolve_chat_render_plan(self.host_mode, self.needs_setup, self.loading_providers);

        // Focus lifecycle: only the host that owns its composer may grab focus.
        if plan.owns_focus {
            if self.pending_auto_focus {
                self.pending_auto_focus = false;
                self.focus_handle.focus(window, cx);
            }

            // In setup mode, ensure focus handle is focused so keyboard events route here
            if self.needs_setup {
                self.focus_handle.focus(window, cx);
            }
        }

        // Input lifecycle: cursor-blink startup, pending-submit, and
        // initial-response only run for a host that owns its local composer.
        if plan.owns_input_lifecycle {
            // Start cursor blink timer on first render (only needed when not in setup mode)
            if !self.needs_setup && !self.cursor_blink_started {
                self.cursor_blink_started = true;
                self.start_cursor_blink(cx);
            }

            // Process pending_submit on first render (used when Tab opens chat with query)
            // Skip if in setup mode or while providers are still loading
            if !self.needs_setup
                && !self.loading_providers
                && self.pending_submit
                && !self.input.is_empty()
            {
                self.pending_submit = false;
                logging::log(
                    "CHAT",
                    "Processing pending_submit - auto-submitting query from Tab",
                );
                self.handle_submit(cx);
            }

            // Process needs_initial_response on first render (used for scriptlets with pre-populated messages)
            // Skip if in setup mode or loading providers, requires built-in AI to be enabled
            if !self.needs_setup
                && !self.loading_providers
                && self.needs_initial_response
                && self.has_builtin_ai()
            {
                self.needs_initial_response = false;
                logging::log(
                    "CHAT",
                    "Processing needs_initial_response - auto-responding to initial messages",
                );
                self.handle_initial_response(cx);
            }
        }

        self.ensure_conversation_turns_cache();
        // Every assistant answer region is drawn by the SHARED selectable
        // `TextView`, which needs a persistent `TextViewState` per turn. Build
        // and extend those states here, before the turn list closure captures
        // `&self` — a region whose state is missing silently degrades to the
        // old unselectable renderer.
        self.reconcile_assistant_text_views(cx);

        let theme_colors = &self.theme.colors;

        let needs_setup = self.needs_setup;
        let on_configure = self.on_configure.clone();
        let on_claude_code = self.on_claude_code.clone();

        let handle_key = cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
            let key = event.keystroke.key.as_str();
            let key_lower = event.keystroke.key.to_ascii_lowercase();
            let key_char = event.keystroke.key_char.as_deref();
            let modifiers = &event.keystroke.modifiers;

            // Setup mode: keyboard navigation for Configure / Claude Code buttons
            if needs_setup {
                let (next_index, action, changed) = resolve_setup_card_key(
                    key,
                    event.keystroke.modifiers.shift,
                    this.setup_focus_index,
                );
                let handled = changed || !matches!(action, SetupCardAction::None);

                if changed {
                    this.setup_focus_index = next_index;
                    cx.notify();
                }

                match action {
                    SetupCardAction::ActivateConfigure => {
                        if let Some(ref callback) = on_configure {
                            logging::log("CHAT", "Setup key activate configure");
                            callback();
                        }
                    }
                    SetupCardAction::ActivateClaudeCode => {
                        if let Some(ref callback) = on_claude_code {
                            logging::log("CHAT", "Setup key activate Claude Code");
                            callback();
                        }
                    }
                    SetupCardAction::Escape => this.handle_escape(cx),
                    SetupCardAction::None => {}
                }

                if handled {
                    cx.stop_propagation();
                }
                return;
            }

            // Note: Actions menu keyboard navigation is handled by ActionsDialog window
            // We just need to handle ⌘K to open it via callback

            let commands = this.conversation_command_bindings();
            match resolve_chat_input_key_action_with_commands(
                key,
                modifiers.platform,
                modifiers.shift,
                &commands,
            ) {
                ChatInputKeyAction::Escape => {
                    // Dismissal is never Stop. Non-surviving active work is
                    // blocked by the typed dismiss binding until explicit ⌘..
                    let _ = this.request_dismiss(
                        crate::components::conversation_actions::ConversationDismissTrigger::Escape,
                        cx,
                    );
                }
                ChatInputKeyAction::StopStreaming => {
                    let _ = this.execute_conversation_command(
                        crate::components::conversation_actions::ChatPromptConversationCommand::Stop,
                        cx,
                    );
                }
                ChatInputKeyAction::Retry => {
                    let _ = this.execute_conversation_command(
                        crate::components::conversation_actions::ChatPromptConversationCommand::Retry,
                        cx,
                    );
                }
                ChatInputKeyAction::DismissCommandW => {
                    let _ = this.request_dismiss(
                        crate::components::conversation_actions::ConversationDismissTrigger::CommandW,
                        cx,
                    );
                }
                ChatInputKeyAction::JumpToLatest => {
                    this.force_scroll_turns_to_bottom();
                    this.user_has_scrolled_up = false;
                    cx.notify();
                }
                ChatInputKeyAction::ToggleActions => this.toggle_actions_menu(cx),
                ChatInputKeyAction::ContinueInChat => {
                    if this.script_generation_mode {
                        this.handle_script_generation_action(
                            ScriptGenerationAction::SaveAndRun,
                            cx,
                        );
                    } else {
                        this.handle_continue_in_chat(cx);
                    }
                }
                ChatInputKeyAction::Submit => {
                    let _ = this.execute_conversation_command(
                        crate::components::conversation_actions::ChatPromptConversationCommand::Send,
                        cx,
                    );
                }
                ChatInputKeyAction::InsertNewline => {
                    this.input.insert_char('\n');
                    this.reset_cursor_blink();
                    cx.notify();
                }
                ChatInputKeyAction::CopyLastResponse => {
                    let _ = this.execute_conversation_command(
                        crate::components::conversation_actions::ChatPromptConversationCommand::CopyLastResponse,
                        cx,
                    );
                }
                ChatInputKeyAction::ClearConversation => this.handle_clear(cx),
                ChatInputKeyAction::Paste => {
                    if !this.handle_paste_for_image(cx) {
                        this.paste_text_from_clipboard(cx);
                    }
                }
                ChatInputKeyAction::DelegateToInput => {
                    if crate::ui_foundation::is_key_backspace(key)
                        || crate::ui_foundation::is_key_delete(key)
                    {
                        if let Some((next_text, next_cursor)) =
                            crate::pasted_text::remove_pasted_text_token_at_cursor(
                                this.input.text(),
                                this.input.cursor(),
                                crate::ui_foundation::is_key_delete(key),
                                &mut this.pasted_text_tokens,
                            )
                        {
                            this.input.set_text(next_text);
                            this.input.set_cursor(next_cursor);
                            this.reset_cursor_blink();
                            cx.notify();
                            cx.stop_propagation();
                            return;
                        }
                    }

                    let handled = this.input.handle_key(
                        key_lower.as_str(),
                        key_char,
                        modifiers.platform, // Cmd key on macOS
                        modifiers.alt,
                        modifiers.shift,
                        cx,
                    );

                    if handled {
                        this.sync_pasted_text_tokens();
                        this.reset_cursor_blink();
                        cx.notify();
                    }
                }
                ChatInputKeyAction::Ignore => {}
            }
        });

        let container_bg: Option<Hsla> = get_vibrancy_background(&self.theme).map(Hsla::from);
        let input_is_focused = self.focus_handle.is_focused(window);

        // If needs_setup, render setup card instead of normal chat.
        if self.needs_setup {
            let mut root = div()
                .id("chat-prompt-setup")
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .when_some(container_bg, |d, bg| d.bg(bg));
            if plan.install_key_handlers {
                root = root
                    .key_context("chat_prompt_setup")
                    .track_focus(&self.focus_handle)
                    .on_key_down(handle_key);
            }
            if plan.render_header {
                // Header with back button and title
                root = root.child(self.render_header());
            }
            // Setup card content (the only body a transcript-only host paints).
            return root.child(self.render_setup_card(cx)).into_any_element();
        }

        // If loading_providers, show a "Connecting to AI..." placeholder
        if self.loading_providers {
            let mut root = div()
                .id("chat-prompt-loading")
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .when_some(container_bg, |d, bg| d.bg(bg));
            if plan.install_key_handlers {
                root = root
                    .key_context("chat_prompt_loading")
                    .track_focus(&self.focus_handle)
                    .on_key_down(handle_key);
            }
            if plan.render_header {
                root = root.child(self.render_header());
            }
            return root
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(theme_colors.text.secondary))
                                .child("Connecting to AI..."),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme_colors.text.tertiary))
                                .child("Loading providers and models"),
                        ),
                )
                .into_any_element();
        }

        // Input area at TOP — mini mode uses shared chrome tokens to match mini main window
        let has_pending_image = self.pending_image.is_some();
        let divider_rgba = (theme_colors.ui.border << 8) | alpha_from_opacity(DIVIDER_OPACITY);

        // Mini mode: use shared mini layout tokens to match mini main window exactly
        let mini = self.mini_mode();
        let input_px = if mini {
            crate::ui::chrome::HEADER_PADDING_X
        } else {
            CHAT_LAYOUT_PADDING_X
        };
        let input_py = if mini {
            crate::ui::chrome::HEADER_PADDING_Y
        } else {
            CHAT_LAYOUT_SECTION_PADDING_Y
        };

        // A transcript-only host owns the composer; build the internal input
        // area only when the plan says to render it.
        let input_area = plan.render_input.then(|| {
            div()
                .w_full()
                .px(px(input_px))
                .py(px(input_py))
                .flex()
                .flex_col()
                .when(!mini, |d| d.gap(px(8.0)))
                .when(mini, |d| {
                    d.border_b(px(DIVIDER_HEIGHT))
                        .border_color(rgba(divider_rgba))
                })
                .when(!mini, |d| {
                    d.border_b_1().border_color(rgba(
                        (theme_colors.ui.border << 8) | CHAT_LAYOUT_BORDER_ALPHA,
                    ))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                    this.handle_file_drop(paths, cx);
                }))
                .when(has_pending_image, |d| {
                    d.child(self.render_pending_image_preview(cx))
                })
                .child(self.render_input(input_is_focused))
        });

        // Message list (conversation turns) - virtualized for large chats
        let has_turns = !self.conversation_turns_cache.is_empty();
        let messages_content = if has_turns {
            let entity = cx.entity();
            // KNOWN: Vec clone needed to move turns snapshot into list closure; Arc-backed snapshots would avoid per-render cloning.
            let turns_snapshot = self.conversation_turns_cache.clone();
            // ONE follow-tail authority, shared with Agent Chat. This used to
            // read `user_has_scrolled_up`, a shadow flag Flow maintained
            // alongside the list's own state — two sources of truth for one
            // question, which agree right up until they don't.
            //
            // `set_follow_tail(has_turns && !user_has_scrolled_up)` below keeps
            // the ListState in sync with the flag, so this is the same answer
            // asked of the component that actually owns it.
            let show_scroll_to_latest =
                crate::components::list_scroll_affordance::should_show_jump_to_latest(
                    has_turns,
                    self.turns_list_state.is_following_tail(),
                );
            let turns_list = list(self.turns_list_state.clone(), move |ix, _window, cx| {
                entity.update(cx, |this, cx| {
                    if let Some(turn) = turns_snapshot.get(ix) {
                        div()
                            .w_full()
                            .pb(px(8.0))
                            .child(this.render_turn(turn, ix, cx))
                            .into_any_element()
                    } else {
                        div().w_full().into_any_element()
                    }
                })
            })
            .with_sizing_behavior(ListSizingBehavior::Auto)
            .size_full()
            .px(px(CHAT_LAYOUT_PADDING_X))
            .py(px(CHAT_LAYOUT_MESSAGES_PADDING_Y));

            div()
                .id("chat-messages")
                .debug_selector(|| "chat-transcript-viewport".to_string())
                .relative()
                .flex_1()
                .min_h(px(0.))
                .on_scroll_wheel(
                    cx.listener(move |this, event: &ScrollWheelEvent, _window, cx| {
                        let delta_y =
                            crate::scrolling::free_scroll::normalized_vertical_delta_px(event);
                        let direction = if delta_y > 0.0 {
                            ChatScrollDirection::Up
                        } else if delta_y < 0.0 {
                            ChatScrollDirection::Down
                        } else {
                            ChatScrollDirection::None
                        };

                        let previous_manual_mode = this.user_has_scrolled_up;
                        let at_bottom = this.turns_list_is_at_bottom();

                        tracing::debug!(
                            target: "script_kit::chat_scroll",
                            event = "wheel_observed",
                            direction = ?direction,
                            delta_y_px = delta_y,
                            at_bottom,
                            previous_manual_mode,
                            turn_count = this.conversation_turns_cache.len(),
                            scroll_top_item_ix = this.turns_list_state.logical_scroll_top().item_ix,
                        );

                        this.apply_scroll_follow_decision(
                            "wheel", direction, at_bottom, at_bottom, cx,
                        );

                        let has_turns = !this.conversation_turns_cache.is_empty();
                        this.turns_list_state
                            .set_follow_tail(has_turns && !this.user_has_scrolled_up);
                        cx.stop_propagation();
                    }),
                )
                .child(turns_list)
                .vertical_scrollbar(&self.turns_list_state)
                .when(show_scroll_to_latest, |el| {
                    el.child(
                        div()
                            .id("chat-scroll-to-latest-pill")
                            .absolute()
                            .bottom(px(12.0))
                            .left_0()
                            .right_0()
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .id("chat-scroll-to-latest-button")
                                    .px(px(10.0))
                                    .py(px(5.0))
                                    .rounded_full()
                                    .bg(rgba((theme_colors.ui.border << 8) | 0xCC))
                                    .text_color(rgb(theme_colors.text.primary))
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|d| d.bg(rgba((theme_colors.ui.border << 8) | 0xE6)))
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.force_scroll_turns_to_bottom();
                                        cx.notify();
                                    }))
                                    .child("Jump to latest"),
                            ),
                    )
                })
                .into_any_element()
        } else {
            div()
                .id("chat-messages")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .px(px(CHAT_LAYOUT_PADDING_X))
                .py(px(CHAT_LAYOUT_MESSAGES_PADDING_Y))
                .child(self.render_conversation_starters(cx))
                .into_any_element()
        };

        let mut prompt = div()
            .id("chat-prompt")
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .when_some(container_bg, |d, bg| d.bg(bg));

        // A transcript-only host is the single lifecycle/key owner: install
        // NO Enter/Escape handlers here. A standalone host owns its keys.
        if plan.install_key_handlers {
            prompt = prompt
                .key_context("chat_prompt")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key);
        }

        // Header: suppressed in mini chrome (matches the mini main window) and
        // under a transcript-only host (it draws its own identity row) — never
        // stack a second title. `plan.render_header` folds both in.
        if plan.render_header {
            prompt = prompt.child(self.render_header());
        }

        // Composer: a transcript-only host owns the shared main input with its
        // context-attachment features, so the internal input is never built
        // (`input_area` is None). Never render a second input box under it.
        if let Some(input_area) = input_area {
            prompt = prompt.child(input_area);
        }

        prompt = prompt.child(messages_content);

        // Footer: a transcript-only host owns the native footer, so the
        // internal hint strip is suppressed — one footer, and no
        // transport-mismatched model label.
        if plan.render_footer {
            prompt = prompt.child(self.render_footer(cx));
        }

        prompt.into_any_element()
    }
}

#[cfg(test)]
mod chat_footer_hint_strip_tests {
    const CHAT_RENDER_CORE_SOURCE: &str = include_str!("render_core.rs");

    #[test]
    fn test_chat_footer_uses_hint_strip_for_both_modes() {
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_main_window_footer_slot_for_prompt_surface(")
                && CHAT_RENDER_CORE_SOURCE.contains("\"chat_prompt\""),
            "Chat footer should route native-footer policy through the prompt surface helper"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_mini_hint_strip"),
            "Chat should have a mini hint strip renderer"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_gpui_footer"),
            "Chat should keep GPUI fallback footer rendering separate from native slot policy"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("if self.mini_mode"),
            "Chat footer should branch on mini_mode"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("universal_prompt_hints()"),
            "Both modes should use the shared universal prompt hints"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_simple_hint_strip("),
            "Both modes should delegate to the shared hint strip renderer"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("emit_prompt_hint_audit(\"prompts::chat::mini\""),
            "Mini hint strip should emit a prompt hint audit"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("emit_prompt_hint_audit(\"prompts::chat\""),
            "Full-mode hint strip should emit a prompt hint audit"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_hint_strip_leading_text("),
            "Full-mode footer should include leading status text"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("footer_status_text()"),
            "Full-mode footer should use the status text helper"
        );
        let render_code = &CHAT_RENDER_CORE_SOURCE[..CHAT_RENDER_CORE_SOURCE
            .find("#[cfg(test)]")
            .unwrap_or(CHAT_RENDER_CORE_SOURCE.len())];
        assert!(
            !render_code.contains("active_main_window_footer_surface()"),
            "Chat render code should not call the global native footer state directly"
        );
        // Split the needle to avoid self-matching via include_str!
        let needle = format!("{}::new", "PromptFooter");
        assert!(
            !CHAT_RENDER_CORE_SOURCE.contains(&needle),
            "Chat should no longer use PromptFooter component"
        );
    }
}
