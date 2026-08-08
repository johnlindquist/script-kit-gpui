// `SettingsItem`, `SettingsAction`, `SettingsActionDescriptor`, the pure
// item census (`get_settings_items_for`), the filter helpers, the count-label
// formatter, and the layout resolver live in `settings_contract.rs` (same
// include chain), shared verbatim with the lib-side design-token exporter.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsEmptyState {
    NoSettingsAvailable,
    NoFilteredMatches,
}

impl SettingsEmptyState {
    fn from_filter(filter: &str) -> Self {
        if filter.is_empty() {
            Self::NoSettingsAvailable
        } else {
            Self::NoFilteredMatches
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoSettingsAvailable => "No settings available",
            Self::NoFilteredMatches => "No settings match your filter",
        }
    }
}

/// Which surface activated a Settings action (GEO-006). Every route funnels
/// into the SAME ID-based execution boundary; the source is observability,
/// never a behavior fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // NativeFooter/GpuiFallback arm land with INT-GEO-006-ACTION-PROJECTIONS
enum SettingsActivationSource {
    Enter,
    Click,
    NativeFooter,
    GpuiFallback,
}

/// Runtime wrapper: the ONLY place the live window-state config feeds the
/// settings census. Everything downstream (renderer, exporter, tests) goes
/// through the pure `get_settings_items_for(has_custom_positions)` in
/// `settings_contract.rs`.
fn get_settings_items() -> Vec<SettingsItem> {
    get_settings_items_for(crate::window_state::has_custom_positions())
}

impl ScriptListApp {
    fn settings_visible_row_names(&self, filter: &str) -> Vec<String> {
        self.settings_visible_row_labels(filter)
    }

    fn settings_filtered_rows<'a>(
        &self,
        items: &'a [SettingsItem],
        filter: &str,
    ) -> Vec<&'a SettingsItem> {
        filtered_settings_items(items, filter)
    }

    fn settings_visible_row_labels(&self, filter: &str) -> Vec<String> {
        let items = get_settings_items();
        self.settings_filtered_rows(&items, filter)
            .into_iter()
            .map(|item| item.name.to_string())
            .collect()
    }

    fn settings_dataset_and_visible_counts(&self, filter: &str) -> (usize, usize) {
        let items = get_settings_items();
        let visible_count = self.settings_filtered_rows(&items, filter).len();
        (items.len(), visible_count)
    }

    fn settings_selected_visible_row(&self, filter: &str, selected_index: usize) -> Option<String> {
        let items = get_settings_items();
        self.settings_filtered_rows(&items, filter)
            .get(selected_index)
            .map(|item| item.name.to_string())
    }

    fn settings_selected_visible_row_name(
        &self,
        filter: &str,
        selected_index: usize,
    ) -> Option<String> {
        self.settings_selected_visible_row(filter, selected_index)
    }

    /// Real runtime prerequisites for Settings actions. The ONLY currently
    /// honest disabled state is a missing configure-snap-mode builtin; do
    /// not manufacture others.
    fn settings_action_availability(&self) -> SettingsActionAvailability {
        SettingsActionAvailability {
            configure_snap_mode: crate::builtins::get_builtin_entries(&self.config.get_builtins())
                .iter()
                .any(|entry| entry.id == "builtin/configure-snap-mode"),
        }
    }

    /// The one submission funnel both routes (and, post-integration, the
    /// native footer bridge) call with the selected descriptor's action ID.
    fn submit_settings_action(
        &mut self,
        action_id: SettingsActionId,
        source: SettingsActivationSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.execute_settings_action(action_id, source, window, cx);
    }

    /// Execute a settings action by its stable ID (GEO-006). The ID is the
    /// only execution currency; the enabled state is rechecked HERE so a
    /// stale projection can never invoke a disabled action.
    fn execute_settings_action(
        &mut self,
        action_id: SettingsActionId,
        source: SettingsActivationSource,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(action) = SettingsAction::from_id(action_id) else {
            tracing::error!(
                action_id = action_id.as_str(),
                source = ?source,
                "settings.unknown_action_id"
            );
            return;
        };
        let descriptor = action.descriptor(self.settings_action_availability());
        if !descriptor.enabled {
            tracing::info!(
                action_id = descriptor.action_id.as_str(),
                reason = descriptor.disabled_reason.unwrap_or("unavailable"),
                source = ?source,
                "settings.action_refused"
            );
            // Preserve the pre-descriptor user-visible feedback for the one
            // real refusal (missing snap-mode builtin): an error toast with
            // the same reason the descriptor projects.
            if let Some(reason) = descriptor.disabled_reason {
                self.show_error_toast(reason, cx);
            }
            return;
        }
        tracing::info!(
            correlation_id = "settings-hub",
            action_id = descriptor.action_id.as_str(),
            primary_verb = descriptor.primary_verb,
            destination_surface = descriptor.destination.surface,
            destination_operation = descriptor.destination.operation,
            source = ?source,
            "settings.action_executed"
        );

        match action {
            SettingsAction::ChooseTheme => {
                self.open_theme_chooser_view(cx);
            }
            SettingsAction::DictationSetup => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/dictation-setup"),
                    name: "Dictation Setup".to_string(),
                    description: "Check dictation model, microphone, and hotkey readiness"
                        .to_string(),
                    keywords: vec![
                        "dictation".to_string(),
                        "setup".to_string(),
                        "microphone".to_string(),
                        "parakeet".to_string(),
                        "hotkey".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::SettingsCommand(
                        crate::builtins::SettingsCommandType::DictationSetup,
                    ),
                    icon: Some("sliders-horizontal".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::SelectMicrophone => {
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
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/clear-suggested"),
                    name: "Clear Suggested Items".to_string(),
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
            SettingsAction::SetupPermissions => {
                self.open_permissions_wizard(cx);
            }
            SettingsAction::AllowAccessibility => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id("builtin/allow-accessibility"),
                    name: "Open Accessibility Assistant".to_string(),
                    description: "Open the Permission Assistant for Accessibility".to_string(),
                    keywords: vec![
                        "allow".to_string(),
                        "accessibility".to_string(),
                        "permission".to_string(),
                        "privacy".to_string(),
                        "assistant".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::PermissionCommand(
                        crate::builtins::PermissionCommandType::AllowAccessibility,
                    ),
                    icon: Some("shield-check".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::AllowScreenRecording => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id(
                        "builtin/allow-screen-recording",
                    ),
                    name: "Open Screen Recording Assistant".to_string(),
                    description: "Open the Permission Assistant for Screen Recording".to_string(),
                    keywords: vec![
                        "allow".to_string(),
                        "screen".to_string(),
                        "recording".to_string(),
                        "permission".to_string(),
                        "privacy".to_string(),
                        "assistant".to_string(),
                    ],
                    feature: crate::builtins::BuiltInFeature::PermissionCommand(
                        crate::builtins::PermissionCommandType::AllowScreenRecording,
                    ),
                    icon: Some("shield-check".to_string()),
                    group: crate::builtins::BuiltInGroup::Core,
                };

                self.execute_builtin(&entry, cx);
            }
            SettingsAction::RequestAccessibilityPermission => {
                let entry = crate::builtins::BuiltInEntry {
                    id: crate::config::canonical_builtin_command_id(
                        "builtin/request-accessibility",
                    ),
                    name: "Request Accessibility Access".to_string(),
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
                    description: "Open Accessibility settings in macOS System Settings".to_string(),
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
            SettingsAction::ConfigureSnapMode => {
                let entry = crate::builtins::get_builtin_entries(&self.config.get_builtins())
                    .into_iter()
                    .find(|entry| entry.id == "builtin/configure-snap-mode");

                if let Some(entry) = entry {
                    self.execute_builtin(&entry, cx);
                } else {
                    // Defensive: availability rechecked above should have
                    // refused already; keep the exact legacy feedback.
                    self.show_error_toast(SETTINGS_CONFIGURE_SNAP_MODE_DISABLED_REASON, cx);
                }
            }
            SettingsAction::ResetWindowPositions => {
                self.reset_window_positions_to_default_main_menu(cx);
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
        // Resolve the ACTIVE main-menu theme def FIRST: it owns section
        // geometry/typography for the shared leading separator (GEO-007).
        let menu_def = self.current_main_menu_theme.def();
        // Shared with the design-token exporter (settings_contract.rs).
        let hub_layout = resolved_settings_hub_layout_for(design_spacing, menu_def);
        let _design_typography = tokens.typography();

        let chrome = theme::AppChromeColors::from_theme(&self.theme);

        let items = get_settings_items();
        let filtered_items = filtered_settings_items(&items, &filter);
        let item_count = filtered_items.len();
        let list_colors = ListItemColors::from_theme(&self.theme);
        let availability = self.settings_action_availability();
        // GEO-006: the selected descriptor is the ONLY source for the footer
        // hint (and, post-integration, the native footer + AX projections).
        let selected_action =
            selected_settings_action_descriptor(&items, &filter, selected_index, availability);

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;
                let modifiers = &event.keystroke.modifiers;

                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::BuiltinList,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {}
                    ActionsRoute::Handled => {
                        tracing::debug!(
                            target: "script_kit::actions",
                            event = "builtin_view_actions_key_routed",
                            surface = "settings",
                            key = %key,
                        );
                        cx.stop_propagation();
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::BuiltinList,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        cx.stop_propagation();
                        return;
                    }
                }

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
                        this.builtin_row_stack_scroll_handle
                            .scroll_to_item(current_selected - 1);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_selected < filtered_count.saturating_sub(1) {
                        if let AppView::SettingsView { selected_index, .. } = &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                        }
                        this.builtin_row_stack_scroll_handle
                            .scroll_to_item(current_selected + 1);
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_enter(key) {
                    // GEO-006: Enter resolves the selected descriptor and
                    // submits its stable action ID — never the enum directly.
                    if let Some(descriptor) = selected_settings_action_descriptor(
                        &settings_items,
                        &current_filter,
                        current_selected,
                        this.settings_action_availability(),
                    ) {
                        this.submit_settings_action(
                            descriptor.action_id,
                            SettingsActivationSource::Enter,
                            window,
                            cx,
                        );
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
                let entity_click = entity.clone();
                let entity_hover = entity.clone();
                let desc = item.description.to_string();

                div()
                    .id(ix)
                    .cursor_pointer()
                    .on_click(move |event, window, cx| {
                        if let Some(app) = entity_click.upgrade() {
                            app.update(cx, |this, cx| {
                                let was_selected =
                                    if let AppView::SettingsView { selected_index, .. } =
                                        &mut this.current_view
                                    {
                                        let was_selected = *selected_index == ix;
                                        *selected_index = ix;
                                        was_selected
                                    } else {
                                        false
                                    };
                                let click_count = event.click_count();
                                if crate::ui_foundation::should_submit_selected_row_click(
                                    was_selected,
                                    click_count,
                                ) {
                                    // GEO-006: the executing click resolves the
                                    // SAME selected descriptor as Enter and
                                    // submits its action ID.
                                    let current_filter = if let AppView::SettingsView {
                                        filter,
                                        ..
                                    } = &this.current_view
                                    {
                                        filter.clone()
                                    } else {
                                        String::new()
                                    };
                                    let settings_items = get_settings_items();
                                    if let Some(descriptor) = selected_settings_action_descriptor(
                                        &settings_items,
                                        &current_filter,
                                        ix,
                                        this.settings_action_availability(),
                                    ) {
                                        this.submit_settings_action(
                                            descriptor.action_id,
                                            SettingsActivationSource::Click,
                                            window,
                                            cx,
                                        );
                                    }
                                } else {
                                    cx.notify();
                                }
                            });
                        }
                        cx.stop_propagation();
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
                        // GEO-007: Settings rows are structurally iconless —
                        // `SettingsItem` has no icon field and no parser runs.
                        ListItem::new(item.name.to_string(), list_colors)
                            .icon_kind_opt(None)
                            .description_opt(Some(desc))
                            .selected(is_selected)
                            .hovered(is_hovered),
                    )
                    .into_any_element()
            })
            .collect();

        let list_element: AnyElement = if item_count == 0 {
            let state = SettingsEmptyState::from_filter(&filter);
            crate::components::render_simple_empty_state(
                "settings-empty",
                state.message(),
                "settings",
                None,
                &self.theme,
                cx,
            )
        } else {
            crate::components::scrollbar::render_tracked_scroll_column(
                "settings-row-stack",
                &self.builtin_row_stack_scroll_handle,
                list_items,
            )
        };

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .overflow_hidden()
            .py(px(hub_layout.list_padding_y))
            .child(
                // Every list leads with a persistent section separator
                // (POLISH.md layout-stability bar; same rule as the main
                // menu's "Results" header, 4d76327b8): the label may swap but
                // the row never appears or disappears, so filtering can't
                // shift the rows below it.
                crate::components::builtin_leading_separator::render_builtin_leading_separator(
                    if filter.trim().is_empty() {
                        SETTINGS_HUB_EMPTY_FILTER_SECTION_LABEL
                    } else {
                        SETTINGS_HUB_FILTERED_SECTION_LABEL
                    },
                    None,
                    list_colors,
                ),
            )
            .child(div().relative().flex_1().min_h(px(0.)).child(list_element));

        // GEO-006: the GPUI fallback footer derives its verb from the
        // selected descriptor — "Open" is never reconstructed locally. With
        // no selectable row the footer is honestly Back-only; a disabled
        // descriptor shows its reason instead of an executable hint.
        let footer_hints: Vec<gpui::SharedString> = match selected_action {
            Some(action) if action.enabled => vec![
                gpui::SharedString::from(format!("\u{21B5} {}", action.primary_verb)),
                gpui::SharedString::from("Esc Back"),
            ],
            Some(action) => vec![
                gpui::SharedString::from(
                    action.disabled_reason.unwrap_or("Unavailable").to_string(),
                ),
                gpui::SharedString::from("Esc Back"),
            ],
            None => vec![gpui::SharedString::from("Esc Back")],
        };
        let footer = self
            .main_window_footer_slot(crate::components::render_simple_hint_strip(
                footer_hints,
                None,
            ));

        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .text_color(rgb(chrome.text_primary_hex))
                .font_family(self.theme_font_family())
                .key_context("settings")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(
                    vec![
                        self.render_builtin_main_input_count_label(format_settings_count_label(
                            item_count,
                        )),
                    ],
                    cx,
                ),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: content.into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
    }
}
