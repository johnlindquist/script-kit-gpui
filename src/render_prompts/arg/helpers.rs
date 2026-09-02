/// Pure clamp step for the arg-input character window: how many characters of
/// the measured `char_width` fit in `usable_width`, bounded so tiny windows
/// still show a useful window and huge windows don't disable windowing.
#[inline]
fn arg_input_visible_chars_for_width(usable_width: f64, char_width: f64) -> usize {
    const ARG_INPUT_MIN_VISIBLE_CHARS: usize = 24;
    const ARG_INPUT_MAX_VISIBLE_CHARS: usize = 240;

    let visible_chars = (usable_width.max(200.0) / char_width.max(1.0)).floor() as usize;
    visible_chars.clamp(ARG_INPUT_MIN_VISIBLE_CHARS, ARG_INPUT_MAX_VISIBLE_CHARS)
}

#[inline]
fn prompt_actions_dialog_offsets(padding_sm: f32, border_thin: f32) -> (f32, f32) {
    // Keep dialog anchored just below the shared header + divider.
    let top = crate::panel::HEADER_TOTAL_HEIGHT + padding_sm - border_thin;
    let right = padding_sm;
    (top, right)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinPromptSubmitTransition {
    ReturnToScriptList,
    CloseMainWindow,
}

impl BuiltinPromptSubmitTransition {
    fn apply(self, app: &mut ScriptListApp, cx: &mut gpui::Context<ScriptListApp>) {
        match self {
            Self::ReturnToScriptList => {
                app.reset_to_script_list(cx);
                cx.notify();
            }
            Self::CloseMainWindow => {
                app.close_and_reset_window(cx);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct PromptRenderContext<'a> {
    theme: &'a crate::theme::Theme,
    design_colors: crate::designs::DesignColors,
    design_spacing: crate::designs::DesignSpacing,
    design_typography: crate::designs::DesignTypography,
    design_visual: crate::designs::DesignVisual,
    actions_dialog_top: f32,
    actions_dialog_right: f32,
}

impl<'a> PromptRenderContext<'a> {
    #[inline]
    fn new(theme: &'a crate::theme::Theme, current_design: DesignVariant) -> Self {
        let tokens = get_tokens(current_design);
        let design_colors = tokens.colors();
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();
        let (actions_dialog_top, actions_dialog_right) =
            prompt_actions_dialog_offsets(design_spacing.padding_sm, design_visual.border_thin);

        Self {
            theme,
            design_colors,
            design_spacing,
            design_typography,
            design_visual,
            actions_dialog_top,
            actions_dialog_right,
        }
    }
}

#[derive(Clone, Copy)]
struct ActionsBackdropConfig {
    backdrop_id: &'static str,
    close_host: ActionsDialogHost,
    backdrop_log_message: &'static str,
    show_pointer_cursor: bool,
}

#[derive(Clone, Copy)]
enum ActionsBackdropVerticalAnchor {
    Top(f32),
    Bottom(f32),
}

#[inline]
fn render_actions_backdrop_with_vertical_anchor(
    show_actions_popup: bool,
    actions_dialog: Option<Entity<ActionsDialog>>,
    actions_dialog_right: f32,
    vertical_anchor: ActionsBackdropVerticalAnchor,
    config: ActionsBackdropConfig,
    cx: &mut Context<ScriptListApp>,
) -> Option<gpui::Div> {
    if !show_actions_popup {
        return None;
    }

    let dialog = actions_dialog?;
    let backdrop_click = cx.listener(
        move |this: &mut ScriptListApp,
              _event: &gpui::ClickEvent,
              window: &mut Window,
              cx: &mut Context<ScriptListApp>| {
            logging::log("FOCUS", config.backdrop_log_message);
            this.close_actions_popup(config.close_host, window, cx);
        },
    );

    let dialog_container = match vertical_anchor {
        ActionsBackdropVerticalAnchor::Top(actions_dialog_top) => div()
            .absolute()
            .top(px(actions_dialog_top))
            .right(px(actions_dialog_right)),
        ActionsBackdropVerticalAnchor::Bottom(actions_dialog_bottom) => div()
            .absolute()
            .bottom(px(actions_dialog_bottom))
            .right(px(actions_dialog_right)),
    };

    Some(
        div()
            .absolute()
            .inset_0()
            .child(
                div()
                    .id(config.backdrop_id)
                    .absolute()
                    .inset_0()
                    .when(config.show_pointer_cursor, |d| d.cursor_pointer())
                    .on_click(backdrop_click),
            )
            .child(dialog_container.child(dialog)),
    )
}

#[inline]
fn render_actions_backdrop(
    show_actions_popup: bool,
    actions_dialog: Option<Entity<ActionsDialog>>,
    actions_dialog_top: f32,
    actions_dialog_right: f32,
    config: ActionsBackdropConfig,
    cx: &mut Context<ScriptListApp>,
) -> Option<gpui::Div> {
    render_actions_backdrop_with_vertical_anchor(
        show_actions_popup,
        actions_dialog,
        actions_dialog_right,
        ActionsBackdropVerticalAnchor::Top(actions_dialog_top),
        config,
        cx,
    )
}

#[inline]
fn render_actions_backdrop_bottom_anchored(
    show_actions_popup: bool,
    actions_dialog: Option<Entity<ActionsDialog>>,
    actions_dialog_bottom: f32,
    actions_dialog_right: f32,
    config: ActionsBackdropConfig,
    cx: &mut Context<ScriptListApp>,
) -> Option<gpui::Div> {
    render_actions_backdrop_with_vertical_anchor(
        show_actions_popup,
        actions_dialog,
        actions_dialog_right,
        ActionsBackdropVerticalAnchor::Bottom(actions_dialog_bottom),
        config,
        cx,
    )
}

#[allow(dead_code)]
#[inline]
fn running_status_text(context: &str) -> String {
    crate::panel::running_status_message(context)
}

#[allow(dead_code)]
#[inline]
fn prompt_footer_colors_for_prompt(
    design_colors: &crate::designs::DesignColors,
    _is_light_mode: bool,
) -> PromptFooterColors {
    PromptFooterColors::from_design(design_colors)
}

#[allow(dead_code)]
#[inline]
fn prompt_footer_config_with_status(
    primary_label: &str,
    show_secondary: bool,
    helper_text: Option<String>,
    info_label: Option<String>,
) -> PromptFooterConfig {
    let mut config = PromptFooterConfig::new()
        .primary_label(primary_label)
        .primary_shortcut("↵")
        .secondary_label("Actions")
        .secondary_shortcut("⌘K")
        .show_secondary(show_secondary);

    if let Some(helper) = helper_text {
        config = config.helper_text(helper);
    }

    if let Some(info) = info_label {
        config = config.info_label(info);
    }

    config
}

#[inline]
fn key_preamble(
    app: &mut ScriptListApp,
    event: &gpui::KeyDownEvent,
    stop_propagation_on_global_shortcut: bool,
    cx: &mut Context<ScriptListApp>,
) -> bool {
    // When active, the shortcut recorder owns key handling for the prompt.
    if app.shortcut_recorder_state.is_some() {
        return true;
    }

    // Escape-closes derives from the per-view DismissPolicy table — renderers
    // may not declare their own dismissability.
    if !app.show_actions_popup
        && app.handle_global_shortcut_with_options(
            event,
            GlobalShortcutEscape::FromDismissPolicy,
            cx,
        )
    {
        if stop_propagation_on_global_shortcut {
            cx.stop_propagation();
        }
        return true;
    }

    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SdkActionShortcutMatch {
    action_name: String,
    shortcut_key: String,
}

#[inline]
fn check_sdk_action_shortcut(
    action_shortcuts: &std::collections::HashMap<String, String>,
    key: &str,
    modifiers: &gpui::Modifiers,
) -> Option<SdkActionShortcutMatch> {
    let shortcut_key = shortcuts::keystroke_to_shortcut(key, modifiers);
    action_shortcuts
        .get(&shortcut_key)
        .cloned()
        .map(|action_name| SdkActionShortcutMatch {
            action_name,
            shortcut_key,
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ArgSubmitOutcome {
    SubmitChoice(String),
    SubmitText(String),
    InvalidEmpty,
}

#[inline]
fn resolve_arg_submit_outcome(
    selected_choice_value: Option<&str>,
    input_text: &str,
) -> ArgSubmitOutcome {
    if let Some(value) = selected_choice_value {
        return ArgSubmitOutcome::SubmitChoice(value.to_string());
    }

    if input_text.is_empty() {
        return ArgSubmitOutcome::InvalidEmpty;
    }

    ArgSubmitOutcome::SubmitText(input_text.to_string())
}

#[inline]
fn resolve_arg_tab_completion(
    filtered: &[(usize, &Choice)],
    selected_index: usize,
) -> Option<String> {
    if filtered.is_empty() {
        return None;
    }

    if filtered.len() == 1 {
        return filtered.first().map(|(_, choice)| choice.name.clone());
    }

    filtered
        .get(selected_index)
        .or_else(|| filtered.first())
        .map(|(_, choice)| choice.name.clone())
}

#[inline]
#[cfg(test)]
fn arg_prompt_hints(has_actions: bool) -> Vec<gpui::SharedString> {
    let mut hints = vec![
        gpui::SharedString::from("↵ Continue"),
        gpui::SharedString::from("Esc Back"),
    ];

    if has_actions {
        hints.insert(1, gpui::SharedString::from("⌘K Actions"));
    }

    hints
}

impl ScriptListApp {
    /// Every selection owner (keys, automation, filtering and reset) uses this
    /// transition so moving away and back still invalidates retained authority.
    pub(crate) fn set_arg_selected_index(&mut self, index: usize) -> bool {
        if self.arg_selected_index == index {
            return false;
        }
        self.arg_selected_index = index;
        self.arg_selection_revision = self.arg_selection_revision.strict_add(1);
        self.mark_main_data_changed();
        true
    }

    #[inline]
    fn arg_prompt_has_choices(&self) -> bool {
        matches!(&self.current_view, AppView::ArgPrompt { choices, .. } if !choices.is_empty())
    }

    #[inline]
    fn sync_arg_prompt_after_text_change(
        &mut self,
        prev_original_idx: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let has_choices = self.arg_prompt_has_choices();
        let (new_selected_idx, filtered_len) = {
            let filtered = self.filtered_arg_choices();
            let new_idx = if let Some(prev_idx) = prev_original_idx {
                filtered
                    .iter()
                    .position(|(orig_idx, _)| *orig_idx == prev_idx)
                    .unwrap_or(0)
            } else {
                0
            };

            (new_idx, filtered.len())
        };

        self.set_arg_selected_index(new_selected_idx);

        // Defer resize through window_ops to avoid RefCell borrow conflicts during native callbacks.
        // GEO-002: ask the shared Arg resolver for the current target height
        // (renderer-derived row slot, real header chrome, rendered footer
        // reservation) instead of the retired model-only 40px formula.
        let intended_choice_count = if filtered_len == 0 && has_choices {
            // Choices exist but the filter matched none: keep the one-row
            // empty-state surface the renderer paints.
            0
        } else {
            filtered_len
        };
        let (_, target_height) =
            crate::window_resize::arg_layout::current_resolved_arg_layout_for_target(
                crate::window_resize::arg_layout::ArgPresentationMode::Full,
                intended_choice_count.max(usize::from(has_choices)),
                f32::from(crate::window_resize::layout::MIN_HEIGHT),
                crate::window_resize::current_standard_height(),
            );
        crate::window_ops::queue_resize(target_height, window, &mut *cx);
    }

    #[inline]
    fn resolve_current_arg_submit_outcome(&self) -> ArgSubmitOutcome {
        let filtered = self.filtered_arg_choices();
        let selected_choice_value = filtered
            .get(self.arg_selected_index)
            .map(|(_, choice)| choice.value.as_str());
        resolve_arg_submit_outcome(selected_choice_value, self.arg_input.text())
    }

    #[inline]
    fn submit_arg_prompt_from_current_state(&mut self, prompt_id: &str, cx: &mut Context<Self>) {
        match self.resolve_current_arg_submit_outcome() {
            ArgSubmitOutcome::SubmitChoice(value) | ArgSubmitOutcome::SubmitText(value) => {
                // Intercept built-in mic selection prompt — persist device and
                // return to the script list instead of sending to a script.
                if prompt_id == BUILTIN_MIC_SELECT_PROMPT_ID {
                    if !self.is_valid_builtin_mic_selection(&value) {
                        self.show_error_toast("Select a microphone from the list", cx);
                        return;
                    }
                    self.record_submit_diagnostic(
                        "prompt",
                        "submit_arg_prompt_from_current_state",
                        Some(prompt_id),
                        Some(value.as_str()),
                        true,
                    );
                    self.handle_builtin_mic_selection(&value, cx);
                    return;
                }

                // Intercept dictation model download consent prompt.
                if prompt_id == BUILTIN_DICTATION_MODEL_PROMPT_ID {
                    if !matches!(
                        value.as_str(),
                        BUILTIN_DICTATION_MODEL_DOWNLOAD
                            | BUILTIN_DICTATION_MODEL_USE_RECOMMENDED
                            | BUILTIN_DICTATION_MODEL_CANCEL
                            | BUILTIN_DICTATION_MODEL_HIDE
                            | BUILTIN_DICTATION_MODEL_START_PENDING
                    ) {
                        self.show_error_toast("Choose Download or Not now", cx);
                        return;
                    }
                    self.record_submit_diagnostic(
                        "prompt",
                        "submit_arg_prompt_from_current_state",
                        Some(prompt_id),
                        Some(value.as_str()),
                        true,
                    );
                    self.handle_dictation_model_selection(&value, cx);
                    return;
                }

                // Intercept dictation setup prompt.
                if prompt_id == BUILTIN_DICTATION_SETUP_PROMPT_ID {
                    self.record_submit_diagnostic(
                        "prompt",
                        "submit_arg_prompt_from_current_state",
                        Some(prompt_id),
                        Some(value.as_str()),
                        true,
                    );
                    self.handle_dictation_setup_selection(&value, cx);
                    return;
                }

                // Intercept snap mode configuration prompt.
                if prompt_id == BUILTIN_SNAP_MODE_PROMPT_ID {
                    if !self.is_valid_builtin_snap_mode_selection(&value) {
                        self.show_error_toast("Select a snap mode from the list", cx);
                        return;
                    }
                    self.record_submit_diagnostic(
                        "prompt",
                        "submit_arg_prompt_from_current_state",
                        Some(prompt_id),
                        Some(value.as_str()),
                        true,
                    );
                    self.handle_builtin_snap_mode_selection(&value, cx);
                    return;
                }
                self.record_submit_diagnostic(
                    "prompt",
                    "submit_arg_prompt_from_current_state",
                    Some(prompt_id),
                    Some(value.as_str()),
                    true,
                );
                self.submit_prompt_response(prompt_id.to_string(), Some(value), cx);
            }
            ArgSubmitOutcome::InvalidEmpty => {
                self.show_hud(
                    "Type a value to continue".to_string(),
                    Some(HUD_SHORT_MS),
                    cx,
                );
            }
        }
    }

    fn is_valid_builtin_mic_selection(&self, value: &str) -> bool {
        value == BUILTIN_MIC_DEFAULT_VALUE
            || matches!(
                &self.current_view,
                AppView::ArgPrompt { choices, .. } | AppView::MiniPrompt { choices, .. }
                    if choices.iter().any(|choice| choice.value == value)
            )
    }

    /// Persist the microphone selection and return to the script list.
    fn handle_builtin_mic_selection(&mut self, value: &str, cx: &mut Context<Self>) {
        let action = if value == BUILTIN_MIC_DEFAULT_VALUE {
            crate::dictation::DictationDeviceSelectionAction::UseSystemDefault
        } else {
            crate::dictation::DictationDeviceSelectionAction::UseDevice(
                crate::dictation::DictationDeviceId(value.to_string()),
            )
        };

        match crate::dictation::apply_device_selection(&action) {
            Ok(()) => {
                let label = match &action {
                    crate::dictation::DictationDeviceSelectionAction::UseDevice(_) => {
                        // Find the device name from the choices for a nicer HUD message.
                        let choices_ref = match &self.current_view {
                            AppView::ArgPrompt { ref choices, .. }
                            | AppView::MiniPrompt { ref choices, .. } => Some(choices),
                            _ => None,
                        };
                        if let Some(choices) = choices_ref {
                            choices
                                .iter()
                                .find(|c| c.value == value)
                                .map(|c| c.name.trim_end_matches(" (current)").to_string())
                                .unwrap_or_else(|| "Selected".to_string())
                        } else {
                            "Selected".to_string()
                        }
                    }
                    crate::dictation::DictationDeviceSelectionAction::UseSystemDefault => {
                        "System Default".to_string()
                    }
                };
                self.show_hud(format!("Microphone: {label}"), Some(HUD_SHORT_MS), cx);
            }
            Err(error) => {
                tracing::error!(
                    category = "DICTATION",
                    error = %error,
                    "Failed to save microphone preference"
                );
                self.show_error_toast(format!("Failed to save microphone: {error}"), cx);
            }
        }

        BuiltinPromptSubmitTransition::ReturnToScriptList.apply(self, cx);
    }

    fn is_valid_builtin_snap_mode_selection(&self, value: &str) -> bool {
        matches!(value, "off" | "simple" | "expanded" | "precision")
    }

    /// Persist the snap mode selection and close the main window.
    fn handle_builtin_snap_mode_selection(&mut self, value: &str, cx: &mut Context<Self>) {
        let target_mode = match value {
            "off" => crate::window_control::SnapMode::Off,
            "simple" => crate::window_control::SnapMode::Simple,
            "expanded" => crate::window_control::SnapMode::Expanded,
            "precision" => crate::window_control::SnapMode::Precision,
            _ => {
                self.show_error_toast("Invalid snap mode selection", cx);
                return;
            }
        };

        let previous = crate::window_control::current_snap_mode();
        let runtime_active = crate::window_control::is_snap_runtime_active();

        let mode = match crate::window_control::persist_snap_mode(target_mode) {
            Ok(mode) => mode,
            Err(error) => {
                tracing::error!(
                    category = "WINDOW",
                    %error,
                    ?target_mode,
                    "Failed to persist snap mode from built-in prompt"
                );
                self.show_error_toast(format!("Failed to update snap mode: {error}"), cx);
                return;
            }
        };

        if runtime_active {
            let runtime_result = if mode == crate::window_control::SnapMode::Off {
                crate::window_control::cancel_snap_runtime(cx)
            } else {
                crate::window_control::refresh_snap_runtime_for_mode(cx)
            };

            if let Err(error) = runtime_result {
                tracing::warn!(
                    category = "WINDOW",
                    %error,
                    ?mode,
                    "Failed to apply runtime transition after snap mode change"
                );
            }
        }

        tracing::info!(
            category = "WINDOW",
            previous = ?previous,
            ?mode,
            runtime_active,
            "Updated snap mode from built-in prompt choice"
        );

        let hud_text = match mode {
            crate::window_control::SnapMode::Off => "Window snapping disabled",
            crate::window_control::SnapMode::Simple => "Snap mode: Simple",
            crate::window_control::SnapMode::Expanded => "Snap mode: Expanded",
            crate::window_control::SnapMode::Precision => "Snap mode: Precision",
        };

        self.show_hud(hud_text.to_string(), Some(HUD_SHORT_MS), cx);
        BuiltinPromptSubmitTransition::CloseMainWindow.apply(self, cx);
    }

    #[inline]
    fn apply_arg_tab_completion(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (completion_text, prev_original_idx) = {
            let filtered = self.filtered_arg_choices();
            (
                resolve_arg_tab_completion(&filtered, self.arg_selected_index),
                filtered
                    .get(self.arg_selected_index)
                    .map(|(original_idx, _)| *original_idx),
            )
        };

        let Some(completion_text) = completion_text else {
            return false;
        };

        if self.arg_input.text() == completion_text {
            return true;
        }

        self.arg_input.set_text(completion_text);
        self.arg_input.move_to_end(false);
        self.sync_arg_prompt_after_text_change(prev_original_idx, window, cx);
        cx.notify();
        true
    }
}
