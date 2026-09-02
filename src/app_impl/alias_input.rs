use super::*;

impl ScriptListApp {
    pub(crate) fn show_alias_input(
        &mut self,
        command_id: String,
        command_name: String,
        cx: &mut Context<Self>,
    ) {
        logging::log(
            "ALIAS",
            &format!(
                "Showing alias input for '{}' (id: {})",
                command_name, command_id
            ),
        );

        // Load existing alias if any
        let existing_alias = if let Some(sources) = self.main_services.owned_sources() {
            sources
                .alias_overrides
                .get(&command_id)
                .cloned()
                .unwrap_or_default()
        } else {
            crate::aliases::load_alias_overrides()
                .ok()
                .and_then(|overrides| {
                    overrides.get(&command_id).cloned().or_else(|| {
                        self.get_selected_result()
                            .and_then(|selected| selected.command_preference_identity())
                            .filter(|identity| identity.exact_id == command_id)
                            .and_then(|identity| overrides.get(&identity.legacy_id).cloned())
                    })
                })
                .unwrap_or_default()
        };

        // Store state
        self.alias_input_state = Some(AliasInputState {
            command_id: command_id.clone(),
            command_name: command_name.clone(),
            alias_text: existing_alias.clone(),
        });

        // Close actions popup if open
        self.clear_actions_popup_state();
        let theme = self.theme.clone();
        let input_entity = cx.new(|cx| {
            crate::components::alias_input::AliasInput::new(cx, theme)
                .with_command_id(command_id)
                .with_command_name(command_name)
                .with_current_alias((!existing_alias.is_empty()).then_some(existing_alias))
        });
        let mut previous = input_entity.read(cx).semantic_token();
        self.alias_input_subscription = Some(cx.observe(&input_entity, move |this, input, cx| {
            if this
                .alias_input_entity
                .as_ref()
                .map(|entity| entity.entity_id())
                != Some(input.entity_id())
            {
                return;
            }
            let (token, action) = input.update(cx, |input, _| {
                (input.semantic_token(), input.take_pending_action())
            });
            if token != previous {
                previous = token;
                if let Some(state) = &mut this.alias_input_state {
                    state.alias_text = input.read(cx).text().to_owned();
                }
                this.mark_main_data_changed();
                cx.notify();
            }
            use crate::components::alias_input::AliasInputAction;
            match action {
                Some(AliasInputAction::Save(alias)) => this.save_alias_with_text(Some(alias), cx),
                Some(AliasInputAction::Clear) => this.save_alias_with_text(Some(String::new()), cx),
                Some(AliasInputAction::Cancel) => this.close_alias_input(cx),
                None => {}
            }
        }));
        self.alias_input_entity = Some(input_entity);
        self.mark_main_data_changed();
        self.mark_main_presentation_changed();

        cx.notify();
    }

    /// Close the alias input and clear state.
    /// Returns focus to the main filter input.
    pub fn close_alias_input(&mut self, cx: &mut Context<Self>) {
        if self.alias_input_state.is_some() || self.alias_input_entity.is_some() {
            logging::log(
                "ALIAS",
                "Closing alias input, returning focus to main filter",
            );
            self.alias_input_state = None;
            self.alias_input_entity = None; // Clear entity to reset for next open
            self.alias_input_subscription = None;
            self.mark_main_data_changed();
            self.mark_main_presentation_changed();
            // Return focus to the main filter input (like close_shortcut_recorder does)
            self.pending_focus = Some(FocusTarget::MainFilter);
            cx.notify();
        }
    }

    /// Update the alias text in the input state.
    /// Updates the same retained field used by keyboard editing.
    pub(crate) fn update_alias_text(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(ref mut state) = self.alias_input_state {
            state.alias_text = text;
            if let Some(input) = &self.alias_input_entity {
                input.update(cx, |input, cx| {
                    input.input.set_text(state.alias_text.clone());
                    cx.notify();
                });
            }
            cx.notify();
        }
    }

    /// Save the current alias and close the input.
    /// If alias_from_entity is provided, use that; otherwise fall back to state.alias_text.
    pub(crate) fn save_alias_with_text(
        &mut self,
        alias_from_entity: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.alias_input_state.as_ref() else {
            return;
        };
        let command_id = state.command_id.clone();
        let command_name = state.command_name.clone();
        let alias_text = alias_from_entity
            .unwrap_or_else(|| state.alias_text.clone())
            .trim()
            .to_string();
        if !alias_text
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            self.show_error_toast(
                "Alias must contain only letters, numbers, hyphens, or underscores",
                cx,
            );
            return;
        }
        if let Some(scope) = crate::runtime_policy::owned_evaluation() {
            if let Err(error) = scope.require_owned_path(&crate::aliases::default_aliases_path()) {
                self.show_error_toast(format!("Alias was not saved: {error}"), cx);
                return;
            }
        }
        let mut save_error = None;
        self.commit_main_menu_results_refresh("alias-override", None, cx, |app, _cx| {
            let result = if alias_text.is_empty() {
                crate::aliases::remove_alias_override(&command_id)
            } else {
                crate::aliases::save_alias_override(&command_id, &alias_text)
            };
            if let Err(error) = result {
                save_error = Some(error);
                return false;
            }
            if let MainServices::OwnedFixtures(sources) = &mut app.main_services {
                let overrides = &mut Arc::make_mut(sources).alias_overrides;
                if alias_text.is_empty() {
                    overrides.remove(&command_id);
                } else {
                    overrides.insert(command_id.clone(), alias_text.clone());
                }
            }
            app.rebuild_registries();
            app.mark_main_data_changed();
            true
        });
        if let Some(error) = save_error {
            self.show_error_toast(format!("Failed to save alias: {error}"), cx);
            return;
        }
        let message = if alias_text.is_empty() {
            "Alias removed".to_string()
        } else {
            format!("Alias set: {} → {}", alias_text, command_name)
        };
        if self.main_services.owned_sources().is_some() {
            self.toast_manager.push(
                components::toast::Toast::success(message, &self.theme)
                    .duration_ms(Some(TOAST_SUCCESS_MS)),
            );
            crate::runtime_policy::record_completed_fixture_effect();
        } else {
            self.show_hud(message, Some(HUD_MEDIUM_MS), cx);
        }
        self.close_alias_input(cx);
    }

    /// Render the alias input overlay if state is set.
    ///
    /// Returns None if no alias input is active.
    ///
    /// The alias input entity is created once and persisted to maintain keyboard focus.
    /// This follows the legacy in-window modal pattern; shortcut recording now
    /// uses a detached native popup so it can own raw key capture.
    pub(crate) fn render_alias_input_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.alias_input_state.as_ref()?;
        let input = self.alias_input_entity.clone()?;
        let theme = self.theme.clone();
        input.update(cx, |input, cx| {
            if !Arc::ptr_eq(&input.theme, &theme) {
                input.update_theme(theme);
                cx.notify();
            }
        });
        let focus = input.read(cx).focus_handle.clone();
        if !focus.is_focused(window) {
            window.focus(&focus, cx);
        }
        Some(input.into_any_element())
    }
}
