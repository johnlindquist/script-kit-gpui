/// Register a callback to be invoked when the user confirms stop via
/// Enter or the Stop button in the overlay.
pub fn set_overlay_abort_callback(callback: impl Fn(&mut App) + Send + Sync + 'static) {
    *OVERLAY_ABORT_CALLBACK.lock() = Some(Box::new(callback));
}

/// Register a callback to be invoked when the user clicks Stop on the
/// recording overlay.
pub fn set_overlay_submit_callback(callback: impl Fn(&mut App) + Send + Sync + 'static) {
    *OVERLAY_SUBMIT_CALLBACK.lock() = Some(Box::new(callback));
}

pub fn set_overlay_retarget_callback(
    callback: impl Fn(
            crate::dictation::DictationTarget,
            &mut App,
        ) -> Result<crate::dictation::DictationTargetSelection, String>
        + Send
        + Sync
        + 'static,
) {
    *OVERLAY_RETARGET_CALLBACK.lock() = Some(Box::new(callback));
}

pub fn set_overlay_recovery_callback(
    callback: impl Fn(crate::dictation::DictationRecoveryAction, &mut App) -> Result<(), String>
        + Send
        + Sync
        + 'static,
) {
    *OVERLAY_RECOVERY_CALLBACK.lock() = Some(Arc::new(callback));
}

#[allow(
    dead_code,
    reason = "the separately compiled application binary checks recovery readiness before opening dictation"
)]
pub(crate) fn overlay_recovery_callback_installed() -> bool {
    OVERLAY_RECOVERY_CALLBACK.lock().is_some()
}

fn dictation_target_action_icon(
    descriptor: &crate::dictation::DictationTargetDescriptor,
) -> crate::designs::icon_variations::IconName {
    use crate::designs::icon_variations::IconName;

    // ActionsDialog uses the repository's compact icon enum while the overlay
    // renders Lucide names. Keep this adapter driven by the descriptor's icon
    // token so the two hosts cannot choose target icons independently.
    match descriptor.icon {
        "search" => IconName::MagnifyingGlass,
        "text-cursor-input" => IconName::Pencil,
        "notebook-tabs" => IconName::File,
        "bot" => IconName::MessageCircle,
        "clipboard-paste" => IconName::Copy,
        "calendar-days" => IconName::File,
        "sparkles" => IconName::BoltFilled,
        _ => IconName::MessageCircle,
    }
}

pub(crate) fn dictation_target_actions() -> Vec<crate::actions::Action> {
    crate::dictation::DictationTarget::action_descriptors()
        .map(|descriptor| {
            crate::actions::Action::new(
                format!("{DICTATION_TARGET_ACTION_PREFIX}{}", descriptor.stable_id),
                descriptor.selector_label,
                Some(format!(
                    "{} — {}",
                    descriptor.delivery_verb, descriptor.description
                )),
                crate::actions::ActionCategory::ScriptContext,
            )
            .with_icon(dictation_target_action_icon(descriptor))
            .with_section("Destination")
        })
        .collect()
}

pub(crate) fn dictation_target_from_action_id(
    action_id: &str,
) -> Option<crate::dictation::DictationTarget> {
    let stable_id = action_id.strip_prefix(DICTATION_TARGET_ACTION_PREFIX)?;
    crate::dictation::DictationTarget::action_descriptors()
        .find(|descriptor| descriptor.stable_id == stable_id)
        .map(|descriptor| descriptor.target)
}

fn dictation_recovery_actions(
    capabilities: crate::dictation::DictationFailureRecoveryCapabilities,
) -> Vec<crate::actions::Action> {
    use crate::actions::{Action, ActionCategory};
    use crate::designs::icon_variations::IconName;

    let mut actions = Vec::new();
    if capabilities.choose_destination {
        actions.push(
            Action::new(
                DICTATION_RECOVERY_CHOOSE_ACTION_ID,
                "Choose Destination",
                Some("Pick a new frozen destination for the saved transcript".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::MagnifyingGlass)
            .with_section("Recovery"),
        );
    }
    if capabilities.copy_transcript {
        actions.push(
            Action::new(
                DICTATION_RECOVERY_COPY_ACTION_ID,
                "Copy Transcript",
                Some("Copy the saved transcript without retrying delivery".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::Copy)
            .with_section("Recovery"),
        );
    }
    if capabilities.open_dictation_history {
        actions.push(
            Action::new(
                DICTATION_RECOVERY_HISTORY_ACTION_ID,
                "Open Dictation History",
                Some("Open the saved History entry".to_string()),
                ActionCategory::ScriptContext,
            )
            .with_icon(IconName::File)
            .with_section("Recovery"),
        );
    }
    actions
}

impl DictationOverlay {
    fn open_destination_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(
            self.state.phase,
            DictationSessionPhase::Recording | DictationSessionPhase::Confirming
        ) {
            return;
        }

        let _ = self.dismiss_microphone_popup_top_layer("open_destination_actions", cx);
        self.destination_command_bar
            .set_actions(dictation_target_actions(), cx);
        self.destination_command_bar.open_centered(window, cx);
        self.wire_destination_actions_activation(window, cx);
    }

    fn open_recovery_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let DictationSessionPhase::Failed(failure) = &self.state.phase else {
            return;
        };
        let actions = dictation_recovery_actions(failure.capabilities);
        if actions.is_empty() {
            return;
        }
        self.destination_command_bar.set_actions(actions, cx);
        self.destination_command_bar.open_centered(window, cx);
        self.wire_recovery_actions_activation(window, cx);
    }

    fn invoke_recovery_action(
        &mut self,
        action: crate::dictation::DictationRecoveryAction,
        cx: &mut Context<Self>,
    ) {
        let Some(callback) = OVERLAY_RECOVERY_CALLBACK.lock().clone() else {
            tracing::warn!(
                category = "DICTATION",
                action = ?action,
                "Dictation recovery callback is unavailable"
            );
            return;
        };
        // Recovery can close or replace the overlay. Defer out of the overlay
        // entity update so those window operations never re-enter this borrow.
        cx.defer(move |cx| {
            let result = callback(action, cx);
            if let Err(error) = result {
                tracing::warn!(
                    category = "DICTATION",
                    action = ?action,
                    error_fingerprint = %crate::dictation::redacted_transcript_fingerprint(&error),
                    "Dictation recovery action failed"
                );
            }
        });
    }

    fn wire_recovery_actions_activation(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.destination_command_bar.dialog().cloned() else {
            return;
        };
        let overlay = cx.entity().downgrade();
        let activation_window = window.window_handle();
        let close_window = activation_window;
        let close_overlay = overlay.clone();
        dialog.update(cx, |dialog, _cx| {
            dialog.set_on_close(Arc::new(move |cx| {
                let overlay = close_overlay.clone();
                cx.defer(move |cx| {
                    let _ = close_window.update(cx, |_root, window, cx| {
                        let Some(overlay) = overlay.upgrade() else {
                            return;
                        };
                        overlay.update(cx, |overlay, cx| {
                            overlay.destination_command_bar.mark_closed_externally();
                            overlay.focus_handle.focus(window, cx);
                        });
                    });
                });
            }));
            dialog.set_on_activation(Arc::new(move |activation, _popup_window, cx| {
                let crate::actions::ActionsDialogActivation::Executed { action_id, .. } =
                    activation
                else {
                    return;
                };
                let overlay = overlay.clone();
                cx.defer(move |cx| {
                    let _ = activation_window.update(cx, |_root, window, cx| {
                        let Some(overlay) = overlay.upgrade() else {
                            return;
                        };
                        overlay.update(cx, |overlay, cx| {
                            overlay.destination_command_bar.mark_closed_externally();
                            match action_id.as_str() {
                                DICTATION_RECOVERY_CHOOSE_ACTION_ID => {
                                    overlay
                                        .destination_command_bar
                                        .set_actions(dictation_target_actions(), cx);
                                    overlay.destination_command_bar.open_centered(window, cx);
                                    overlay.wire_recovery_destination_activation(window, cx);
                                }
                                DICTATION_RECOVERY_COPY_ACTION_ID => {
                                    overlay.destination_command_bar.close(cx);
                                    overlay.invoke_recovery_action(
                                        crate::dictation::DictationRecoveryAction::CopyTranscript,
                                        cx,
                                    );
                                }
                                DICTATION_RECOVERY_HISTORY_ACTION_ID => {
                                    overlay.destination_command_bar.close(cx);
                                    overlay.invoke_recovery_action(
                                        crate::dictation::DictationRecoveryAction::OpenDictationHistory,
                                        cx,
                                    );
                                }
                                _ => {}
                            }
                            overlay.focus_handle.focus(window, cx);
                        });
                    });
                });
            }));
        });
    }

    fn wire_recovery_destination_activation(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.destination_command_bar.dialog().cloned() else {
            return;
        };
        let overlay = cx.entity().downgrade();
        let activation_window = window.window_handle();
        dialog.update(cx, |dialog, _cx| {
            dialog.set_on_activation(Arc::new(move |activation, _popup_window, cx| {
                let crate::actions::ActionsDialogActivation::Executed { action_id, .. } = activation
                else {
                    return;
                };
                let Some(target) = dictation_target_from_action_id(&action_id) else {
                    return;
                };
                let overlay = overlay.clone();
                cx.defer(move |cx| {
                    let _ = activation_window.update(cx, |_root, window, cx| {
                        let Some(overlay) = overlay.upgrade() else {
                            return;
                        };
                        overlay.update(cx, |overlay, cx| {
                            overlay.destination_command_bar.close(cx);
                            let selection = OVERLAY_RETARGET_CALLBACK
                                .lock()
                                .as_ref()
                                .ok_or_else(|| {
                                    "Dictation destination selector is unavailable".to_string()
                                })
                                .and_then(|callback| callback(target, cx));
                            match selection {
                                Ok(selection) => {
                                    crate::dictation::retain_frozen_selection_for_delivery(selection);
                                    overlay.invoke_recovery_action(
                                        crate::dictation::DictationRecoveryAction::ChooseDestination,
                                        cx,
                                    );
                                }
                                Err(error) => {
                                    tracing::warn!(
                                        category = "DICTATION",
                                        ?target,
                                        error_fingerprint = %crate::dictation::redacted_transcript_fingerprint(&error),
                                        "Dictation recovery destination refused"
                                    );
                                }
                            }
                            overlay.focus_handle.focus(window, cx);
                        });
                    });
                });
            }));
        });
    }

    fn wire_destination_actions_activation(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(dialog) = self.destination_command_bar.dialog().cloned() else {
            return;
        };
        let overlay = cx.entity().downgrade();
        let activation_window = window.window_handle();
        let close_window = activation_window;
        let close_overlay = overlay.clone();
        dialog.update(cx, |dialog, _cx| {
            dialog.set_on_close(Arc::new(move |cx| {
                let overlay = close_overlay.clone();
                cx.defer(move |cx| {
                    let _ = close_window.update(cx, |_root, window, cx| {
                        let Some(overlay) = overlay.upgrade() else {
                            return;
                        };
                        overlay.update(cx, |overlay, cx| {
                            overlay.destination_command_bar.mark_closed_externally();
                            overlay.focus_handle.focus(window, cx);
                        });
                    });
                });
            }));
            dialog.set_on_activation(Arc::new(move |activation, _popup_window, cx| {
                let crate::actions::ActionsDialogActivation::Executed { action_id, .. } =
                    activation
                else {
                    return;
                };
                let overlay = overlay.clone();
                cx.defer(move |cx| {
                    let _ = activation_window.update(cx, |_root, window, cx| {
                        let Some(overlay) = overlay.upgrade() else {
                            return;
                        };
                        overlay.update(cx, |overlay, cx| {
                            overlay.destination_command_bar.mark_closed_externally();
                            if let Some(target) = dictation_target_from_action_id(&action_id) {
                                overlay.select_destination(target, cx);
                            }
                            overlay.focus_handle.focus(window, cx);
                        });
                    });
                });
            }));
        });
    }
}
