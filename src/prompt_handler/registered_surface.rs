// Shared production dispatch for exact automation lifetimes. Included at crate root.

pub(crate) use crate::windows::automation_transaction_provider::apply_registered_root_layer_selection;

#[expect(
    clippy::too_many_arguments,
    reason = "Keep exact window, revision, and dispatch authority explicit across the async batch boundary"
)]
fn select_main_window_semantic_id_for_batch(
    this: &gpui::WeakEntity<ScriptListApp>,
    main_window_handle: Option<gpui::AnyWindowHandle>,
    target: Option<&protocol::AutomationWindowInfo>,
    expected: Option<&protocol::AutomationTargetIdentitySnapshot>,
    guard: &crate::platform::gpui_event_simulator::DispatchTarget,
    semantic_id: &str,
    submit: bool,
    cx: &mut gpui::AsyncApp,
) -> anyhow::Result<String> {
    if semantic_id.starts_with("root-dialog:") || semantic_id.starts_with("root-notification:") {
        let fallback;
        let target = if let Some(target) = target {
            target
        } else {
            fallback = crate::windows::resolve_automation_window(Some(
                &protocol::AutomationWindowTarget::Main,
            ))?;
            &fallback
        };
        let exact_handle = target.generation.and_then(|generation| {
            crate::windows::get_runtime_window_handle_for_generation(&target.id, generation)
        });
        anyhow::ensure!(
            exact_handle.is_some()
                && exact_handle == main_window_handle.or_else(crate::get_main_window_handle),
            "main_root_layer_target_mismatch"
        );
        return cx
            .update(|cx| {
                validate_batch_app_effect(expected, guard, this, cx)?;
                apply_registered_root_layer_selection(target, semantic_id, submit, cx)
            })?
            .ok_or_else(|| protocol::TransactionError::element_not_found(semantic_id).into());
    }
    let semantic_id = semantic_id.to_string();
    if let Some(handle) = main_window_handle.or_else(crate::get_main_window_handle) {
        return handle.update(cx, |_root, window, cx| {
            validate_batch_window_effect(expected, guard, this, window, cx)?;
            this.update(cx, |app, cx| {
                app.select_choice_by_semantic_id_in_window(&semantic_id, submit, window, cx)
            })
        })??;
    }

    this.update(cx, |app, cx| {
        validate_batch_main_effect(app, expected, guard, cx)?;
        app.select_choice_by_semantic_id(&semantic_id, submit, cx)
    })?
}

fn registered_surface_target(
    resolved: &protocol::AutomationWindowInfo,
) -> anyhow::Result<protocol::AutomationWindowInfo> {
    let current = crate::windows::automation_surface_collector::current_surface_metadata(resolved)
        .ok_or_else(|| anyhow::anyhow!("registered_surface_target_stale"))?;
    anyhow::ensure!(
        current.kind != protocol::AutomationWindowKind::Main,
        "registered_surface_requires_secondary_owner"
    );
    let generation = current
        .generation
        .ok_or_else(|| anyhow::anyhow!("registered_surface_generation_missing"))?;
    // Registration verifies the actual native host. Its policy cannot change
    // during this exact lifetime, so reads need no mutable Window borrow.
    let policy = crate::windows::runtime_window_host_policy(&current.id, generation)?;
    policy.validate()?;
    anyhow::ensure!(
        !policy.is_hidden() || (!current.visible && !current.focused),
        "registered_surface_hidden_metadata_mismatch"
    );
    Ok(current)
}

fn registered_notes_owner(
    resolved: &protocol::AutomationWindowInfo,
    cx: &gpui::App,
) -> anyhow::Result<(
    gpui::Entity<crate::notes::NotesApp>,
    gpui::WindowHandle<crate::Root>,
)> {
    let generation = resolved
        .generation
        .ok_or_else(|| anyhow::anyhow!("notes_generation_missing"))?;
    let (entity, handle) =
        crate::notes::get_notes_app_entity_and_handle_for_generation(generation, cx)
            .ok_or_else(|| anyhow::anyhow!("notes_target_unavailable"))?;
    anyhow::ensure!(
        crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)
            == Some(handle.into()),
        "notes_target_owner_mismatch"
    );
    Ok((entity, handle))
}

pub(crate) fn registered_surface_ui_snapshot(
    resolved: &protocol::AutomationWindowInfo,
    cx: &gpui::App,
) -> anyhow::Result<protocol::UiStateSnapshot> {
    use crate::windows::automation_surface_collector as collector;
    use crate::windows::automation_transaction_provider as provider;
    let target = registered_surface_target(resolved)?;
    let generation = target
        .generation
        .ok_or_else(|| anyhow::anyhow!("registered_surface_generation_missing"))?;
    match target.kind {
        protocol::AutomationWindowKind::AgentChatDetached => {
            let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                &target.id, generation,
            )
            .ok_or_else(|| anyhow::anyhow!("chat_target_unavailable"))?;
            Ok(provider::detached_agent_chat_ui_snapshot(
                &target, &entity, cx,
            ))
        }
        protocol::AutomationWindowKind::ActionsDialog => {
            let entity = collector::exact_actions_dialog_entity(&target, cx)
                .ok_or_else(|| anyhow::anyhow!("actions_target_unavailable"))?;
            Ok(provider::actions_dialog_ui_snapshot(&target, &entity, cx))
        }
        _ => {
            let (elements, focused_semantic_id, selected_semantic_id) =
                if target.id == "shortcut-recorder-popup" {
                    let elements = crate::shortcut_recorder::shortcut_fixture_elements(
                        &target.id, generation, cx,
                    )?;
                    let focused = elements
                        .iter()
                        .find(|element| element.focused == Some(true))
                        .map(|element| element.semantic_id.clone());
                    let selected = elements
                        .iter()
                        .find(|element| element.selected == Some(true))
                        .map(|element| element.semantic_id.clone());
                    (elements, focused, selected)
                } else if target.semantic_surface.as_deref() == Some("footerOverlay") {
                    (
                        crate::footer_popup::footer_fixture_elements(&target.id, generation, cx)?,
                        None,
                        None,
                    )
                } else {
                    let surface = collector::collect_surface_snapshot(&target, usize::MAX, cx)
                        .ok_or_else(|| {
                            anyhow::anyhow!("registered_surface_state_unavailable:{}", target.id)
                        })?;
                    (
                        surface.elements,
                        surface.focused_semantic_id,
                        surface.selected_semantic_id,
                    )
                };
            let input_value = if target.kind == protocol::AutomationWindowKind::Notes {
                let (entity, _) = registered_notes_owner(&target, cx)?;
                Some(entity.read(cx).editor_state.read(cx).value().to_string())
            } else if target.kind == protocol::AutomationWindowKind::Dictation {
                Some(
                    crate::dictation::get_dictation_overlay_state_for_instance(generation, cx)
                        .ok_or_else(|| anyhow::anyhow!("dictation_target_unavailable"))?
                        .transcript
                        .to_string(),
                )
            } else if target.id
                == crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID
            {
                Some(crate::ai::agent_chat::ui::history_popup::get_history_popup_snapshot_for_generation(generation, cx)
                    .ok_or_else(|| anyhow::anyhow!("history_target_unavailable"))?.query.to_string())
            } else {
                elements
                    .iter()
                    .find(|element| element.element_type == protocol::ElementType::Input)
                    .and_then(|element| element.value.clone())
            };
            let selected_value = selected_semantic_id.as_ref().and_then(|id| {
                elements
                    .iter()
                    .find(|element| &element.semantic_id == id)
                    .and_then(|element| element.value.clone().or_else(|| element.text.clone()))
            });
            Ok(protocol::UiStateSnapshot {
                window_visible: target.visible,
                window_focused: target.focused,
                prompt_type: Some(target.kind.as_camel_case().to_string()),
                input_value,
                selected_value,
                choice_count: elements
                    .iter()
                    .filter(|element| element.element_type == protocol::ElementType::Choice)
                    .count(),
                visible_semantic_ids: elements
                    .into_iter()
                    .map(|element| element.semantic_id)
                    .collect(),
                focused_semantic_id,
                ..Default::default()
            })
        }
    }
}

pub(crate) fn registered_surface_wait_satisfied(
    resolved: &protocol::AutomationWindowInfo,
    condition: &protocol::WaitCondition,
    cx: &gpui::App,
) -> anyhow::Result<bool> {
    let target = registered_surface_target(resolved)?;
    if is_agent_chat_wait_condition(condition) {
        if target.kind != protocol::AutomationWindowKind::AgentChatDetached {
            return Ok(false);
        }
        let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
            &target.id,
            target
                .generation
                .ok_or_else(|| anyhow::anyhow!("chat_generation_missing"))?,
        )
        .ok_or_else(|| anyhow::anyhow!("chat_target_unavailable"))?;
        let view = entity.read(cx);
        let protocol::WaitCondition::Detailed(condition) = condition else {
            unreachable!()
        };
        return Ok(
            protocol::transaction_executor::matches_agent_chat_wait_condition(
                condition,
                &view.collect_agent_chat_state_snapshot(cx),
                || view.test_probe_snapshot(1, cx),
            )
            .unwrap_or(false),
        );
    }
    let snapshot = registered_surface_ui_snapshot(&target, cx)?;
    Ok(
        protocol::transaction_executor::matches_ui_wait_condition(&snapshot, condition)
            .unwrap_or(false),
    )
}

fn registered_surface_transaction_error(error: anyhow::Error) -> protocol::TransactionError {
    match error.downcast::<protocol::TransactionError>() {
        Ok(error) => error,
        Err(error) => protocol::TransactionError::action_failed(error.to_string()),
    }
}

pub(crate) fn apply_registered_surface_command(
    resolved: &protocol::AutomationWindowInfo,
    command: &protocol::BatchCommand,
    cx: &mut gpui::App,
) -> anyhow::Result<Option<String>> {
    use crate::protocol::transaction_executor::TransactionStateProvider;
    use crate::windows::automation_transaction_provider::{
        ActionsDialogTransactionProvider, DetachedAgentChatTransactionProvider,
    };
    let target = registered_surface_target(resolved)?;
    if let protocol::BatchCommand::SelectBySemanticId {
        semantic_id,
        submit,
    } = command
    {
        if let Some(selected) =
            apply_registered_root_layer_selection(&target, semantic_id, *submit, cx)?
        {
            return Ok(Some(selected));
        }
    }
    let generation = target
        .generation
        .ok_or_else(|| anyhow::anyhow!("registered_surface_generation_missing"))?;
    let kind = registered_batch_target_kind(&target);
    let selected = match target.kind {
        protocol::AutomationWindowKind::AgentChatDetached => {
            let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                &target.id, generation,
            )
            .ok_or_else(|| anyhow::anyhow!("chat_target_unavailable"))?;
            let mut provider = DetachedAgentChatTransactionProvider {
                cx,
                entity,
                target: target.clone(),
            };
            match command {
                protocol::BatchCommand::SetInput { text } => {
                    provider.set_input(text)?;
                    return Ok(None);
                }
                protocol::BatchCommand::SelectByValue { value, submit } => {
                    provider.select_by_value(value, *submit)?
                }
                protocol::BatchCommand::SelectBySemanticId {
                    semantic_id,
                    submit,
                } => provider.select_by_semantic_id(semantic_id, *submit)?,
                _ => return Err(unsupported_batch_command_error(kind, command).into()),
            }
        }
        protocol::AutomationWindowKind::ActionsDialog => {
            let entity = crate::windows::automation_surface_collector::exact_actions_dialog_entity(
                &target, cx,
            )
            .ok_or_else(|| anyhow::anyhow!("actions_target_unavailable"))?;
            let mut provider = ActionsDialogTransactionProvider {
                cx,
                entity,
                target: target.clone(),
            };
            match command {
                protocol::BatchCommand::SetInput { text } => {
                    provider.set_input(text)?;
                    return Ok(None);
                }
                protocol::BatchCommand::SelectByValue { value, submit } => {
                    provider.select_by_value(value, *submit)?
                }
                protocol::BatchCommand::SelectBySemanticId {
                    semantic_id,
                    submit,
                } => provider.select_by_semantic_id(semantic_id, *submit)?,
                _ => return Err(unsupported_batch_command_error(kind, command).into()),
            }
        }
        protocol::AutomationWindowKind::Notes => {
            let (entity, handle) = registered_notes_owner(&target, cx)?;
            match command {
                protocol::BatchCommand::SetInput { text } => {
                    crate::notes::update_notes_window_detached(handle, cx, |window, cx| {
                        entity.update(cx, |app, cx| {
                            app.set_editor_text_for_automation(text.clone(), window, cx)
                        });
                    })?
                }
                protocol::BatchCommand::OpenActions => {
                    crate::notes::update_notes_window_detached(handle, cx, |window, cx| {
                        entity.update(cx, |app, cx| app.open_actions_panel(window, cx));
                    })?
                }
                protocol::BatchCommand::TogglePreview => {
                    crate::notes::update_notes_window_detached(handle, cx, |window, cx| {
                        entity.update(cx, |app, cx| app.toggle_preview(window, cx));
                    })?
                }
                _ => return Err(unsupported_batch_command_error(kind, command).into()),
            }
            return Ok(None);
        }
        protocol::AutomationWindowKind::PromptPopup => {
            let subtype = resolve_prompt_popup_subtype(&target)?;
            revalidate_prompt_popup_target(&target, subtype)?;
            if subtype == PromptPopupSubtype::Confirm {
                anyhow::ensure!(
                    crate::confirm::get_confirm_popup_snapshot(cx, generation, None).is_some(),
                    "confirm_target_owner_mismatch"
                );
            }
            match command {
                protocol::BatchCommand::SetInput { text } if subtype == PromptPopupSubtype::AgentChatHistory => {
                    crate::ai::agent_chat::ui::history_popup::batch_set_history_popup_input(generation, text, cx)?;
                    return Ok(None);
                }
                protocol::BatchCommand::SelectByValue { value, submit } => match subtype {
                    PromptPopupSubtype::Confirm => crate::confirm::batch_select_confirm_button_by_value(generation, value, *submit, cx)?,
                    PromptPopupSubtype::DictationMicrophone => crate::dictation::batch_select_dictation_microphone_popup_row_by_value(generation, value, *submit, cx)?,
                    PromptPopupSubtype::AgentChatHistory => crate::ai::agent_chat::ui::history_popup::batch_select_history_popup_by_value(generation, value, *submit, cx)?,
                },
                protocol::BatchCommand::SelectBySemanticId { semantic_id, submit } => match subtype {
                    PromptPopupSubtype::Confirm => crate::confirm::batch_select_confirm_button_by_semantic_id(generation, semantic_id, *submit, cx)?,
                    PromptPopupSubtype::DictationMicrophone => crate::dictation::batch_select_dictation_microphone_popup_row_by_semantic_id(generation, semantic_id, *submit, cx)?,
                    PromptPopupSubtype::AgentChatHistory => crate::ai::agent_chat::ui::history_popup::batch_select_history_popup_by_semantic_id(generation, semantic_id, *submit, cx)?,
                },
                protocol::BatchCommand::SetThemeControl { control, value } => {
                    // A popup may operate only on its registered parent, never an
                    // arbitrary launcher (and never create or focus one).
                    let parent = target.parent_window_id.as_ref().zip(target.parent_window_generation)
                        .and_then(|(id, generation)| crate::windows::get_runtime_window_handle_for_generation(id, generation))
                        .ok_or_else(|| anyhow::anyhow!("setThemeControl requires ThemeChooserView"))?;
                    let root = parent.read(cx, |root: gpui::Entity<crate::Root>, _| root)?;
                    let app = root.read(cx).view().clone().downcast::<ScriptListApp>()
                        .map_err(|_| anyhow::anyhow!("setThemeControl requires ThemeChooserView"))?;
                    return app.update(cx, |app, cx| {
                        anyhow::ensure!(matches!(app.current_view, AppView::ThemeChooserView { .. }), "setThemeControl requires ThemeChooserView");
                        app.set_theme_chooser_control_from_devtools(control, value, cx).map(Some)
                    });
                }
                _ => return Err(unsupported_batch_command_error(kind, command).into()),
            }
        }
        _ => return Err(unsupported_batch_command_error(kind, command).into()),
    };
    if selected.is_some() {
        return Ok(selected);
    }
    let error = match command {
        protocol::BatchCommand::SelectByValue { value, .. }
            if kind == AutomationBatchTargetKind::AgentChatDetached =>
        {
            protocol::TransactionError::selection_not_found(format!(
                "selectByValue could not find '{value}' in detached Agent Chat picker"
            ))
        }
        protocol::BatchCommand::SelectBySemanticId { semantic_id, .. }
            if kind == AutomationBatchTargetKind::AgentChatDetached =>
        {
            protocol::TransactionError::selection_not_found(format!(
                "selectBySemanticId could not find '{semantic_id}' in detached Agent Chat picker"
            ))
        }
        protocol::BatchCommand::SelectByValue { value, .. } => {
            protocol::TransactionError::selection_not_found(value)
        }
        protocol::BatchCommand::SelectBySemanticId { semantic_id, .. } => {
            protocol::TransactionError::element_not_found(semantic_id)
        }
        _ => unreachable!(),
    };
    Err(error.into())
}

pub(crate) fn registered_surface_state_result(
    request_id: &str,
    resolved: &protocol::AutomationWindowInfo,
    cx: &gpui::App,
) -> anyhow::Result<protocol::Message> {
    let target = registered_surface_target(resolved)?;
    let snapshot = registered_surface_ui_snapshot(&target, cx)?;
    let notes_state = if target.kind == protocol::AutomationWindowKind::Notes {
        Some(
            registered_notes_owner(&target, cx)?
                .0
                .read(cx)
                .automation_state(cx),
        )
    } else {
        None
    };
    let actions_dialog = if target.kind == protocol::AutomationWindowKind::ActionsDialog {
        Some(
            crate::windows::automation_surface_collector::exact_actions_dialog_entity(&target, cx)
                .ok_or_else(|| anyhow::anyhow!("actions_target_unavailable"))?
                .read(cx)
                .automation_state("actionsDialog", cx),
        )
    } else {
        None
    };
    let dictation_state = if target.kind == protocol::AutomationWindowKind::Dictation {
        Some(crate::dictation::automation_state())
    } else {
        None
    };
    Ok(protocol::Message::StateResult {
        request_id: request_id.to_string(),
        prompt_type: snapshot
            .prompt_type
            .unwrap_or_else(|| target.kind.as_camel_case().to_string()),
        prompt_id: Some(format!("target:{:?}:{}", target.kind, target.id)),
        input_value: snapshot.input_value.unwrap_or_default(),
        choice_count: snapshot.choice_count,
        visible_choice_count: snapshot.choice_count,
        selected_index: -1,
        selected_value: snapshot.selected_value,
        is_focused: snapshot.window_focused,
        window_visible: snapshot.window_visible,
        notes_state,
        actions_dialog,
        dictation_state,
        surface_contract: None,
        active_popup_contract: None,
        active_footer: None,
        submit_diagnostics: None,
        placeholder: None,
        mini_ai: None,
        focused_text_agent_chat: None,
        filter_input_decorations: None,
        filter_input_diagnostics: None,
        menu_syntax_main_hint: None,
        capture_history_picker: None,
        main_window_preflight: None,
        root_file_search: None,
        main_list_scroll: None,
        active_list_scroll: None,
        screenshot_identity: None,
        drop_state: None,
        path_state: None,
        day_page_state: None,
        ghost_prediction: None,
        flow_ux: None,
        backgrounded_sessions: None,
    })
}

impl ScriptListApp {
    fn select_choice_by_semantic_id_in_window(
        &mut self,
        semantic_id: &str,
        submit: bool,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        if matches!(self.current_view, AppView::ScriptList)
            && self.spine_projection_owns_main_list()
        {
            let selected = self.select_main_menu_choice_by_semantic_id(semantic_id, false, cx)?;
            if submit {
                anyhow::ensure!(
                    self.accept_spine_projection_row(window, cx),
                    "spine_selection_not_accepted"
                );
            }
            return Ok(selected);
        }

        if let AppView::DayPage { entity } = &self.current_view {
            let entity = entity.clone();
            if semantic_id == script_kit_gpui::day_page::FRAGMENT_BACK_ID {
                return entity.update(cx, |view, cx| {
                    if !view.session.is_viewing_fragment() {
                        anyhow::bail!("Day Page fragment back is not visible");
                    }
                    if submit {
                        view.return_to_day_page(window, cx);
                    }
                    Ok(semantic_id.to_string())
                });
            }
        }

        self.select_choice_by_semantic_id(semantic_id, submit, cx)
    }
}
