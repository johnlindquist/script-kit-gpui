// --- merged from part_000.rs ---
pub(super) fn app_shell_footer_colors(theme: &crate::theme::Theme) -> PromptFooterColors {
    PromptFooterColors::from_theme(theme)
}

pub(super) fn script_list_footer_info_label(
    window_tweaker_enabled: bool,
    is_dark_mode: bool,
    opacity_percent: i32,
    material: &str,
    appearance: &str,
) -> Option<String> {
    if window_tweaker_enabled && !is_dark_mode {
        Some(format!(
            "{}% | {} | {} | ⌘-/+ ⌘M ⌘⇧A",
            opacity_percent, material, appearance
        ))
    } else {
        None
    }
}

/// Agent Chat's contextual footer controls may disappear while its content is
/// unavailable, but that state must not remove the main window's persistent
/// footer chrome.
fn main_window_footer_chrome_should_render(
    is_agent_chat: bool,
    agent_chat_controls_visible: Option<bool>,
) -> bool {
    // This value controls Agent Chat's contextual buttons, not the chrome
    // around the main window. Keep it observable without coupling it to
    // whether the persistent footer exists.
    let _contextual_controls_hidden = is_agent_chat && agent_chat_controls_visible == Some(false);
    true
}

fn term_prompt_footer_buttons(
    enabled: bool,
    actions_open: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", "Continue").enabled(enabled),
        FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
            .selected(actions_open)
            .enabled(enabled),
        FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel").enabled(enabled),
    ]
}

fn about_footer_buttons(enabled: bool) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back").enabled(enabled)]
}

fn micro_prompt_footer_buttons(enabled: bool) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(enabled),
        FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel").enabled(enabled),
    ]
}

fn sdk_reference_footer_buttons(
    enabled: bool,
    has_selection: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", "Copy Markdown")
            .enabled(enabled && has_selection),
        FooterButtonConfig::new(FooterAction::Copy, "⌘C", "Copy").enabled(enabled && has_selection),
        FooterButtonConfig::new(FooterAction::Close, "Esc", "Back").enabled(enabled),
    ]
}

fn script_template_catalog_footer_buttons(
    enabled: bool,
    has_selection: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", "Create Local Script")
            .enabled(enabled && has_selection),
        FooterButtonConfig::new(FooterAction::Copy, "⌘C", "Copy").enabled(enabled && has_selection),
        FooterButtonConfig::new(FooterAction::Close, "Esc", "Back").enabled(enabled),
    ]
}

fn create_ai_preset_footer_buttons(
    enabled: bool,
    can_submit: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", "Save Preset")
            .enabled(enabled && can_submit),
        FooterButtonConfig::new(FooterAction::Ai, "⇥", "Next Field").enabled(enabled),
        FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel").enabled(enabled),
    ]
}

fn notes_browse_footer_buttons(
    enabled: bool,
    destination: crate::notes::search_model::NoteSearchDestination,
    has_selection: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    let close_label =
        if destination == crate::notes::search_model::NoteSearchDestination::AttachNote {
            "Cancel"
        } else {
            "Back"
        };
    vec![
        FooterButtonConfig::new(FooterAction::Run, "↵", destination.primary_verb())
            .enabled(enabled && has_selection),
        FooterButtonConfig::new(FooterAction::Close, "Esc", close_label).enabled(enabled),
    ]
}


pub(crate) fn flow_session_footer_buttons(
    working: bool,
    enabled: bool,
    actions_open: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    // Footer grammar: idle = Send · Actions · Background; working = Stop ·
    // Actions · Background. Terminate Runtime is destructive, confirmed, and
    // Actions-only; it has no hidden shortcut. No disabled "Working…"
    // pseudo-button either: the leading status text already says
    // Working/Connecting.
    //
    // Stop replaces Send while a turn is in flight. `⌘.` was already bound and
    // already cancelled the turn, but the footer never named it, so the status
    // text said the session was busy without saying how to stop it — leaving
    // `Esc Background` (walk away, it keeps running) as the only visible exit.
    let mut buttons = Vec::with_capacity(3);
    if working {
        buttons.push(
            FooterButtonConfig::new(
                FooterAction::Stop,
                crate::components::footer_chrome::FOOTER_AI_STOP_KEY,
                crate::components::footer_chrome::FOOTER_AI_STOP_LABEL,
            )
            .enabled(enabled),
        );
    } else {
        buttons.push(FooterButtonConfig::new(FooterAction::Run, "↵", "Send").enabled(enabled));
    }
    buttons.push(
        FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
            .selected(actions_open)
            .enabled(enabled),
    );
    buttons
        .push(FooterButtonConfig::new(FooterAction::Close, "Esc", "Background").enabled(enabled));
    buttons
}

/// Footer left slot for the shared main-list loading treatment: the braille
/// spinner frame for `elapsed_secs` plus the loading kind's status label.
pub(crate) fn main_list_loading_left_info(
    kind: super::main_list_loading::MainListLoadingKind,
    elapsed_secs: f32,
) -> crate::footer_popup::FooterLeftInfo {
    crate::footer_popup::FooterLeftInfo {
        model_name: kind.footer_label().to_string(),
        spinner_glyph: Some(
            crate::components::braille_loading::footer_braille_frame(elapsed_secs).to_string(),
        ),
        ..Default::default()
    }
}

pub(crate) fn compact_ai_view_type_for_mode(mode: MainWindowMode) -> ViewType {
    match mode {
        MainWindowMode::Mini => ViewType::MiniAiChat,
        MainWindowMode::Full => ViewType::DivPrompt,
    }
}

pub(crate) fn mini_prompt_view_type() -> ViewType {
    ViewType::MiniPrompt
}

fn footer_frontmost_app_name() -> Option<String> {
    crate::frontmost_app_tracker::get_last_real_app().and_then(|app| {
        let trimmed = app.name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn paste_into_frontmost_app_label(frontmost_app_name: Option<&str>) -> String {
    match frontmost_app_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        Some(app_name) => format!("Paste into {app_name}"),
        None => "Paste into Active App".to_string(),
    }
}

fn main_window_result_action_label(
    result: &crate::scripts::SearchResult,
    frontmost_app_name: Option<&str>,
) -> String {
    match result {
        crate::scripts::SearchResult::Scriptlet(sm)
            if matches!(sm.scriptlet.tool.as_str(), "paste" | "snippet") =>
        {
            paste_into_frontmost_app_label(frontmost_app_name)
        }
        _ => result
            .command_descriptor()
            .ok()
            .and_then(|descriptor| {
                descriptor
                    .primary_action()
                    .map(|action| action.title.clone())
            })
            .unwrap_or_else(|| result.get_default_action_text().to_string()),
    }
}

fn main_window_run_footer_button(
    run_label: String,
    footer_disabled: bool,
    command_block_reason: Option<&'static str>,
) -> crate::footer_popup::FooterButtonConfig {
    crate::components::footer_chrome::launcher_primary_footer_button(
        run_label,
        footer_disabled,
        command_block_reason,
    )
}

fn has_selected_clipboard_entry(app: &ScriptListApp) -> bool {
    let AppView::ClipboardHistoryView {
        filter,
        selected_index,
    } = &app.current_view
    else {
        return false;
    };

    let filtered_entries: Vec<_> = if filter.is_empty() {
        app.cached_clipboard_entries.iter().collect()
    } else {
        let filter_lower = filter.to_lowercase();
        app.cached_clipboard_entries
            .iter()
            .filter(|entry| {
                entry.text_preview.to_lowercase().contains(&filter_lower)
                    || entry
                        .ocr_text
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&filter_lower)
            })
            .collect()
    };

    filtered_entries.get(*selected_index).is_some()
}

fn has_selected_emoji_entry(app: &ScriptListApp) -> bool {
    let AppView::EmojiPickerView {
        filter,
        selected_index,
        selected_category,
    } = &app.current_view
    else {
        return false;
    };

    crate::emoji::filtered_ordered_emojis(filter, *selected_category)
        .get(*selected_index)
        .is_some()
}

fn has_selected_dictation_history_entry(app: &ScriptListApp) -> bool {
    let AppView::DictationHistoryView {
        filter,
        selected_index,
        visible_limit,
    } = &app.current_view
    else {
        return false;
    };

    app.dictation_history_current_or_previous_page(filter, *visible_limit)
        .is_some_and(|page| page.rows.get(*selected_index).is_some())
}

impl ScriptListApp {
    fn copy_selected_sdk_reference_markdown(&mut self, cx: &mut Context<Self>) -> bool {
        let selected = match &self.current_view {
            AppView::SdkReferenceView {
                filter,
                selected_index,
                entries,
            } => crate::mcp_resources::sdk_reference_visible_rows(entries, filter)
                .get(*selected_index)
                .map(|row| row.entry.clone()),
            _ => None,
        };
        let Some(entry) = selected else {
            return false;
        };

        let markdown = crate::mcp_resources::format_sdk_reference_entry_markdown(&entry);
        match crate::platform::copy_text_to_clipboard(&markdown) {
            Ok(()) => self.show_hud(format!("Copied {} reference", entry.name), Some(2000), cx),
            Err(error) => tracing::warn!(
                %error,
                "sdk_reference footer copy_text_to_clipboard failed"
            ),
        }
        true
    }

    fn selected_script_template(&self) -> Option<crate::mcp_resources::ScriptTemplateRef> {
        match &self.current_view {
            AppView::ScriptTemplateCatalogView {
                filter,
                selected_index,
                templates,
            } => crate::mcp_resources::script_template_catalog_visible_rows(templates, filter)
                .get(*selected_index)
                .map(|row| row.template.clone()),
            _ => None,
        }
    }

    fn copy_selected_script_template_markdown(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(template) = self.selected_script_template() else {
            return false;
        };

        let markdown = crate::mcp_resources::format_script_template_markdown(&template);
        match crate::platform::copy_text_to_clipboard(&markdown) {
            Ok(()) => self.show_hud(
                format!("Copied {} template", template.title),
                Some(2000),
                cx,
            ),
            Err(error) => tracing::warn!(
                %error,
                "script_template_catalog footer copy_text_to_clipboard failed"
            ),
        }
        true
    }

    fn activate_selected_note_search_result_from_footer(&mut self, cx: &mut Context<Self>) -> bool {
        let selected_row = match &self.current_view {
            AppView::NotesBrowseView { search } => Self::notes_browse_selected_visible_row(search),
            _ => None,
        };
        let Some(row) = selected_row else {
            return false;
        };
        self.activate_notes_browse_row(&row, cx)
    }

    /// Route a footer action to the Day Page kit:// resource preview when one
    /// is open (the preview's actions live on the native footer). Returns
    /// true when the action was handled by the preview.
    fn dispatch_day_page_preview_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let AppView::DayPage { entity } = &self.current_view else {
            return false;
        };
        let Some(availability) = entity.read(cx).kit_resource_preview_action_availability() else {
            return false;
        };
        let action_id = match action {
            crate::footer_popup::FooterAction::Run if availability.open_source_target.is_some() => {
                crate::DAY_PAGE_PREVIEW_OPEN_SOURCE_ACTION_ID
            }
            crate::footer_popup::FooterAction::Ai if availability.can_add_to_agent_chat => {
                crate::DAY_PAGE_PREVIEW_ADD_TO_AGENT_CHAT_ACTION_ID
            }
            crate::footer_popup::FooterAction::Copy => crate::DAY_PAGE_PREVIEW_COPY_URI_ACTION_ID,
            crate::footer_popup::FooterAction::Close => crate::DAY_PAGE_PREVIEW_CLOSE_ACTION_ID,
            _ => return false,
        };
        self.execute_day_page_action(action_id, window, cx)
    }

    pub(crate) fn main_window_footer_shortcut_is_blocked(
        &self,
        canonical_shortcut: &str,
        cx: &gpui::App,
    ) -> bool {
        self.main_window_footer_config_with_cx(Some(cx))
            .is_some_and(|config| {
                config.has_canonical_shortcut_candidate(canonical_shortcut)
                    && config
                        .action_for_canonical_shortcut(canonical_shortcut)
                        .is_none()
            })
    }

    pub(crate) fn dispatch_main_window_footer_shortcut(
        &mut self,
        canonical_shortcut: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
        source: &'static str,
    ) -> bool {
        let Some(config) = self.main_window_footer_config_with_cx(Some(&*cx)) else {
            return false;
        };
        let Some(action) = config.action_for_canonical_shortcut(canonical_shortcut) else {
            return false;
        };
        self.dispatch_main_window_footer_action(action, window, cx, source);
        true
    }

    fn try_run_ready_agent_chat_script(&mut self, cx: &mut Context<Self>) -> bool {
        if !matches!(self.current_view, AppView::AgentChatView { .. }) {
            return false;
        }
        let Some(path) = self.agent_chat_ready_script_path.clone() else {
            return false;
        };
        let path_str = path.to_string_lossy().to_string();
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "agent_chat_footer_run_dispatched",
            path = %path_str,
        );
        self.execute_script_by_path(&path_str, cx);
        true
    }

    /// Paste assistant output into the frontmost app. When `text_override` is
    /// `Some`, that text is pasted directly. Otherwise the current Agent Chat view
    /// resolves pastable text (selected focused-text variation when present,
    /// else the latest assistant message).
    pub(crate) fn paste_latest_agent_chat_response_to_frontmost(
        &mut self,
        text_override: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = text_override.or_else(|| self.latest_agent_chat_assistant_response(cx))
        else {
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "agent_chat_footer_paste_response_ignored",
                "Ignored Paste Response footer action because no assistant response exists"
            );
            return;
        };

        crate::platform::defer_hide_main_window(cx);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let injector = crate::text_injector::TextInjector::new();
            if let Err(error) = injector.paste_text(&text) {
                tracing::warn!(
                    target: "script_kit::footer_popup",
                    event = "agent_chat_footer_paste_response_failed",
                    %error,
                    "Failed to paste Agent Chat response into frontmost app"
                );
            }
        });

        tracing::info!(
            target: "script_kit::footer_popup",
            event = "agent_chat_footer_paste_response_dispatched",
            "Dispatched latest Agent Chat assistant response to frontmost app"
        );
    }

    fn quick_terminal_footer_buttons(&self) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let footer_disabled = self.main_window_footer_buttons_blocked();
        let enabled = !footer_disabled;
        let can_apply = self.quick_terminal_can_apply_back();
        let can_attach_to_agent = self.quick_terminal_can_attach_to_agent_chat();

        let mut buttons = Vec::with_capacity(if can_apply || can_attach_to_agent {
            2
        } else {
            1
        });
        if can_apply {
            buttons
                .push(FooterButtonConfig::new(FooterAction::Apply, "⌘↩", "Apply").enabled(enabled));
        } else if can_attach_to_agent {
            buttons.push(FooterButtonConfig::new(FooterAction::Ai, "⌘↩", "Agent").enabled(enabled));
        }
        buttons.push(FooterButtonConfig::new(FooterAction::Close, "⌘W", "Close").enabled(enabled));

        tracing::info!(
            target: "script_kit::footer_popup",
            event = "quick_terminal_footer_buttons_resolved",
            can_apply,
            can_attach_to_agent,
            footer_disabled,
            button_count = buttons.len(),
            "Resolved quick-terminal native footer buttons"
        );

        buttons
    }

    /// Footer buttons for an in-window `ConfirmPrompt`. Reuses the native
    /// Apply/Close slots so no AppKit ObjC selector wiring needs to change —
    /// only the labels and `selected` flag change per options + focused button.
    fn confirm_prompt_footer_buttons(
        &self,
        options: &crate::confirm::ParentConfirmOptions,
        focused_button: ConfirmFocusedButton,
    ) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let confirm_focused = matches!(focused_button, ConfirmFocusedButton::Confirm);
        let cancel_focused = matches!(focused_button, ConfirmFocusedButton::Cancel);

        vec![
            FooterButtonConfig::new(FooterAction::Apply, "↵", options.confirm_text.to_string())
                .selected(confirm_focused)
                .enabled(true),
            FooterButtonConfig::new(FooterAction::Close, "Esc", options.cancel_text.to_string())
                .selected(cancel_focused)
                .enabled(true),
        ]
    }

    fn latest_agent_chat_assistant_response(&self, cx: &App) -> Option<String> {
        let AppView::AgentChatView { entity } = &self.current_view else {
            return None;
        };

        entity.read(cx).pastable_response_text(cx)
    }
}
