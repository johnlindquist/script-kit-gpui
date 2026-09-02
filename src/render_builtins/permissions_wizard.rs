/// Permissions wizard: guided grant flow for the macOS permissions Script Kit
/// uses (model lives in `crate::permissions_wizard`).
///
/// Rows re-detect TCC status on every render, and a 2s poll keeps the surface
/// live while the user flips to System Settings and back — TCC publishes no
/// grant notifications, so polling is the reliable pattern here.
///
/// Grant flow per row (Enter/click): fire the one-shot native OS prompt via
/// `permissions_wizard::request_permission`; if the grant doesn't land
/// immediately, fall back to the Permiso assistant (Accessibility / Screen
/// Recording) or the matching System Settings pane, and let the poll flip the
/// card to Granted when the user returns.
impl ScriptListApp {
    fn permissions_wizard_rows(
        &self,
    ) -> Vec<(
        crate::permissions_wizard::PermissionKind,
        crate::platform::permiso_detect::PermissionStatus,
    )> {
        match self.main_services.owned_sources() {
            Some(sources) => sources.permissions.clone(),
            None => crate::permissions_wizard::PermissionKind::all()
                .iter()
                .map(|&kind| (kind, crate::permissions_wizard::detect_permission(kind)))
                .collect(),
        }
    }
}

fn permissions_wizard_requirement_label(
    kind: crate::permissions_wizard::PermissionKind,
) -> &'static str {
    use crate::permissions_wizard::PermissionRequirement;
    match kind.requirement() {
        PermissionRequirement::Required => "Required",
        PermissionRequirement::Recommended => "Recommended",
        PermissionRequirement::Optional => "Optional",
    }
}

impl ScriptListApp {
    /// Apply the permission onboarding decision at startup.
    ///
    /// Fresh installs (or installs that never completed onboarding) with
    /// missing required permissions get the full wizard and the launcher is
    /// shown; installs that already completed onboarding get a reminder toast
    /// plus an OS notification (the launcher is usually hidden at startup, so
    /// the notification is the part the user actually sees).
    pub(crate) fn apply_permission_startup_intent(
        &mut self,
        is_fresh_install: bool,
        cx: &mut Context<Self>,
    ) {
        match crate::permissions_wizard::startup_intent(is_fresh_install) {
            crate::permissions_wizard::PermissionStartupIntent::OpenFullWizard => {
                tracing::info!(
                    event = "permission_onboarding_full_wizard",
                    is_fresh_install,
                    "Opening permissions wizard at startup"
                );
                self.open_permissions_wizard(cx);
                script_kit_gpui::request_show_main_window();
            }
            crate::permissions_wizard::PermissionStartupIntent::ShowReminder { missing } => {
                let names = missing
                    .iter()
                    .map(|kind| kind.name())
                    .collect::<Vec<_>>()
                    .join(", ");
                tracing::info!(
                    event = "permission_onboarding_reminder",
                    missing = %names,
                    "Missing required permissions at startup"
                );
                self.toast_manager.push(
                    components::toast::Toast::warning(
                        format!(
                            "Missing required permissions: {names}. Run \u{201c}Set Up Permissions\u{201d} to grant them."
                        ),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_WARNING_MS)),
                );
                let _ = notify_rust::Notification::new()
                    .summary("Script Kit needs permissions")
                    .body(&format!(
                        "{names} not granted. Open Script Kit and run \u{201c}Set Up Permissions\u{201d}."
                    ))
                    .show();
                cx.notify();
            }
            crate::permissions_wizard::PermissionStartupIntent::None => {}
        }
    }

    pub(crate) fn open_permissions_wizard(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
            self.current_view = AppView::PermissionsWizardView { selected_index: 0 };
            self.note_main_route_changed();
            self.spawn_permissions_wizard_poll(cx);
        }
        cx.notify();
    }

    /// Re-render the wizard every 2s while it is the active view so cards flip
    /// to Granted after the user toggles Script Kit in System Settings.
    /// The loop exits as soon as the view changes, so reopening the wizard
    /// (which spawns a fresh loop) never accumulates pollers.
    fn spawn_permissions_wizard_poll(&mut self, cx: &mut Context<Self>) {
        if !self.main_services.is_production() {
            return;
        }
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;

            let mut keep_polling = false;
            cx.update(|cx| {
                let _ = this.update(cx, |app, cx| {
                    keep_polling =
                        matches!(app.current_view, AppView::PermissionsWizardView { .. });
                    if keep_polling && script_kit_gpui::is_main_window_visible() {
                        cx.notify();
                    }
                });
            });

            if !keep_polling {
                break;
            }
        })
        .detach();
    }

    fn execute_permissions_wizard_grant(
        &mut self,
        kind: crate::permissions_wizard::PermissionKind,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
        {
            self.show_error_toast(error.to_string(), cx);
            return;
        }
        use crate::platform::permiso_detect::PermissionStatus;

        if crate::permissions_wizard::detect_permission(kind) == PermissionStatus::Authorized {
            self.show_hud(
                format!("{} already granted", kind.name()),
                Some(HUD_SHORT_MS),
                cx,
            );
            cx.notify();
            return;
        }

        if crate::permissions_wizard::request_permission(kind) == PermissionStatus::Authorized {
            self.show_hud(format!("{} granted", kind.name()), Some(HUD_SHORT_MS), cx);
            cx.notify();
            return;
        }

        let opened = match kind {
            crate::permissions_wizard::PermissionKind::Accessibility => {
                platform::permiso::PermisoAssistant::present_retained(
                    platform::permiso::PermisoPanel::Accessibility,
                )
                .is_ok()
            }
            crate::permissions_wizard::PermissionKind::ScreenRecording => {
                platform::permiso::PermisoAssistant::present_retained(
                    platform::permiso::PermisoPanel::ScreenRecording,
                )
                .is_ok()
            }
            _ => crate::permissions_wizard::open_permission_settings(kind).is_ok(),
        };

        if opened {
            self.toast_manager.push(
                components::toast::Toast::info(
                    format!(
                        "Enable \u{201c}{}\u{201d} for Script Kit in System Settings — this list updates automatically",
                        kind.name()
                    ),
                    &self.theme,
                )
                .duration_ms(Some(TOAST_INFO_MS)),
            );
        } else {
            self.show_error_toast(
                format!("Couldn't open System Settings for {}", kind.name()),
                cx,
            );
        }
        cx.notify();
    }

    pub(crate) fn dispatch_permissions_wizard_action(
        &mut self,
        action: crate::permissions_wizard::PermissionsWizardAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::PermissionsWizardView { .. }) {
            return false;
        }

        match action {
            crate::permissions_wizard::PermissionsWizardAction::GrantSelected => {
                let AppView::PermissionsWizardView { selected_index } = &self.current_view else {
                    return false;
                };
                if let Some(&kind) =
                    crate::permissions_wizard::PermissionKind::all().get(*selected_index)
                {
                    self.execute_permissions_wizard_grant(kind, cx);
                }
            }
            crate::permissions_wizard::PermissionsWizardAction::Done => {
                crate::permissions_wizard::mark_onboarding_completed();
                self.go_back_or_close(window, cx);
            }
        }
        true
    }

    /// Render the permissions wizard using the same contracted shell as other
    /// built-in views, with a static title in place of the filter input.
    fn render_permissions_wizard(
        &mut self,
        selected_index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("permissions_wizard", true),
        );

        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let chrome = theme::AppChromeColors::from_theme(&self.theme);
        let info_palette = crate::components::info_palette(&self.theme);
        let success_color = gpui::rgb(self.theme.colors.ui.success);

        let rows = self.permissions_wizard_rows();
        let granted_count = rows
            .iter()
            .filter(|(_, status)| {
                *status == crate::platform::permiso_detect::PermissionStatus::Authorized
            })
            .count();
        let row_count = rows.len();
        let all_required_granted = rows.iter().all(|(kind, status)| {
            kind.requirement() != crate::permissions_wizard::PermissionRequirement::Required
                || *status == crate::platform::permiso_detect::PermissionStatus::Authorized
        });
        let list_colors = ListItemColors::from_theme(&self.theme);

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                if let Some(action) =
                    crate::permissions_wizard::PermissionsWizardActions::for_key(key, has_cmd)
                {
                    this.dispatch_permissions_wizard_action(action, window, cx);
                    cx.stop_propagation();
                    return;
                }

                let current_selected =
                    if let AppView::PermissionsWizardView { selected_index } = &this.current_view {
                        *selected_index
                    } else {
                        return;
                    };

                let row_count = crate::permissions_wizard::PermissionKind::all().len();

                if is_key_up(key) {
                    if current_selected > 0 {
                        if let AppView::PermissionsWizardView { selected_index } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected - 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else if is_key_down(key) {
                    if current_selected < row_count.saturating_sub(1) {
                        if let AppView::PermissionsWizardView { selected_index } =
                            &mut this.current_view
                        {
                            *selected_index = current_selected + 1;
                        }
                        cx.notify();
                    }
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            },
        );

        let entity = cx.entity().downgrade();
        let hovered = self.hovered_index;

        let list_items: Vec<AnyElement> = rows
            .iter()
            .enumerate()
            .map(|(ix, (kind, status))| {
                let kind = *kind;
                let granted = *status
                    == crate::platform::permiso_detect::PermissionStatus::Authorized;
                let is_selected = ix == selected_index;
                let is_hovered = hovered == Some(ix);
                let entity_click = entity.clone();
                let entity_hover = entity.clone();

                let badge: AnyElement = if granted {
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(success_color)
                        .child("✓ Granted")
                        .into_any_element()
                } else {
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(info_palette.hint)
                        .child(permissions_wizard_requirement_label(kind))
                        .into_any_element()
                };

                div()
                    .id(ix)
                    .cursor_pointer()
                    .on_click(move |event, window, cx| {
                        if let Some(app) = entity_click.upgrade() {
                            app.update(cx, |this, cx| {
                                let was_selected =
                                    if let AppView::PermissionsWizardView { selected_index } =
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
                                    this.dispatch_permissions_wizard_action(
                                        crate::permissions_wizard::PermissionsWizardAction::GrantSelected,
                                        window,
                                        cx,
                                    );
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
                        ListItem::new(kind.name().to_string(), list_colors)
                            .icon_kind_opt(crate::list_item::IconKind::from_icon_hint(kind.icon()))
                            .description_opt(Some(kind.subtitle().to_string()))
                            .trailing_accessory(badge)
                            .selected(is_selected)
                            .hovered(is_hovered),
                    )
                    .into_any_element()
            })
            .collect();

        let menu_def = self.current_main_menu_theme.def();
        let frame =
            crate::components::main_view_chrome::main_view_content_frame(menu_def, design_spacing);
        let intro = div()
            .w_full()
            .child(crate::components::render_info_state_full_width_panel(
                crate::components::permission_onboarding_intro_spec(
                    granted_count,
                    row_count,
                    all_required_granted,
                ),
                &self.theme,
                frame.text_inset_x(),
                cx,
            ));

        let title = crate::components::main_view_chrome::render_main_view_text_plane(
            frame,
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(chrome.text_primary_hex))
                .child("Set Up Permissions")
                .into_any_element(),
        );
        let content = crate::components::main_view_chrome::render_main_view_content_frame(
            frame,
            [
                title,
                intro.into_any_element(),
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .min_h(px(0.))
                    .children(list_items)
                    .into_any_element(),
            ],
        );

        let footer_hints: Vec<gpui::SharedString> =
            crate::permissions_wizard::PermissionsWizardActions::ALL
                .iter()
                .map(|action| gpui::SharedString::from(format!("{} {}", action.key, action.label)))
                .collect();
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            footer_hints,
            None,
        ));

        // `PermissionsWizardView` is RootContextOnly: render_impl owns the
        // one MainViewShell/Header/Main stack. This renderer contributes only
        // the body and footer safe area, avoiding nested shared stable IDs.
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(rgb(chrome.text_primary_hex))
            .font_family(self.theme_font_family())
            .key_context("permissions_wizard")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .overflow_hidden()
                    .child(content),
            )
            .when_some(footer, |surface, footer| surface.child(footer))
            .into_any_element()
    }
}
