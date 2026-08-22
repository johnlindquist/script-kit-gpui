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
        let existing_alias = crate::aliases::load_alias_overrides()
            .ok()
            .and_then(|overrides| {
                overrides.get(&command_id).cloned().or_else(|| {
                    self.get_selected_result()
                        .and_then(|selected| selected.command_preference_identity())
                        .filter(|identity| identity.exact_id == command_id)
                        .and_then(|identity| overrides.get(&identity.legacy_id).cloned())
                })
            })
            .unwrap_or_default();

        // Store state
        self.alias_input_state = Some(AliasInputState {
            command_id,
            command_name,
            alias_text: existing_alias,
        });

        // Close actions popup if open
        self.clear_actions_popup_state();

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
                                            // Return focus to the main filter input (like close_shortcut_recorder does)
            self.pending_focus = Some(FocusTarget::MainFilter);
            cx.notify();
        }
    }

    /// Update the alias text in the input state.
    /// Currently unused - will be connected when real text input is added.
    #[allow(dead_code)]
    pub(crate) fn update_alias_text(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(ref mut state) = self.alias_input_state {
            state.alias_text = text;
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
        let Some(ref state) = self.alias_input_state else {
            logging::log("ALIAS", "No alias input state when trying to save");
            return;
        };

        let command_id = state.command_id.clone();
        let command_name = state.command_name.clone();
        // Prefer alias from entity if provided, else use state
        let alias_text = alias_from_entity
            .unwrap_or_else(|| state.alias_text.clone())
            .trim()
            .to_string();

        if alias_text.is_empty() {
            // Empty alias means remove it
            match crate::aliases::remove_alias_override(&command_id) {
                Ok(()) => {
                    logging::log("ALIAS", &format!("Removed alias for: {}", command_id));
                    self.show_hud("Alias removed".to_string(), Some(HUD_MEDIUM_MS), cx);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to remove alias");
                    self.show_error_toast(format!("Failed to remove alias: {}", e), cx);
                }
            }
        } else {
            // Validate alias: should be alphanumeric with optional hyphens/underscores
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

            logging::log(
                "ALIAS",
                &format!(
                    "Saving alias for '{}' ({}): {}",
                    command_name, command_id, alias_text
                ),
            );

            match crate::aliases::save_alias_override(&command_id, &alias_text) {
                Ok(()) => {
                    logging::log("ALIAS", "Alias saved to aliases.json");
                    self.show_hud(
                        format!("Alias set: {} → {}", alias_text, command_name),
                        Some(HUD_MEDIUM_MS),
                        cx,
                    );
                    // Refresh scripts to update the alias registry
                    self.refresh_scripts(cx);
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to save alias");
                    self.show_error_toast(format!("Failed to save alias: {}", e), cx);
                }
            }
        }

        // Close the input and restore focus
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
        use crate::components::alias_input::{AliasInput, AliasInputAction};

        // Check if we have state but no entity yet - need to create the input
        let state = self.alias_input_state.as_ref()?;

        // Create entity if needed (only once per show)
        if self.alias_input_entity.is_none() {
            let command_id = state.command_id.clone();
            let command_name = state.command_name.clone();
            let current_alias = if state.alias_text.is_empty() {
                None
            } else {
                Some(state.alias_text.clone())
            };
            let theme = std::sync::Arc::clone(&self.theme);

            let input_entity = cx.new(move |cx| {
                // Create the alias input with its own focus handle from its own context
                // This is CRITICAL for keyboard events to work
                AliasInput::new(cx, theme)
                    .with_command_name(command_name)
                    .with_command_id(command_id)
                    .with_current_alias(current_alias)
            });

            self.alias_input_entity = Some(input_entity);
            logging::log("ALIAS", "Created new alias input entity");
        }

        // Get the existing entity - clone it early to avoid borrow conflicts
        let input_entity = self.alias_input_entity.clone()?;

        // ALWAYS focus the input entity to ensure it captures keyboard input
        // This is critical for modal behavior - the input must have focus
        let input_fh = input_entity.read(cx).focus_handle.clone();
        let was_focused = input_fh.is_focused(window);
        window.focus(&input_fh, cx);
        if !was_focused {
            logging::log("ALIAS", "Focused alias input (was not focused)");
        }

        // Check for pending actions from the input entity (Save, Cancel, or Clear)
        // We need to update() the entity to take the pending action
        let pending_action = input_entity.update(cx, |input, _cx| input.take_pending_action());

        if let Some(action) = pending_action {
            match action {
                AliasInputAction::Save(alias) => {
                    logging::log("ALIAS", &format!("Handling save action: {}", alias));
                    // Handle the save - need to defer to avoid borrow issues
                    let app_entity = cx.entity().downgrade();
                    cx.spawn(async move |_this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(1))
                            .await;
                        cx.update(|cx| {
                            if let Some(app) = app_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.save_alias_with_text(Some(alias), cx);
                                });
                            }
                        });
                    })
                    .detach();
                }
                AliasInputAction::Cancel => {
                    logging::log("ALIAS", "Handling cancel action");
                    self.close_alias_input(cx);
                }
                AliasInputAction::Clear => {
                    logging::log("ALIAS", "Handling clear action (remove alias)");
                    // Clear means remove the alias - save with empty string
                    let app_entity = cx.entity().downgrade();
                    cx.spawn(async move |_this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(1))
                            .await;
                        cx.update(|cx| {
                            if let Some(app) = app_entity.upgrade() {
                                app.update(cx, |this, cx| {
                                    this.save_alias_with_text(Some(String::new()), cx);
                                });
                            }
                        });
                    })
                    .detach();
                }
            }
        }

        // Return the entity's view as an element
        Some(input_entity.into_any_element())
    }
}
