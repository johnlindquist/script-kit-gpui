use super::*;
use std::sync::Once;

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
    in_portal: bool,
    has_selection: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    let close_label = if in_portal { "Cancel" } else { "Back" };
    let mut buttons = Vec::new();
    if in_portal {
        buttons.push(
            FooterButtonConfig::new(FooterAction::Run, "↵", "Attach Note")
                .enabled(enabled && has_selection),
        );
    }
    buttons.push(FooterButtonConfig::new(FooterAction::Close, "Esc", close_label).enabled(enabled));
    buttons
}

static MAIN_FOOTER_ACTION_LISTENER: Once = Once::new();

pub(crate) fn flow_session_footer_buttons(
    working: bool,
    enabled: bool,
    actions_open: bool,
) -> Vec<crate::footer_popup::FooterButtonConfig> {
    use crate::footer_popup::{FooterAction, FooterButtonConfig};

    // Footer grammar (Oracle 2026-07-21, Footer-A): idle = Send · Actions ·
    // Desk; working = Actions · Desk. No permanent Terminate — it is a
    // destructive expert command that lives in ⌘K Actions (the ⇧⌘⎋ shortcut
    // still works; macOS eats plain ⌘⎋ at the window server, verified
    // 2026-07-11). No disabled "Working…" pseudo-button either: the leading
    // status text already says Working/Connecting.
    let mut buttons = Vec::with_capacity(3);
    if !working {
        buttons.push(FooterButtonConfig::new(FooterAction::Run, "↵", "Send").enabled(enabled));
    }
    buttons.push(
        FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
            .selected(actions_open)
            .enabled(enabled),
    );
    buttons.push(FooterButtonConfig::new(FooterAction::Close, "Esc", "Desk").enabled(enabled));
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

/// Thin wrapper delegating to the canonical implementation in `window_resize`.
fn main_window_sizing_from_grouped_items(
    grouped_items: &[GroupedListItem],
) -> crate::window_resize::MainWindowSizing {
    crate::window_resize::main_window_sizing_from_grouped_items(grouped_items)
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
        _ => result.get_default_action_text().to_string(),
    }
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
    } = &app.current_view
    else {
        return false;
    };

    crate::dictation::search_history(filter, 100)
        .get(*selected_index)
        .is_some()
}

impl ScriptListApp {
    pub(crate) fn main_window_primary_action_label(&self) -> String {
        let frontmost_app_name = footer_frontmost_app_name();

        match &self.current_view {
            AppView::ClipboardHistoryView { .. } => {
                return if has_selected_clipboard_entry(self) {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                } else {
                    "Run".to_string()
                };
            }
            AppView::EmojiPickerView { .. } => {
                return if has_selected_emoji_entry(self) {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                } else {
                    "Run".to_string()
                };
            }
            AppView::DictationHistoryView { .. } => {
                return if has_selected_dictation_history_entry(self) {
                    paste_into_frontmost_app_label(frontmost_app_name.as_deref())
                } else {
                    "Run".to_string()
                };
            }
            AppView::ThemeChooserView { .. } => {
                return "Apply".to_string();
            }
            AppView::ProfileSearchView { .. } => {
                return "Switch Profile".to_string();
            }
            AppView::ScriptList => {}
            _ => return "Run".to_string(),
        }

        // Unarmed empty colon mode: no row is selected, so the footer must
        // not advertise the internal selection's verb ("Attach ↵") while
        // Enter is consumed without attaching. Mirror the ghost-text
        // affordance instead.
        if self.spine_empty_subsearch_selection_suppressed() {
            return "Type to Search".to_string();
        }

        if let Some(label) = self.menu_syntax_filter_accept_primary_label() {
            return label.to_string();
        }

        let Some(selected_index) = crate::list_item::coerce_selection(
            self.main_menu_result_caches.grouped_items(),
            self.selected_index,
        ) else {
            return "Run".to_string();
        };

        let Some(result_idx) = self
            .main_menu_result_caches
            .flat_result_index_for_grouped_item(selected_index)
        else {
            return "Run".to_string();
        };

        if self
            .inline_calculator_for_result_index(result_idx)
            .is_some()
        {
            return "Copy".to_string();
        }

        self.main_menu_result_caches
            .search_result_for_flat_index(result_idx)
            .map(|result| main_window_result_action_label(result, frontmost_app_name.as_deref()))
            .unwrap_or_else(|| "Run".to_string())
    }

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

    fn attach_selected_note_from_footer(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.is_in_attachment_portal() {
            return false;
        }
        let selected_note = match &self.current_view {
            AppView::NotesBrowseView {
                filter,
                selected_index,
            } => Self::notes_browse_selected_visible_row(filter, *selected_index),
            _ => None,
        };
        let Some(note) = selected_note else {
            return false;
        };

        let part = self.build_notes_browse_portal_part(&note);
        self.close_attachment_portal_with_part(part, cx);
        true
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

    pub(crate) fn dispatch_main_window_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_action_dispatch",
            source,
            action = ?action,
            view = ?self.current_view,
            main_window_mode = ?self.main_window_mode,
            "Dispatching main-window footer action"
        );

        // Standard macOS menu dismissal: clicking a real footer button while
        // the actions popup is open closes the popup AND performs the clicked
        // action in the same event. Swallowing the click (close-only) made
        // visible, enabled-looking footer buttons dead until a second click.
        let shared_actions_open = self.show_actions_popup;
        let detached_actions_open = crate::actions::is_actions_window_open();
        if (shared_actions_open || detached_actions_open) && !action.is_actions() {
            let mut closed = false;
            if let super::actions_dialog::ActionsSupport::SharedDialog(host) =
                self.actions_support_for_view()
            {
                self.close_actions_popup(host, window, cx);
                closed = true;
            }
            if detached_actions_open {
                crate::actions::close_actions_window(cx);
                closed = true;
            }
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_action_closed_actions_then_dispatched",
                source,
                action = ?action,
                main_window_mode = ?self.main_window_mode,
                closed,
                "Closed actions dialog from footer click, dispatching the clicked action"
            );
        }

        match action {
            crate::footer_popup::FooterAction::Tips => {
                let builtins = crate::config::load_config().get_builtins();
                if let Some(entry) =
                    crate::builtins::resolve_builtin_entry("builtin/tips", &builtins)
                {
                    self.execute_builtin(&entry, cx);
                }
            }
            crate::footer_popup::FooterAction::Run => {
                if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
                    self.dispatch_permissions_wizard_action(
                        crate::permissions_wizard::PermissionsWizardAction::GrantSelected,
                        window,
                        cx,
                    );
                } else if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.submit_with_expanded_tokens(cx);
                    });
                } else if let AppView::TermPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |term, _cx| {
                        if let Err(error) = term.send_raw_input("\r") {
                            tracing::warn!(
                                target: "script_kit::footer_popup",
                                event = "term_prompt_footer_continue_failed",
                                %error,
                                "Failed to send Enter from terminal footer"
                            );
                        }
                    });
                } else if let AppView::MicroPrompt { id, .. } = &self.current_view {
                    let prompt_id = id.clone();
                    self.submit_arg_prompt_from_current_state(&prompt_id, cx);
                } else if matches!(self.current_view, AppView::SdkReferenceView { .. }) {
                    let _ = self.copy_selected_sdk_reference_markdown(cx);
                } else if matches!(self.current_view, AppView::ScriptTemplateCatalogView { .. }) {
                    if let Some(template) = self.selected_script_template() {
                        self.show_naming_dialog_for_script_template(template, window, cx);
                    }
                } else if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.handle_create_ai_preset_key("enter", window, cx);
                } else if matches!(self.current_view, AppView::NotesBrowseView { .. }) {
                    let _ = self.attach_selected_note_from_footer(cx);
                } else if let AppView::FlowSessionView { session_id } = self.current_view {
                    // One shared draft transaction with the keyboard Enter
                    // path: the draft clears only when the submit consumed it,
                    // so a Busy race can never destroy it (Oracle 2026-07-21).
                    let _ = self.submit_flow_session_draft(session_id, window, cx);
                } else if matches!(self.current_view, AppView::FlowUxView { .. }) {
                    self.flow_desk_activate_selected(false, window, cx);
                } else if let AppView::ScriptIssuesView { report } = &self.current_view {
                    let report = report.clone();
                    self.fix_script_issues_in_agent(&report, cx);
                } else if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                } else if self.dispatch_kit_store_primary_footer_action(cx) {
                } else if self.dispatch_migrate_v1_primary_footer_action(cx) {
                } else if matches!(self.current_view, AppView::ThemeChooserView { .. }) {
                    self.submit_theme_chooser_from_input_enter(window, cx);
                } else if matches!(self.current_view, AppView::TipsView { .. }) {
                    self.tips_copy_selected_example(cx);
                } else if let AppView::TemplatePrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.submit(cx));
                } else if let AppView::EditorPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |editor, cx| editor.submit(cx));
                } else if matches!(self.current_view, AppView::WebcamView { .. }) {
                    if self.capture_webcam_photo(cx) {
                        self.hide_main_and_reset(cx);
                    }
                } else if let AppView::PathPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.handle_enter(cx));
                } else if let AppView::EnvPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.submit(cx));
                } else if let AppView::DropPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, _cx| prompt.submit());
                } else if let AppView::FormPrompt { id, entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    let prompt_id = id.clone();
                    let validation_message = entity.read(cx).submit_validation_message(cx);
                    if let Some(message) = validation_message {
                        self.show_hud(message, Some(HUD_LONG_MS), cx);
                        return;
                    }
                    let values = entity.read(cx).collect_values(cx);
                    self.submit_prompt_response(prompt_id, Some(values), cx);
                } else if !self.try_run_ready_agent_chat_script(cx) {
                    self.execute_selected(cx);
                }
            }
            crate::footer_popup::FooterAction::Actions => {
                let handled = self.dispatch_actions_toggle_for_current_view(window, cx, source);
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_actions_routed",
                    source,
                    handled,
                    selected_index = self.selected_index,
                    show_actions_popup = self.show_actions_popup,
                    actions_window_open = crate::actions::is_actions_window_open(),
                    "Routed footer Actions through shared dispatcher"
                );
            }
            crate::footer_popup::FooterAction::Ai => {
                if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.handle_create_ai_preset_key("tab", window, cx);
                } else if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.open_profile_trigger_picker_in_window(window, cx);
                    });
                } else if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                } else if self.day_page_context_return.is_some() {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_ai_ignored_day_page_context_return",
                        "Ignored stale Agent footer action while Day Page @ context round trip is active"
                    );
                } else if matches!(self.current_view, AppView::DayPage { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_ai_ignored_day_page",
                        "Ignored stale Day Page Agent footer action"
                    );
                } else if let AppView::QuickTerminalView { entity } = &self.current_view {
                    let entity = entity.clone();
                    self.open_agent_chat_with_quick_terminal_output(entity, cx);
                } else if let AppView::TemplatePrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |prompt, cx| prompt.next_input(cx));
                } else if let AppView::FormPrompt { entity, .. } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |form, cx| form.focus_next(window, cx));
                } else {
                    self.open_tab_ai_agent_chat_with_entry_intent(None, cx);
                }
            }
            crate::footer_popup::FooterAction::Stop => {
                if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        let _ = chat.cancel_streaming_from_escape(cx);
                    });
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_stop_ignored",
                        view = ?self.current_view,
                        "Ignored Stop footer action outside Agent Chat chat"
                    );
                }
            }
            crate::footer_popup::FooterAction::PasteResponse => {
                self.paste_latest_agent_chat_response_to_frontmost(None, cx);
            }
            crate::footer_popup::FooterAction::Replace
            | crate::footer_popup::FooterAction::Append
            | crate::footer_popup::FooterAction::Copy
            | crate::footer_popup::FooterAction::Expand
            | crate::footer_popup::FooterAction::Retry => {
                if action == crate::footer_popup::FooterAction::Copy
                    && matches!(self.current_view, AppView::SdkReferenceView { .. })
                {
                    let _ = self.copy_selected_sdk_reference_markdown(cx);
                    return;
                }
                if action == crate::footer_popup::FooterAction::Copy
                    && matches!(self.current_view, AppView::ScriptTemplateCatalogView { .. })
                {
                    let _ = self.copy_selected_script_template_markdown(cx);
                    return;
                }
                if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                    return;
                }
                if let AppView::AgentChatView { entity } = &self.current_view {
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.dispatch_footer_button(action, window, cx);
                    });
                }
            }
            crate::footer_popup::FooterAction::Apply => {
                if matches!(self.current_view, AppView::FlowUxView { .. }) {
                    self.flow_desk_activate_selected(true, window, cx);
                } else if let AppView::ScriptIssuesView { report } = &self.current_view {
                    let report = report.clone();
                    self.copy_script_issues_to_clipboard(&report, cx);
                } else if self.dispatch_kit_store_remove_footer_action(cx) {
                } else if matches!(self.current_view, AppView::ConfirmPrompt { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "confirm_prompt_footer_apply",
                        "Confirming in-window confirm prompt from native footer"
                    );
                    self.resolve_confirm_prompt(true, window, cx);
                } else if let AppView::QuickTerminalView { entity } = &self.current_view {
                    let entity = entity.clone();
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "quick_terminal_footer_apply",
                        "Applying quick-terminal result from native footer"
                    );
                    self.apply_tab_ai_result_from_terminal(entity, cx);
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_apply_ignored",
                        view = ?self.current_view,
                        "Ignored Apply footer action outside QuickTerminalView"
                    );
                }
            }
            crate::footer_popup::FooterAction::Close => {
                if matches!(self.current_view, AppView::About { .. }) {
                    self.dismiss_about(cx);
                } else if matches!(self.current_view, AppView::MicroPrompt { .. }) {
                    self.go_back_or_close(window, cx);
                } else if matches!(
                    self.current_view,
                    AppView::SdkReferenceView { .. } | AppView::ScriptTemplateCatalogView { .. }
                ) {
                    if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else if matches!(self.current_view, AppView::CreateAiPresetView { .. }) {
                    self.go_back_or_close(window, cx);
                } else if matches!(self.current_view, AppView::NotesBrowseView { .. }) {
                    if self.is_in_attachment_portal() {
                        self.close_attachment_portal_cancel(cx);
                    } else if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else if matches!(self.current_view, AppView::FlowSessionView { .. }) {
                    // Native `Esc Desk` matches keyboard Esc: BACKGROUND the
                    // session (keep it alive in the Desk). Terminate is the
                    // destructive expert command in ⌘K Actions with the ⇧⌘⎋
                    // shortcut (Oracle 2026-07-21 footer adjudication — the
                    // native Close used to terminate, destroying conversations
                    // from a non-destructive-looking control).
                    self.background_flow_session(window, cx);
                } else if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
                    self.dispatch_permissions_wizard_action(
                        crate::permissions_wizard::PermissionsWizardAction::Done,
                        window,
                        cx,
                    );
                } else if self.dispatch_kit_store_browse_back_footer_action(window, cx) {
                } else if self.dispatch_day_page_preview_footer_action(action, window, cx) {
                } else if matches!(self.current_view, AppView::ConfirmPrompt { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "confirm_prompt_footer_close",
                        "Cancelling in-window confirm prompt from native footer"
                    );
                    self.resolve_confirm_prompt(false, window, cx);
                } else if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "quick_terminal_footer_close",
                        "Closing quick terminal from native footer"
                    );
                    self.close_quick_terminal_main_window_state_first(cx);
                } else if let AppView::TermPrompt { id, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "term_prompt_footer_cancel",
                        "Cancelling terminal prompt from native footer"
                    );
                    self.submit_prompt_response(id.clone(), None, cx);
                } else if let AppView::HotkeyPrompt { id, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "hotkey_prompt_footer_cancel",
                        "Cancelling hotkey prompt from native footer"
                    );
                    self.submit_prompt_response(id.clone(), None, cx);
                    self.cancel_script_execution(cx);
                } else if let AppView::EditorPrompt { entity, .. } = &self.current_view {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "editor_prompt_footer_cancel",
                        "Cancelling editor prompt from native footer (script receives None)"
                    );
                    let entity = entity.clone();
                    entity.update(cx, |editor, _| editor.submit_cancel());
                } else if matches!(self.current_view, AppView::TipsView { .. }) {
                    // Mirror the in-view Escape ladder: clear the filter
                    // first, then go back to the launcher.
                    if !self.clear_builtin_view_filter(cx) {
                        self.go_back_or_close(window, cx);
                    }
                } else {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_close_ignored",
                        view = ?self.current_view,
                        "Ignored Close footer action outside QuickTerminalView"
                    );
                }
            }
            crate::footer_popup::FooterAction::Cwd => {
                // Click on the CWD chip → open the directory picker the
                // same way Tab does (see startup.rs tab_interceptor
                // ScriptList arm). Works from ScriptList; from other views
                // we first return to the launcher.
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_cwd_chip_clicked",
                    view = ?self.current_view,
                    "Opening CWD picker from footer chip"
                );
                if !matches!(self.current_view, AppView::ScriptList) {
                    self.current_view = AppView::ScriptList;
                    self.reset_main_menu_selection_user_moved();
                }
                self.cwd_pick_mode = true;
                self.open_file_search_view("~/".to_string(), FileSearchPresentation::Full, cx);
                self.suppress_filter_events = true;
                self.gpui_input_state.update(cx, |state, cx| {
                    state.set_value("~/".to_string(), window, cx);
                    let len = "~/".len();
                    state.set_selection(len, len, window, cx);
                });
                self.suppress_filter_events = false;
                cx.notify();
            }
            crate::footer_popup::FooterAction::AgentModel => {
                // Click on the profile/model chip. From Agent Chat, keep the
                // chat surface alive and use the same in-chat Profile picker
                // path as Shift+Tab. From ScriptList, use the global
                // Profile Switcher.
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_window_footer_agent_model_chip_clicked",
                    view = ?self.current_view,
                    "Opening Profile Switcher from footer chip"
                );
                if let AppView::AgentChatView { entity, .. } = &self.current_view {
                    if self.show_actions_popup || crate::actions::is_actions_window_open() {
                        tracing::info!(
                            target: "script_kit::footer_popup",
                            event = "main_window_footer_agent_model_chip_ignored_actions_open",
                            view = ?self.current_view,
                            "Ignored profile/model chip while actions dialog owns input"
                        );
                        return;
                    }
                    let entity = entity.clone();
                    entity.update(cx, |chat, cx| {
                        chat.open_profile_trigger_picker_in_window(window, cx);
                    });
                    return;
                }
                if self.day_page_context_return.is_some() {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_agent_model_ignored_day_page_context_return",
                        "Ignored stale Agent/Model footer action while Day Page @ context round trip is active"
                    );
                    return;
                }
                if matches!(self.current_view, AppView::DayPage { .. }) {
                    tracing::info!(
                        target: "script_kit::footer_popup",
                        event = "main_window_footer_agent_model_ignored_day_page",
                        "Ignored stale Day Page Agent/Model footer action"
                    );
                    return;
                }
                if !matches!(self.current_view, AppView::ScriptList) {
                    self.current_view = AppView::ScriptList;
                    self.reset_main_menu_selection_user_moved();
                }
                self.open_profile_search(cx);
            }
        }
    }

    /// If the current view is an Agent Chat chat with a validated `SCRIPT_READY` receipt,
    /// execute that specific script and return `true`. Otherwise return `false`
    /// so the caller can fall back to `execute_selected`.
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

    fn latest_agent_chat_assistant_response(&self, cx: &App) -> Option<String> {
        let AppView::AgentChatView { entity } = &self.current_view else {
            return None;
        };

        entity.read(cx).pastable_response_text(cx)
    }

    /// Start a one-time async bridge that drains `footer_action_channel()` and
    /// dispatches each action into the existing `ScriptListApp` methods.
    fn ensure_main_footer_action_listener(&self, window: &Window, cx: &mut Context<Self>) {
        MAIN_FOOTER_ACTION_LISTENER.call_once(|| {
            let rx = crate::footer_popup::footer_action_channel().1.clone();
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "native_footer_listener_started",
                "Started native footer action listener"
            );
            cx.spawn_in(window, async move |this, cx| {
                while let Ok(action) = rx.recv().await {
                    if let Err(error) = this.update_in(cx, |app, window, cx| {
                        app.handle_main_footer_action(action, window, cx);
                    }) {
                        tracing::warn!(
                            target: "script_kit::footer_popup",
                            event = "native_footer_action_dispatch_failed",
                            action = ?action,
                            %error,
                            "Failed to dispatch native footer action into ScriptListApp"
                        );
                    }
                }
            })
            .detach();
        });
    }

    fn standard_main_window_footer_buttons(&self) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let footer_disabled = self.main_window_footer_buttons_blocked();
        let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
        let run_label = self.main_window_primary_action_label();

        let mut buttons = Vec::new();

        buttons.push(
            FooterButtonConfig::new(FooterAction::Run, "↵", run_label).enabled(!footer_disabled),
        );

        if self.current_view_supports_shared_actions() {
            let chip = FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                .selected(actions_open);
            // Selection-gated views (clipboard/dictation/favorites) only have
            // per-entry actions, so the advertised ⌘K would be a dead key
            // without a selected row — grey it out with the reason instead of
            // lying (audit finding #29). Stay enabled while the popup is open
            // so the chip can still close it.
            let chip = match self.actions_toggle_dead_without_selection_reason() {
                Some(reason) if !actions_open && !footer_disabled => chip.disabled_reason(reason),
                _ => chip.enabled(!footer_disabled),
            };
            buttons.push(chip);
        }
        if matches!(self.current_view, AppView::ScriptList) {
            // Style-only input (`.professional`): Enter already rewrites
            // via Agent Chat, so the Agent ⌘↵ button is dropped here.
            let style_owns_submit = self.spine_enabled
                && crate::spine::prompt_plan::spine_parse_is_style_only(&self.spine_parse);
            if !style_owns_submit {
                buttons.push(
                    FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent")
                        .enabled(!footer_disabled),
                );
            }
        }
        buttons
    }

    fn main_window_footer_buttons_blocked(&self) -> bool {
        crate::confirm::is_confirm_window_open()
    }

    /// Views whose actions are all per-entry have a dead ⌘K toggle when no
    /// row is selected. Returns the user-facing reason in that state so both
    /// the footer chip (disabled) and the key press (HUD) explain themselves.
    pub(crate) fn actions_toggle_dead_without_selection_reason(&self) -> Option<&'static str> {
        match &self.current_view {
            AppView::ClipboardHistoryView { .. } if !has_selected_clipboard_entry(self) => {
                Some("Select an entry to see actions")
            }
            AppView::DictationHistoryView { .. } if !has_selected_dictation_history_entry(self) => {
                Some("Select an entry to see actions")
            }
            AppView::FavoritesBrowseView { .. } if self.selected_favorite_id().is_none() => {
                Some("Select a favorite to see actions")
            }
            _ => None,
        }
    }

    fn main_window_footer_surface(&self) -> Option<&'static str> {
        self.current_view.native_footer_surface()
    }

    /// Quick Terminal footer buttons. Scoped to actions actually meaningful in
    /// the Quick Terminal surface: always Close (⌘W), plus Apply (⌘↩) only
    /// when a tab-AI apply-back route AND its return view are both present.
    /// Run/AI/Actions are intentionally omitted — Quick Terminal shares the
    /// main menu's native footer chrome but not its main-menu-specific actions.
    pub(crate) fn quick_terminal_can_apply_back(&self) -> bool {
        self.tab_ai_harness_apply_back_route.is_some() && self.tab_ai_harness_return_view.is_some()
    }

    pub(crate) fn quick_terminal_can_attach_to_agent_chat(&self) -> bool {
        matches!(self.current_view, AppView::QuickTerminalView { .. })
            && !self.quick_terminal_can_apply_back()
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

    fn main_window_footer_buttons_for_current_view(
        &self,
        cx: Option<&gpui::App>,
    ) -> Vec<crate::footer_popup::FooterButtonConfig> {
        if matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let enabled = !self.main_window_footer_buttons_blocked();
            return crate::permissions_wizard::PermissionsWizardActions::ALL
                .iter()
                .map(|spec| {
                    let footer_action = match spec.action {
                        crate::permissions_wizard::PermissionsWizardAction::GrantSelected => {
                            FooterAction::Run
                        }
                        crate::permissions_wizard::PermissionsWizardAction::Done => {
                            FooterAction::Close
                        }
                    };
                    FooterButtonConfig::new(footer_action, spec.key, spec.label).enabled(enabled)
                })
                .collect();
        }

        let enabled = !self.main_window_footer_buttons_blocked();

        if matches!(self.current_view, AppView::About { .. }) {
            return about_footer_buttons(enabled);
        }

        if matches!(self.current_view, AppView::MicroPrompt { .. }) {
            return micro_prompt_footer_buttons(enabled);
        }

        if let AppView::SdkReferenceView {
            filter,
            selected_index,
            entries,
        } = &self.current_view
        {
            let has_selection = crate::mcp_resources::sdk_reference_visible_rows(entries, filter)
                .get(*selected_index)
                .is_some();
            return sdk_reference_footer_buttons(enabled, has_selection);
        }

        if let AppView::ScriptTemplateCatalogView {
            filter,
            selected_index,
            templates,
        } = &self.current_view
        {
            let has_selection =
                crate::mcp_resources::script_template_catalog_visible_rows(templates, filter)
                    .get(*selected_index)
                    .is_some();
            return script_template_catalog_footer_buttons(enabled, has_selection);
        }

        if let AppView::CreateAiPresetView { name, .. } = &self.current_view {
            return create_ai_preset_footer_buttons(enabled, !name.trim().is_empty());
        }

        if let AppView::NotesBrowseView {
            filter,
            selected_index,
        } = &self.current_view
        {
            let has_selection =
                Self::notes_browse_selected_visible_row(filter, *selected_index).is_some();
            return notes_browse_footer_buttons(
                enabled,
                self.is_in_attachment_portal(),
                has_selection,
            );
        }

        // ConfirmPrompt: Apply (Confirm) + Close (Cancel) labeled per options.
        if let AppView::ConfirmPrompt {
            options,
            focused_button,
            ..
        } = &self.current_view
        {
            let buttons = self.confirm_prompt_footer_buttons(options, *focused_button);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved ConfirmPrompt footer buttons"
            );
            return buttons;
        }

        // Quick Terminal: scoped Close (+ optional Apply) — never Run/AI/Actions.
        if matches!(self.current_view, AppView::QuickTerminalView { .. }) {
            let buttons = self.quick_terminal_footer_buttons();
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Quick Terminal footer buttons"
            );
            return buttons;
        }

        // Full terminal prompt: mirror the keyboard grammar previously shown
        // by its GPUI hint strip through the persistent native footer.
        if matches!(self.current_view, AppView::TermPrompt { .. }) {
            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let buttons = term_prompt_footer_buttons(!footer_disabled, actions_open);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved terminal prompt footer buttons"
            );
            return buttons;
        }

        if let AppView::SelectPrompt { entity, .. } = &self.current_view {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let has_safe_submission = cx.is_some_and(|cx| {
                let prompt = entity.read(cx);
                crate::prompts::select_submission_is_allowed(prompt.multiple, prompt.selected.len())
            });
            let run = if has_safe_submission {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Run")
                    .enabled(!self.main_window_footer_buttons_blocked())
            } else {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Select one")
                    .disabled_reason("no_selection")
            };
            return vec![run];
        }

        // Flow session (Threadline): the footer mirrors the chat grammar —
        // Send + Actions, with Send honestly disabled while a turn runs.
        if let AppView::FlowSessionView { session_id } = self.current_view {
            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let working = self
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == session_id)
                .is_some_and(|(meta, _)| meta.active_turn.is_some());
            let buttons = flow_session_footer_buttons(working, enabled, actions_open);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Flow Session footer buttons"
            );
            return buttons;
        }

        // Flow Desk: Converse / Run Once / Actions — the desk grammar.
        if matches!(self.current_view, AppView::FlowUxView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Converse").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Apply, "⇧↵", "Run Once").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Flow Desk footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::DayPage { .. }) {
            let buttons = day_page_footer_buttons(self, cx);
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Day Page footer buttons"
            );
            return buttons;
        }

        // Agent Chat owns its own footer state: Send/Paste Response/Stop + Actions.
        if matches!(self.current_view, AppView::AgentChatView { .. }) {
            let buttons = self.agent_chat_footer_buttons();
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Agent Chat footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::TemplatePrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Ai, "⇥", "Next Field").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved TemplatePrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::FormPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Submit").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Ai, "⇥", "Next Field").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved FormPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::HotkeyPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved HotkeyPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::ScriptIssuesView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Fix in Agent").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Apply, "⌘C", "Copy Issues").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Script Issues footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::EnvPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled;
            let buttons =
                vec![FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(enabled)];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved EnvPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::WebcamView { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Capture Photo").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Webcam footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::PathPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let enabled = !footer_disabled;
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Select").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved PathPrompt footer buttons"
            );
            return buttons;
        }

        if matches!(self.current_view, AppView::DropPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let has_files = match (&self.current_view, cx) {
                (AppView::DropPrompt { entity, .. }, Some(cx)) => {
                    !entity.read(cx).dropped_files.is_empty()
                }
                _ => false,
            };
            let submit_button = if footer_disabled {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(false)
            } else if has_files {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit").enabled(true)
            } else {
                FooterButtonConfig::new(FooterAction::Run, "↵", "Submit")
                    .disabled_reason("no_files")
            };
            let buttons = vec![
                submit_button,
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                has_files,
                "Resolved DropPrompt footer buttons"
            );
            return buttons;
        }

        if let AppView::BrowseKitsView {
            selected_index,
            results,
            query,
            ..
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled && results.get(*selected_index).is_some();
            let secondary_label = if query.is_empty() {
                "Back"
            } else {
                "Clear Search"
            };
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Install").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Close, "Esc", secondary_label)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Kit Store browse footer buttons"
            );
            return buttons;
        }

        if let AppView::MigrateV1View {
            filter,
            selected_index,
            board,
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let visible = Self::migrate_visible_rows(&board.rows, filter);
            let selected = visible
                .get(*selected_index)
                .and_then(|row_ix| board.rows.get(*row_ix));
            let buttons = match board.phase {
                MigrateBoardPhase::Report => vec![
                    FooterButtonConfig::new(FooterAction::Run, "↵", "Port all")
                        .enabled(!footer_disabled && !board.rows.is_empty()),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                        .enabled(!footer_disabled),
                ],
                MigrateBoardPhase::Porting => vec![
                    FooterButtonConfig::new(FooterAction::Run, "", "Porting…").enabled(false),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Hide").enabled(true),
                ],
                MigrateBoardPhase::Done => vec![
                    FooterButtonConfig::new(FooterAction::Run, "↵", "Port with AI").enabled(
                        !footer_disabled
                            && selected
                                .and_then(|row| row.status.as_deref())
                                .is_some_and(|status| status == "needs-review"),
                    ),
                    FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                        .enabled(!footer_disabled),
                ],
                _ => vec![FooterButtonConfig::new(FooterAction::Close, "Esc", "Back")
                    .enabled(!footer_disabled)],
            };
            return buttons;
        }

        if let AppView::InstalledKitsView {
            filter,
            selected_index,
            kits,
            ..
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let enabled = !footer_disabled
                && Self::kit_store_installed_selected_visible_kit(kits, filter, *selected_index)
                    .is_some();
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Update").enabled(enabled),
                FooterButtonConfig::new(FooterAction::Apply, "⌦", "Remove").enabled(enabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Kit Store installed footer buttons"
            );
            return buttons;
        }

        // Tips browser: Copy Example + Back, honestly disabled when the
        // selected tip has no example to copy.
        if let AppView::TipsView {
            filter,
            selected_index,
            entries,
        } = &self.current_view
        {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let visible = script_kit_gpui::tips::visible_tip_indices(entries, filter);
            let has_example = visible
                .get(*selected_index)
                .and_then(|index| entries.get(*index))
                .is_some_and(|tip| !tip.examples.is_empty());
            let secondary_label = if filter.trim().is_empty() {
                "Back"
            } else {
                "Clear Search"
            };
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "↵", "Copy Example")
                    .enabled(!footer_disabled && has_example),
                FooterButtonConfig::new(FooterAction::Close, "Esc", secondary_label)
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved Tips footer buttons"
            );
            return buttons;
        }

        // EditorPrompt: Enter inserts a newline (submit is ⌘↵/⌘S), so the
        // standard "↵ Run" native footer would lie on this surface.
        if matches!(self.current_view, AppView::EditorPrompt { .. }) {
            use crate::footer_popup::{FooterAction, FooterButtonConfig};

            let footer_disabled = self.main_window_footer_buttons_blocked();
            let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
            let buttons = vec![
                FooterButtonConfig::new(FooterAction::Run, "⌘↵", "Submit")
                    .enabled(!footer_disabled),
                FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                    .selected(actions_open)
                    .enabled(!footer_disabled),
                FooterButtonConfig::new(FooterAction::Close, "Esc", "Cancel")
                    .enabled(!footer_disabled),
            ];
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_buttons_resolved",
                view = ?self.current_view,
                button_count = buttons.len(),
                "Resolved EditorPrompt footer buttons"
            );
            return buttons;
        }

        let buttons = self.standard_main_window_footer_buttons();
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_buttons_resolved",
            view = ?self.current_view,
            button_count = buttons.len(),
            "Resolved main-window native footer buttons"
        );
        buttons
    }

    /// Build footer buttons for the Agent Chat chat surface from the child-owned
    /// composer/thread state snapshot.
    fn agent_chat_footer_buttons(&self) -> Vec<crate::footer_popup::FooterButtonConfig> {
        use crate::footer_popup::{FooterAction, FooterButtonConfig};

        let footer_disabled = self.main_window_footer_buttons_blocked();
        let actions_open = self.show_actions_popup || crate::actions::is_actions_window_open();
        let enabled = !footer_disabled;

        if let Some(snapshot) = self.agent_chat_footer_snapshot.as_ref() {
            if !snapshot.visible {
                return Vec::new();
            }
            return snapshot
                .buttons
                .iter()
                .map(|button| {
                    let mut config =
                        FooterButtonConfig::new(button.action, button.key, button.label)
                            .selected(button.selected)
                            .enabled(enabled && button.enabled);
                    if let Some(reason) = button.disabled_reason {
                        config = config.disabled_reason(reason);
                    }
                    config
                })
                .collect();
        }

        vec![
            FooterButtonConfig::new(FooterAction::Run, "↵", "Send")
                .disabled_reason("loading_agent_chat"),
            FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions")
                .selected(actions_open)
                .enabled(enabled),
        ]
    }

    pub(crate) fn main_window_footer_config(
        &self,
    ) -> Option<crate::footer_popup::MainWindowFooterConfig> {
        self.main_window_footer_config_with_cx(None)
    }

    pub(crate) fn main_window_footer_config_with_cx(
        &self,
        cx: Option<&gpui::App>,
    ) -> Option<crate::footer_popup::MainWindowFooterConfig> {
        use crate::footer_popup::MainWindowFooterConfig;

        if let AppView::AgentChatView { entity } = &self.current_view {
            let controls_visible = cx
                .map(|cx| entity.read(cx).main_window_footer_visible(cx))
                .or_else(|| {
                    self.agent_chat_footer_snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.visible)
                });
            if !main_window_footer_chrome_should_render(true, controls_visible) {
                return None;
            }
        }

        let surface = self.main_window_footer_surface()?;
        let buttons = self.main_window_footer_buttons_for_current_view(cx);

        // debug!: resolved on every render frame and every state collection;
        // info-level logging here is per-frame I/O during arrow-key scroll.
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_config_resolved",
            view = ?self.current_view,
            surface,
            button_count = buttons.len(),
            "Resolved main-window native footer config"
        );

        let mut config = MainWindowFooterConfig::new(surface, buttons);
        if matches!(self.current_view, AppView::ScriptList)
            && self.filter_text.trim().is_empty()
            && crate::config::load_config().is_tips_enabled()
        {
            if let Some(tip) = script_kit_gpui::tips::current_footer_tip() {
                config.left_info = Some(crate::footer_popup::FooterLeftInfo {
                    model_name: tip.hint.clone(),
                    icon_token: Some("lightbulb".to_string()),
                    keycap: tip.hint_key.clone(),
                    action: Some(crate::footer_popup::FooterAction::Tips),
                    ..Default::default()
                });
            }
        }
        // Main list slow-filling (explicit tabs:/history: fetch, visible
        // root file search): pair the constellation layer with a braille
        // spinner + per-kind status label in the footer's left slot. Tips
        // only render on an empty query, so this never clobbers them.
        if let Some(kind) = self.main_list_loading_kind() {
            config.left_info = Some(main_list_loading_left_info(
                kind,
                self.main_list_loading_elapsed_secs(),
            ));
        }
        Some(config)
    }

    pub(crate) fn main_window_uses_native_footer(&self) -> bool {
        self.main_window_footer_surface()
            .is_some_and(|expected_surface| {
                // Tahoe glass has one canonical footer owner: the native host
                // in the main NSWindow. Logical visibility flips false before
                // AppKit's exit fade completes, so it must not participate in
                // this ownership decision; doing so paints an in-stage GPUI
                // fallback beside the still-visible detached footer.
                //
                // Do not paint a GPUI fallback on the installation frame
                // either. Non-glass mode still waits for the installed-host
                // receipt before suppressing its fallback.
                crate::footer_popup::glass_scroll_bands_active()
                    || crate::footer_popup::active_main_window_footer_surface()
                        == Some(expected_surface)
            })
    }

    /// When the native main-window footer is active, replace the GPUI footer
    /// with either the blur-era spacer or, in Tahoe glass mode, an absolute
    /// hover blocker that lets scroll content continue beneath the glass band.
    pub(crate) fn main_window_footer_slot(
        &self,
        gpui_footer: gpui::AnyElement,
    ) -> Option<gpui::AnyElement> {
        let is_agent_chat = matches!(self.current_view, AppView::AgentChatView { .. });
        let agent_chat_controls_visible = self
            .agent_chat_footer_snapshot
            .as_ref()
            .map(|snapshot| snapshot.visible);
        if !main_window_footer_chrome_should_render(is_agent_chat, agent_chat_controls_visible) {
            return None;
        }
        if self.main_window_uses_native_footer() {
            if crate::footer_popup::glass_scroll_bands_active() {
                Some(
                    crate::components::prompt_layout_shell::render_native_main_window_footer_hover_blocker(),
                )
            } else {
                Some(
                    crate::components::prompt_layout_shell::render_native_main_window_footer_spacer(
                    ),
                )
            }
        } else {
            Some(gpui_footer)
        }
    }

    fn handle_main_footer_action(
        &mut self,
        action: crate::footer_popup::FooterAction,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_action_dispatch",
            source = "native_footer",
            action = ?action,
            view = ?self.current_view,
            main_window_mode = ?self.main_window_mode,
            "Dispatching main-window footer action"
        );

        if self.main_window_footer_config_with_cx(Some(&*cx)).is_none()
            || !crate::is_main_window_visible()
        {
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "main_window_footer_action_ignored_inactive_surface",
                source = "native_footer",
                action = ?action,
                view = ?self.current_view,
                main_window_mode = ?self.main_window_mode,
                "Ignored native footer action because current view is not using the native footer"
            );
            return;
        }

        self.dispatch_main_window_footer_action(action, window, cx, "native_footer");
    }

    pub(crate) fn sync_main_window_resize_lock(
        &self,
        window: &mut gpui::Window,
        cx: &Context<Self>,
    ) {
        let should_lock = match &self.current_view {
            AppView::AgentChatView { entity } => {
                entity.read_with(cx, |view, _cx| view.locks_main_window_resize())
            }
            _ => false,
        };
        crate::platform::set_window_resizable(window, !should_lock);
    }

    pub(crate) fn sync_main_footer_popup(&self, window: &mut gpui::Window, cx: &mut Context<Self>) {
        self.ensure_main_footer_action_listener(window, cx);

        let mut config = if crate::is_main_window_visible() {
            self.main_window_footer_config_with_cx(Some(&*cx))
        } else {
            None
        };

        // Enrich with Agent Chat streaming/model info when on the Agent Chat chat view.
        if let Some(ref mut cfg) = config {
            self.enrich_footer_config_with_agent_chat_info(cfg);
        }

        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "main_window_footer_sync",
            view = ?self.current_view,
            show = config.is_some(),
            surface = config.as_ref().map(|c| c.surface).unwrap_or("none"),
            button_count = config.as_ref().map(|c| c.buttons.len()).unwrap_or(0),
            "Syncing native main window footer"
        );

        crate::footer_popup::sync_main_footer_popup(window, config.as_ref(), &mut *cx);
    }

    /// The global working-directory footer chip, sourced from `spine_cwd_label`
    /// so the main menu and Agent Chat show the same persistent cwd. Returns
    /// `None` only when no cwd is established.
    pub(crate) fn global_footer_cwd_chip(&self) -> Option<crate::footer_popup::FooterCwdChip> {
        self.spine_cwd_label
            .as_ref()
            .map(|label| crate::footer_popup::FooterCwdChip {
                label: label.clone(),
                icon_token: "folder".to_string(),
                key: Some("⇥".to_string()),
                tooltip: None,
            })
    }

    /// Combined "Agent · Model" footer label, derived from the persisted Pi
    /// provider/model selection (`spine_agent_label` / `spine_model_label`).
    /// Returns `None` when neither label is known so the chip stays hidden.
    pub(crate) fn agent_model_footer_label(&self) -> Option<String> {
        match (
            self.spine_agent_label.as_ref(),
            self.spine_model_label.as_ref(),
        ) {
            (Some(agent), Some(model)) => Some(format!("{agent} · {model}")),
            (Some(agent), None) => Some(agent.clone()),
            (None, Some(model)) => Some(model.clone()),
            (None, None) => None,
        }
    }

    /// Refresh the passive "selected text" sniff that powers the header hint
    /// chip. Runs `get_selected_text_ax_only` on the background executor — it
    /// never posts keystrokes and never reads or writes the pasteboard, so it
    /// is safe to run speculatively on every show. Clears the previous hint
    /// synchronously so a stale selection from the last show can never flash.
    pub(crate) fn refresh_shown_selection_hint(&mut self, cx: &mut Context<Self>) {
        self.shown_selection_hint_token = self.shown_selection_hint_token.wrapping_add(1);
        let token = self.shown_selection_hint_token;
        if self.shown_selection_hint_text.take().is_some() {
            cx.notify();
        }
        // Target the app the user came FROM by pid: once our panel shows, the
        // system-wide focused element can already point at Script Kit itself.
        let source_app = crate::frontmost_app_tracker::get_last_real_app();
        let source_pid = source_app.as_ref().map(|app| app.pid);
        let source_name = source_app.map(|app| app.name);
        cx.spawn(async move |this, cx| {
            // Passive AX-only reads: selection first, then the focused
            // field's whole text ("draft") so the style rows can preview what
            // a rewrite would capture even when nothing is selected.
            let result = cx
                .background_executor()
                .spawn(async move {
                    match crate::platform::accessibility::focused_text::selected_text_for_app_ax_only(
                        source_pid,
                    ) {
                        Ok(Some(text)) => Ok((Some(text), false)),
                        Ok(None) | Err(_) => {
                            crate::platform::accessibility::focused_text::focused_text_for_app_ax_only(
                                source_pid,
                            )
                            .map(|draft| (draft, true))
                        }
                    }
                })
                .await;
            let error = result.as_ref().err().map(|e| e.to_string());
            let (selection, is_draft) = match result {
                Ok((text, is_draft)) => (text.filter(|t| !t.trim().is_empty()), is_draft),
                Err(_) => (None, false),
            };
            let _ = this.update(cx, |app, cx| {
                if app.shown_selection_hint_token != token {
                    return;
                }
                tracing::info!(
                    target: "script_kit::selection_hint",
                    event = "selection_hint_sniff_complete",
                    has_selection = selection.is_some(),
                    is_draft,
                    chars = selection.as_deref().map(|s| s.chars().count()).unwrap_or(0),
                    source_app = source_name.as_deref().unwrap_or("unknown"),
                    source_pid = source_pid.unwrap_or(-1),
                    error = error.as_deref().unwrap_or(""),
                );
                // The header chip stays selection-only (a chip for every
                // focused text field would be noise); the preview cache gets
                // both so style rows and the submit-freeze agree.
                app.shown_selection_hint_text = if is_draft { None } else { selection.clone() };
                app.spine_live_preview_cache.seed_selection_preview(
                    selection,
                    is_draft,
                    source_name.clone(),
                );
                cx.notify();
            });
        })
        .detach();
    }

    /// Header hint chip shown only while the main menu is up AND the passive
    /// sniff found selected text in the app the user came from.
    pub(crate) fn selection_hint_chip(
        &self,
    ) -> Option<crate::components::main_view_chrome::MainViewSelectionHintChip> {
        if !matches!(self.current_view, AppView::ScriptList) {
            return None;
        }
        let text = self.shown_selection_hint_text.as_deref()?;
        Some(
            crate::components::main_view_chrome::MainViewSelectionHintChip {
                label: format!(
                    "Rewrite \u{201c}{}\u{201d}",
                    crate::components::main_view_chrome::selection_hint_snippet(text, 24)
                ),
            },
        )
    }

    /// What Tab actually does on the current surface — the single source of
    /// truth for the header Tab chip. MUST mirror the Tab interceptor's
    /// branch order in `startup.rs` (menu-syntax pickers → empty-input cwd
    /// pick → directory-browse completion → Quick AI) so the chip never
    /// advertises an action Tab won't take.
    pub(crate) fn main_header_tab_chip_action(
        &self,
    ) -> crate::components::main_view_chrome::MainViewTabChipAction {
        use crate::components::main_view_chrome::MainViewTabChipAction;

        if !matches!(self.current_view, AppView::ScriptList) || self.show_actions_popup {
            return MainViewTabChipAction::Inactive;
        }
        // Menu-syntax pickers/forms claim Tab first and show their own hints.
        if self.menu_syntax_object_selector_owns_main_keyboard()
            || self.menu_syntax_trigger_picker_owns_main_keyboard()
            || self.menu_syntax_capture_form_owns_input()
        {
            return MainViewTabChipAction::Inactive;
        }
        if self.filter_text.trim().is_empty() {
            if self.spine_enabled {
                return MainViewTabChipAction::ChangeCwd;
            }
            return MainViewTabChipAction::Inactive;
        }
        // Directory-browse queries keep Tab for path completion.
        if crate::file_search::looks_like_root_directory_browse_query(&self.filter_text) {
            return MainViewTabChipAction::Inactive;
        }
        MainViewTabChipAction::QuickAi
    }

    /// Whether Shift+Tab opens the profile (agent/model) picker on the
    /// current surface — mirrors `try_open_profile_search_from_script_list_shift_tab`.
    fn main_header_shift_tab_key_active(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
            && self.spine_enabled
            && !self.show_actions_popup
    }

    pub(crate) fn main_view_context_labels(
        &self,
    ) -> crate::components::main_view_chrome::MainViewContextLabels {
        let cwd_label = self
            .global_footer_cwd_chip()
            .map(|chip| chip.label)
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|cwd| crate::file_search::shorten_path(&cwd.to_string_lossy()))
            })
            .unwrap_or_else(|| {
                crate::components::main_view_chrome::MAIN_VIEW_CWD_UNAVAILABLE_LABEL.to_string()
            });

        // In a flow session the active agent IS the flow: the shared
        // Agent·Model chip carries "<flow> · <engine>" (the engine label
        // grows the model once the thread reports it) instead of the global
        // spine agent, which is not what this conversation talks to.
        let flow_session_label = match &self.current_view {
            AppView::FlowSessionView { session_id } => self
                .flow_sessions
                .iter()
                .find(|(meta, _)| meta.id == *session_id)
                .map(|(meta, _)| format!("{} · {}", meta.friendly_name, meta.engine)),
            _ => None,
        };
        let agent_model_label = flow_session_label
            .or_else(|| self.agent_model_footer_label())
            .unwrap_or_else(|| {
                crate::components::main_view_chrome::MAIN_VIEW_AGENT_MODEL_UNAVAILABLE_LABEL
                    .to_string()
            });

        crate::components::main_view_chrome::MainViewContextLabels::new(
            cwd_label,
            agent_model_label,
        )
        .with_tab_action(self.main_header_tab_chip_action())
        .with_shift_tab_key_active(self.main_header_shift_tab_key_active())
    }

    pub(crate) fn render_clickable_main_view_context_zone(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_zone_required(
            &self.theme,
            menu_def,
            self.main_view_context_labels(),
            self.selection_hint_chip(),
            cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                // The chip is a button for whatever Tab currently does:
                // Quick AI submit when the input has text, cwd picker otherwise.
                if matches!(
                    this.main_header_tab_chip_action(),
                    crate::components::main_view_chrome::MainViewTabChipAction::QuickAi
                ) {
                    let query = this.filter_text.clone();
                    this.open_quick_ai_from_launcher(query, window, cx);
                    return;
                }
                this.dispatch_main_window_footer_action(
                    crate::footer_popup::FooterAction::Cwd,
                    window,
                    cx,
                    "main_view_context_click",
                );
            }),
            cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                this.dispatch_main_window_footer_action(
                    crate::footer_popup::FooterAction::AgentModel,
                    window,
                    cx,
                    "main_view_context_click",
                );
            }),
            cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                // Single-click on-ramp into the instant rewrite mini: capture
                // the focused text and immediately stream three rewrite
                // variations. No `.` sigil is surfaced to the user.
                tracing::info!(
                    target: "script_kit::selection_hint",
                    event = "selection_hint_chip_clicked",
                );
                let _ = window;
                this.open_instant_rewrite_mini("selection_hint_chip", cx);
            }),
        )
    }

    pub(crate) fn render_clickable_main_view_context_header(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_header(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
        )
    }

    #[allow(dead_code)]
    pub(crate) fn render_inert_main_view_context_zone(
        &self,
        menu_def: crate::designs::MainMenuThemeDef,
    ) -> gpui::AnyElement {
        crate::components::main_view_chrome::render_main_view_context_zone_required(
            &self.theme,
            menu_def,
            self.main_view_context_labels(),
            None,
            |_event, _window, _cx| {},
            |_event, _window, _cx| {},
            |_event, _window, _cx| {},
        )
    }

    pub(crate) fn enrich_footer_config_with_agent_chat_info(
        &self,
        config: &mut crate::footer_popup::MainWindowFooterConfig,
    ) {
        if matches!(self.current_view, AppView::AgentChatView { .. }) {
            // Cwd and Agent/Model now live in the shared main-view header. Keep
            // the native footer scoped to surface actions only, and make sure
            // stale Agent Chat left-info state cannot reintroduce duplicate model/cwd
            // chips beside the footer buttons.
            config.left_info = None;
        }
    }

    pub(crate) fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        cx.notify();
    }

    /// Toggle the focused-info panel visibility (Cmd+I / "Show Info" action).
    pub(crate) fn toggle_info_panel(&mut self, cx: &mut Context<Self>) {
        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.show_info_panel = !self.show_info_panel;
        tracing::info!(
            category = "UI",
            event = "toggle_info_panel",
            visible = self.show_info_panel,
            "Info panel toggled"
        );
        cx.notify();
    }

    /// Hide the mouse cursor while typing.
    /// The cursor will be shown again when the mouse moves.
    pub(crate) fn hide_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        if !self.mouse_cursor_hidden {
            self.mouse_cursor_hidden = true;
            crate::platform::hide_cursor_until_mouse_moves();
            cx.notify();
        }
    }

    /// Show the mouse cursor (called when mouse moves).
    /// Also switches to Mouse input mode to re-enable hover effects.
    /// Only calls cx.notify() when state actually changes, to avoid
    /// render churn during passive scrolling.
    pub(crate) fn show_mouse_cursor(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;

        if !matches!(self.input_mode, InputMode::Mouse) {
            self.input_mode = InputMode::Mouse;
            changed = true;
        }

        if self.mouse_cursor_hidden {
            self.mouse_cursor_hidden = false;
            changed = true;
        }

        if changed {
            cx.notify();
        }
    }

    /// Calculate view type and item count for window sizing.
    /// Extracted from update_window_size for reuse.
    pub(crate) fn calculate_window_size_params(&mut self) -> Option<(ViewType, usize)> {
        self.calculate_window_size_params_with_app(None)
    }

    pub(crate) fn calculate_window_size_params_with_app(
        &mut self,
        cx: Option<&gpui::App>,
    ) -> Option<(ViewType, usize)> {
        match &self.current_view {
            AppView::ScriptList => {
                // Get grouped results which includes section headers (cached)
                let (grouped_items, _) = self.get_grouped_results_cached();
                let count = grouped_items.len();
                let view_type = match self.main_window_mode {
                    MainWindowMode::Full => ViewType::ScriptList,
                    MainWindowMode::Mini => ViewType::MainWindow,
                };
                Some((view_type, count))
            }
            AppView::About { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::ArgPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    Some((ViewType::ArgPromptNoChoices, 0))
                } else {
                    Some((ViewType::ArgPromptWithChoices, filtered.len()))
                }
            }
            AppView::MiniPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                if filtered.is_empty() && choices.is_empty() {
                    Some((mini_prompt_view_type(), 0))
                } else {
                    Some((mini_prompt_view_type(), filtered.len().min(5)))
                }
            }
            AppView::MicroPrompt { .. } => Some((ViewType::ArgPromptNoChoices, 0)),
            AppView::DivPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::FormPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Use DivPrompt size for forms
            AppView::EditorPrompt { .. } => Some((ViewType::EditorPrompt, 0)),
            AppView::SelectPrompt { entity, .. } => Some((
                ViewType::SelectPrompt,
                cx.map(|cx| entity.read(cx).filtered_choices.len())
                    .unwrap_or(1),
            )),
            AppView::PathPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::EnvPrompt { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::DropPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Drop prompt uses div size for drop zone
            AppView::TemplatePrompt { .. } => Some((ViewType::DivPrompt, 0)), // Template prompt uses div size
            AppView::HotkeyPrompt { .. } => Some((ViewType::DivPrompt, 0)), // Hotkey prompt uses compact recorder surface
            AppView::ChatPrompt { .. } => {
                Some((compact_ai_view_type_for_mode(self.main_window_mode), 0))
            }
            AppView::TermPrompt { .. } => Some((ViewType::TermPrompt, 0)),
            AppView::ActionsDialog => {
                // Actions dialog is an overlay, don't resize
                None
            }
            // Preview/detail builtins widen from the mini launcher without
            // increasing height, so the shared header/input stays fixed.
            // View state only - data comes from self fields
            AppView::ClipboardHistoryView { filter, .. } => {
                let entries = &self.cached_clipboard_entries;
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .filter(|e| e.text_preview.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::EmojiPickerView {
                filter,
                selected_category,
                ..
            } => {
                let row_count = crate::emoji::filtered_grid_row_count(filter, *selected_category);
                Some((ViewType::MainWindow, row_count))
            }
            AppView::AppLauncherView { filter, .. } => {
                let apps = &self.apps;
                let filtered_count = if filter.is_empty() {
                    apps.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    apps.iter()
                        .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::WindowSwitcherView { filter, .. } => {
                let windows = &self.cached_windows;
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
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::ProcessManagerView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_processes.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_processes
                        .iter()
                        .filter(|p| p.script_path.to_lowercase().contains(&filter_lower))
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::FlowUxView { filter, .. } => {
                let cwd = self.flow_ux_cwd();
                let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
                let count = crate::flows::catalog::filter_flows(&roster.flows, filter).len();
                Some((ViewType::MainWindow, count))
            }
            AppView::CurrentAppCommandsView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_current_app_entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    self.cached_current_app_entries
                        .iter()
                        .filter(|e| {
                            e.name.to_lowercase().contains(&filter_lower)
                                || e.keywords.iter().any(|k| k.contains(&filter_lower))
                        })
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::BrowserTabsView { filter, .. } => {
                let filtered_count = if filter.is_empty() {
                    self.cached_browser_tabs.len()
                } else {
                    crate::browser_tabs::fuzzy_search_browser_tabs(
                        &self.cached_browser_tabs,
                        filter,
                    )
                    .len()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::ScratchPadView { .. } => Some((ViewType::EditorPrompt, 0)),
            AppView::QuickTerminalView { .. } => Some((ViewType::TermPrompt, 0)),
            AppView::FlowSessionView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::WebcamView { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::FileSearchView {
                ref query,
                presentation,
                ..
            } => {
                let results = &self.cached_file_results;
                let filtered_count = if query.is_empty() {
                    results.len()
                } else {
                    let query_lower = query.to_lowercase();
                    results
                        .iter()
                        .filter(|r| r.name.to_lowercase().contains(&query_lower))
                        .count()
                };
                let view_type = match presentation {
                    FileSearchPresentation::Mini => ViewType::MainWindow,
                    FileSearchPresentation::Full => ViewType::MainWindow,
                };
                Some((view_type, filtered_count))
            }
            AppView::ProfileSearchView { filter, .. } => Some((
                ViewType::MainWindow,
                self.profile_search_visible_len(filter),
            )),
            AppView::ThemeChooserView { ref filter, .. } => {
                // Size against the unified catalog (user themes + presets) so
                // the window height matches what the gallery actually shows.
                let catalog = Self::theme_chooser_catalog();
                let filtered_count =
                    Self::theme_chooser_catalog_filtered_indices(filter, &catalog).len();
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::CreationFeedback { .. } => Some((ViewType::DivPrompt, 0)),
            AppView::ScriptIssuesView { .. } => Some((ViewType::ArgPromptNoChoices, 0)),
            AppView::SdkReferenceView {
                entries, filter, ..
            } => {
                let (_, count) =
                    crate::mcp_resources::sdk_reference_dataset_and_visible_counts(entries, filter);
                Some((ViewType::MainWindow, count))
            }
            AppView::TipsView {
                entries, filter, ..
            } => {
                let count = script_kit_gpui::tips::visible_tip_indices(entries, filter).len();
                Some((ViewType::MainWindow, count))
            }
            AppView::ScriptTemplateCatalogView {
                templates, filter, ..
            } => {
                let (_, count) =
                    crate::mcp_resources::script_template_catalog_dataset_and_visible_counts(
                        templates, filter,
                    );
                Some((ViewType::MainWindow, count))
            }
            AppView::NamingPrompt { .. } => Some((ViewType::ArgPromptNoChoices, 0)),
            AppView::BrowseKitsView { results, .. } => Some((ViewType::MainWindow, results.len())),
            AppView::MigrateV1View { filter, board, .. } => Some((
                ViewType::MainWindow,
                Self::migrate_visible_rows(&board.rows, filter).len(),
            )),
            AppView::InstalledKitsView { filter, kits, .. } => Some((
                ViewType::MainWindow,
                Self::kit_store_installed_visible_rows(kits, filter).len(),
            )),
            AppView::SearchAiPresetsView { .. } => {
                // Presets list - defaults (5) + user presets
                let count = crate::ai::presets::load_presets()
                    .map(|p| 5 + p.len())
                    .unwrap_or(5);
                Some((ViewType::MainWindow, count))
            }
            AppView::CreateAiPresetView { .. } => {
                // Fixed-size form with 3 fields
                Some((ViewType::ArgPromptNoChoices, 0))
            }
            AppView::SettingsView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::PermissionsWizardView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::FavoritesBrowseView { .. } => Some((ViewType::MainWindow, 0)),
            AppView::AgentChatHistoryView { filter, .. } => {
                let entries = crate::ai::agent_chat::ui::history::load_history();
                let filtered_count = if filter.is_empty() {
                    entries.len()
                } else {
                    let filter_lower = filter.to_lowercase();
                    entries
                        .iter()
                        .filter(|entry| {
                            entry.first_message.to_lowercase().contains(&filter_lower)
                                || entry.timestamp.to_lowercase().contains(&filter_lower)
                        })
                        .count()
                };
                Some((ViewType::MainWindow, filtered_count))
            }
            AppView::BrowserHistoryView { filter, .. } => Some((
                ViewType::MainWindow,
                crate::browser_history::fuzzy_search_browser_history(
                    &self.cached_browser_history,
                    filter,
                )
                .len(),
            )),
            AppView::DictationHistoryView { filter, .. } => Some((
                ViewType::MainWindow,
                crate::dictation::search_history(filter, 100).len(),
            )),
            AppView::NotesBrowseView { filter, .. } => Some((
                ViewType::MainWindow,
                if filter.is_empty() {
                    crate::notes::get_all_notes()
                        .map(|notes| notes.len())
                        .unwrap_or(0)
                } else {
                    crate::notes::search_notes(filter)
                        .map(|notes| notes.len())
                        .unwrap_or(0)
                },
            )),
            AppView::AgentChatView { entity } => {
                if let Some(cx) = cx {
                    if let Some(item_count) = entity.read(cx).focused_text_mini_sizing_count(cx) {
                        return Some((ViewType::FocusedTextMini, item_count));
                    }
                }
                Some((compact_ai_view_type_for_mode(self.main_window_mode), 0))
            }
            AppView::DayPage { .. } => Some((ViewType::MainWindow, 0)),
            // In-window confirm participates in the canonical 480px main
            // shell; only its body controls differ from searchable views.
            AppView::ConfirmPrompt { .. } => Some((ViewType::MainWindow, 0)),
        }
    }

    pub(crate) fn set_main_window_mode(
        &mut self,
        mode: MainWindowMode,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        let old = self.main_window_mode;
        if old == mode {
            return;
        }

        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.main_window_mode = mode;

        if let AppView::ChatPrompt { entity, .. } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat, _cx| {
                chat.set_mini_mode(mode == MainWindowMode::Mini);
            });
        }

        let shared_actions_open = self.show_actions_popup;
        let detached_actions_open = crate::actions::is_actions_window_open();
        if shared_actions_open {
            if let super::actions_dialog::ActionsSupport::SharedDialog(host) =
                self.actions_support_for_view()
            {
                self.close_actions_popup(host, window, cx);
            } else {
                self.clear_actions_popup_state();
            }
        }
        if detached_actions_open {
            crate::actions::close_actions_window(cx);
        }

        self.update_window_size_deferred(window, cx);
        self.sync_main_footer_popup(window, cx);
        tracing::info!(
            target: "script_kit::window_mode",
            event = "main_window_mode_changed",
            source,
            old = ?old,
            new = ?mode,
            view = ?self.current_view,
            "Main window mode changed atomically"
        );
    }

    pub(crate) fn set_main_window_mode_state_only(
        &mut self,
        mode: MainWindowMode,
        cx: &mut Context<Self>,
        source: &'static str,
    ) {
        let old = self.main_window_mode;
        if old == mode {
            return;
        }

        self.reset_main_list_boundary_affordance(
            crate::scrolling::boundary_affordance::SettleReason::Reset,
        );
        self.main_window_mode = mode;
        if let AppView::ChatPrompt { entity, .. } = &self.current_view {
            let entity = entity.clone();
            entity.update(cx, |chat, _cx| {
                chat.set_mini_mode(mode == MainWindowMode::Mini);
            });
        }
        tracing::info!(
            target: "script_kit::window_mode",
            event = "main_window_mode_changed",
            source,
            old = ?old,
            new = ?mode,
            view = ?self.current_view,
            "Main window mode changed without window handle"
        );
    }

    /// Calculate sizing only when the current view still matches the caller's
    /// expected async resize target.
    pub(crate) fn calculate_window_size_params_if_current_view(
        &mut self,
        reason: &'static str,
        is_expected_view: impl FnOnce(&AppView) -> bool,
    ) -> Option<(ViewType, usize)> {
        if !is_expected_view(&self.current_view) {
            tracing::debug!(
                target: "WINDOW_RESIZE",
                reason,
                current_view = ?self.current_view,
                "Skipping stale deferred resize for inactive view"
            );
            return None;
        }

        self.calculate_window_size_params()
    }

    /// Returns the focused button when the active view is `ConfirmPrompt`.
    pub(crate) fn confirm_prompt_focused_button(&self) -> Option<ConfirmFocusedButton> {
        if let AppView::ConfirmPrompt { focused_button, .. } = &self.current_view {
            Some(*focused_button)
        } else {
            None
        }
    }

    /// Flip Tab focus between Confirm and Cancel inside an active `ConfirmPrompt`.
    pub(crate) fn toggle_confirm_prompt_focus(&mut self, cx: &mut Context<Self>) {
        if let AppView::ConfirmPrompt { focused_button, .. } = &mut self.current_view {
            *focused_button = focused_button.toggled();
            cx.notify();
        }
    }

    /// Send the confirm/cancel result to the awaiting caller and restore the
    /// previous launcher view. No-op if the active view is not `ConfirmPrompt`.
    pub(crate) fn resolve_confirm_prompt(
        &mut self,
        confirmed: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let restore = if let AppView::ConfirmPrompt {
            sender, previous, ..
        } = &self.current_view
        {
            let _ = sender.try_send(confirmed);
            Some((**previous).clone())
        } else {
            None
        };

        if let Some(previous) = restore {
            self.current_view = previous;
            self.sync_main_footer_popup(window, cx);
            cx.notify();
        }
    }

    /// Update window size using deferred execution (SAFE during render/event cycles).
    ///
    /// Uses Window::defer to schedule the resize at the end of the current effect cycle,
    /// preventing RefCell borrow conflicts that can occur when calling platform APIs
    /// during GPUI's render or event processing.
    ///
    /// Use this version when you have access to `window` and `cx`.
    pub(crate) fn update_window_size_deferred(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::FilterChanged,
                sizing,
                f32::from(target_height),
            );
            crate::window_resize::defer_resize_to_main_window(sizing, window, &mut *cx);
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            crate::window_resize::defer_resize_to_view(view_type, item_count, window, &mut *cx);
        }
    }

    /// Update window size synchronously.
    ///
    /// SAFETY: Only call from async handlers (cx.spawn closures, message handlers)
    /// that run OUTSIDE the GPUI render cycle. Calling during render will cause
    /// RefCell borrow panics.
    ///
    /// Prefer `update_window_size_deferred` when you have window/cx access.
    pub(crate) fn update_window_size(&mut self) {
        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::GroupedResultsChanged,
                sizing,
                f32::from(target_height),
            );
            crate::window_resize::resize_to_main_window_sync(sizing);
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            crate::window_resize::resize_to_view_sync(view_type, item_count);
        }
    }

    /// Resize the current surface to its canonical height while restoring an
    /// explicit width.
    pub(crate) fn resize_current_view_to_width(&mut self, target_width: f32) {
        if !target_width.is_finite() || target_width <= 0.0 {
            self.update_window_size();
            return;
        }

        // Content-aware mini mode sizing bypasses the flat (ViewType, item_count) path.
        if matches!(self.current_view, AppView::ScriptList)
            && self.main_window_mode == MainWindowMode::Mini
        {
            let (grouped_items, _) = self.get_grouped_results_cached();
            let sizing = main_window_sizing_from_grouped_items(&grouped_items);
            let target_height = crate::window_resize::height_for_main_window(sizing);
            crate::window_resize::log_main_window_sizing(
                crate::window_resize::ResizeReason::GroupedResultsChanged,
                sizing,
                f32::from(target_height),
            );
            let width = if self.main_window_mode == MainWindowMode::Mini {
                crate::window_resize::width_for_view(ViewType::MainWindow).unwrap_or(target_width)
            } else {
                target_width
            };
            crate::window_resize::resize_first_window_to_size(target_height, Some(width));
            return;
        }

        if let Some((view_type, item_count)) = self.calculate_window_size_params() {
            let target_height = crate::window_resize::height_for_view(view_type, item_count);
            let width = if self.main_window_mode == MainWindowMode::Mini {
                crate::window_resize::width_for_view(ViewType::MainWindow).unwrap_or(target_width)
            } else {
                target_width
            };
            crate::window_resize::resize_first_window_to_size(target_height, Some(width));
        }
    }

    /// Try to insert text into the current prompt's input field.
    ///
    /// Returns `true` when the current view accepted the text (i.e. there is an
    /// active prompt with an input field), `false` otherwise.  Used by dictation
    /// to decide whether to fall back to paste-to-frontmost-app.
    /// Returns `true` when the launcher/main-menu filter is active and can
    /// accept dictated text (i.e. `AppView::ScriptList`).
    pub(crate) fn can_accept_dictation_into_main_filter(&self) -> bool {
        matches!(self.current_view, AppView::ScriptList)
    }

    /// Returns `true` when the current view can accept dictated text directly.
    pub(crate) fn can_accept_dictation_into_prompt(&self) -> bool {
        matches!(
            &self.current_view,
            AppView::ArgPrompt { .. }
                | AppView::MiniPrompt { .. }
                | AppView::MicroPrompt { .. }
                | AppView::PathPrompt { .. }
                | AppView::SelectPrompt { .. }
                | AppView::EnvPrompt { .. }
                | AppView::TemplatePrompt { .. }
                | AppView::FormPrompt { .. }
                | AppView::FileSearchView { .. }
        )
    }

    pub(crate) fn try_set_prompt_input(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        match &mut self.current_view {
            AppView::ArgPrompt { .. } => {
                self.arg_input.set_text(text);
                self.arg_selected_index = 0;
                self.arg_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.update_window_size();
                cx.notify();
                true
            }
            AppView::MiniPrompt { .. } | AppView::MicroPrompt { .. } => {
                self.arg_input.set_text(text);
                self.arg_selected_index = 0;
                self.arg_list_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.update_window_size();
                cx.notify();
                true
            }
            AppView::PathPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::SelectPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::EnvPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::TemplatePrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::FormPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::AgentChatView { entity } => {
                entity.update(cx, |view, cx| view.set_input(text, cx));
                true
            }
            AppView::ChatPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.set_input(text, cx));
                true
            }
            AppView::FileSearchView {
                query,
                selected_index,
                ..
            } => {
                let results = ScriptListApp::resolve_file_search_results(&text);
                logging::log(
                    "EXEC",
                    &format!(
                        "File search setInput '{}' found {} results",
                        text,
                        results.len()
                    ),
                );
                *query = text.clone();
                *selected_index = 0;
                self.update_file_search_results(results);
                self.file_search_scroll_handle
                    .scroll_to_item(0, ScrollStrategy::Top);
                self.filter_text = text;
                self.pending_filter_sync = true;
                cx.notify();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn set_prompt_input(&mut self, text: String, cx: &mut Context<Self>) {
        let _ = self.try_set_prompt_input(text, cx);
    }

    /// Helper to get filtered arg choices without cloning
    pub(crate) fn get_filtered_arg_choices<'a>(&self, choices: &'a [Choice]) -> Vec<&'a Choice> {
        if self.arg_input.is_empty() {
            choices.iter().collect()
        } else {
            let filter = self.arg_input.text().to_lowercase();
            choices
                .iter()
                .filter(|c| c.name.to_lowercase().contains(&filter))
                .collect()
        }
    }

    pub(crate) fn focus_main_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focused_input = FocusedInput::MainFilter;
        let input_state = self.gpui_input_state.clone();
        input_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    /// Apply a dictated transcript to the launcher's shared main-filter input.
    ///
    /// Returns `true` when the launcher was active and the text was applied,
    /// `false` otherwise (caller should fall back to frontmost-app paste).
    pub(crate) fn try_set_main_window_filter_from_dictation(
        &mut self,
        text: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_accept_dictation_into_main_filter() {
            return false;
        }

        let text = crate::components::text_input::core::normalize_single_line_text(text);

        tracing::info!(
            category = "DICTATION",
            event = "dictation_set_main_window_filter",
            text_len = text.len(),
            "Applying dictated transcript to launcher filter"
        );

        self.filter_text = text.clone();
        self.pending_filter_sync = true;
        self.pending_focus = Some(FocusTarget::MainFilter);
        self.focused_input = FocusedInput::MainFilter;
        self.hovered_index = None;
        self.selected_index = 0;
        self.queue_filter_compute(text, cx);
        cx.notify();
        true
    }

    /// Clear the cached preflight receipt so it is rebuilt on the next
    /// call to `rebuild_main_window_preflight_if_needed`.
    /// Kept as explicit API for context-chip toggles and view transitions.
    #[allow(dead_code)]
    pub(crate) fn invalidate_main_window_preflight(&mut self) {
        self.cached_main_window_preflight = None;
        self.main_window_preflight_cache_key.clear();
    }

    /// Rebuild the preflight receipt when the cache key has changed.
    /// Call this from mutation paths (filter change, selection change)
    /// — never from `render()`.
    ///
    /// The cache key covers the row-shaping inputs (filter text + view).
    /// When only `selected_index` changed — the arrow-key scroll hot path —
    /// the cached receipt's visible rows/fingerprints/counts are still valid,
    /// so only the selection-dependent fields are refreshed (O(1)) instead of
    /// rebuilding the full O(visible rows) receipt on every keypress.
    pub(crate) fn rebuild_main_window_preflight_if_needed(&mut self) {
        let rows_key = format!("{}:{:?}", self.filter_text, self.current_view);
        if rows_key == self.main_window_preflight_cache_key {
            let Some(mut receipt) = self.cached_main_window_preflight.take() else {
                // Rows unchanged and the view is not preflight-eligible;
                // selection changes cannot make it eligible.
                return;
            };
            if receipt.selected_index != self.selected_index {
                crate::main_window_preflight::refresh_main_window_preflight_selection(
                    self,
                    &mut receipt,
                );
                if crate::logging::filter_perf_trace_enabled() {
                    crate::main_window_preflight::log_main_window_preflight_receipt(&receipt);
                }
            }
            self.cached_main_window_preflight = Some(receipt);
            return;
        }
        self.main_window_preflight_cache_key = rows_key;
        let receipt = crate::main_window_preflight::build_main_window_preflight_receipt(self);
        if crate::logging::filter_perf_trace_enabled() {
            if let Some(ref r) = receipt {
                crate::main_window_preflight::log_main_window_preflight_receipt(r);
            }
        }
        self.cached_main_window_preflight = receipt;
    }
}

#[cfg(test)]
#[path = "ui_window_tests.rs"]
mod tests;
