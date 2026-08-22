// Shortcut and alias action handlers for handle_action dispatch.
//
// Contains: configure_shortcut, add_shortcut, update_shortcut, remove_shortcut,
// add_alias, update_alias, remove_alias.
//
// All branches retain the legacy `SearchResult::launcher_command_id()`
// compatibility contract, then bind new preferences to the exact source
// identity so same-name commands cannot overwrite one another.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutRecorderAction {
    Configure,
    Add,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasInputAction {
    Add,
    Update,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutAliasRemoveAction {
    Shortcut,
    Alias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShortcutAliasTargetError {
    NoSelection,
    UnsupportedItemType,
    MissingCommandId,
}

impl ShortcutRecorderAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "configure_shortcut" => Some(Self::Configure),
            "add_shortcut" => Some(Self::Add),
            "update_shortcut" => Some(Self::Update),
            _ => None,
        }
    }

    fn unsupported_message(self) -> &'static str {
        match self {
            Self::Configure | Self::Add | Self::Update => {
                "Shortcuts not supported for this item type"
            }
        }
    }

    fn cannot_assign_message(self) -> &'static str {
        match self {
            Self::Configure | Self::Add | Self::Update => {
                "Cannot assign shortcut for this item type"
            }
        }
    }

    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
        match error {
            ShortcutAliasTargetError::NoSelection => {
                selection_required_message_for_action(action_id).to_string()
            }
            ShortcutAliasTargetError::UnsupportedItemType => self.unsupported_message().to_string(),
            ShortcutAliasTargetError::MissingCommandId => self.cannot_assign_message().to_string(),
        }
    }
}

impl AliasInputAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "add_alias" => Some(Self::Add),
            "update_alias" => Some(Self::Update),
            _ => None,
        }
    }

    fn unsupported_message(self) -> &'static str {
        match self {
            Self::Add | Self::Update => "Aliases not supported for this item type",
        }
    }

    fn cannot_assign_message(self) -> &'static str {
        match self {
            Self::Add | Self::Update => "Cannot assign alias for this item type",
        }
    }

    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
        match error {
            ShortcutAliasTargetError::NoSelection => {
                selection_required_message_for_action(action_id).to_string()
            }
            ShortcutAliasTargetError::UnsupportedItemType => self.unsupported_message().to_string(),
            ShortcutAliasTargetError::MissingCommandId => self.cannot_assign_message().to_string(),
        }
    }
}

impl ShortcutAliasRemoveAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "remove_shortcut" => Some(Self::Shortcut),
            "remove_alias" => Some(Self::Alias),
            _ => None,
        }
    }

    fn success_hud(self) -> &'static str {
        match self {
            Self::Shortcut => "Shortcut removed",
            Self::Alias => "Alias removed",
        }
    }

    fn cannot_remove_message(self) -> &'static str {
        match self {
            Self::Shortcut => "Cannot remove shortcut for this item type",
            Self::Alias => "Cannot remove alias for this item type",
        }
    }

    fn target_error_message(self, error: ShortcutAliasTargetError, action_id: &str) -> String {
        match error {
            ShortcutAliasTargetError::NoSelection => {
                selection_required_message_for_action(action_id).to_string()
            }
            ShortcutAliasTargetError::UnsupportedItemType
            | ShortcutAliasTargetError::MissingCommandId => {
                self.cannot_remove_message().to_string()
            }
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::Shortcut => format!("Failed to remove shortcut: {error}"),
            Self::Alias => format!("Failed to remove alias: {error}"),
        }
    }
}

impl ScriptListApp {
    /// Handle shortcut and alias actions. Returns `DispatchOutcome` indicating if handled.
    fn handle_shortcut_alias_action(
        &mut self,
        action_id: &str,
        dctx: &DispatchContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> DispatchOutcome {
        let _ = dctx; // Reserved for future async path logging
        match action_id {
            // Handle both legacy "configure_shortcut" and new dynamic actions
            // "add_shortcut" and "update_shortcut" open the shortcut recorder
            "configure_shortcut" | "add_shortcut" | "update_shortcut" => {
                let Some(shortcut_action) = ShortcutRecorderAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", action = action_id, "action triggered");
                if let Some(result) = self.get_selected_result() {
                    // Skills and windows are non-bindable
                    match result {
                        scripts::SearchResult::Window(_)
                        | scripts::SearchResult::Skill(_)
                        | scripts::SearchResult::Note(_)
                        | scripts::SearchResult::Todo(_)
                        | scripts::SearchResult::BrowserTab(_)
                        | scripts::SearchResult::BrowserHistory(_) => {
                            return DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                shortcut_action.target_error_message(
                                    ShortcutAliasTargetError::UnsupportedItemType,
                                    action_id,
                                ),
                            );
                        }
                        scripts::SearchResult::Agent(_) => {
                            return DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                shortcut_action.target_error_message(
                                    ShortcutAliasTargetError::UnsupportedItemType,
                                    action_id,
                                ),
                            );
                        }
                        _ => {}
                    }

                    let Some(command_id) = result.launcher_command_id() else {
                        self.hide_main_and_reset(cx);
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            shortcut_action.target_error_message(
                                ShortcutAliasTargetError::MissingCommandId,
                                action_id,
                            ),
                        );
                    };
                    let command_id = result
                        .command_preference_identity()
                        .map_or(command_id, |identity| identity.exact_id);
                    let command_name = result.launcher_command_name();

                    tracing::info!(
                        command_id = %command_id,
                        command_name = %command_name,
                        item_type = result.type_label(),
                        "launcher_shortcut_recorder_requested"
                    );
                    self.show_shortcut_recorder(command_id, command_name, window, cx);
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        shortcut_action
                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
                    );
                }
                DispatchOutcome::success()
            }
            // "remove_shortcut" removes the existing shortcut from the registry
            "remove_shortcut" => {
                let Some(remove_action) = ShortcutAliasRemoveAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "remove shortcut action");
                if let Some(result) = self.get_selected_result() {
                    let command_id_opt = result.launcher_command_id().map(|legacy_id| {
                        result
                            .command_preference_identity()
                            .map_or(legacy_id, |identity| {
                                identity.existing_id(|id| {
                                    self.config.get_command_shortcut(id).is_some()
                                })
                            })
                    });

                    if let Some(command_id) = command_id_opt {
                        tracing::info!(
                            command_id = %command_id,
                            item_type = result.type_label(),
                            "launcher_shortcut_remove_requested"
                        );
                        // Remove only the config.ts command shortcut, preserving other command fields.
                        match self.remove_config_command_shortcut(&command_id) {
                            Ok(()) => {
                                match crate::hotkeys::unregister_dynamic_shortcut(&command_id) {
                                    Ok(()) => {
                                        tracing::info!(
                                            category = "SHORTCUT",
                                            command_id = %command_id,
                                            "Unregistered live shortcut after config removal"
                                        );
                                    }
                                    Err(error) => {
                                        tracing::warn!(
                                            category = "SHORTCUT",
                                            command_id = %command_id,
                                            error = %error,
                                            "Config shortcut removed, but live unregister failed"
                                        );
                                    }
                                }
                                tracing::info!(
                                    category = "SHORTCUT",
                                    command_id = %command_id,
                                    "Removed config shortcut"
                                );
                                self.show_hud(
                                    remove_action.success_hud().to_string(),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                                // Refresh scripts to update shortcut display
                                self.refresh_scripts(cx);
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "failed to remove shortcut");
                                self.hide_main_and_reset(cx);
                                return DispatchOutcome::error(
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    remove_action.failure_message(e),
                                );
                            }
                        }
                    } else {
                        self.hide_main_and_reset(cx);
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            remove_action.target_error_message(
                                ShortcutAliasTargetError::MissingCommandId,
                                action_id,
                            ),
                        );
                    }
                    self.hide_main_and_reset(cx);
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        remove_action
                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
                    );
                }
                DispatchOutcome::success()
            }
            // Alias actions: add_alias, update_alias open the alias input
            "add_alias" | "update_alias" => {
                let Some(alias_action) = AliasInputAction::from_action_id(action_id) else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", action = action_id, "action triggered");
                if let Some(result) = self.get_selected_result() {
                    // Skills, windows, and legacy agents are non-bindable
                    match result {
                        scripts::SearchResult::Window(_)
                        | scripts::SearchResult::Skill(_)
                        | scripts::SearchResult::Note(_)
                        | scripts::SearchResult::Todo(_)
                        | scripts::SearchResult::BrowserTab(_)
                        | scripts::SearchResult::BrowserHistory(_)
                        | scripts::SearchResult::Agent(_) => {
                            return DispatchOutcome::error(
                                crate::action_helpers::ERROR_ACTION_FAILED,
                                alias_action.target_error_message(
                                    ShortcutAliasTargetError::UnsupportedItemType,
                                    action_id,
                                ),
                            );
                        }
                        _ => {}
                    }

                    let Some(command_id) = result.launcher_command_id() else {
                        self.hide_main_and_reset(cx);
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            alias_action.target_error_message(
                                ShortcutAliasTargetError::MissingCommandId,
                                action_id,
                            ),
                        );
                    };
                    let command_id = result
                        .command_preference_identity()
                        .map_or(command_id, |identity| identity.exact_id);
                    let command_name = result.launcher_command_name();

                    tracing::info!(
                        command_id = %command_id,
                        command_name = %command_name,
                        item_type = result.type_label(),
                        "launcher_alias_input_requested"
                    );
                    self.show_alias_input(command_id, command_name, cx);
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        alias_action
                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
                    );
                }
                DispatchOutcome::success()
            }
            // "remove_alias" removes the existing alias from persistence
            "remove_alias" => {
                let Some(remove_action) = ShortcutAliasRemoveAction::from_action_id(action_id)
                else {
                    return DispatchOutcome::not_handled();
                };
                tracing::info!(category = "UI", "remove alias action");
                if let Some(result) = self.get_selected_result() {
                    let overrides = crate::aliases::get_cached_alias_overrides();
                    let command_id_opt = result.launcher_command_id().map(|legacy_id| {
                        result
                            .command_preference_identity()
                            .map_or(legacy_id, |identity| {
                                identity.existing_id(|id| overrides.contains_key(id))
                            })
                    });

                    if let Some(command_id) = command_id_opt {
                        tracing::info!(
                            command_id = %command_id,
                            item_type = result.type_label(),
                            "launcher_alias_remove_requested"
                        );
                        // Remove the alias override from persistence
                        match crate::aliases::remove_alias_override(&command_id) {
                            Ok(()) => {
                                tracing::info!(
                                    category = "ALIAS",
                                    command_id = %command_id,
                                    "Removed alias override"
                                );
                                self.show_hud(
                                    remove_action.success_hud().to_string(),
                                    Some(HUD_MEDIUM_MS),
                                    cx,
                                );
                                // Refresh scripts to update alias display and registry
                                self.refresh_scripts(cx);
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "failed to remove alias");
                                self.hide_main_and_reset(cx);
                                return DispatchOutcome::error(
                                    crate::action_helpers::ERROR_ACTION_FAILED,
                                    remove_action.failure_message(e),
                                );
                            }
                        }
                    } else {
                        self.hide_main_and_reset(cx);
                        return DispatchOutcome::error(
                            crate::action_helpers::ERROR_ACTION_FAILED,
                            remove_action.target_error_message(
                                ShortcutAliasTargetError::MissingCommandId,
                                action_id,
                            ),
                        );
                    }
                    self.hide_main_and_reset(cx);
                } else {
                    return DispatchOutcome::error(
                        crate::action_helpers::ERROR_ACTION_FAILED,
                        remove_action
                            .target_error_message(ShortcutAliasTargetError::NoSelection, action_id),
                    );
                }
                DispatchOutcome::success()
            }
            _ => DispatchOutcome::not_handled(),
        }
    }
}
