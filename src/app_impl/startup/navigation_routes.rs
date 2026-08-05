struct SecondarySurfaceWindowState {
    is_notes: bool,
    is_detached_agent_chat: bool,
    is_shortcut_recorder: bool,
}

impl SecondarySurfaceWindowState {
    fn inspect(window: &Window) -> Self {
        Self {
            is_notes: crate::notes::is_notes_window(window),
            is_detached_agent_chat: crate::ai::agent_chat::ui::chat_window::is_chat_window(window),
            is_shortcut_recorder: super::shortcut_recorder::is_shortcut_recorder_window(window),
        }
    }

    fn is_secondary(&self) -> bool {
        self.is_notes || self.is_detached_agent_chat || self.is_shortcut_recorder
    }
}

fn is_secondary_surface_window(window: &Window) -> bool {
    SecondarySurfaceWindowState::inspect(window).is_secondary()
}

impl ScriptListApp {
    fn route_day_page_note_switcher_key(
        &mut self,
        key: &str,
        key_char: Option<&str>,
        modifiers: &gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::DayPage { entity } = &self.current_view else {
            return false;
        };
        if !entity.read(cx).is_day_switcher_open() {
            return false;
        }

        if crate::actions::route_key_to_detached_actions_window(key, key_char, modifiers, cx) {
            return true;
        }

        let entity = entity.clone();
        let key_lower = key.to_ascii_lowercase();
        entity.update(cx, |view, cx| {
            view.handle_day_switcher_key(
                &key_lower,
                modifiers.platform,
                modifiers.shift,
                modifiers.alt,
                modifiers.control,
                window,
                cx,
            )
        })
    }

    /// Exit cwd-pick mode (Tab → FileSearchView) and return to the main menu
    /// without setting a cwd. Used by Escape and by the second Backspace at
    /// the disk root.
    fn exit_cwd_pick_to_main_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        tracing::info!(
            target: "script_kit::spine",
            event = "cwd_pick_exit_to_main_menu",
            "Left cwd-pick FileSearchView without selecting a cwd"
        );
        self.cwd_pick_mode = false;
        self.reset_to_script_list(cx);
        self.clear_filter(window, cx);
    }

    /// Re-seed the cwd-pick FileSearchView with `query` (e.g. "/"), keeping the
    /// shared gpui input in sync without re-triggering the filter change
    /// handler.
    fn reseed_cwd_pick_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_search_view(query.to_string(), FileSearchPresentation::Full, cx);
        self.suppress_filter_events = true;
        self.gpui_input_state.update(cx, |state, cx| {
            state.set_value(query.to_string(), window, cx);
            let len = query.len();
            state.set_selection(len, len, window, cx);
        });
        self.suppress_filter_events = false;
    }

    /// Handle Escape / Backspace inside the cwd-pick FileSearchView.
    ///
    /// - Escape: return to the main menu in one press.
    /// - Backspace from "~/": collapse to "/" (disk root).
    /// - Backspace from "/": return to the main menu (so two deletes from the
    ///   initial "~/" land back on the launcher).
    ///
    /// Returns `true` when the key was consumed. Any other state (deeper paths,
    /// modified keys) is left to normal input editing.
    fn try_handle_cwd_pick_nav_key(
        &mut self,
        event: &gpui::KeystrokeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.cwd_pick_mode || !matches!(self.current_view, AppView::FileSearchView { .. }) {
            return false;
        }

        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        let has_modifier =
            modifiers.platform || modifiers.alt || modifiers.control || modifiers.shift;

        if crate::ui_foundation::is_key_escape(key) && !has_modifier {
            self.exit_cwd_pick_to_main_menu(window, cx);
            return true;
        }

        let is_backspace =
            key.eq_ignore_ascii_case("backspace") || key.eq_ignore_ascii_case("delete");
        if is_backspace && !has_modifier {
            let query = match &self.current_view {
                AppView::FileSearchView { query, .. } => query.clone(),
                _ => return false,
            };
            match query.as_str() {
                "~/" => {
                    tracing::info!(
                        target: "script_kit::spine",
                        event = "cwd_pick_backspace_to_disk_root",
                        "Backspace collapsed cwd-pick query from ~/ to /"
                    );
                    self.reseed_cwd_pick_query("/", window, cx);
                    return true;
                }
                "/" => {
                    self.exit_cwd_pick_to_main_menu(window, cx);
                    return true;
                }
                _ => return false,
            }
        }

        false
    }

    fn close_main_window_from_top_level_cmd_w(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::keyboard",
            event = "top_level_cmd_w_close_main_window",
            current_view = %self.app_view_name(),
            actions_open = crate::actions::is_actions_window_open(),
            confirm_open = crate::confirm::is_confirm_window_open(),
            show_actions_popup = self.show_actions_popup,
        );

        if crate::confirm::is_confirm_window_open() {
            crate::confirm::route_key_to_confirm_popup("escape", cx);
            return;
        }

        if crate::actions::is_actions_window_open() || self.show_actions_popup {
            self.close_actions_popup_for_current_view(window, cx);
            return;
        }

        match &self.current_view {
            AppView::QuickTerminalView { .. } => {
                self.close_quick_terminal_main_window_state_first(cx);
            }
            AppView::AgentChatView { entity } => {
                let entity = entity.clone();
                entity.update(cx, |chat, cx| {
                    let _ = chat.request_conversation_dismiss(
                        crate::components::conversation_actions::ConversationDismissTrigger::CommandW,
                        window,
                        cx,
                    );
                });
            }
            AppView::ThemeChooserView { .. } => {
                // Memory-only restore: a cancelled Theme Designer session never
                // writes theme.json (nothing was persisted while previewing).
                if let Some(original) = self.theme_before_chooser.take() {
                    self.restore_theme_chooser_theme(
                        original,
                        "theme_chooser_top_level_cmd_w_undo",
                        cx,
                    );
                }
                self.clear_theme_chooser_controls();
                self.close_and_reset_window(cx);
            }
            _ => {
                self.close_and_reset_window(cx);
            }
        }
    }
}
