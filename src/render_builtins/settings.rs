/// Settings item definition for the hub view.
struct SettingsItem {
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    action: SettingsAction,
}

/// Action to execute when a settings item is selected.
#[derive(Clone)]
enum SettingsAction {
    ChooseTheme,
    ConfigureVercelApiKey,
    ConfigureOpenAiApiKey,
    ConfigureAnthropicApiKey,
    EditMcpServers,
    SelectMicrophone,
    ClearSuggested,
    CheckPermissions,
    RequestAccessibilityPermission,
    OpenAccessibilitySettings,
    DisableWindowSnapping,
    SnapModeSimple,
    SnapModeExpanded,
    SnapModePrecision,
    ResetWindowPositions,
}

fn settings_item_matches_filter(item: &SettingsItem, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }

    let filter_lower = filter.to_lowercase();
    item.name.to_lowercase().contains(&filter_lower)
        || item.description.to_lowercase().contains(&filter_lower)
}

fn filtered_settings_items<'a>(items: &'a [SettingsItem], filter: &str) -> Vec<&'a SettingsItem> {
    items
        .iter()
        .filter(|item| settings_item_matches_filter(item, filter))
        .collect()
}

fn get_settings_items() -> Vec<SettingsItem> {
    let mut items = vec![
        SettingsItem {
            name: "Theme Designer",
            description: "Design your color theme with live preview",
            icon: "palette",
            action: SettingsAction::ChooseTheme,
        },
        SettingsItem {
            name: "Configure Vercel AI Gateway",
            description: "Set up the Vercel AI Gateway API key for ACP Chat",
            icon: "key-round",
            action: SettingsAction::ConfigureVercelApiKey,
        },
        SettingsItem {
            name: "Configure OpenAI API Key",
            description: "Set up the OpenAI API key for ACP Chat",
            icon: "key-round",
            action: SettingsAction::ConfigureOpenAiApiKey,
        },
        SettingsItem {
            name: "Configure Anthropic API Key",
            description: "Set up the Anthropic API key for ACP Chat",
            icon: "key-round",
            action: SettingsAction::ConfigureAnthropicApiKey,
        },
        SettingsItem {
            name: "MCP Servers",
            description: "Edit external MCP servers for scripts and ACP Chat in config.ts",
            icon: "plug",
            action: SettingsAction::EditMcpServers,
        },
        SettingsItem {
            name: "Select Microphone",
            description: "Choose which microphone to use for dictation",
            icon: "mic",
            action: SettingsAction::SelectMicrophone,
        },
        SettingsItem {
            name: "Clear Suggested Items",
            description: "Reset Suggested and Recently Used launcher history",
            icon: "eraser",
            action: SettingsAction::ClearSuggested,
        },
        SettingsItem {
            name: "Check Permissions",
            description: "Run a check for the macOS permissions Script Kit needs",
            icon: "circle-check",
            action: SettingsAction::CheckPermissions,
        },
        SettingsItem {
            name: "Request Accessibility Permission",
            description: "Prompt macOS to grant Script Kit accessibility access",
            icon: "key-round",
            action: SettingsAction::RequestAccessibilityPermission,
        },
        SettingsItem {
            name: "Open Accessibility Settings",
            description: "Open the Accessibility pane in macOS System Settings",
            icon: "accessibility",
            action: SettingsAction::OpenAccessibilitySettings,
        },
    ];

    let snap_mode = crate::window_control::current_snap_mode();

    if snap_mode != crate::window_control::SnapMode::Off {
        items.push(SettingsItem {
            name: "Disable Window Snapping",
            description: "Turn off drag snapping and snap overlays until a snap mode is re-enabled",
            icon: "ban",
            action: SettingsAction::DisableWindowSnapping,
        });
    }

    if snap_mode != crate::window_control::SnapMode::Simple {
        items.push(SettingsItem {
            name: "Snap Mode: Simple",
            description:
                "Use halves, quadrants, center, and almost-maximize targets while dragging windows",
            icon: "square-split-horizontal",
            action: SettingsAction::SnapModeSimple,
        });
    }

    if snap_mode != crate::window_control::SnapMode::Expanded {
        items.push(SettingsItem {
            name: "Snap Mode: Expanded",
            description:
                "Use halves, quadrants, thirds, and two-thirds targets while dragging windows",
            icon: "columns-3",
            action: SettingsAction::SnapModeExpanded,
        });
    }

    if snap_mode != crate::window_control::SnapMode::Precision {
        items.push(SettingsItem {
            name: "Snap Mode: Precision",
            description: "Use the full snap grid including sixths for finer placements",
            icon: "grid-3x2",
            action: SettingsAction::SnapModePrecision,
        });
    }

    if crate::window_state::has_custom_positions() {
        items.push(SettingsItem {
            name: "Reset Window Positions",
            description: "Restore all windows to default positions",
            icon: "refresh-cw",
            action: SettingsAction::ResetWindowPositions,
        });
    }

    items
}

impl ScriptListApp {
    /// Execute a settings action selected from the settings hub.
    fn execute_settings_action(
        &mut self,
        action: &SettingsAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            SettingsAction::ChooseTheme => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "choose_theme",
                    "settings.action_executed"
                );
                self.open_theme_chooser_view(cx);
            }
            SettingsAction::ConfigureVercelApiKey => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "configure_vercel_api_key",
                    "settings.action_executed"
                );
                self.show_api_key_prompt(
                    "SCRIPT_KIT_VERCEL_API_KEY",
                    "Enter your Vercel AI Gateway API key",
                    "Vercel AI Gateway",
                    cx,
                );
            }
            SettingsAction::ConfigureOpenAiApiKey => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "configure_openai_api_key",
                    "settings.action_executed"
                );
                self.show_api_key_prompt(
                    "SCRIPT_KIT_OPENAI_API_KEY",
                    "Enter your OpenAI API key",
                    "OpenAI",
                    cx,
                );
            }
            SettingsAction::ConfigureAnthropicApiKey => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "configure_anthropic_api_key",
                    "settings.action_executed"
                );
                self.show_api_key_prompt(
                    "SCRIPT_KIT_ANTHROPIC_API_KEY",
                    "Enter your Anthropic API key",
                    "Anthropic",
                    cx,
                );
            }
            SettingsAction::EditMcpServers => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "edit_mcp_servers",
                    "settings.action_executed"
                );

                let path = crate::setup::get_kit_path().join("kit").join("config.ts");
                match crate::script_creation::open_in_editor(&path, &self.config) {
                    Ok(()) => {
                        self.show_hud("Opened MCP config".to_string(), Some(HUD_SHORT_MS), cx);
                        self.close_and_reset_window(cx);
                    }
                    Err(error) => {
                        self.show_error_toast(format!("Failed to open MCP config: {error}"), cx);
                    }
                }
            }
            SettingsAction::SelectMicrophone => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "select_microphone",
                    "settings.action_executed"
                );

                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/select-microphone"),
                    name: "Select Microphone".to_string(),
                    description: "Choose which microphone to use for dictation".to_string(),
                    keywords: vec![
                        "microphone".to_string(),
                        "mic".to_string(),
                        "audio".to_string(),
                        "input".to_string(),
                        "dictation".to_string(),
                        "device".to_string(),
                        "recording".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::SettingsCommand(
                        crate::builtins::SettingsCommandType::SelectMicrophone,
                    ),
                    icon: Some("mic".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::ClearSuggested => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "clear_suggested",
                    "settings.action_executed"
                );

                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/clear-suggested"),
                    name: "Clear Suggested".to_string(),
                    description: "Clear all items from Suggested / Recently Used".to_string(),
                    keywords: vec![
                        "clear".to_string(),
                        "suggested".to_string(),
                        "recent".to_string(),
                        "frecency".to_string(),
                        "reset".to_string(),
                        "history".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::FrecencyCommand(
                        crate::builtins::FrecencyCommandType::ClearSuggested,
                    ),
                    icon: Some("eraser".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::CheckPermissions => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/check-permissions"),
                    name: "Check Permissions".to_string(),
                    description: "Run a check for all required macOS permissions".to_string(),
                    keywords: vec![
                        "check".to_string(),
                        "permissions".to_string(),
                        "accessibility".to_string(),
                        "privacy".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::PermissionCommand(
                        crate::builtins::PermissionCommandType::CheckPermissions,
                    ),
                    icon: Some("circle-check".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::RequestAccessibilityPermission => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id(
                        "builtin/request-accessibility",
                    ),
                    name: "Request Accessibility Permission".to_string(),
                    description:
                        "Request accessibility permission for Script Kit in System Settings"
                            .to_string(),
                    keywords: vec![
                        "request".to_string(),
                        "accessibility".to_string(),
                        "permission".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::PermissionCommand(
                        crate::builtins::PermissionCommandType::RequestAccessibility,
                    ),
                    icon: Some("key-round".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::OpenAccessibilitySettings => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id(
                        "builtin/accessibility-settings",
                    ),
                    name: "Open Accessibility Settings".to_string(),
                    description: "Open Accessibility settings in System Preferences".to_string(),
                    keywords: vec![
                        "accessibility".to_string(),
                        "settings".to_string(),
                        "permission".to_string(),
                        "open".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::PermissionCommand(
                        crate::builtins::PermissionCommandType::OpenAccessibilitySettings,
                    ),
                    icon: Some("accessibility".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::DisableWindowSnapping => {
                let entry = crate::builtins::get_builtin_entries(&self.config.get_builtins())
                    .into_iter()
                    .find(|entry| entry.id == "builtin/disable-window-snapping");

                if let Some(entry) = entry {
                    self.execute_builtin(&entry, cx);
                } else {
                    self.show_error_toast("Disable Window Snapping is unavailable", cx);
                }
            }
            SettingsAction::SnapModeSimple => {
                let entry = crate::builtins::get_builtin_entries(&self.config.get_builtins())
                    .into_iter()
                    .find(|entry| entry.id == "builtin/snap-mode-simple");

                if let Some(entry) = entry {
                    self.execute_builtin(&entry, cx);
                } else {
                    self.show_error_toast("Snap Mode: Simple is unavailable", cx);
                }
            }
            SettingsAction::SnapModeExpanded => {
                let entry = crate::builtins::get_builtin_entries(&self.config.get_builtins())
                    .into_iter()
                    .find(|entry| entry.id == "builtin/snap-mode-expanded");

                if let Some(entry) = entry {
                    self.execute_builtin(&entry, cx);
                } else {
                    self.show_error_toast("Snap Mode: Expanded is unavailable", cx);
                }
            }
            SettingsAction::SnapModePrecision => {
                let entry = crate::builtins::get_builtin_entries(&self.config.get_builtins())
                    .into_iter()
                    .find(|entry| entry.id == "builtin/snap-mode-precision");

                if let Some(entry) = entry {
                    self.execute_builtin(&entry, cx);
                } else {
                    self.show_error_toast("Snap Mode: Precision is unavailable", cx);
                }
            }
            SettingsAction::ResetWindowPositions => {
                tracing::info!(
                    correlation_id = "settings-hub",
                    action = "reset_window_positions",
                    "settings.action_executed"
                );
                crate::window_state::suppress_save();
                crate::window_state::reset_all_positions();
                self.show_hud(
                    "Window positions reset - takes effect next open".to_string(),
                    Some(HUD_SHORT_MS),
                    cx,
                );
                self.close_and_reset_window(cx);
            }
        }
    }

    /// Render the settings hub using the same contracted shell as other built-in views.
    fn render_settings(
        &mut self,
        filter: String,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("settings", true),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let design_visual = tokens.visual();

        let text_primary = self.theme.colors.text.primary;
        let text_dimmed = self.theme.colors.text.dimmed;
        let text_muted = self.theme.colors.text.muted;

        let items = get_settings_items();
        let filtered_items = filtered_settings_items(&items, &filter);
        let item_count = filtered_items.len();
        let list_colors = ListItemColors::from_theme(&self.theme);

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                if is_key_escape(key) {
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    cx.stop_propagation();
                    return;
                }

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let (current_filter, current_selected) = if let AppView::SettingsView {
                    filter,
                    selected_index,
                } = &this.current_view
                {
                    (filter.clone(), *selected_index)
                } else {
                    return;
                };

                let settings_items = get_settings_items();
                let filtered_items = filtered_settings_items(&settings_items, &current_filter);
                let filtered_count = filtered_items.len();

                if is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::SettingsView { selected_index, .. } = &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_selected < filtered_count.saturating_sub(1) {
                        if let AppView::SettingsView { selected_index, .. } = &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    if let Some(item) = filtered_items.get(current_selected) {
                        let action = item.action.clone();
                        this.execute_settings_action(&action, window, cx);
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let entity = cx.entity().downgrade();
        let hovered = self.hovered_index;

        let list_items: Vec<AnyElement> = filtered_items
            .iter()
            .enumerate()
            .map(|(ix, item)| {
                let is_selected = ix == selected_index;
                let is_hovered = hovered == Some(ix);
                let action = item.action.clone();
                let entity_click = entity.clone();
                let entity_hover = entity.clone();
                let desc = item.description.to_string();

                div()
                    .id(ix)
                    .cursor_pointer()
                    .on_click(move |_event, window, cx| {
                        if let Some(app) = entity_click.upgrade() {
                            app.update(cx, |this, cx| {
                                this.execute_settings_action(&action, window, cx);
                            });
                        }
                    })
                    .on_hover({
                        let entity_h = entity_hover;
                        move |is_hovered: &bool, _window: &mut Window, cx: &mut gpui::App| {
                            if let Some(app) = entity_h.upgrade() {
                                app.update(cx, |this, cx| {
                                    if *is_hovered {
                                        this.input_mode = InputMode::Mouse;
                                        if this.hovered_index != Some(ix) {
                                            this.hovered_index = Some(ix);
                                            cx.notify();
                                        }
                                    } else if this.hovered_index == Some(ix) {
                                        this.hovered_index = None;
                                        cx.notify();
                                    }
                                });
                            }
                        }
                    })
                    .child(
                        ListItem::new(item.name.to_string(), list_colors)
                            .icon_kind_opt(crate::list_item::IconKind::from_icon_hint(item.icon))
                            .description_opt(Some(desc))
                            .selected(is_selected)
                            .hovered(is_hovered)
                            .with_accent_bar(is_selected),
                    )
                    .into_any_element()
            })
            .collect();

        let list_element: AnyElement = if item_count == 0 {
            div()
                .w_full()
                .py(px(design_spacing.padding_xl))
                .text_center()
                .text_color(rgb(text_muted))
                .font_family(design_typography.font_family)
                .child(if filter.is_empty() {
                    "No settings available"
                } else {
                    "No settings match your filter"
                })
                .into_any_element()
        } else {
            div()
                .w_full()
                .flex()
                .flex_col()
                .min_h(px(0.))
                .children(list_items)
                .into_any_element()
        };

        let header = div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .child(
                div().flex_1().flex().flex_row().items_center().child(
                    Input::new(&self.gpui_input_state)
                        .w_full()
                        .h(px(28.))
                        .px(px(0.))
                        .py(px(0.))
                        .with_size(Size::Size(px(design_typography.font_size_xl)))
                        .appearance(false)
                        .bordered(false)
                        .focus_bordered(false),
                ),
            )
            .child(div().text_sm().text_color(rgb(text_dimmed)).child(format!(
                "{} setting{}",
                item_count,
                if item_count == 1 { "" } else { "s" }
            )));

        let content = div()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .py(px(design_spacing.padding_xs))
            .child(list_element);

        let footer = if matches!(
            crate::footer_popup::active_main_window_footer_surface(),
            Some("settings")
        ) {
            Some(crate::components::prompt_layout_shell::render_native_main_window_footer_spacer())
        } else {
            Some(crate::components::render_simple_hint_strip(
                vec![
                    gpui::SharedString::from("↵ Open"),
                    gpui::SharedString::from("Esc Back"),
                ],
                None,
            ))
        };

        crate::components::render_minimal_list_prompt_shell_with_footer(
            design_visual.radius_lg,
            crate::ui_foundation::get_vibrancy_background(&self.theme),
            header,
            content,
            footer,
        )
        .text_color(rgb(text_primary))
        .font_family(design_typography.font_family)
        .key_context("settings")
        .track_focus(&self.focus_handle)
        .on_key_down(handle_key)
        .into_any_element()
    }
}
