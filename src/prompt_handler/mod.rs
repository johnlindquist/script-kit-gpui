// Prompt message handling methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs

// --- merged from part_000.rs ---
fn unhandled_message_warning(message_type: &str) -> String {
    format!(
        "'{}' is not supported yet. Update the script to a supported message type or update Script Kit GPUI.",
        message_type
    )
}

fn prompt_coming_soon_warning(prompt_name: &str) -> String {
    format!("{prompt_name} prompt coming soon.")
}

fn should_restore_main_window_after_script_exit(
    script_hid_window: bool,
    keep_tab_ai_save_offer_open: bool,
) -> bool {
    script_hid_window && keep_tab_ai_save_offer_open
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScriptErrorAcpContextBundle {
    script_snapshot_path: String,
    script_snapshot_label: String,
    error_report_path: String,
    error_report_label: String,
}

fn sanitize_script_error_context_name(value: &str, fallback: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

fn build_script_error_acp_prompt(
    script_path: &str,
    error_message: &str,
    exit_code: Option<i32>,
    suggestions: &[String],
) -> String {
    let script_name = std::path::Path::new(script_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("script");

    let mut prompt = format!(
        "The script `{script_name}` just failed when I ran it. Use the attached script snapshot and error report as context, diagnose the root cause, fix it, and verify the fix by rerunning the script or giving the exact verification result.\n\nError summary: {error_message}"
    );

    if let Some(code) = exit_code {
        prompt.push_str(&format!("\nExit code: {code}"));
    }

    if !suggestions.is_empty() {
        prompt.push_str("\nSuggested clues:");
        for suggestion in suggestions {
            prompt.push_str(&format!("\n- {suggestion}"));
        }
    }

    prompt
}

fn build_script_error_report_markdown(
    script_path: &str,
    error_message: &str,
    stderr_output: Option<&str>,
    exit_code: Option<i32>,
    stack_trace: Option<&str>,
    suggestions: &[String],
) -> String {
    let mut report = format!(
        "# Script Failure Report\n\n## Script Path\n`{script_path}`\n\n## Error Summary\n{error_message}\n"
    );

    if let Some(code) = exit_code {
        report.push_str(&format!("\n## Exit Code\n`{code}`\n"));
    }

    if !suggestions.is_empty() {
        report.push_str("\n## Suggestions\n");
        for suggestion in suggestions {
            report.push_str(&format!("- {suggestion}\n"));
        }
    }

    if let Some(stderr) = stderr_output {
        report.push_str("\n## Stderr\n```text\n");
        report.push_str(stderr);
        if !stderr.ends_with('\n') {
            report.push('\n');
        }
        report.push_str("```\n");
    }

    if let Some(trace) = stack_trace {
        report.push_str("\n## Stack Trace\n```text\n");
        report.push_str(trace);
        if !trace.ends_with('\n') {
            report.push('\n');
        }
        report.push_str("```\n");
    }

    report
}

fn persist_script_error_acp_context_bundle_in_dir(
    root_dir: &std::path::Path,
    script_path: &str,
    error_message: &str,
    stderr_output: Option<&str>,
    exit_code: Option<i32>,
    stack_trace: Option<&str>,
    suggestions: &[String],
) -> Result<ScriptErrorAcpContextBundle, String> {
    let bundle_dir = root_dir.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&bundle_dir).map_err(|error| {
        format!(
            "failed to create script-error ACP context directory '{}': {error}",
            bundle_dir.display()
        )
    })?;

    let source_path = std::path::Path::new(script_path);
    let script_snapshot_label = source_path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| sanitize_script_error_context_name(value, "script.ts"))
        .unwrap_or_else(|| "script.ts".to_string());
    let script_snapshot_path = bundle_dir.join(&script_snapshot_label);

    let script_snapshot_contents = match std::fs::read_to_string(source_path) {
        Ok(contents) => contents,
        Err(error) => format!(
            "// Script snapshot unavailable\n// Original path: {script_path}\n// Read error: {error}\n"
        ),
    };

    std::fs::write(&script_snapshot_path, script_snapshot_contents).map_err(|error| {
        format!(
            "failed to write script snapshot '{}': {error}",
            script_snapshot_path.display()
        )
    })?;

    let script_stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| sanitize_script_error_context_name(value, "script"))
        .unwrap_or_else(|| "script".to_string());
    let error_report_label = format!("{script_stem}-error-report.md");
    let error_report_path = bundle_dir.join(&error_report_label);
    let error_report = build_script_error_report_markdown(
        script_path,
        error_message,
        stderr_output,
        exit_code,
        stack_trace,
        suggestions,
    );

    std::fs::write(&error_report_path, error_report).map_err(|error| {
        format!(
            "failed to write script error report '{}': {error}",
            error_report_path.display()
        )
    })?;

    Ok(ScriptErrorAcpContextBundle {
        script_snapshot_path: script_snapshot_path.to_string_lossy().into_owned(),
        script_snapshot_label,
        error_report_path: error_report_path.to_string_lossy().into_owned(),
        error_report_label,
    })
}

/// Resolve an automation window target and reject non-main windows.
///
/// Main-window-only executors (getElements, waitFor, batch) call this
/// before any collection, polling, or mutation. If the resolved target
/// is not the main window, an `ActionFailed` error is returned so the
/// caller can send a structured failure response without inspecting
/// main-window state.
fn resolve_main_only_target(
    request_id: &str,
    op: &'static str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
) -> Result<crate::protocol::AutomationWindowInfo, crate::protocol::TransactionError> {
    let resolved = crate::windows::resolve_automation_window(target).map_err(|err| {
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            op = op,
            error = %err,
            "automation.target.resolve_failed"
        );
        crate::protocol::TransactionError::action_failed(format!(
            "{op} target resolution failed: {err}"
        ))
    })?;

    if resolved.kind != crate::protocol::AutomationWindowKind::Main {
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            op = op,
            window_id = %resolved.id,
            kind = ?resolved.kind,
            "automation.target.main_only_rejected"
        );
        return Err(crate::protocol::TransactionError::action_failed(format!(
            "{op} currently supports only the main automation window; resolved {} ({:?})",
            resolved.id, resolved.kind
        )));
    }

    Ok(resolved)
}

/// Which window an ACP read should target.
#[derive(Clone)]
enum AcpReadTarget {
    /// Read from the main window's ACP view (current behavior).
    Main {
        info: Option<crate::protocol::AutomationWindowInfo>,
    },
    /// Read from the detached ACP chat window's entity.
    Detached {
        info: crate::protocol::AutomationWindowInfo,
        entity: gpui::Entity<crate::ai::acp::view::AcpChatView>,
    },
}

/// Resolved automation target for batch/waitFor operations.
///
/// Extends `AcpReadTarget` to also accept Notes and ActionsDialog windows.
#[derive(Clone)]
enum AutomationReadTarget {
    /// Main window (default).
    Main {
        info: Option<crate::protocol::AutomationWindowInfo>,
    },
    /// Detached ACP chat window.
    AcpDetached {
        info: crate::protocol::AutomationWindowInfo,
        entity: gpui::Entity<crate::ai::acp::view::AcpChatView>,
    },
    /// Notes window.
    Notes {
        info: crate::protocol::AutomationWindowInfo,
        entity: gpui::Entity<crate::notes::NotesApp>,
        handle: gpui::WindowHandle<crate::Root>,
    },
    /// Actions dialog popup.
    ActionsDialog {
        info: crate::protocol::AutomationWindowInfo,
        entity: gpui::Entity<crate::actions::ActionsDialog>,
    },
    /// Prompt popup (mention picker, model selector, or confirm dialog).
    PromptPopup {
        info: crate::protocol::AutomationWindowInfo,
    },
}

/// Resolve an automation target that accepts Main, AcpDetached, Notes, and ActionsDialog.
///
/// Used by `batch` and `waitFor` to route commands to the correct window.
fn resolve_automation_read_target(
    request_id: &str,
    op: &'static str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
    cx: &gpui::App,
) -> Result<AutomationReadTarget, crate::protocol::TransactionError> {
    let Some(target) = target else {
        return Ok(AutomationReadTarget::Main { info: None });
    };

    let resolved = crate::windows::resolve_automation_window(Some(target)).map_err(|err| {
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            op = op,
            error = %err,
            "automation.target.resolve_failed"
        );
        crate::protocol::TransactionError::action_failed(format!(
            "{op} target resolution failed: {err}"
        ))
    })?;

    match resolved.kind {
        crate::protocol::AutomationWindowKind::Main => Ok(AutomationReadTarget::Main {
            info: Some(resolved),
        }),
        crate::protocol::AutomationWindowKind::AcpDetached => {
            match crate::ai::acp::chat_window::get_detached_acp_view_entity() {
                Some(entity) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        kind = ?resolved.kind,
                        "automation.target.acp_detached_resolved"
                    );
                    Ok(AutomationReadTarget::AcpDetached {
                        info: resolved,
                        entity,
                    })
                }
                None => Err(crate::protocol::TransactionError::action_failed(format!(
                    "{op} resolved detached ACP target {} but no live view entity is available",
                    resolved.id
                ))),
            }
        }
        crate::protocol::AutomationWindowKind::Notes => {
            match crate::notes::get_notes_app_entity_and_handle() {
                Some((entity, handle)) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        kind = ?resolved.kind,
                        "automation.target.notes_resolved"
                    );
                    Ok(AutomationReadTarget::Notes {
                        info: resolved,
                        entity,
                        handle,
                    })
                }
                None => Err(crate::protocol::TransactionError::action_failed(format!(
                    "{op} resolved Notes target {} but no live Notes entity is available",
                    resolved.id
                ))),
            }
        }
        crate::protocol::AutomationWindowKind::ActionsDialog => {
            match crate::actions::get_actions_dialog_entity(cx) {
                Some(entity) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        kind = ?resolved.kind,
                        "automation.target.actions_dialog_resolved"
                    );
                    Ok(AutomationReadTarget::ActionsDialog {
                        info: resolved,
                        entity,
                    })
                }
                None => Err(crate::protocol::TransactionError::action_failed(format!(
                    "{op} resolved ActionsDialog target {} but no live dialog entity is available",
                    resolved.id
                ))),
            }
        }
        crate::protocol::AutomationWindowKind::PromptPopup => {
            // PromptPopup is a union of mention picker, model selector, and confirm dialog.
            // We verify at least one popup is open. The specific sub-type is detected at
            // batch-execution time since the popup could change between resolution and use.
            let any_open = crate::ai::acp::picker_popup::is_mention_popup_window_open()
                || crate::ai::acp::model_selector_popup::is_model_selector_popup_window_open()
                || crate::confirm::is_confirm_popup_window_open();
            if any_open {
                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    op = op,
                    window_id = %resolved.id,
                    kind = ?resolved.kind,
                    "automation.target.prompt_popup_resolved"
                );
                Ok(AutomationReadTarget::PromptPopup { info: resolved })
            } else {
                Err(crate::protocol::TransactionError::action_failed(format!(
                    "{op} resolved PromptPopup target {} but no popup window is currently open",
                    resolved.id
                )))
            }
        }
        other_kind => {
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                op = op,
                window_id = %resolved.id,
                kind = ?other_kind,
                "automation.target.unsupported_kind"
            );
            Err(crate::protocol::TransactionError::action_failed(format!(
                "{op} supports Main, AcpDetached, Notes, ActionsDialog, and PromptPopup targets; resolved {} ({:?})",
                resolved.id, other_kind
            )))
        }
    }
}

/// Resolve an automation target for ACP read operations (getAcpState, getAcpTestProbe).
///
/// Allows `Main` and `AcpDetached` kinds. Rejects all other secondary targets
/// with a structured error. For `AcpDetached`, returns the live entity from the
/// detached chat window (or errors if no detached window is open).
fn resolve_acp_read_target(
    request_id: &str,
    op: &'static str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
) -> Result<AcpReadTarget, crate::protocol::TransactionError> {
    // No explicit target → default to main window (preserves existing behavior).
    let Some(target) = target else {
        return Ok(AcpReadTarget::Main { info: None });
    };

    let resolved = crate::windows::resolve_automation_window(Some(target)).map_err(|err| {
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            op = op,
            error = %err,
            "automation.acp_target.resolve_failed"
        );
        crate::protocol::TransactionError::action_failed(format!(
            "{op} target resolution failed: {err}"
        ))
    })?;

    match resolved.kind {
        crate::protocol::AutomationWindowKind::Main => {
            tracing::debug!(
                target: "script_kit::automation",
                request_id = %request_id,
                op = op,
                window_id = %resolved.id,
                "automation.acp_target.main"
            );
            Ok(AcpReadTarget::Main {
                info: Some(resolved),
            })
        }
        crate::protocol::AutomationWindowKind::AcpDetached => {
            // Try to get the live entity from the detached window.
            match crate::ai::acp::chat_window::get_detached_acp_view_entity() {
                Some(entity) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        kind = ?resolved.kind,
                        "automation.acp_target.detached_resolved"
                    );
                    Ok(AcpReadTarget::Detached {
                        info: resolved,
                        entity,
                    })
                }
                None => {
                    tracing::warn!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        "automation.acp_target.detached_no_entity"
                    );
                    Err(crate::protocol::TransactionError::action_failed(format!(
                        "{op} resolved detached ACP target {} but no live view entity is available \
                         (window may be a placeholder or closed)",
                        resolved.id
                    )))
                }
            }
        }
        other_kind => {
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                op = op,
                window_id = %resolved.id,
                kind = ?other_kind,
                "automation.acp_target.non_acp_rejected"
            );
            Err(crate::protocol::TransactionError::action_failed(format!(
                "{op} supports only Main and AcpDetached targets; resolved {} ({:?})",
                resolved.id, other_kind
            )))
        }
    }
}

/// Build an `AcpResolvedTarget` from a resolved `AcpReadTarget` and emit
/// a structured `acp_target_resolved` log line.
fn build_acp_resolved_target(
    request_id: &str,
    op: &'static str,
    acp_target: &AcpReadTarget,
) -> Option<crate::protocol::AcpResolvedTarget> {
    let (window_id, window_kind, title) = match acp_target {
        AcpReadTarget::Main { info } => {
            if let Some(info) = info {
                (info.id.clone(), "main".to_string(), info.title.clone())
            } else {
                (
                    "main".to_string(),
                    "main".to_string(),
                    Some("Script Kit".to_string()),
                )
            }
        }
        AcpReadTarget::Detached { info, .. } => (
            info.id.clone(),
            "acpDetached".to_string(),
            info.title.clone(),
        ),
    };

    tracing::info!(
        target: "script_kit::automation",
        event = "acp_target_resolved",
        request_id = %request_id,
        window_id = %window_id,
        kind = %window_kind,
        title = ?title,
        op = op,
    );

    Some(crate::protocol::AcpResolvedTarget {
        window_id,
        window_kind,
        title,
    })
}

/// Build a `UiStateSnapshot` from a live Notes entity.
///
/// Used by `waitFor` and `batch` to evaluate generic conditions
/// (elementExists, elementFocused, inputEmpty, stateMatch) against
/// the Notes window instead of the main window.
fn build_notes_ui_snapshot(
    entity: &gpui::Entity<crate::notes::NotesApp>,
    cx: &gpui::App,
) -> crate::protocol::UiStateSnapshot {
    let editor_text = entity.read(cx).editor_state.read(cx).value().to_string();
    let surface = crate::windows::automation_surface_collector::collect_surface_snapshot(
        &crate::protocol::AutomationWindowInfo {
            id: "notes".to_string(),
            kind: crate::protocol::AutomationWindowKind::Notes,
            title: Some("Notes".to_string()),
            bounds: None,
            visible: true,
            focused: true,
            semantic_surface: Some("notes".to_string()),
            parent_window_id: None,
            parent_kind: None,
        },
        200,
        cx,
    );
    let (semantic_ids, focused_id) = match surface {
        Some(ref snap) => (
            snap.elements
                .iter()
                .map(|e| e.semantic_id.clone())
                .collect(),
            snap.focused_semantic_id.clone(),
        ),
        None => (Vec::new(), None),
    };
    crate::protocol::UiStateSnapshot {
        window_visible: true,
        window_focused: true,
        input_value: Some(editor_text),
        selected_value: None,
        choice_count: 0,
        visible_semantic_ids: semantic_ids,
        focused_semantic_id: focused_id,
    }
}

/// Check whether a generic wait condition is satisfied against Notes state.
///
/// Only generic conditions (elementExists, elementFocused, inputEmpty,
/// windowVisible, windowFocused, stateMatch) are meaningful for Notes.
/// ACP-specific conditions always return `false`.
fn notes_wait_condition_satisfied(
    entity: &gpui::Entity<crate::notes::NotesApp>,
    condition: &crate::protocol::WaitCondition,
    cx: &gpui::App,
) -> bool {
    let snapshot = build_notes_ui_snapshot(entity, cx);
    match condition {
        crate::protocol::WaitCondition::Named(crate::protocol::WaitNamedCondition::InputEmpty) => {
            snapshot.input_value.as_deref().unwrap_or("").is_empty()
        }
        crate::protocol::WaitCondition::Named(
            crate::protocol::WaitNamedCondition::WindowVisible,
        ) => snapshot.window_visible,
        crate::protocol::WaitCondition::Named(
            crate::protocol::WaitNamedCondition::WindowFocused,
        ) => snapshot.window_focused,
        crate::protocol::WaitCondition::Named(
            crate::protocol::WaitNamedCondition::ChoicesRendered,
        ) => {
            // Notes has no choices
            false
        }
        crate::protocol::WaitCondition::Detailed(
            crate::protocol::WaitDetailedCondition::ElementExists { semantic_id }
            | crate::protocol::WaitDetailedCondition::ElementVisible { semantic_id },
        ) => snapshot
            .visible_semantic_ids
            .iter()
            .any(|id| id == semantic_id),
        crate::protocol::WaitCondition::Detailed(
            crate::protocol::WaitDetailedCondition::ElementFocused { semantic_id },
        ) => snapshot.focused_semantic_id.as_deref() == Some(semantic_id.as_str()),
        crate::protocol::WaitCondition::Detailed(
            crate::protocol::WaitDetailedCondition::StateMatch { state },
        ) => {
            use crate::protocol::transaction_executor::matches_state_spec;
            matches_state_spec(&snapshot, state)
        }
        // ACP-specific conditions are not applicable to Notes.
        _ => false,
    }
}

fn resolve_ai_start_chat_provider(
    registry: &crate::ai::ProviderRegistry,
    model_id: &str,
) -> Option<String> {
    registry
        .find_provider_for_model(model_id)
        .map(|provider| provider.provider_id().to_string())
}

#[cfg(any(test, target_os = "windows"))]
fn escape_windows_cmd_open_target(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '^' | '&' | '|' | '<' | '>' | '(' | ')' | '%' | '!' | '"' => {
                escaped.push('^');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptMessageRoute {
    ConfirmDialog,
    UnhandledWarning,
    Other,
}
#[inline]
fn classify_prompt_message_route(message: &PromptMessage) -> PromptMessageRoute {
    match message {
        PromptMessage::ShowConfirm { .. } => PromptMessageRoute::ConfirmDialog,
        PromptMessage::UnhandledMessage { .. } => PromptMessageRoute::UnhandledWarning,
        _ => PromptMessageRoute::Other,
    }
}

fn prompt_message_from_protocol_message(
    message: crate::protocol::Message,
) -> Option<PromptMessage> {
    match message {
        Message::GetState { request_id, target } => {
            Some(PromptMessage::GetState { request_id, target })
        }
        Message::GetElements {
            request_id,
            limit,
            target,
        } => Some(PromptMessage::GetElements {
            request_id,
            limit,
            target,
        }),
        Message::GetAcpState { request_id, target } => {
            Some(PromptMessage::GetAcpState { request_id, target })
        }
        Message::PerformAcpSetupAction {
            request_id,
            action,
            agent_id,
            target,
        } => Some(PromptMessage::PerformAcpSetupAction {
            request_id,
            action,
            agent_id,
            target,
        }),
        Message::ResetAcpTestProbe { request_id, target } => {
            Some(PromptMessage::ResetAcpTestProbe { request_id, target })
        }
        Message::GetAcpTestProbe {
            request_id,
            tail,
            target,
        } => Some(PromptMessage::GetAcpTestProbe {
            request_id,
            tail,
            target,
        }),
        Message::GetLayoutInfo { request_id, target } => {
            Some(PromptMessage::GetLayoutInfo { request_id, target })
        }
        Message::InspectAutomationWindow {
            request_id,
            target,
            hi_dpi,
            probes,
        } => Some(PromptMessage::InspectAutomationWindow {
            request_id,
            target,
            hi_dpi,
            probes,
        }),
        Message::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            trace,
            target,
        } => Some(PromptMessage::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            trace,
            target,
        }),
        Message::Batch {
            request_id,
            commands,
            options,
            trace,
            target,
        } => Some(PromptMessage::Batch {
            request_id,
            commands,
            options,
            trace,
            target,
        }),
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
        } => Some(PromptMessage::SimulateGpuiEvent {
            request_id,
            target,
            event,
        }),
        _ => None,
    }
}

// --- merged from part_001.rs ---
impl ScriptListApp {
    pub(crate) fn handle_stdin_protocol_message(
        &mut self,
        message: crate::protocol::Message,
        cx: &mut Context<Self>,
    ) {
        if let Some(prompt_message) = prompt_message_from_protocol_message(message.clone()) {
            self.handle_prompt_message(prompt_message, cx);
            return;
        }

        match message {
            Message::ListAutomationWindows { request_id } => {
                let windows = crate::windows::list_automation_windows();
                let focused_window_id = crate::windows::focused_automation_window_id();
                let response =
                    Message::automation_window_list_result(request_id, windows, focused_window_id);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for listAutomationWindows"
                    );
                }
            }
            other => {
                let message_type = serde_json::to_value(&other)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("type")
                            .and_then(|ty| ty.as_str())
                            .map(str::to_owned)
                    })
                    .unwrap_or_else(|| "unknown".to_string());
                tracing::warn!(
                    category = "STDIN",
                    message_type = %message_type,
                    "Unsupported protocol message received via stdin"
                );
            }
        }
    }

    pub(crate) fn make_submit_callback(
        &self,
        dropped_label: &'static str,
    ) -> Arc<dyn Fn(String, Option<String>) + Send + Sync> {
        let response_sender = self.response_sender.clone();
        Arc::new(move |id, value| {
            if let Some(ref sender) = response_sender {
                let response = Message::Submit { id, value };
                // Use try_send to avoid blocking UI thread
                match sender.try_send(response) {
                    Ok(()) => {}
                    Err(std::sync::mpsc::TrySendError::Full(_)) => {
                        tracing::warn!(
                            category = "WARN",
                            dropped_label = %dropped_label,
                            "Response channel full - response dropped"
                        );
                    }
                    Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                        tracing::info!(
                            category = "UI",
                            "Response channel disconnected - script exited"
                        );
                    }
                }
            }
        })
    }

    pub(crate) fn prepare_window_for_prompt(
        &self,
        log_target: &str,
        prompt_kind: &str,
        bench_marker: &str,
    ) {
        // Clear NEEDS_RESET when receiving a UI prompt from an active script.
        // This prevents the window from resetting when shown.
        if NEEDS_RESET.swap(false, Ordering::SeqCst) {
            tracing::info!(
                category = log_target,
                prompt_kind = %prompt_kind,
                "Cleared NEEDS_RESET - script is showing prompt UI"
            );
        }

        // Show window if hidden (script may have called hide() for getSelectedText)
        if !script_kit_gpui::is_main_window_visible() {
            if !bench_marker.is_empty() {
                logging::bench_log(bench_marker);
            }
            tracing::info!(
                category = log_target,
                prompt_kind = %prompt_kind,
                "Window hidden - requesting show for prompt UI"
            );
            script_kit_gpui::set_main_window_visible(true);
            script_kit_gpui::request_show_main_window();
        }
    }

    pub(crate) fn set_sdk_actions_and_shortcuts(
        &mut self,
        actions: Vec<ProtocolAction>,
        log_target: &str,
        log_shortcuts: bool,
    ) {
        // Store SDK actions for trigger_action_by_name lookup
        self.sdk_actions = Some(actions.clone());

        // Register keyboard shortcuts for visible SDK actions only
        self.action_shortcuts.clear();
        for action in &actions {
            if action.is_visible() {
                if let Some(shortcut) = &action.shortcut {
                    let normalized = shortcuts::normalize_shortcut(shortcut);
                    if log_shortcuts {
                        tracing::info!(
                            category = log_target,
                            shortcut = %shortcut,
                            action_name = %action.name,
                            normalized = %normalized,
                            "Registering action shortcut"
                        );
                    }
                    self.action_shortcuts
                        .insert(normalized, action.name.clone());
                }
            }
        }
    }

    fn script_error_acp_view_entity(&self) -> Option<gpui::Entity<crate::ai::acp::AcpChatView>> {
        crate::ai::acp::chat_window::get_detached_acp_view_entity().or_else(|| {
            if let AppView::AcpChatView { entity } = &self.current_view {
                Some(entity.clone())
            } else {
                None
            }
        })
    }

    fn ensure_script_error_acp_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<crate::ai::acp::AcpChatView>> {
        if let Some(entity) = self.script_error_acp_view_entity() {
            return Some(entity);
        }

        self.open_tab_ai_acp_with_entry_intent(None, cx);
        self.script_error_acp_view_entity()
    }

    fn route_script_error_to_acp(
        &mut self,
        script_path: &str,
        error_message: &str,
        stderr_output: Option<&str>,
        exit_code: Option<i32>,
        stack_trace: Option<&str>,
        suggestions: &[String],
        cx: &mut Context<Self>,
    ) {
        let context_root = crate::setup::get_kit_path()
            .join("acp")
            .join("script-error-context");
        let bundle = match persist_script_error_acp_context_bundle_in_dir(
            &context_root,
            script_path,
            error_message,
            stderr_output,
            exit_code,
            stack_trace,
            suggestions,
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                tracing::warn!(
                    target: "script_kit::tab_ai",
                    event = "script_error_acp_context_bundle_failed",
                    script_path = %script_path,
                    error = %error,
                );
                return;
            }
        };

        let Some(view_entity) = self.ensure_script_error_acp_view(cx) else {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "script_error_acp_view_unavailable",
                script_path = %script_path,
            );
            return;
        };

        let prompt =
            build_script_error_acp_prompt(script_path, error_message, exit_code, suggestions);
        if let Err(error) =
            Self::stage_script_error_context_on_acp_view(view_entity, bundle, prompt, cx)
        {
            tracing::warn!(
                target: "script_kit::tab_ai",
                event = "script_error_acp_stage_failed",
                script_path = %script_path,
                error = %error,
            );
        }
    }

    fn stage_script_error_context_on_acp_view(
        view_entity: gpui::Entity<crate::ai::acp::AcpChatView>,
        bundle: ScriptErrorAcpContextBundle,
        prompt: String,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let script_part = crate::ai::AiContextPart::FilePath {
            path: bundle.script_snapshot_path.clone(),
            label: bundle.script_snapshot_label.clone(),
        };
        let report_part = crate::ai::AiContextPart::FilePath {
            path: bundle.error_report_path.clone(),
            label: bundle.error_report_label.clone(),
        };
        let parts = vec![script_part, report_part];
        let mention_tokens = parts
            .iter()
            .filter_map(crate::ai::context_mentions::part_to_inline_token)
            .collect::<Vec<_>>();
        let composer_text = if mention_tokens.is_empty() {
            prompt
        } else {
            format!("{}\n\n{}", mention_tokens.join(" "), prompt)
        };

        let mut stage_result: Result<(), String> = Ok(());
        view_entity.update(cx, |view, cx| {
            let Some(thread_entity) = view.thread() else {
                stage_result = Err("ACP chat is in setup mode".to_string());
                return;
            };

            for part in &parts {
                if let Some(token) = crate::ai::context_mentions::part_to_inline_token(part) {
                    view.register_typed_alias(token.clone(), part.clone());
                    view.register_inline_owned_token(token);
                }
            }

            thread_entity.update(cx, |thread, cx| {
                for part in &parts {
                    thread.add_context_part(part.clone(), cx);
                }
                thread.set_input(composer_text.clone(), cx);
                if let Err(error) = thread.submit_input(cx) {
                    stage_result = Err(error);
                }
            });
        });

        stage_result
    }

    fn show_prompt_coming_soon_toast(&mut self, prompt_name: &str, cx: &mut Context<Self>) {
        let toast = Toast::warning(prompt_coming_soon_warning(prompt_name), &self.theme)
            .duration_ms(Some(TOAST_WARNING_MS));
        self.toast_manager.push(toast);
        cx.notify();
    }

    /// Handle a prompt message from the script
    #[tracing::instrument(skip(self, cx), fields(msg_type = ?msg))]
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        let route = classify_prompt_message_route(&msg);
        tracing::debug!(target: "prompt_handler", ?route, "Routing prompt message");

        match msg {
            PromptMessage::ShowArg {
                id,
                placeholder,
                choices,
                actions,
            } => {
                self.prepare_window_for_prompt("UI", "arg", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    choice_count = choices.len(),
                    action_count = actions.as_ref().map(|a| a.len()).unwrap_or(0),
                    "Showing arg prompt"
                );
                let choice_count = choices.len();

                // If actions were provided, store them in the SDK actions system
                // so they can be triggered via shortcuts and Cmd+K
                if let Some(ref action_list) = actions {
                    self.set_sdk_actions_and_shortcuts(action_list.clone(), "UI", false);
                } else {
                    // Clear any previous SDK actions
                    self.sdk_actions = None;
                    self.action_shortcuts.clear();
                }

                let pending_placeholder = placeholder.clone();
                self.current_view = AppView::ArgPrompt {
                    id,
                    placeholder,
                    choices,
                    actions,
                };
                self.arg_input.clear();
                self.filter_text.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                self.pending_filter_sync = true;
                self.pending_placeholder = Some(pending_placeholder);
                self.pending_focus = Some(FocusTarget::MainFilter);
                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                resize_to_view_sync(view_type, choice_count);
                cx.notify();
            }
            PromptMessage::ShowMini {
                id,
                placeholder,
                choices,
            } => {
                self.prepare_window_for_prompt("UI", "mini", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    choice_count = choices.len(),
                    "Showing mini prompt"
                );
                let choice_count = choices.len();

                // Clear any previous SDK actions (mini has no actions)
                self.sdk_actions = None;
                self.action_shortcuts.clear();

                let pending_placeholder = placeholder.clone();
                self.current_view = AppView::MiniPrompt {
                    id,
                    placeholder,
                    choices,
                };
                self.arg_input.clear();
                self.filter_text.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                self.pending_filter_sync = true;
                self.pending_placeholder = Some(pending_placeholder);
                self.pending_focus = Some(FocusTarget::MainFilter);
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                resize_to_view_sync(view_type, choice_count);
                cx.notify();
            }
            PromptMessage::ShowMicro {
                id,
                placeholder,
                choices,
            } => {
                self.prepare_window_for_prompt("UI", "micro", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    choice_count = choices.len(),
                    "Showing micro prompt"
                );

                // Clear any previous SDK actions (micro has no actions)
                self.sdk_actions = None;
                self.action_shortcuts.clear();

                self.current_view = AppView::MicroPrompt {
                    id,
                    placeholder,
                    choices,
                };
                self.arg_input.clear();
                self.arg_selected_index = 0;
                self.focused_input = FocusedInput::ArgPrompt;
                self.pending_focus = Some(FocusTarget::AppRoot);
                // Micro always uses compact (no-choices) height
                resize_to_view_sync(ViewType::ArgPromptNoChoices, 0);
                cx.notify();
            }
            PromptMessage::ShowDiv {
                id,
                html,
                container_classes,
                actions,
                placeholder: _placeholder, // TODO: render in header
                hint: _hint,               // TODO: render hint
                footer: _footer,           // TODO: render footer
                container_bg,
                container_padding,
                opacity,
            } => {
                self.prepare_window_for_prompt("UI", "div", "");

                tracing::info!(category = "UI", id = %id, "Showing div prompt");
                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                let submit_callback = self.make_submit_callback("div");

                // Create focus handle for div prompt
                let div_focus_handle = cx.focus_handle();

                // Build container options from protocol message
                let container_options = ContainerOptions {
                    background: container_bg,
                    padding: container_padding.and_then(|v| {
                        if v.is_string() && v.as_str() == Some("none") {
                            Some(ContainerPadding::None)
                        } else if let Some(n) = v.as_f64() {
                            Some(ContainerPadding::Pixels(n as f32))
                        } else {
                            v.as_i64().map(|n| ContainerPadding::Pixels(n as f32))
                        }
                    }),
                    opacity,
                    container_classes,
                };

                // Create DivPrompt entity with proper HTML rendering
                let div_prompt = DivPrompt::with_options(
                    id.clone(),
                    html,
                    None, // tailwind param deprecated - use container_classes in options
                    div_focus_handle,
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                    crate::designs::DesignVariant::Default,
                    container_options,
                );

                let entity = cx.new(|_| div_prompt);
                self.current_view = AppView::DivPrompt { id, entity };
                self.focused_input = FocusedInput::None; // DivPrompt has no text input
                self.pending_focus = Some(FocusTarget::AppRoot); // DivPrompt uses parent focus
                resize_to_view_sync(ViewType::DivPrompt, 0);
                cx.notify();
            }
            PromptMessage::ShowForm { id, html, actions } => {
                self.prepare_window_for_prompt("UI", "form", "");

                tracing::info!(category = "UI", id = %id, "Showing form prompt");

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create form field colors from theme
                let colors = FormFieldColors::from_theme(&self.theme);

                // Create FormPromptState entity with parsed fields
                let form_state = FormPromptState::new(id.clone(), html, colors, cx);
                let field_count = form_state.fields.len();
                let entity = cx.new(|_| form_state);

                self.current_view = AppView::FormPrompt { id, entity };
                self.focused_input = FocusedInput::None; // FormPrompt has its own focus handling
                self.pending_focus = Some(FocusTarget::FormPrompt);

                // Resize based on field count (more fields = taller window)
                let view_type = if field_count > 0 {
                    ViewType::ArgPromptWithChoices
                } else {
                    ViewType::DivPrompt
                };
                resize_to_view_sync(view_type, field_count);
                cx.notify();
            }
            PromptMessage::ShowTerm {
                id,
                command,
                actions,
            } => {
                self.prepare_window_for_prompt("UI", "term", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    command = ?command,
                    "Showing term prompt"
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                let submit_callback = self.make_submit_callback("terminal");

                // Get the target height for terminal view (subtract footer height)
                let term_height =
                    window_resize::layout::MAX_HEIGHT - px(window_resize::layout::FOOTER_HEIGHT);

                // Create terminal with explicit height - GPUI entities don't inherit parent flex sizing
                match term_prompt::TermPrompt::with_height(
                    id.clone(),
                    command,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                    std::sync::Arc::new(self.config.clone()),
                    Some(term_height),
                ) {
                    Ok(term_prompt) => {
                        let entity = cx.new(|_| term_prompt);
                        self.current_view = AppView::TermPrompt { id, entity };
                        self.focused_input = FocusedInput::None; // Terminal handles its own cursor
                        self.pending_focus = Some(FocusTarget::TermPrompt);
                        // DEFERRED RESIZE: Avoid RefCell borrow error by deferring window resize
                        // to after the current GPUI update cycle completes. Synchronous Cocoa
                        // setFrame: calls during render can trigger events that re-borrow GPUI state.
                        cx.spawn(async move |_this, _cx| {
                            resize_to_view_sync(ViewType::TermPrompt, 0);
                        })
                        .detach();
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::error!(category = "ERROR", error = %e, "Failed to create terminal");
                    }
                }
            }
            PromptMessage::ShowEditor {
                id,
                content,
                language,
                template,
                actions,
            } => {
                self.prepare_window_for_prompt("UI", "editor", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    language = ?language,
                    has_template = template.is_some(),
                    "Showing editor prompt"
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                let submit_callback = self.make_submit_callback("editor");

                // CRITICAL: Create a SEPARATE focus handle for the editor.
                // Using the parent's focus handle causes keyboard event routing issues
                // because the parent checks is_focused() in its render and both parent
                // and child would be tracking the same handle.
                let editor_focus_handle = cx.focus_handle();

                // Get the target height for editor view (subtract footer height for unified footer)
                let editor_height = px(700.0 - window_resize::layout::FOOTER_HEIGHT);

                // Create editor v2 (gpui-component based with Find/Replace)
                // Default to markdown for all editor content
                let resolved_language = language.unwrap_or_else(|| "markdown".to_string());

                // Use with_template if template provided, or if content contains tabstop patterns
                // This auto-detects VSCode-style templates like ${1:name} or $1
                let content_str = content.unwrap_or_default();
                let has_tabstops =
                    crate::snippet::analysis::contains_explicit_tabstops(&content_str);

                let editor_prompt = if let Some(template_str) = template {
                    EditorPrompt::with_template(
                        id.clone(),
                        template_str,
                        resolved_language.clone(),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::clone(&self.theme),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                } else if has_tabstops {
                    // Auto-detect template in content
                    tracing::info!(
                        category = "UI",
                        content = %content_str,
                        "Auto-detected template in content"
                    );
                    EditorPrompt::with_template(
                        id.clone(),
                        content_str,
                        resolved_language.clone(),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::clone(&self.theme),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                } else {
                    EditorPrompt::with_height(
                        id.clone(),
                        content_str,
                        resolved_language.clone(),
                        editor_focus_handle.clone(),
                        submit_callback,
                        std::sync::Arc::clone(&self.theme),
                        std::sync::Arc::new(self.config.clone()),
                        Some(editor_height),
                    )
                };

                let entity = cx.new(|_| editor_prompt);
                self.current_view = AppView::EditorPrompt {
                    id,
                    entity,
                    focus_handle: editor_focus_handle,
                };
                self.focused_input = FocusedInput::None; // Editor handles its own focus
                self.pending_focus = Some(FocusTarget::EditorPrompt);

                // DEFERRED RESIZE: Avoid RefCell borrow error by deferring window resize
                // to after the current GPUI update cycle completes.
                cx.spawn(async move |_this, _cx| {
                    resize_to_view_sync(ViewType::EditorPrompt, 0);
                })
                .detach();
                cx.notify();
            }

            PromptMessage::ScriptExit => {
                tracing::info!(
                    category = "VISIBILITY",
                    "=== ScriptExit message received ==="
                );

                // Complete pending Tab AI execution on clean exit.
                // If ScriptError already consumed the record, this is a no-op.
                self.complete_tab_ai_execution(true, None, cx);

                let was_visible = script_kit_gpui::is_main_window_visible();
                let script_hid_window = script_kit_gpui::script_requested_hide();
                tracing::info!(
                    category = "VISIBILITY",
                    was_visible,
                    script_hid_window,
                    "Window visibility state before script exit reset"
                );

                // Reset the script-requested-hide flag
                script_kit_gpui::set_script_requested_hide(false);
                tracing::info!(
                    category = "VISIBILITY",
                    "SCRIPT_REQUESTED_HIDE reset to: false"
                );

                let keep_tab_ai_save_offer_open = self.tab_ai_save_offer_state.is_some();
                let keep_acp_chat_open = matches!(self.current_view, AppView::AcpChatView { .. });

                if keep_tab_ai_save_offer_open {
                    tracing::info!(
                        category = "VISIBILITY",
                        keep_tab_ai_save_offer_open,
                        keep_acp_chat_open,
                        "Tab AI active after script exit - preserving view"
                    );

                    if should_restore_main_window_after_script_exit(script_hid_window, true) {
                        tracing::info!(
                            category = "VISIBILITY",
                            "Script had hidden window - requesting show for follow-up UI"
                        );
                        script_kit_gpui::request_show_main_window();
                    }

                    return;
                } else if keep_acp_chat_open {
                    tracing::info!(
                        category = "VISIBILITY",
                        keep_tab_ai_save_offer_open,
                        keep_acp_chat_open,
                        "Tab AI active after script exit - preserving view"
                    );

                    if should_restore_main_window_after_script_exit(script_hid_window, true) {
                        tracing::info!(
                            category = "VISIBILITY",
                            "Script had hidden window - requesting show for follow-up UI"
                        );
                        script_kit_gpui::request_show_main_window();
                    }

                    return;
                }

                // Set flag so next hotkey show will reset to script list
                NEEDS_RESET.store(true, Ordering::SeqCst);
                tracing::info!(category = "VISIBILITY", "NEEDS_RESET set to: true");

                self.reset_to_script_list(cx);
                tracing::info!(category = "VISIBILITY", "reset_to_script_list() called");

                if !script_hid_window {
                    // Script didn't hide window, so it was user-initiated hide or already visible
                    // Restore window height to main menu size in case a prompt (like EnvPrompt)
                    // had shrunk the window
                    resize_to_view_sync(ViewType::ScriptList, 0);
                    self.hide_main_and_reset(cx);
                    tracing::info!(
                        category = "VISIBILITY",
                        "Script didn't hide window - restored height and hid/reset main window"
                    );
                }
            }
            PromptMessage::HideWindow => {
                tracing::info!(
                    category = "VISIBILITY",
                    "=== HideWindow message received ==="
                );
                let was_visible = script_kit_gpui::is_main_window_visible();
                tracing::info!(
                    category = "VISIBILITY",
                    was_visible,
                    "Window visibility state before hide request"
                );

                // Mark that script requested hide - so ScriptExit knows to show window again
                script_kit_gpui::set_script_requested_hide(true);
                tracing::info!(
                    category = "VISIBILITY",
                    "SCRIPT_REQUESTED_HIDE set to: true"
                );

                self.hide_main_and_reset(cx);
                tracing::info!(
                    category = "VISIBILITY",
                    "hide_main_and_reset() called - main window hidden and reset requested"
                );
            }
            PromptMessage::OpenBrowser { url } => {
                tracing::info!(category = "UI", url = %url, "Opening browser");
                #[cfg(target_os = "macos")]
                {
                    match std::process::Command::new("open").arg(&url).spawn() {
                        Ok(_) => tracing::info!(
                            category = "UI",
                            url = %url,
                            "Successfully opened URL in browser"
                        ),
                        Err(e) => {
                            tracing::error!(
                                category = "ERROR",
                                url = %url,
                                error = %e,
                                "Failed to open URL"
                            )
                        }
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    match std::process::Command::new("xdg-open").arg(&url).spawn() {
                        Ok(_) => tracing::info!(
                            category = "UI",
                            url = %url,
                            "Successfully opened URL in browser"
                        ),
                        Err(e) => {
                            tracing::error!(
                                category = "ERROR",
                                url = %url,
                                error = %e,
                                "Failed to open URL"
                            )
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    let escaped_url = escape_windows_cmd_open_target(&url);
                    match std::process::Command::new("cmd")
                        .args(["/C", "start", ""])
                        .arg(&escaped_url)
                        .spawn()
                    {
                        Ok(_) => tracing::info!(
                            category = "UI",
                            url = %url,
                            "Successfully opened URL in browser"
                        ),
                        Err(e) => {
                            tracing::error!(
                                category = "ERROR",
                                url = %url,
                                error = %e,
                                "Failed to open URL"
                            )
                        }
                    }
                }
            }
            PromptMessage::RunScript { path } => {
                tracing::info!(category = "EXEC", path = %path, "RunScript command received");

                // Create a Script struct from the path
                let script_path = std::path::PathBuf::from(&path);
                let script_name = script_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let extension = script_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ts")
                    .to_string();

                let script = scripts::Script {
                    name: script_name.clone(),
                    description: Some(format!("External script: {}", path)),
                    path: script_path,
                    extension,
                    icon: None,
                    alias: None,
                    shortcut: None,
                    typed_metadata: None,
                    schema: None,
                    plugin_id: String::new(),
                    plugin_title: None,
                    kit_name: None,
                    body: None,
                };

                tracing::info!(
                    category = "EXEC",
                    script_name = %script_name,
                    "Executing script"
                );
                self.execute_interactive(&script, cx);
            }
            PromptMessage::ScriptError {
                error_message,
                stderr_output,
                exit_code,
                stack_trace,
                script_path,
                suggestions,
            } => {
                tracing::error!(
                    category = "ERROR",
                    error_message = %error_message,
                    exit_code = ?exit_code,
                    script_path = %script_path,
                    "Script error received"
                );
                if let Some(ref stderr) = stderr_output {
                    tracing::error!(
                        category = "ERROR",
                        script_path = %script_path,
                        stderr = %stderr,
                        "Script stderr output"
                    );
                }
                if let Some(ref trace) = stack_trace {
                    tracing::error!(
                        category = "ERROR",
                        script_path = %script_path,
                        stack_trace = %trace,
                        "Script stack trace"
                    );
                }

                // CRITICAL: Show error via HUD (highly visible floating window)
                // This ensures the user sees the error even if the main window is hidden/dismissed
                // HUD appears at bottom-center of screen for 5 seconds
                let hud_message = if error_message.chars().count() > 140 {
                    // Use chars().take() to safely handle multi-byte UTF-8 characters
                    let truncated: String = error_message.chars().take(137).collect();
                    format!("Script Error: {}...", truncated)
                } else {
                    format!("Script Error: {}", error_message)
                };
                self.show_hud(hud_message, Some(HUD_SLOW_MS), cx);

                // Also create in-app toast with expandable details (for when window is visible)
                // Use stderr_output if available, otherwise use stack_trace
                let details_text = stderr_output.clone().or_else(|| stack_trace.clone());
                let toast = Toast::error(error_message.clone(), &self.theme)
                    .details_opt(details_text.clone())
                    .duration_ms(Some(TOAST_CRITICAL_MS)); // 10 seconds for errors

                // Add copy button action if we have stderr/stack trace
                let toast = if let Some(ref trace) = details_text {
                    let trace_clone = trace.clone();
                    toast.action(ToastAction::new(
                        "Copy Error",
                        Box::new(move |_, _, _| {
                            // Copy to clipboard
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(trace_clone.clone());
                                tracing::info!(category = "UI", "Error copied to clipboard");
                            }
                        }),
                    ))
                } else {
                    toast
                };

                // Log suggestions if present
                if !suggestions.is_empty() {
                    tracing::error!(
                        category = "ERROR",
                        suggestions = ?suggestions,
                        "Script error suggestions"
                    );
                }

                // Push toast to manager
                let toast_id = self.toast_manager.push(toast);
                tracing::info!(
                    category = "UI",
                    script_path = %script_path,
                    toast_id = %toast_id,
                    "Toast created for script error"
                );

                self.route_script_error_to_acp(
                    &script_path,
                    &error_message,
                    stderr_output.as_deref(),
                    exit_code,
                    stack_trace.as_deref(),
                    &suggestions,
                    cx,
                );

                // Complete pending Tab AI execution on failure.
                // Consumes the record so the subsequent ScriptExit is a no-op.
                let tab_ai_error_msg = format!(
                    "Tab AI script exited with code {:?}: {}",
                    exit_code, error_message
                );
                self.complete_tab_ai_execution(false, Some(tab_ai_error_msg), cx);

                cx.notify();
            }
            PromptMessage::ProtocolError {
                correlation_id,
                summary,
                details,
                severity,
                script_path,
            } => {
                tracing::warn!(
                    correlation_id = %correlation_id,
                    script_path = %script_path,
                    summary = %summary,
                    "Protocol parse issue received"
                );

                let mut toast = Toast::from_severity(summary.clone(), severity, &self.theme)
                    .details_opt(details.clone())
                    .duration_ms(Some(TOAST_ERROR_DETAILED_MS));

                if let Some(ref detail_text) = details {
                    let detail_clone = detail_text.clone();
                    toast = toast.action(ToastAction::new(
                        "Copy Details",
                        Box::new(move |_, _, _| {
                            use arboard::Clipboard;
                            if let Ok(mut clipboard) = Clipboard::new() {
                                let _ = clipboard.set_text(detail_clone.clone());
                            }
                        }),
                    ));
                }

                self.toast_manager.push(toast);
                cx.notify();
            }
            PromptMessage::UnhandledMessage { message_type } => {
                tracing::warn!(
                    category = "WARN",
                    message_type = %message_type,
                    "Displaying unhandled message warning"
                );

                let toast = Toast::warning(unhandled_message_warning(&message_type), &self.theme)
                    .duration_ms(Some(TOAST_WARNING_MS));

                self.toast_manager.push(toast);
                cx.notify();
            }

            PromptMessage::GetState { request_id, target } => {
                tracing::info!(
                    category = "UI",
                    request_id = %request_id,
                    target = ?target,
                    "Collecting state for request"
                );

                // Validate target if explicitly specified.
                // Non-main targets are not yet wired to secondary window
                // state providers, so we return an honest diagnostic with
                // machine-readable promptType and promptId fields instead
                // of a bare "error" string.
                if let Some(ref t) = target {
                    if !matches!(
                        t,
                        protocol::AutomationWindowTarget::Main
                            | protocol::AutomationWindowTarget::Focused
                    ) {
                        match crate::windows::resolve_automation_window(Some(t)) {
                            Ok(resolved)
                                if resolved.kind != protocol::AutomationWindowKind::Main =>
                            {
                                tracing::warn!(
                                    target: "script_kit::automation",
                                    request_id = %request_id,
                                    resolved_kind = ?resolved.kind,
                                    resolved_id = %resolved.id,
                                    "getState: secondary window state not yet routed, returning unsupported diagnostic"
                                );
                                if let Some(ref sender) = self.response_sender {
                                    let _ = sender.try_send(Message::state_result(
                                        request_id.clone(),
                                        "unsupported".to_string(),
                                        Some(format!("target_unsupported:{:?}", resolved.kind)),
                                        None,
                                        String::new(),
                                        0,
                                        0,
                                        -1,
                                        None,
                                        false,
                                        resolved.visible,
                                    ));
                                }
                                return;
                            }
                            Err(err) => {
                                tracing::warn!(
                                    target: "script_kit::automation",
                                    request_id = %request_id,
                                    error = %err,
                                    "getState: target resolution failed"
                                );
                                if let Some(ref sender) = self.response_sender {
                                    let _ = sender.try_send(Message::state_result(
                                        request_id.clone(),
                                        "target_resolution_failed".to_string(),
                                        Some(format!("target_error:{}", err)),
                                        None,
                                        String::new(),
                                        0,
                                        0,
                                        -1,
                                        None,
                                        false,
                                        false,
                                    ));
                                }
                                return;
                            }
                            _ => {} // Main kind — fall through to normal handling
                        }
                    }
                }

                // Collect current UI state
                let (
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                ) = match &self.current_view {
                    AppView::ScriptList => {
                        let filtered_len = self.filtered_results().len();
                        let selected_value = if self.selected_index < filtered_len {
                            self.filtered_results()
                                .get(self.selected_index)
                                .map(|r| match r {
                                    scripts::SearchResult::Script(m) => m.script.name.clone(),
                                    scripts::SearchResult::Scriptlet(m) => m.scriptlet.name.clone(),
                                    scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
                                    scripts::SearchResult::App(m) => m.app.name.clone(),
                                    scripts::SearchResult::Window(m) => m.window.title.clone(),
                                    scripts::SearchResult::Agent(m) => m.agent.name.clone(),
                                    scripts::SearchResult::Skill(m) => m.skill.title.clone(),
                                    scripts::SearchResult::Fallback(m) => {
                                        m.fallback.display_label()
                                    }
                                })
                        } else {
                            None
                        };
                        (
                            "none".to_string(),
                            None,
                            None,
                            self.filter_text.clone(),
                            self.scripts.len()
                                + self.scriptlets.len()
                                + self.builtin_entries.len()
                                + self.apps.len(),
                            filtered_len,
                            self.selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ArgPrompt {
                        id,
                        placeholder,
                        choices,
                        actions: _,
                    } => {
                        let filtered = self.get_filtered_arg_choices(choices);
                        let selected_value = if self.arg_selected_index < filtered.len() {
                            filtered
                                .get(self.arg_selected_index)
                                .map(|c| c.value.clone())
                        } else {
                            None
                        };
                        (
                            "arg".to_string(),
                            Some(id.clone()),
                            Some(placeholder.clone()),
                            self.arg_input.text().to_string(),
                            choices.len(),
                            filtered.len(),
                            self.arg_selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::DivPrompt { id, .. } => (
                        "div".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::FormPrompt { id, .. } => (
                        "form".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TermPrompt { id, .. } => (
                        "term".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EditorPrompt { id, .. } => (
                        "editor".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::SelectPrompt { id, .. } => (
                        "select".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::PathPrompt { id, .. } => (
                        "path".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::EnvPrompt { id, .. } => (
                        "env".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::DropPrompt { id, .. } => (
                        "drop".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::TemplatePrompt { id, .. } => (
                        "template".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::ChatPrompt { id, .. } => (
                        "chat".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::MiniPrompt {
                        id,
                        placeholder,
                        choices,
                    } => {
                        let filtered = self.get_filtered_arg_choices(choices);
                        let selected_value = filtered
                            .get(self.arg_selected_index)
                            .map(|c| c.value.clone());
                        (
                            "mini".to_string(),
                            Some(id.clone()),
                            Some(placeholder.clone()),
                            self.arg_input.text().to_string(),
                            choices.len(),
                            filtered.len(),
                            self.arg_selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::MicroPrompt {
                        id,
                        placeholder,
                        choices,
                    } => {
                        let filtered = self.get_filtered_arg_choices(choices);
                        let selected_value = filtered
                            .get(self.arg_selected_index)
                            .map(|c| c.value.clone());
                        (
                            "micro".to_string(),
                            Some(id.clone()),
                            Some(placeholder.clone()),
                            self.arg_input.text().to_string(),
                            choices.len(),
                            filtered.len(),
                            self.arg_selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ActionsDialog => (
                        "actions".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    // P0 FIX: View state only - data comes from self.cached_clipboard_entries
                    AppView::ClipboardHistoryView {
                        filter,
                        selected_index,
                    } => {
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
                        (
                            "clipboardHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::AcpHistoryView {
                        filter,
                        selected_index,
                    } => {
                        let entries = crate::ai::acp::history::load_history();
                        let filtered_entries: Vec<&crate::ai::acp::history::AcpHistoryEntry> =
                            if filter.is_empty() {
                                entries.iter().collect()
                            } else {
                                let filter_lower = filter.to_lowercase();
                                entries
                                    .iter()
                                    .filter(|entry| {
                                        entry.first_message.to_lowercase().contains(&filter_lower)
                                            || entry
                                                .timestamp
                                                .to_lowercase()
                                                .contains(&filter_lower)
                                    })
                                    .collect()
                            };
                        let selected_value = filtered_entries
                            .get(*selected_index)
                            .map(|entry| entry.first_message.clone());
                        (
                            "acpHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            filtered_entries.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::NotesBrowseView {
                        filter,
                        selected_index,
                    } => {
                        let entries = if filter.is_empty() {
                            crate::notes::get_all_notes().unwrap_or_default()
                        } else {
                            crate::notes::search_notes(filter).unwrap_or_default()
                        };
                        let selected_value = entries.get(*selected_index).map(|entry| {
                            if entry.title.trim().is_empty() {
                                "Untitled Note".to_string()
                            } else {
                                entry.title.clone()
                            }
                        });
                        (
                            "notesBrowse".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            entries.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    // P0 FIX: View state only - data comes from self.apps
                    AppView::AppLauncherView {
                        filter,
                        selected_index,
                    } => {
                        let apps = &self.apps;
                        let filtered_count = if filter.is_empty() {
                            apps.len()
                        } else {
                            let filter_lower = filter.to_lowercase();
                            apps.iter()
                                .filter(|a| a.name.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "appLauncher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            apps.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    // P0 FIX: View state only - data comes from self.cached_windows
                    AppView::WindowSwitcherView {
                        filter,
                        selected_index,
                    } => {
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
                        (
                            "windowSwitcher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            windows.len(),
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::DesignGalleryView {
                        filter,
                        selected_index,
                    } => {
                        let total_items = designs::separator_variations::SeparatorStyle::count()
                            + designs::icon_variations::total_icon_count()
                            + 8
                            + 6; // headers
                        (
                            "designGallery".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total_items,
                            total_items,
                            *selected_index as i32,
                            None,
                        )
                    }
                    #[cfg(feature = "storybook")]
                    AppView::DesignExplorerView { .. } => (
                        "designExplorer".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        0,
                        None,
                    ),
                    AppView::ScratchPadView { .. } => (
                        "scratchPad".to_string(),
                        Some("scratch-pad".to_string()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::QuickTerminalView { .. } => (
                        "quickTerminal".to_string(),
                        Some("quick-terminal".to_string()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::FileSearchView {
                        ref query,
                        selected_index,
                        ..
                    } => (
                        "fileSearch".to_string(),
                        Some("file-search".to_string()),
                        None,
                        query.clone(),
                        self.cached_file_results.len(),
                        self.cached_file_results.len(),
                        *selected_index as i32,
                        self.cached_file_results
                            .get(*selected_index)
                            .map(|f| f.name.clone()),
                    ),
                    AppView::ThemeChooserView { selected_index, .. } => (
                        "themeChooser".to_string(),
                        Some("theme-chooser".to_string()),
                        None,
                        String::new(),
                        0,
                        0,
                        *selected_index as i32,
                        None,
                    ),
                    AppView::EmojiPickerView {
                        filter,
                        selected_index,
                        selected_category,
                    } => {
                        let filtered_count = crate::emoji::search_emojis(filter)
                            .into_iter()
                            .filter(|emoji| {
                                selected_category
                                    .map(|category| emoji.category == category)
                                    .unwrap_or(true)
                            })
                            .count();
                        (
                            "emojiPicker".to_string(),
                            Some("emoji-picker".to_string()),
                            None,
                            filter.clone(),
                            filtered_count,
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::WebcamView { .. } => (
                        "webcam".to_string(),
                        Some("webcam".to_string()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::CreationFeedback { .. } => (
                        "creationFeedback".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::NamingPrompt { id, .. } => (
                        "namingPrompt".to_string(),
                        Some(id.clone()),
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::BrowseKitsView {
                        query,
                        selected_index,
                        results,
                    } => (
                        "browseKits".to_string(),
                        None,
                        None,
                        query.clone(),
                        results.len(),
                        results.len(),
                        *selected_index as i32,
                        None,
                    ),
                    AppView::InstalledKitsView {
                        selected_index,
                        kits,
                    } => (
                        "installedKits".to_string(),
                        None,
                        None,
                        String::new(),
                        kits.len(),
                        kits.len(),
                        *selected_index as i32,
                        None,
                    ),
                    AppView::ProcessManagerView {
                        filter,
                        selected_index,
                    } => {
                        let total = self.cached_processes.len();
                        let filtered_count = if filter.is_empty() {
                            total
                        } else {
                            let filter_lower = filter.to_lowercase();
                            self.cached_processes
                                .iter()
                                .filter(|p| p.script_path.to_lowercase().contains(&filter_lower))
                                .count()
                        };
                        (
                            "processManager".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::CurrentAppCommandsView {
                        filter,
                        selected_index,
                    } => {
                        let total = self.cached_current_app_entries.len();
                        let filtered_count = if filter.is_empty() {
                            total
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
                        (
                            "currentAppCommands".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            filtered_count,
                            *selected_index as i32,
                            None,
                        )
                    }
                    AppView::SearchAiPresetsView {
                        filter,
                        selected_index,
                    } => (
                        "searchAiPresets".to_string(),
                        None,
                        None,
                        filter.clone(),
                        0,
                        0,
                        *selected_index as i32,
                        None,
                    ),
                    AppView::CreateAiPresetView { .. } => (
                        "createAiPreset".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        0,
                        None,
                    ),
                    AppView::SettingsView {
                        filter,
                        selected_index,
                    } => (
                        "settings".to_string(),
                        None,
                        None,
                        filter.clone(),
                        0,
                        0,
                        *selected_index as i32,
                        None,
                    ),
                    AppView::FavoritesBrowseView {
                        filter,
                        selected_index,
                    } => (
                        "favorites".to_string(),
                        None,
                        None,
                        filter.clone(),
                        0,
                        0,
                        *selected_index as i32,
                        None,
                    ),
                    AppView::AcpChatView { .. } => (
                        "acpChat".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                };

                // Focus state: we use focused_input as a proxy since we don't have Window access here.
                // When window is visible and we're tracking an input, we're focused.
                let window_visible = script_kit_gpui::is_main_window_visible();
                let is_focused = window_visible && self.focused_input != FocusedInput::None;

                // Create the response
                let response = Message::state_result(
                    request_id.clone(),
                    prompt_type,
                    prompt_id,
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                    is_focused,
                    window_visible,
                );

                tracing::info!(
                    category = "UI",
                    request_id = %request_id,
                    "Sending state result for request"
                );

                // Send the response - use try_send to avoid blocking UI
                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "WARN",
                                "Response channel full - state result dropped"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "UI",
                                "Response channel disconnected - script exited"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        category = "ERROR",
                        "No response sender available for state result"
                    );
                }
            }

            PromptMessage::GetAcpState { request_id, target } => {
                tracing::info!(
                    category = "ACP_STATE",
                    request_id = %request_id,
                    target = ?target,
                    "acp_state.request"
                );

                // Resolve target: Main → main window, AcpDetached → detached entity,
                // anything else → structured error.
                let acp_target =
                    match resolve_acp_read_target(&request_id, "getAcpState", target.as_ref()) {
                        Ok(t) => t,
                        Err(error) => {
                            let mut state = protocol::AcpStateSnapshot::default();
                            state.warnings = vec![format!("target_unsupported: {}", error.message)];
                            let response = Message::acp_state_result(request_id.clone(), state);
                            if let Some(ref sender) = self.response_sender {
                                let _ = sender.try_send(response);
                            }
                            return;
                        }
                    };

                let resolved_target =
                    build_acp_resolved_target(&request_id, "getAcpState", &acp_target);

                let mut state = match &acp_target {
                    AcpReadTarget::Main { .. } => self.collect_acp_state(cx),
                    AcpReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.collect_acp_state_snapshot(cx)
                    }
                };
                state.resolved_target = resolved_target;

                tracing::info!(
                    target: "script_kit::acp_telemetry",
                    category = "ACP_STATE",
                    request_id = %request_id,
                    status = %state.status,
                    cursor_index = state.cursor_index,
                    picker_open = state.picker.as_ref().map_or(false, |p| p.open),
                    message_count = state.message_count,
                    context_ready = state.context_ready,
                    "acp_state.result"
                );

                let response = Message::acp_state_result(request_id.clone(), state);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "ACP_STATE",
                                request_id = %request_id,
                                "acp_state.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "ACP_STATE",
                                request_id = %request_id,
                                "acp_state.response_channel_disconnected"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        category = "ACP_STATE",
                        request_id = %request_id,
                        "acp_state.no_response_sender"
                    );
                }
            }

            PromptMessage::PerformAcpSetupAction {
                request_id,
                action,
                agent_id,
                target,
            } => {
                tracing::info!(
                    category = "ACP_SETUP_ACTION",
                    request_id = %request_id,
                    action = ?action,
                    agent_id = ?agent_id,
                    target = ?target,
                    "acp_setup_action.request"
                );

                // Resolve the ACP target — now accepts both Main and AcpDetached.
                let acp_target = match resolve_acp_read_target(
                    &request_id,
                    "performAcpSetupAction",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let response = Message::acp_setup_action_result_error(
                            request_id.clone(),
                            error.message,
                        );
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                // For Main targets, verify the main window is actually showing AcpChatView.
                if matches!(acp_target, AcpReadTarget::Main { .. }) {
                    if !matches!(self.current_view, AppView::AcpChatView { .. }) {
                        tracing::warn!(
                            target: "script_kit::automation",
                            request_id = %request_id,
                            "automation.acp_action_target_main_view_missing"
                        );
                        let response = Message::acp_setup_action_result_error(
                            request_id.clone(),
                            "performAcpSetupAction resolved the main ACP target but the main window is not currently showing AcpChatView".to_string(),
                        );
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                }

                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    resolved_target = match &acp_target {
                        AcpReadTarget::Main { .. } => "main",
                        AcpReadTarget::Detached { .. } => "detached",
                    },
                    "automation.acp_action_target_resolved"
                );

                let resolved_target =
                    build_acp_resolved_target(&request_id, "performAcpSetupAction", &acp_target);

                // Dispatch the action to the resolved ACP view.
                let result = match acp_target.clone() {
                    AcpReadTarget::Main { .. } => match &self.current_view {
                        AppView::AcpChatView { entity } => entity.update(cx, |view, cx| {
                            view.perform_setup_automation_action(action, agent_id.as_deref(), cx)
                        }),
                        _ => Err("current main view is not AcpChatView".to_string()),
                    },
                    AcpReadTarget::Detached { entity, .. } => entity.update(cx, |view, cx| {
                        view.perform_setup_automation_action(action, agent_id.as_deref(), cx)
                    }),
                };

                let mut state = match &acp_target {
                    AcpReadTarget::Main { .. } => self.collect_acp_state(cx),
                    AcpReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.collect_acp_state_snapshot(cx)
                    }
                };
                state.resolved_target = resolved_target;

                let response = match result {
                    Ok(()) => Message::acp_setup_action_result_success(request_id.clone(), state),
                    Err(error_msg) => {
                        tracing::warn!(
                            category = "ACP_SETUP_ACTION",
                            request_id = %request_id,
                            error = %error_msg,
                            "acp_setup_action.failed"
                        );
                        Message::AcpSetupActionResult {
                            request_id: request_id.clone(),
                            success: false,
                            error: Some(error_msg),
                            state: Some(state),
                        }
                    }
                };

                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                }
            }

            PromptMessage::ResetAcpTestProbe { request_id, target } => {
                tracing::info!(
                    category = "ACP_PROBE",
                    request_id = %request_id,
                    target = ?target,
                    "acp_test_probe.reset"
                );

                // Resolve target: Main → main window, AcpDetached → detached entity,
                // anything else → structured error.
                let acp_target = match resolve_acp_read_target(
                    &request_id,
                    "resetAcpTestProbe",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let mut probe = protocol::AcpTestProbeSnapshot::default();
                        probe.warnings = vec![format!("target_unsupported: {}", error.message)];
                        let response = Message::acp_test_probe_result(request_id.clone(), probe);
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let resolved_target =
                    build_acp_resolved_target(&request_id, "resetAcpTestProbe", &acp_target);

                match &acp_target {
                    AcpReadTarget::Main { .. } => {
                        self.reset_acp_test_probe(cx);
                    }
                    AcpReadTarget::Detached { entity, .. } => {
                        entity.update(cx, |view, _cx| {
                            view.reset_test_probe();
                        });
                    }
                };

                // Respond with the current (now-empty) probe snapshot.
                let mut probe = match &acp_target {
                    AcpReadTarget::Main { .. } => {
                        self.collect_acp_test_probe(protocol::ACP_TEST_PROBE_MAX_EVENTS, cx)
                    }
                    AcpReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.test_probe_snapshot(protocol::ACP_TEST_PROBE_MAX_EVENTS, cx)
                    }
                };
                probe.state.resolved_target = resolved_target;
                let response = Message::acp_test_probe_result(request_id.clone(), probe);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "ACP_PROBE",
                                request_id = %request_id,
                                "acp_test_probe.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "ACP_PROBE",
                                request_id = %request_id,
                                "acp_test_probe.response_channel_disconnected"
                            );
                        }
                    }
                }
            }

            PromptMessage::GetAcpTestProbe {
                request_id,
                tail,
                target,
            } => {
                let tail = tail
                    .unwrap_or(protocol::ACP_TEST_PROBE_MAX_EVENTS)
                    .clamp(1, protocol::ACP_TEST_PROBE_MAX_EVENTS);
                tracing::info!(
                    category = "ACP_PROBE",
                    request_id = %request_id,
                    tail,
                    target = ?target,
                    "acp_test_probe.request"
                );

                // Resolve target: Main → main window, AcpDetached → detached entity,
                // anything else → structured error.
                let acp_target = match resolve_acp_read_target(
                    &request_id,
                    "getAcpTestProbe",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let mut probe = protocol::AcpTestProbeSnapshot::default();
                        probe.warnings = vec![format!("target_unsupported: {}", error.message)];
                        let response = Message::acp_test_probe_result(request_id.clone(), probe);
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let resolved_target =
                    build_acp_resolved_target(&request_id, "getAcpTestProbe", &acp_target);

                let mut probe = match &acp_target {
                    AcpReadTarget::Main { .. } => self.collect_acp_test_probe(tail, cx),
                    AcpReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.test_probe_snapshot(tail, cx)
                    }
                };
                probe.state.resolved_target = resolved_target;
                let response = Message::acp_test_probe_result(request_id.clone(), probe);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "ACP_PROBE",
                                request_id = %request_id,
                                "acp_test_probe.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "ACP_PROBE",
                                request_id = %request_id,
                                "acp_test_probe.response_channel_disconnected"
                            );
                        }
                    }
                }
            }

            PromptMessage::GetElements {
                request_id,
                limit,
                target,
            } => {
                let max_elements = limit.unwrap_or(50).clamp(1, 1000);

                tracing::info!(
                    category = "UI_ELEMENTS",
                    request_id = %request_id,
                    limit = max_elements,
                    target = ?target,
                    "ui.elements.request"
                );

                // Resolve the target and delegate to the appropriate collector.
                // Non-main targets use the secondary-surface collector; main
                // (or absent target) uses the existing main-window collector.
                let resolved_target = target
                    .as_ref()
                    .map(|t| crate::windows::resolve_automation_window(Some(t)));

                let snapshot = match resolved_target {
                    Some(Ok(ref resolved))
                        if resolved.kind != protocol::AutomationWindowKind::Main =>
                    {
                        crate::windows::automation_surface_collector::collect_surface_snapshot(
                            resolved,
                            max_elements,
                            cx,
                        )
                        .unwrap_or_else(|| {
                            crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                                elements: Vec::new(),
                                total_count: 0,
                                focused_semantic_id: None,
                                selected_semantic_id: None,
                                warnings: vec![format!(
                                    "target_unsupported_non_main: getElements has no collector for {} ({:?})",
                                    resolved.id, resolved.kind
                                )],
                                quality: crate::windows::automation_surface_collector::SnapshotQuality::PanelOnly,
                            }
                        })
                    }
                    Some(Err(ref err)) => {
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(Message::elements_result(
                                request_id.clone(),
                                Vec::new(),
                                0,
                                None,
                                None,
                                vec![format!("target_resolution_failed: {}", err)],
                            ));
                        }
                        return;
                    }
                    _ => {
                        // Main window or no target — use existing collector.
                        let outcome = self.collect_visible_elements(max_elements, cx);
                        crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                            total_count: outcome.total_count,
                            focused_semantic_id: outcome.focused_semantic_id(),
                            selected_semantic_id: outcome.selected_semantic_id(),
                            warnings: outcome.warnings.clone(),
                            elements: outcome.elements,
                            quality: crate::windows::automation_surface_collector::SnapshotQuality::Full,
                        }
                    }
                };

                let returned_count = snapshot.elements.len();
                let truncated = snapshot.total_count > returned_count;

                tracing::info!(
                    category = "UI_ELEMENTS",
                    request_id = %request_id,
                    limit = max_elements,
                    returned_count = returned_count,
                    total_count = snapshot.total_count,
                    truncated = truncated,
                    focused_semantic_id = snapshot.focused_semantic_id.as_deref().unwrap_or(""),
                    selected_semantic_id = snapshot.selected_semantic_id.as_deref().unwrap_or(""),
                    warnings = ?snapshot.warnings,
                    "ui.elements.result"
                );

                let response = Message::elements_result(
                    request_id.clone(),
                    snapshot.elements,
                    snapshot.total_count,
                    snapshot.focused_semantic_id,
                    snapshot.selected_semantic_id,
                    snapshot.warnings,
                );

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "UI_ELEMENTS",
                                request_id = %request_id,
                                "ui.elements.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "UI_ELEMENTS",
                                request_id = %request_id,
                                "ui.elements.response_channel_disconnected"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        category = "UI_ELEMENTS",
                        request_id = %request_id,
                        "ui.elements.no_response_sender"
                    );
                }
            }

            PromptMessage::GetLayoutInfo { request_id, target } => {
                tracing::info!(
                    category = "UI",
                    request_id = %request_id,
                    target = ?target,
                    "Collecting layout info for request"
                );

                // Reject non-main targets — layout info is only available
                // for the main window today. Return an empty LayoutInfo
                // with a log message so agents get honest diagnostics.
                if target.is_some() {
                    match resolve_main_only_target(&request_id, "getLayoutInfo", target.as_ref()) {
                        Ok(_resolved) => { /* main window — proceed */ }
                        Err(error) => {
                            tracing::warn!(
                                target: "script_kit::automation",
                                request_id = %request_id,
                                error = %error.message,
                                "getLayoutInfo: target rejected"
                            );
                            let empty_info = crate::protocol::LayoutInfo::default();
                            let response =
                                Message::layout_info_result(request_id.clone(), empty_info);
                            if let Some(ref sender) = self.response_sender {
                                let _ = sender.try_send(response);
                            }
                            return;
                        }
                    }
                }

                // Build layout info from current window state
                let layout_info = self.build_layout_info(cx);

                // Create the response
                let response = Message::layout_info_result(request_id.clone(), layout_info);

                tracing::info!(
                    category = "UI",
                    request_id = %request_id,
                    "Sending layout info result for request"
                );

                // Send the response - use try_send to avoid blocking UI
                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "WARN",
                                "Response channel full - layout info dropped"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "UI",
                                "Response channel disconnected - script exited"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        category = "ERROR",
                        "No response sender available for layout info result"
                    );
                }
            }
            PromptMessage::InspectAutomationWindow {
                request_id,
                target,
                hi_dpi,
                probes,
            } => {
                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    target = ?target,
                    probe_count = probes.len(),
                    "automation.inspect.request"
                );

                // Step 1: Resolve the automation window target.
                let resolved = match crate::windows::resolve_automation_window(target.as_ref()) {
                    Ok(info) => info,
                    Err(err) => {
                        let snapshot = protocol::AutomationInspectSnapshot {
                            schema_version: protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
                            window_id: String::new(),
                            window_kind: "unknown".to_string(),
                            title: None,
                            resolved_bounds: None,
                            target_bounds_in_screenshot: None,
                            surface_hit_point: None,
                            suggested_hit_points: Vec::new(),
                            elements: Vec::new(),
                            total_count: 0,
                            focused_semantic_id: None,
                            selected_semantic_id: None,
                            screenshot_width: None,
                            screenshot_height: None,
                            pixel_probes: Vec::new(),
                            os_window_id: None,
                            semantic_quality: Some(protocol::SemanticQuality::Unavailable),
                            warnings: vec![format!("target_resolution_failed: {}", err)],
                        };
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(Message::automation_inspect_result(
                                request_id.clone(),
                                snapshot,
                            ));
                        }
                        return;
                    }
                };

                // Step 2: Capture RGBA image for dimensions and pixel probes.
                let hi_dpi_mode = hi_dpi.unwrap_or(false);
                let rgba_result =
                    crate::platform::capture_targeted_rgba_image(target.as_ref(), hi_dpi_mode);

                let (shot_w, shot_h, probe_results, mut warnings) = match rgba_result {
                    Ok(ref rgba_image) => {
                        let w = rgba_image.width();
                        let h = rgba_image.height();
                        let mut results = Vec::with_capacity(probes.len());
                        for probe in &probes {
                            if probe.x < w && probe.y < h {
                                let px = rgba_image.get_pixel(probe.x, probe.y);
                                results.push(protocol::PixelProbeResult {
                                    x: probe.x,
                                    y: probe.y,
                                    r: px[0],
                                    g: px[1],
                                    b: px[2],
                                    a: px[3],
                                });
                            }
                        }
                        (Some(w), Some(h), results, Vec::new())
                    }
                    Err(err) => {
                        tracing::warn!(
                            target: "script_kit::automation",
                            request_id = %request_id,
                            error = %err,
                            "automation.inspect.screenshot_failed"
                        );
                        (
                            None,
                            None,
                            Vec::new(),
                            vec![format!("screenshot_capture_failed: {}", err)],
                        )
                    }
                };

                // Step 3: Collect semantic elements via surface-aware collector.
                let (surface_snapshot, semantic_quality) = if resolved.kind
                    == protocol::AutomationWindowKind::Main
                {
                    let outcome = self.collect_visible_elements(200, cx);
                    (
                        crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                            total_count: outcome.total_count,
                            focused_semantic_id: outcome.focused_semantic_id(),
                            selected_semantic_id: outcome.selected_semantic_id(),
                            warnings: outcome.warnings.clone(),
                            elements: outcome.elements,
                            quality:
                                crate::windows::automation_surface_collector::SnapshotQuality::Full,
                        },
                        protocol::SemanticQuality::Full,
                    )
                } else {
                    match crate::windows::automation_surface_collector::collect_surface_snapshot(
                        &resolved, 200, cx,
                    ) {
                        Some(snap) => {
                            let quality = match snap.quality {
                                crate::windows::automation_surface_collector::SnapshotQuality::Full => {
                                    protocol::SemanticQuality::Full
                                }
                                crate::windows::automation_surface_collector::SnapshotQuality::PanelOnly => {
                                    protocol::SemanticQuality::PanelOnly
                                }
                            };
                            (snap, quality)
                        }
                        None => (
                            crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                                elements: Vec::new(),
                                total_count: 0,
                                focused_semantic_id: None,
                                selected_semantic_id: None,
                                warnings: vec![format!(
                                    "semantic_elements_non_main_pending: no collector for {} ({:?})",
                                    resolved.id, resolved.kind
                                )],
                                quality:
                                    crate::windows::automation_surface_collector::SnapshotQuality::PanelOnly,
                            },
                            protocol::SemanticQuality::Unavailable,
                        ),
                    }
                };
                warnings.extend(surface_snapshot.warnings.clone());
                let elements = surface_snapshot.elements;
                let total_count = surface_snapshot.total_count;
                let focused_semantic_id = surface_snapshot.focused_semantic_id;
                let selected_semantic_id = surface_snapshot.selected_semantic_id;

                // Step 4: Resolve the native OS window ID (CGWindowID) for
                // strict screenshot capture threading.
                let os_window_id = crate::platform::resolve_targeted_os_window_id(target.as_ref());

                // Step 5: Compute screenshot-relative geometry for the target surface.
                let target_bounds_in_screenshot = protocol::target_bounds_in_screenshot(&resolved);
                let surface_hit_point = target_bounds_in_screenshot
                    .as_ref()
                    .map(protocol::default_surface_hit_point);
                let suggested_hit_points = protocol::default_suggested_hit_points(
                    &resolved,
                    target_bounds_in_screenshot.as_ref(),
                );

                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    window_id = %resolved.id,
                    target_bounds_in_screenshot = ?target_bounds_in_screenshot,
                    suggested_hit_count = suggested_hit_points.len(),
                    "automation.inspect.geometry_computed"
                );

                let snapshot = protocol::AutomationInspectSnapshot {
                    schema_version: protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
                    window_id: resolved.id.clone(),
                    window_kind: format!("{:?}", resolved.kind),
                    title: resolved.title.clone(),
                    resolved_bounds: resolved.bounds.clone(),
                    target_bounds_in_screenshot,
                    surface_hit_point,
                    suggested_hit_points,
                    elements,
                    total_count,
                    focused_semantic_id,
                    selected_semantic_id,
                    screenshot_width: shot_w,
                    screenshot_height: shot_h,
                    pixel_probes: probe_results,
                    os_window_id,
                    semantic_quality: Some(semantic_quality),
                    warnings,
                };

                tracing::info!(
                    target: "script_kit::automation",
                    event = "inspect_automation_window",
                    request_id = %request_id,
                    window_id = %resolved.id,
                    window_kind = %snapshot.window_kind,
                    os_window_id = ?os_window_id,
                    screenshot_width = ?snapshot.screenshot_width,
                    screenshot_height = ?snapshot.screenshot_height,
                    element_count = snapshot.elements.len(),
                    warning_count = snapshot.warnings.len(),
                    "automation.inspect.result"
                );

                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(Message::automation_inspect_result(
                        request_id.clone(),
                        snapshot,
                    ));
                }
            }

            PromptMessage::WaitFor {
                request_id,
                condition,
                timeout,
                poll_interval,
                trace: trace_mode,
                target,
            } => {
                let timeout_ms = timeout.unwrap_or(5_000);
                let poll_ms = poll_interval.unwrap_or(25);
                let rid = request_id.clone();

                // Determine if this is an ACP-specific condition.
                let is_acp_condition = matches!(
                    &condition,
                    protocol::WaitCondition::Detailed(
                        protocol::WaitDetailedCondition::AcpReady
                            | protocol::WaitDetailedCondition::AcpPickerOpen
                            | protocol::WaitDetailedCondition::AcpPickerClosed
                            | protocol::WaitDetailedCondition::AcpItemAccepted
                            | protocol::WaitDetailedCondition::AcpCursorAt { .. }
                            | protocol::WaitDetailedCondition::AcpStatus { .. }
                            | protocol::WaitDetailedCondition::AcpInputMatch { .. }
                            | protocol::WaitDetailedCondition::AcpInputContains { .. }
                            | protocol::WaitDetailedCondition::AcpAcceptedViaKey { .. }
                            | protocol::WaitDetailedCondition::AcpAcceptedLabel { .. }
                            | protocol::WaitDetailedCondition::AcpAcceptedCursorAt { .. }
                            | protocol::WaitDetailedCondition::AcpInputLayoutMatch { .. }
                            | protocol::WaitDetailedCondition::AcpSetupVisible
                            | protocol::WaitDetailedCondition::AcpSetupReasonCode { .. }
                            | protocol::WaitDetailedCondition::AcpSetupPrimaryAction { .. }
                            | protocol::WaitDetailedCondition::AcpSetupAgentPickerOpen
                            | protocol::WaitDetailedCondition::AcpSetupSelectedAgent { .. }
                    )
                );

                // Resolve target: ACP conditions accept AcpDetached; generic
                // conditions accept Main, AcpDetached, and Notes.
                let resolved_target: AutomationReadTarget = if target.is_some() {
                    if is_acp_condition {
                        match resolve_acp_read_target(&rid, "waitFor", target.as_ref()) {
                            Ok(AcpReadTarget::Detached { entity, info }) => {
                                AutomationReadTarget::AcpDetached { entity, info }
                            }
                            Ok(AcpReadTarget::Main { info }) => AutomationReadTarget::Main { info },
                            Err(error) => {
                                if let Some(ref sender) = self.response_sender {
                                    let _ = sender.try_send(Message::wait_for_result(
                                        request_id.clone(),
                                        false,
                                        0,
                                        Some(error),
                                    ));
                                }
                                return;
                            }
                        }
                    } else {
                        match resolve_automation_read_target(&rid, "waitFor", target.as_ref(), cx) {
                            Ok(resolved) => resolved,
                            Err(error) => {
                                if let Some(ref sender) = self.response_sender {
                                    let _ = sender.try_send(Message::wait_for_result(
                                        request_id.clone(),
                                        false,
                                        0,
                                        Some(error),
                                    ));
                                }
                                return;
                            }
                        }
                    }
                } else {
                    AutomationReadTarget::Main { info: None }
                };

                // Extract the detached ACP entity for backward-compatible condition checking.
                let detached_entity: Option<gpui::Entity<crate::ai::acp::view::AcpChatView>> =
                    if let AutomationReadTarget::AcpDetached { ref entity, .. } = resolved_target {
                        Some(entity.clone())
                    } else {
                        None
                    };

                tracing::info!(
                    category = "AUTOMATION",
                    request_id = %rid,
                    timeout_ms = timeout_ms,
                    poll_ms = poll_ms,
                    trace_mode = ?trace_mode,
                    "automation.wait_for.started"
                );

                // Check if condition is already satisfied
                let already_satisfied = match &resolved_target {
                    AutomationReadTarget::Notes { entity, .. } => {
                        notes_wait_condition_satisfied(entity, &condition, cx)
                    }
                    _ => self.wait_condition_satisfied_for_target(
                        &condition,
                        detached_entity.as_ref(),
                        cx,
                    ),
                };
                if already_satisfied {
                    let include_trace =
                        protocol::transaction_trace::should_include_trace(trace_mode, true);
                    let trace = if include_trace {
                        let started_at_ms = protocol::transaction_trace::now_epoch_ms();
                        Some(protocol::TransactionTrace {
                            request_id: rid.clone(),
                            status: protocol::TransactionTraceStatus::Ok,
                            started_at_ms,
                            total_elapsed_ms: 0,
                            failed_at: None,
                            commands: vec![protocol::TransactionCommandTrace {
                                index: 0,
                                command: "waitFor".to_string(),
                                started_at_ms,
                                elapsed_ms: 0,
                                before: protocol::UiStateSnapshot::default(),
                                after: protocol::UiStateSnapshot::default(),
                                polls: Vec::new(),
                                error: None,
                            }],
                        })
                    } else {
                        None
                    };
                    tracing::info!(
                        category = "AUTOMATION",
                        request_id = %rid,
                        success = true,
                        elapsed_ms = 0_u64,
                        error_code = "",
                        trace_included = include_trace,
                        "automation.wait_for.completed"
                    );
                    let response = Message::wait_for_result_with_trace(
                        request_id.clone(),
                        true,
                        0,
                        None::<crate::protocol::TransactionError>,
                        trace,
                    );
                    if let Some(ref sender) = self.response_sender {
                        let _ = sender.try_send(response);
                    }
                } else {
                    // Poll asynchronously
                    let sender = self.response_sender.clone();
                    let condition = condition.clone();
                    let detached_entity = detached_entity.clone();
                    let notes_entity: Option<gpui::Entity<crate::notes::NotesApp>> =
                        if let AutomationReadTarget::Notes { ref entity, .. } = resolved_target {
                            Some(entity.clone())
                        } else {
                            None
                        };
                    cx.spawn(async move |this, cx| {
                        let started_at_ms = protocol::transaction_trace::now_epoch_ms();
                        let start = std::time::Instant::now();
                        let timeout_dur = std::time::Duration::from_millis(timeout_ms);
                        let poll_dur = std::time::Duration::from_millis(poll_ms);
                        loop {
                            cx.background_executor().timer(poll_dur).await;
                            if start.elapsed() >= timeout_dur {
                                let elapsed_ms = start.elapsed().as_millis() as u64;
                                let error = crate::protocol::TransactionError {
                                    code:
                                        crate::protocol::TransactionErrorCode::WaitConditionTimeout,
                                    message: format!(
                                        "Timeout after {}ms waiting for {:?}",
                                        timeout_ms, condition
                                    ),
                                    suggestion: None,
                                };
                                let include_trace =
                                    protocol::transaction_trace::should_include_trace(
                                        trace_mode, false,
                                    );
                                let trace = if include_trace {
                                    Some(protocol::TransactionTrace {
                                        request_id: rid.clone(),
                                        status: protocol::TransactionTraceStatus::Timeout,
                                        started_at_ms,
                                        total_elapsed_ms: elapsed_ms,
                                        failed_at: Some(0),
                                        commands: vec![protocol::TransactionCommandTrace {
                                            index: 0,
                                            command: "waitFor".to_string(),
                                            started_at_ms,
                                            elapsed_ms,
                                            before: protocol::UiStateSnapshot::default(),
                                            after: protocol::UiStateSnapshot::default(),
                                            polls: Vec::new(),
                                            error: Some(error.clone()),
                                        }],
                                    })
                                } else {
                                    None
                                };
                                tracing::info!(
                                    category = "AUTOMATION",
                                    request_id = %rid,
                                    success = false,
                                    elapsed_ms = elapsed_ms,
                                    error_code = "wait_condition_timeout",
                                    trace_included = include_trace,
                                    "automation.wait_for.completed"
                                );
                                if let Some(ref s) = sender {
                                    let _ = s.try_send(Message::wait_for_result_with_trace(
                                        rid.clone(),
                                        false,
                                        elapsed_ms,
                                        Some(error),
                                        trace,
                                    ));
                                }
                                break;
                            }
                            let poll_result = if let Some(ref notes_ent) = notes_entity {
                                let ne = notes_ent.clone();
                                this.update(cx, |_this, cx| {
                                    notes_wait_condition_satisfied(&ne, &condition, cx)
                                })
                            } else {
                                this.update(cx, |this, cx| {
                                    this.wait_condition_satisfied_for_target(
                                        &condition,
                                        detached_entity.as_ref(),
                                        cx,
                                    )
                                })
                            };
                            match poll_result {
                                Ok(true) => {
                                    let elapsed_ms = start.elapsed().as_millis() as u64;
                                    let include_trace =
                                        protocol::transaction_trace::should_include_trace(
                                            trace_mode, true,
                                        );
                                    let trace = if include_trace {
                                        Some(protocol::TransactionTrace {
                                            request_id: rid.clone(),
                                            status: protocol::TransactionTraceStatus::Ok,
                                            started_at_ms,
                                            total_elapsed_ms: elapsed_ms,
                                            failed_at: None,
                                            commands: vec![protocol::TransactionCommandTrace {
                                                index: 0,
                                                command: "waitFor".to_string(),
                                                started_at_ms,
                                                elapsed_ms,
                                                before: protocol::UiStateSnapshot::default(),
                                                after: protocol::UiStateSnapshot::default(),
                                                polls: Vec::new(),
                                                error: None,
                                            }],
                                        })
                                    } else {
                                        None
                                    };
                                    tracing::info!(
                                        category = "AUTOMATION",
                                        request_id = %rid,
                                        success = true,
                                        elapsed_ms = elapsed_ms,
                                        error_code = "",
                                        trace_included = include_trace,
                                        "automation.wait_for.completed"
                                    );
                                    if let Some(ref s) = sender {
                                        let _ = s.try_send(Message::wait_for_result_with_trace(
                                            rid.clone(),
                                            true,
                                            elapsed_ms,
                                            None::<crate::protocol::TransactionError>,
                                            trace,
                                        ));
                                    }
                                    break;
                                }
                                Ok(false) => continue,
                                Err(_) => {
                                    let elapsed_ms = start.elapsed().as_millis() as u64;
                                    let error = crate::protocol::TransactionError {
                                        code: crate::protocol::TransactionErrorCode::ActionFailed,
                                        message: "Entity dropped during WaitFor".to_string(),
                                        suggestion: None,
                                    };
                                    let include_trace =
                                        protocol::transaction_trace::should_include_trace(
                                            trace_mode, false,
                                        );
                                    let trace = if include_trace {
                                        Some(protocol::TransactionTrace {
                                            request_id: rid.clone(),
                                            status: protocol::TransactionTraceStatus::Failed,
                                            started_at_ms,
                                            total_elapsed_ms: elapsed_ms,
                                            failed_at: Some(0),
                                            commands: vec![protocol::TransactionCommandTrace {
                                                index: 0,
                                                command: "waitFor".to_string(),
                                                started_at_ms,
                                                elapsed_ms,
                                                before: protocol::UiStateSnapshot::default(),
                                                after: protocol::UiStateSnapshot::default(),
                                                polls: Vec::new(),
                                                error: Some(error.clone()),
                                            }],
                                        })
                                    } else {
                                        None
                                    };
                                    tracing::info!(
                                        category = "AUTOMATION",
                                        request_id = %rid,
                                        success = false,
                                        elapsed_ms = elapsed_ms,
                                        error_code = "action_failed",
                                        trace_included = include_trace,
                                        "automation.wait_for.completed"
                                    );
                                    if let Some(ref s) = sender {
                                        let _ = s.try_send(Message::wait_for_result_with_trace(
                                            rid.clone(),
                                            false,
                                            elapsed_ms,
                                            Some(error),
                                            trace,
                                        ));
                                    }
                                    break;
                                }
                            }
                        }
                    })
                    .detach();
                }
            }

            PromptMessage::Batch {
                request_id,
                commands,
                options,
                trace: trace_mode,
                target,
            } => {
                let opts = options.unwrap_or(protocol::BatchOptions {
                    stop_on_error: true,
                    rollback_on_error: false,
                    timeout: 5_000,
                });
                let rid = request_id.clone();
                let sender = self.response_sender.clone();

                // Resolve target: accept Main, AcpDetached, and Notes.
                let batch_target: AutomationReadTarget = if target.is_some() {
                    match resolve_automation_read_target(&rid, "batch", target.as_ref(), cx) {
                        Ok(resolved) => resolved,
                        Err(error) => {
                            if let Some(ref sender) = self.response_sender {
                                let _ = sender.try_send(Message::batch_result(
                                    request_id.clone(),
                                    false,
                                    vec![crate::protocol::BatchResultEntry {
                                        index: 0,
                                        success: false,
                                        command: "batch".to_string(),
                                        elapsed: Some(0),
                                        value: None,
                                        error: Some(error),
                                    }],
                                    Some(0),
                                    0,
                                ));
                            }
                            return;
                        }
                    }
                } else {
                    AutomationReadTarget::Main { info: None }
                };

                let detached_batch_entity: Option<gpui::Entity<crate::ai::acp::view::AcpChatView>> =
                    if let AutomationReadTarget::AcpDetached { ref entity, .. } = batch_target {
                        Some(entity.clone())
                    } else {
                        None
                    };

                let notes_batch_target: Option<(
                    gpui::Entity<crate::notes::NotesApp>,
                    gpui::WindowHandle<crate::Root>,
                )> = if let AutomationReadTarget::Notes {
                    ref entity,
                    ref handle,
                    ..
                } = batch_target
                {
                    Some((entity.clone(), *handle))
                } else {
                    None
                };

                let actions_dialog_batch_entity: Option<
                    gpui::Entity<crate::actions::ActionsDialog>,
                > = if let AutomationReadTarget::ActionsDialog { ref entity, .. } = batch_target {
                    Some(entity.clone())
                } else {
                    None
                };

                let is_prompt_popup_batch =
                    matches!(batch_target, AutomationReadTarget::PromptPopup { .. });

                tracing::info!(
                    category = "AUTOMATION",
                    request_id = %rid,
                    command_count = commands.len(),
                    trace_mode = ?trace_mode,
                    target = ?target,
                    "automation.batch.started"
                );

                cx.spawn(async move |this, cx| {
                    // ── Detached ACP batch path ──────────────────────────
                    // When targeting a detached ACP entity, route commands
                    // to it instead of the main window. The command set is
                    // limited to setInput, waitFor, selectByValue, and
                    // selectBySemanticId.
                    if let Some(acp_entity) = detached_batch_entity {
                        let batch_start = std::time::Instant::now();
                        let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                        let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                        let mut failed = false;

                        for (index, cmd) in commands.iter().enumerate() {
                            if batch_start.elapsed() >= batch_timeout {
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command: batch_command_name(cmd),
                                    elapsed: Some(0),
                                    value: None,
                                    error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                                });
                                failed = true;
                                break;
                            }

                            let cmd_start = std::time::Instant::now();
                            match cmd {
                                protocol::BatchCommand::SetInput { text } => {
                                    let text = text.clone();
                                    let acp_entity = acp_entity.clone();
                                    let result = this.update(cx, |_this, cx| {
                                        acp_entity.update(cx, |view, cx| {
                                            let Some(thread) = view.thread() else {
                                                return "detached ACP is in setup mode".to_string();
                                            };
                                            thread.update(cx, |thread, cx| {
                                                thread.set_input(&text, cx);
                                            });
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_detached_acp_set_input",
                                                text_len = text.len(),
                                                "detached ACP set_input"
                                            );
                                            String::new() // empty = success
                                        })
                                    });
                                    match result {
                                        Ok(err) if err.is_empty() => {
                                            tracing::info!(category = "BATCH", request_id = %rid, index, command = "setInput", "batch.detached_acp.step.ok");
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None, error: None,
                                            });
                                        }
                                        Ok(err) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(err)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::SelectByValue { value, submit } => {
                                    let submit = *submit;
                                    let value = value.clone();
                                    let acp_entity = acp_entity.clone();
                                    // Returns Option<String>: Some(matched) or None if not found.
                                    let selected = this.update(cx, |_this, cx| {
                                        acp_entity.update(cx, |view, _cx| -> Option<String> {
                                            let session = view.mention_session.as_ref()?;
                                            let idx = session.items.iter().position(|item| {
                                                item.label.as_ref() == value || item.id.as_ref() == value
                                            })?;
                                            view.select_mention_index(idx);
                                            Some(value.clone())
                                        })
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            if submit {
                                                let acp_entity2 = acp_entity.clone();
                                                let _ = this.update(cx, |_this, cx| {
                                                    acp_entity2.update(cx, |view, cx| {
                                                        view.accept_mention_selection(cx);
                                                    });
                                                });
                                            }
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_detached_acp_select_by_value",
                                                value = %v, submit,
                                                "detached ACP select_by_value"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::selection_not_found(
                                                    format!("selectByValue could not find '{value}' in detached ACP picker")
                                                )),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::SelectBySemanticId { semantic_id, submit } => {
                                    let submit = *submit;
                                    let semantic_id = semantic_id.clone();
                                    let acp_entity = acp_entity.clone();
                                    let selected = this.update(cx, |_this, cx| {
                                        acp_entity.update(cx, |view, _cx| -> Option<String> {
                                            let session = view.mention_session.as_ref()?;
                                            let idx = session.items.iter().position(|item| {
                                                item.label.as_ref() == semantic_id || item.id.as_ref() == semantic_id
                                            })?;
                                            view.select_mention_index(idx);
                                            Some(semantic_id.clone())
                                        })
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            if submit {
                                                let acp_entity2 = acp_entity.clone();
                                                let _ = this.update(cx, |_this, cx| {
                                                    acp_entity2.update(cx, |view, cx| {
                                                        view.accept_mention_selection(cx);
                                                    });
                                                });
                                            }
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_detached_acp_select_by_value",
                                                value = %v, submit,
                                                "detached ACP select_by_semantic_id"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::selection_not_found(
                                                    format!("selectBySemanticId could not find '{semantic_id}' in detached ACP picker")
                                                )),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } => {
                                    let wait_timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000));
                                    let wait_poll = std::time::Duration::from_millis(poll_interval.unwrap_or(25));
                                    let wait_start = std::time::Instant::now();
                                    let acp_entity_ref = &acp_entity;

                                    let already = this.update(cx, |this, cx| {
                                        this.wait_condition_satisfied_for_target(condition, Some(acp_entity_ref), cx)
                                    });
                                    match already {
                                        Ok(true) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "waitFor".to_string(),
                                                elapsed: Some(0), value: None, error: None,
                                            });
                                        }
                                        Ok(false) => {
                                            let mut wait_result: Result<Option<String>, protocol::TransactionError> =
                                                Err(protocol::TransactionError::wait_timeout(
                                                    format!("WaitFor timeout after {}ms", wait_timeout.as_millis())
                                                ));
                                            loop {
                                                cx.background_executor().timer(wait_poll).await;
                                                if wait_start.elapsed() >= wait_timeout { break; }
                                                match this.update(cx, |this, cx| {
                                                    this.wait_condition_satisfied_for_target(condition, Some(acp_entity_ref), cx)
                                                }) {
                                                    Ok(true) => { wait_result = Ok(None); break; }
                                                    Ok(false) => continue,
                                                    _ => {
                                                        wait_result = Err(protocol::TransactionError::action_failed(
                                                            "Entity dropped during WaitFor"
                                                        ));
                                                        break;
                                                    }
                                                }
                                            }
                                            match wait_result {
                                                Ok(_) => {
                                                    tracing::info!(
                                                        target: "script_kit::transaction",
                                                        event = "transaction_wait_complete",
                                                        request_id = %rid,
                                                        index,
                                                        target = "acpDetached",
                                                        "batch.detached_acp.wait.ok"
                                                    );
                                                    results.push(protocol::BatchResultEntry {
                                                        index, success: true, command: "waitFor".to_string(),
                                                        elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                        value: None, error: None,
                                                    });
                                                }
                                                Err(e) => {
                                                    tracing::info!(category = "BATCH", request_id = %rid, index, command = "waitFor", error = %e.message, "batch.detached_acp.step.error");
                                                    results.push(protocol::BatchResultEntry {
                                                        index, success: false, command: "waitFor".to_string(),
                                                        elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                        value: None, error: Some(e),
                                                    });
                                                    failed = true;
                                                    if opts.stop_on_error { break; }
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "waitFor".to_string(),
                                                elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed("Entity dropped")),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                _ => {
                                    // Unsupported commands for detached ACP
                                    results.push(protocol::BatchResultEntry {
                                        index,
                                        success: false,
                                        command: batch_command_name(cmd),
                                        elapsed: Some(0),
                                        value: None,
                                        error: Some(protocol::TransactionError {
                                            code: protocol::TransactionErrorCode::UnsupportedCommand,
                                            message: format!(
                                                "{} is not supported for detached ACP batch targets",
                                                batch_command_name(cmd)
                                            ),
                                            suggestion: Some(
                                                "Detached ACP batch supports: setInput, waitFor, selectByValue, selectBySemanticId."
                                                    .to_string(),
                                            ),
                                        }),
                                    });
                                    failed = true;
                                    if opts.stop_on_error { break; }
                                }
                            }
                        }

                        let total_elapsed = batch_start.elapsed().as_millis() as u64;
                        let success = !failed;
                        let failed_at = if failed {
                            results.iter().position(|r| !r.success)
                        } else {
                            None
                        };

                        let include_trace = protocol::transaction_trace::should_include_trace(trace_mode, success);
                        let trace = if include_trace {
                            let started_at_ms = protocol::transaction_trace::now_epoch_ms()
                                .saturating_sub(total_elapsed);
                            Some(protocol::TransactionTrace {
                                request_id: rid.clone(),
                                status: if success {
                                    protocol::TransactionTraceStatus::Ok
                                } else {
                                    protocol::TransactionTraceStatus::Failed
                                },
                                started_at_ms,
                                total_elapsed_ms: total_elapsed,
                                failed_at,
                                commands: results.iter().map(|r| {
                                    protocol::TransactionCommandTrace {
                                        index: r.index,
                                        command: r.command.clone(),
                                        started_at_ms,
                                        elapsed_ms: r.elapsed.unwrap_or(0),
                                        before: protocol::UiStateSnapshot::default(),
                                        after: protocol::UiStateSnapshot::default(),
                                        polls: Vec::new(),
                                        error: r.error.clone(),
                                    }
                                }).collect(),
                            })
                        } else {
                            None
                        };

                        tracing::info!(
                            category = "AUTOMATION",
                            request_id = %rid,
                            success,
                            total_elapsed_ms = total_elapsed,
                            failed_at = ?failed_at,
                            target = "acpDetached",
                            trace_included = include_trace,
                            "automation.batch.detached_acp.completed"
                        );

                        if let Some(ref s) = sender {
                            let _ = s.try_send(Message::batch_result_with_trace(
                                rid.clone(), success, results, failed_at, total_elapsed, trace,
                            ));
                        }
                        return;
                    }

                    // ── Notes batch path ─────────────────────────────────
                    // When targeting the Notes window, route setInput and
                    // waitFor commands to the Notes entity.
                    if let Some((notes_entity, notes_handle)) = notes_batch_target {
                        let batch_start = std::time::Instant::now();
                        let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                        let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                        let mut failed = false;

                        for (index, cmd) in commands.iter().enumerate() {
                            if batch_start.elapsed() >= batch_timeout {
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command: batch_command_name(cmd),
                                    elapsed: Some(0),
                                    value: None,
                                    error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                                });
                                failed = true;
                                break;
                            }

                            let cmd_start = std::time::Instant::now();
                            match cmd {
                                protocol::BatchCommand::SetInput { text } => {
                                    let text = text.clone();
                                    let ne = notes_entity.clone();
                                    let nh = notes_handle;
                                    let result = nh.update(cx, |_root, window, cx| {
                                        ne.update(cx, |app, cx| {
                                            app.editor_state.update(cx, |state, inner_cx| {
                                                state.set_value(text.clone(), window, inner_cx);
                                            });
                                        });
                                        tracing::info!(
                                            target: "script_kit::transaction",
                                            event = "transaction_notes_set_input",
                                            text_len = text.len(),
                                            "Notes set_input"
                                        );
                                    });
                                    match result {
                                        Ok(()) => {
                                            tracing::info!(category = "BATCH", request_id = %rid, index, command = "setInput", "batch.notes.step.ok");
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None, error: None,
                                            });
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } => {
                                    let wait_timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000));
                                    let wait_poll = std::time::Duration::from_millis(poll_interval.unwrap_or(25));
                                    let wait_start = std::time::Instant::now();
                                    let ne = notes_entity.clone();

                                    let already = this.update(cx, |_this, cx| {
                                        notes_wait_condition_satisfied(&ne, condition, cx)
                                    });
                                    match already {
                                        Ok(true) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "waitFor".to_string(),
                                                elapsed: Some(0), value: None, error: None,
                                            });
                                        }
                                        Ok(false) => {
                                            let mut wait_result: Result<Option<String>, protocol::TransactionError> =
                                                Err(protocol::TransactionError::wait_timeout(
                                                    format!("WaitFor timeout after {}ms", wait_timeout.as_millis())
                                                ));
                                            loop {
                                                cx.background_executor().timer(wait_poll).await;
                                                if wait_start.elapsed() >= wait_timeout { break; }
                                                let ne2 = ne.clone();
                                                match this.update(cx, |_this, cx| {
                                                    notes_wait_condition_satisfied(&ne2, condition, cx)
                                                }) {
                                                    Ok(true) => { wait_result = Ok(None); break; }
                                                    Ok(false) => continue,
                                                    _ => {
                                                        wait_result = Err(protocol::TransactionError::action_failed(
                                                            "Entity dropped during WaitFor"
                                                        ));
                                                        break;
                                                    }
                                                }
                                            }
                                            match wait_result {
                                                Ok(_) => {
                                                    tracing::info!(
                                                        target: "script_kit::transaction",
                                                        event = "transaction_notes_wait_complete",
                                                        request_id = %rid,
                                                        index,
                                                        target = "notes",
                                                        "batch.notes.wait.ok"
                                                    );
                                                    results.push(protocol::BatchResultEntry {
                                                        index, success: true, command: "waitFor".to_string(),
                                                        elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                        value: None, error: None,
                                                    });
                                                }
                                                Err(e) => {
                                                    tracing::info!(category = "BATCH", request_id = %rid, index, command = "waitFor", error = %e.message, "batch.notes.step.error");
                                                    results.push(protocol::BatchResultEntry {
                                                        index, success: false, command: "waitFor".to_string(),
                                                        elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                        value: None, error: Some(e),
                                                    });
                                                    failed = true;
                                                    if opts.stop_on_error { break; }
                                                }
                                            }
                                        }
                                        Err(_) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "waitFor".to_string(),
                                                elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed("Entity dropped")),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                _ => {
                                    // Unsupported commands for Notes
                                    results.push(protocol::BatchResultEntry {
                                        index,
                                        success: false,
                                        command: batch_command_name(cmd),
                                        elapsed: Some(0),
                                        value: None,
                                        error: Some(protocol::TransactionError {
                                            code: protocol::TransactionErrorCode::UnsupportedCommand,
                                            message: format!(
                                                "{} is not supported for Notes batch targets",
                                                batch_command_name(cmd)
                                            ),
                                            suggestion: Some(
                                                "Notes batch supports: setInput, waitFor."
                                                    .to_string(),
                                            ),
                                        }),
                                    });
                                    failed = true;
                                    if opts.stop_on_error { break; }
                                }
                            }
                        }

                        let total_elapsed = batch_start.elapsed().as_millis() as u64;
                        let success = !failed;
                        let failed_at = if failed {
                            results.iter().position(|r| !r.success)
                        } else {
                            None
                        };

                        let include_trace = protocol::transaction_trace::should_include_trace(trace_mode, success);
                        let trace = if include_trace {
                            let started_at_ms = protocol::transaction_trace::now_epoch_ms()
                                .saturating_sub(total_elapsed);
                            Some(protocol::TransactionTrace {
                                request_id: rid.clone(),
                                status: if success {
                                    protocol::TransactionTraceStatus::Ok
                                } else {
                                    protocol::TransactionTraceStatus::Failed
                                },
                                started_at_ms,
                                total_elapsed_ms: total_elapsed,
                                failed_at,
                                commands: results.iter().map(|r| {
                                    protocol::TransactionCommandTrace {
                                        index: r.index,
                                        command: r.command.clone(),
                                        started_at_ms,
                                        elapsed_ms: r.elapsed.unwrap_or(0),
                                        before: protocol::UiStateSnapshot::default(),
                                        after: protocol::UiStateSnapshot::default(),
                                        polls: Vec::new(),
                                        error: r.error.clone(),
                                    }
                                }).collect(),
                            })
                        } else {
                            None
                        };

                        tracing::info!(
                            category = "AUTOMATION",
                            request_id = %rid,
                            success,
                            total_elapsed_ms = total_elapsed,
                            failed_at = ?failed_at,
                            target = "notes",
                            trace_included = include_trace,
                            "automation.batch.notes.completed"
                        );

                        if let Some(ref s) = sender {
                            let _ = s.try_send(Message::batch_result_with_trace(
                                rid.clone(), success, results, failed_at, total_elapsed, trace,
                            ));
                        }
                        return;
                    }

                    // ── ActionsDialog batch path ────────────────────────
                    // When targeting the ActionsDialog popup, route setInput,
                    // selectByValue, selectBySemanticId, and waitFor commands
                    // to the dialog entity. Unsupported commands fail closed.
                    if let Some(dialog_entity) = actions_dialog_batch_entity {
                        let batch_start = std::time::Instant::now();
                        let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                        let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                        let mut failed = false;

                        for (index, cmd) in commands.iter().enumerate() {
                            if batch_start.elapsed() >= batch_timeout {
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command: batch_command_name(cmd),
                                    elapsed: Some(0),
                                    value: None,
                                    error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                                });
                                failed = true;
                                break;
                            }

                            let cmd_start = std::time::Instant::now();
                            match cmd {
                                protocol::BatchCommand::SetInput { text } => {
                                    let text = text.clone();
                                    let de = dialog_entity.clone();
                                    let result = this.update(cx, |_this, cx| {
                                        de.update(cx, |dialog, cx| {
                                            dialog.set_search_text(text.clone(), cx);
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_actions_dialog_set_input",
                                                text_len = text.len(),
                                                "ActionsDialog set_input"
                                            );
                                            String::new()
                                        })
                                    });
                                    match result {
                                        Ok(err) if err.is_empty() => {
                                            tracing::info!(category = "BATCH", request_id = %rid, index, command = "setInput", "batch.actions_dialog.step.ok");
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None, error: None,
                                            });
                                        }
                                        Ok(err) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(err)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "setInput".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::SelectByValue { value, submit: _ } => {
                                    let value = value.clone();
                                    let de = dialog_entity.clone();
                                    let selected = this.update(cx, |_this, cx| {
                                        de.update(cx, |dialog, cx| -> Option<String> {
                                            dialog.select_action_by_id(&value, cx)
                                        })
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_actions_dialog_select_by_value",
                                                value = %v,
                                                "ActionsDialog select_by_value"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::selection_not_found(&value)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::SelectBySemanticId { semantic_id, submit: _ } => {
                                    let semantic_id = semantic_id.clone();
                                    let de = dialog_entity.clone();
                                    let selected = this.update(cx, |_this, cx| {
                                        de.update(cx, |dialog, cx| -> Option<String> {
                                            dialog.select_action_by_semantic_id(&semantic_id, cx)
                                        })
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_actions_dialog_select_by_semantic_id",
                                                semantic_id = %v,
                                                "ActionsDialog select_by_semantic_id"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::element_not_found(&semantic_id)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } => {
                                    let wait_timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000));
                                    let wait_poll = std::time::Duration::from_millis(poll_interval.unwrap_or(25));
                                    let wait_start = std::time::Instant::now();

                                    let already = this.update(cx, |this, cx| {
                                        this.wait_condition_satisfied(condition, cx)
                                    });

                                    let mut wait_result: Result<Option<String>, protocol::TransactionError> = match already {
                                        Ok(true) => Ok(None),
                                        Ok(false) => Err(protocol::TransactionError::wait_timeout("not yet")),
                                        Err(ref e) => Err(protocol::TransactionError::action_failed(format!("{e}"))),
                                    };

                                    if wait_result.is_err() && matches!(already, Ok(false)) {
                                        loop {
                                            cx.background_executor().timer(wait_poll).await;
                                            if wait_start.elapsed() >= wait_timeout {
                                                wait_result = Err(protocol::TransactionError::wait_timeout(
                                                    &format!("Timeout after {}ms", wait_timeout.as_millis()),
                                                ));
                                                break;
                                            }
                                            match this.update(cx, |this, cx| this.wait_condition_satisfied(condition, cx)) {
                                                Ok(true) => {
                                                    wait_result = Ok(None);
                                                    break;
                                                }
                                                Ok(false) => continue,
                                                Err(e) => {
                                                    wait_result = Err(protocol::TransactionError::action_failed(format!("{e}")));
                                                    break;
                                                }
                                            }
                                        }
                                    }

                                    match wait_result {
                                        Ok(_) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_actions_dialog_wait_complete",
                                                request_id = %rid,
                                                index,
                                                target = "actionsDialog",
                                                "batch.actions_dialog.wait.ok"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "waitFor".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None, error: None,
                                            });
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "waitFor".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(e),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                other => {
                                    let cmd_name = batch_command_name(other);
                                    tracing::warn!(
                                        target: "script_kit::transaction",
                                        event = "transaction_actions_dialog_unsupported",
                                        command = %cmd_name,
                                        "ActionsDialog batch: unsupported command"
                                    );
                                    results.push(protocol::BatchResultEntry {
                                        index,
                                        success: false,
                                        command: cmd_name,
                                        elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                        value: None,
                                        error: Some(protocol::TransactionError {
                                            code: protocol::TransactionErrorCode::UnsupportedCommand,
                                            message: format!(
                                                "ActionsDialog batch supports: setInput, selectByValue, selectBySemanticId, waitFor. Got: {}",
                                                batch_command_name(other)
                                            ),
                                            suggestion: Some("Use a supported command for ActionsDialog targets.".to_string()),
                                        }),
                                    });
                                    failed = true;
                                    if opts.stop_on_error { break; }
                                }
                            }
                        }

                        let success = !failed;
                        let failed_at = if failed { results.iter().position(|r| !r.success) } else { None };
                        let total_elapsed = batch_start.elapsed().as_millis() as u64;
                        let include_trace = matches!(trace_mode, protocol::TransactionTraceMode::On)
                            || (matches!(trace_mode, protocol::TransactionTraceMode::OnFailure) && !success);
                        let started_at_ms = 0u64;
                        let trace = if include_trace {
                            Some(protocol::TransactionTrace {
                                request_id: rid.clone(),
                                status: if success { protocol::TransactionTraceStatus::Ok } else { protocol::TransactionTraceStatus::Failed },
                                started_at_ms,
                                total_elapsed_ms: total_elapsed,
                                failed_at,
                                commands: results.iter().map(|r| {
                                    protocol::TransactionCommandTrace {
                                        index: r.index,
                                        command: r.command.clone(),
                                        started_at_ms,
                                        elapsed_ms: r.elapsed.unwrap_or(0),
                                        before: protocol::UiStateSnapshot::default(),
                                        after: protocol::UiStateSnapshot::default(),
                                        polls: Vec::new(),
                                        error: r.error.clone(),
                                    }
                                }).collect(),
                            })
                        } else {
                            None
                        };

                        tracing::info!(
                            category = "AUTOMATION",
                            request_id = %rid,
                            success,
                            total_elapsed_ms = total_elapsed,
                            failed_at = ?failed_at,
                            target = "actionsDialog",
                            trace_included = include_trace,
                            "automation.batch.actions_dialog.completed"
                        );

                        if let Some(ref s) = sender {
                            let _ = s.try_send(Message::batch_result_with_trace(
                                rid.clone(), success, results, failed_at, total_elapsed, trace,
                            ));
                        }
                        return;
                    }

                    // ── PromptPopup batch path ─────────────────────────
                    // When targeting a PromptPopup, detect the active popup
                    // sub-type at execution time and route commands.
                    // Supported: selectByValue, selectBySemanticId, waitFor.
                    // setInput fails closed (popups don't have independent input).
                    if is_prompt_popup_batch {
                        let batch_start = std::time::Instant::now();
                        let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                        let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                        let mut failed = false;

                        for (index, cmd) in commands.iter().enumerate() {
                            if batch_start.elapsed() >= batch_timeout {
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command: batch_command_name(cmd),
                                    elapsed: Some(0),
                                    value: None,
                                    error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                                });
                                failed = true;
                                break;
                            }

                            let cmd_start = std::time::Instant::now();
                            match cmd {
                                protocol::BatchCommand::SelectByValue { value, submit: _ } => {
                                    let value = value.clone();
                                    let selected = this.update(cx, |_this, cx| {
                                        // Try each popup sub-type in priority order
                                        if let Some(v) = crate::ai::acp::picker_popup::batch_select_mention_item_by_value(&value, cx) {
                                            return Some(v);
                                        }
                                        if let Some(v) = crate::ai::acp::model_selector_popup::batch_select_model_by_value(&value, cx) {
                                            return Some(v);
                                        }
                                        if let Some(v) = crate::confirm::batch_select_confirm_button_by_value(&value) {
                                            return Some(v);
                                        }
                                        None
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_prompt_popup_select_by_value",
                                                value = %v,
                                                "PromptPopup select_by_value"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::selection_not_found(&value)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectByValue".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::SelectBySemanticId { semantic_id, submit: _ } => {
                                    let semantic_id = semantic_id.clone();
                                    let selected = this.update(cx, |_this, cx| {
                                        if let Some(v) = crate::ai::acp::picker_popup::batch_select_mention_item_by_semantic_id(&semantic_id, cx) {
                                            return Some(v);
                                        }
                                        if let Some(v) = crate::ai::acp::model_selector_popup::batch_select_model_by_semantic_id(&semantic_id, cx) {
                                            return Some(v);
                                        }
                                        if let Some(v) = crate::confirm::batch_select_confirm_button_by_semantic_id(&semantic_id) {
                                            return Some(v);
                                        }
                                        None
                                    });
                                    match selected {
                                        Ok(Some(v)) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_prompt_popup_select_by_semantic_id",
                                                semantic_id = %v,
                                                "PromptPopup select_by_semantic_id"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: Some(v), error: None,
                                            });
                                        }
                                        Ok(None) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::element_not_found(&semantic_id)),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "selectBySemanticId".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } => {
                                    let wait_timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000));
                                    let wait_poll = std::time::Duration::from_millis(poll_interval.unwrap_or(25));
                                    let wait_start = std::time::Instant::now();

                                    let already = this.update(cx, |this, cx| {
                                        this.wait_condition_satisfied(condition, cx)
                                    });

                                    let mut wait_result: Result<Option<String>, protocol::TransactionError> = match already {
                                        Ok(true) => Ok(None),
                                        Ok(false) => Err(protocol::TransactionError::wait_timeout("not yet")),
                                        Err(ref e) => Err(protocol::TransactionError::action_failed(format!("{e}"))),
                                    };

                                    if wait_result.is_err() && matches!(already, Ok(false)) {
                                        loop {
                                            cx.background_executor().timer(wait_poll).await;
                                            if wait_start.elapsed() >= wait_timeout {
                                                wait_result = Err(protocol::TransactionError::wait_timeout(
                                                    &format!("Timeout after {}ms", wait_timeout.as_millis()),
                                                ));
                                                break;
                                            }
                                            match this.update(cx, |this, cx| this.wait_condition_satisfied(condition, cx)) {
                                                Ok(true) => {
                                                    wait_result = Ok(None);
                                                    break;
                                                }
                                                Ok(false) => continue,
                                                Err(e) => {
                                                    wait_result = Err(protocol::TransactionError::action_failed(format!("{e}")));
                                                    break;
                                                }
                                            }
                                        }
                                    }

                                    match wait_result {
                                        Ok(_) => {
                                            tracing::info!(
                                                target: "script_kit::transaction",
                                                event = "transaction_prompt_popup_wait_complete",
                                                request_id = %rid,
                                                index,
                                                target = "promptPopup",
                                                "batch.prompt_popup.wait.ok"
                                            );
                                            results.push(protocol::BatchResultEntry {
                                                index, success: true, command: "waitFor".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None, error: None,
                                            });
                                        }
                                        Err(e) => {
                                            results.push(protocol::BatchResultEntry {
                                                index, success: false, command: "waitFor".to_string(),
                                                elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                                value: None,
                                                error: Some(e),
                                            });
                                            failed = true;
                                            if opts.stop_on_error { break; }
                                        }
                                    }
                                }
                                other => {
                                    let cmd_name = batch_command_name(other);
                                    tracing::warn!(
                                        target: "script_kit::transaction",
                                        event = "transaction_prompt_popup_unsupported",
                                        command = %cmd_name,
                                        "PromptPopup batch: unsupported command"
                                    );
                                    results.push(protocol::BatchResultEntry {
                                        index,
                                        success: false,
                                        command: cmd_name,
                                        elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                        value: None,
                                        error: Some(protocol::TransactionError {
                                            code: protocol::TransactionErrorCode::UnsupportedCommand,
                                            message: format!(
                                                "PromptPopup batch supports: selectByValue, selectBySemanticId, waitFor. Got: {}",
                                                batch_command_name(other)
                                            ),
                                            suggestion: Some("Use a supported command for PromptPopup targets.".to_string()),
                                        }),
                                    });
                                    failed = true;
                                    if opts.stop_on_error { break; }
                                }
                            }
                        }

                        let success = !failed;
                        let failed_at = if failed { results.iter().position(|r| !r.success) } else { None };
                        let total_elapsed = batch_start.elapsed().as_millis() as u64;
                        let include_trace = matches!(trace_mode, protocol::TransactionTraceMode::On)
                            || (matches!(trace_mode, protocol::TransactionTraceMode::OnFailure) && !success);
                        let started_at_ms = 0u64;
                        let trace = if include_trace {
                            Some(protocol::TransactionTrace {
                                request_id: rid.clone(),
                                status: if success { protocol::TransactionTraceStatus::Ok } else { protocol::TransactionTraceStatus::Failed },
                                started_at_ms,
                                total_elapsed_ms: total_elapsed,
                                failed_at,
                                commands: results.iter().map(|r| {
                                    protocol::TransactionCommandTrace {
                                        index: r.index,
                                        command: r.command.clone(),
                                        started_at_ms,
                                        elapsed_ms: r.elapsed.unwrap_or(0),
                                        before: protocol::UiStateSnapshot::default(),
                                        after: protocol::UiStateSnapshot::default(),
                                        polls: Vec::new(),
                                        error: r.error.clone(),
                                    }
                                }).collect(),
                            })
                        } else {
                            None
                        };

                        tracing::info!(
                            category = "AUTOMATION",
                            request_id = %rid,
                            success,
                            total_elapsed_ms = total_elapsed,
                            failed_at = ?failed_at,
                            target = "promptPopup",
                            trace_included = include_trace,
                            "automation.batch.prompt_popup.completed"
                        );

                        if let Some(ref s) = sender {
                            let _ = s.try_send(Message::batch_result_with_trace(
                                rid.clone(), success, results, failed_at, total_elapsed, trace,
                            ));
                        }
                        return;
                    }

                    // ── Main-window batch path (existing) ────────────────
                    let batch_start = std::time::Instant::now();
                    let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                    let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                    let mut failed = false;

                    for (index, cmd) in commands.iter().enumerate() {
                        // Check batch timeout
                        if batch_start.elapsed() >= batch_timeout {
                            let entry = protocol::BatchResultEntry {
                                index,
                                success: false,
                                command: batch_command_name(cmd),
                                elapsed: Some(0),
                                value: None,
                                error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                            };
                            results.push(entry);
                            failed = true;
                            break;
                        }

                        let cmd_start = std::time::Instant::now();
                        match cmd {
                            protocol::BatchCommand::SetInput { text } => {
                                match this.update(cx, |this, cx| {
                                    this.set_input_text(text, cx);
                                }) {
                                    Ok(()) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "setInput", "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "setInput".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: None,
                                        });
                                    }
                                    Err(e) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "setInput".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::SelectByValue { value, submit } => {
                                let submit = *submit;
                                let value = value.clone();
                                match this.update(cx, |this, cx| {
                                    this.select_choice_by_value(&value, submit, cx)
                                }) {
                                    Ok(Ok(v)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectByValue", value = %v, "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "selectByValue".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: Some(v),
                                            error: None,
                                        });
                                    }
                                    Ok(Err(e)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectByValue", error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "selectByValue".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                    Err(e) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "selectByValue".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::SelectBySemanticId { semantic_id, submit } => {
                                let submit = *submit;
                                let semantic_id = semantic_id.clone();
                                match this.update(cx, |this, cx| {
                                    this.select_choice_by_semantic_id(&semantic_id, submit, cx)
                                }) {
                                    Ok(Ok(v)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectBySemanticId", value = %v, "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "selectBySemanticId".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: Some(v),
                                            error: None,
                                        });
                                    }
                                    Ok(Err(e)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectBySemanticId", error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "selectBySemanticId".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::selection_not_found(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                    Err(e) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "selectBySemanticId".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::FilterAndSelect { filter, select_first, submit } => {
                                let filter = filter.clone();
                                let select_first = *select_first;
                                let submit = *submit;
                                match this.update(cx, |this, cx| {
                                    this.set_input_text(&filter, cx);
                                    if select_first {
                                        this.select_first_choice(submit, cx)
                                    } else {
                                        Ok(None)
                                    }
                                }) {
                                    Ok(Ok(selected_value)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "filterAndSelect", filter = %filter, selected = ?selected_value, "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "filterAndSelect".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: selected_value,
                                            error: None,
                                        });
                                    }
                                    Ok(Err(e)) | Err(e) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "filterAndSelect", error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "filterAndSelect".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::TypeAndSubmit { text } => {
                                let text = text.clone();
                                match this.update(cx, |this, cx| {
                                    this.set_input_text(&text, cx);
                                    this.submit_current_value(cx);
                                }) {
                                    Ok(()) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "typeAndSubmit", "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "typeAndSubmit".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: None,
                                        });
                                    }
                                    Err(e) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "typeAndSubmit".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::ForceSubmit { value } => {
                                let value = value.clone();
                                match this.update(cx, |this, cx| {
                                    let prompt_id = match &this.current_view {
                                        AppView::ArgPrompt { id, .. } => Some(id.clone()),
                                        AppView::DivPrompt { id, .. } => Some(id.clone()),
                                        AppView::FormPrompt { id, .. } => Some(id.clone()),
                                        AppView::TermPrompt { id, .. } => Some(id.clone()),
                                        AppView::EditorPrompt { id, .. } => Some(id.clone()),
                                        _ => None,
                                    };
                                    if let Some(id) = prompt_id {
                                        let value_str = match &value {
                                            serde_json::Value::String(s) => s.clone(),
                                            serde_json::Value::Null => String::new(),
                                            other => other.to_string(),
                                        };
                                        this.submit_prompt_response(id, Some(value_str.clone()), cx);
                                        Ok(value_str)
                                    } else {
                                        Err(anyhow::anyhow!("No active prompt to submit to"))
                                    }
                                }) {
                                    Ok(Ok(v)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "forceSubmit", "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "forceSubmit".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: Some(v),
                                            error: None,
                                        });
                                    }
                                    Ok(Err(e)) | Err(e) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "forceSubmit", error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "forceSubmit".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } => {
                                let wait_timeout = std::time::Duration::from_millis(timeout.unwrap_or(5_000));
                                let wait_poll = std::time::Duration::from_millis(poll_interval.unwrap_or(25));
                                let wait_start = std::time::Instant::now();

                                // Check if already satisfied
                                let already = this.update(cx, |this, cx| {
                                    this.wait_condition_satisfied(condition, cx)
                                });
                                match already {
                                    Ok(true) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "waitFor".to_string(),
                                            elapsed: Some(0),
                                            value: None,
                                            error: None,
                                        });
                                    }
                                    Ok(false) => {
                                        // Poll loop
                                        let mut wait_result: Result<Option<String>, protocol::TransactionError> = Err(protocol::TransactionError::wait_timeout(format!("WaitFor timeout after {}ms", wait_timeout.as_millis())));
                                        loop {
                                            cx.background_executor().timer(wait_poll).await;
                                            if wait_start.elapsed() >= wait_timeout {
                                                break;
                                            }
                                            match this.update(cx, |this, cx| {
                                                this.wait_condition_satisfied(condition, cx)
                                            }) {
                                                Ok(true) => { wait_result = Ok(None); break; }
                                                Ok(false) => continue,
                                                _ => { wait_result = Err(protocol::TransactionError::action_failed("Entity dropped during WaitFor")); break; }
                                            }
                                        }
                                        match wait_result {
                                            Ok(_) => {
                                                results.push(protocol::BatchResultEntry {
                                                    index,
                                                    success: true,
                                                    command: "waitFor".to_string(),
                                                    elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                    value: None,
                                                    error: None,
                                                });
                                            }
                                            Err(e) => {
                                                tracing::info!(
                                                    category = "BATCH",
                                                    request_id = %rid,
                                                    index = index,
                                                    command = %batch_command_name(cmd),
                                                    error = %e.message,
                                                    "batch.step.error"
                                                );
                                                results.push(protocol::BatchResultEntry {
                                                    index,
                                                    success: false,
                                                    command: "waitFor".to_string(),
                                                    elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                                    value: None,
                                                    error: Some(e),
                                                });
                                                failed = true;
                                                if opts.stop_on_error { break; }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "waitFor".to_string(),
                                            elapsed: Some(wait_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed("Entity dropped")),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                        }
                    }

                    let total_elapsed = batch_start.elapsed().as_millis() as u64;
                    let success = !failed;
                    let failed_at = if failed {
                        results.iter().position(|r| !r.success)
                    } else {
                        None
                    };

                    let include_trace = protocol::transaction_trace::should_include_trace(trace_mode, success);
                    let trace = if include_trace {
                        let started_at_ms = protocol::transaction_trace::now_epoch_ms()
                            .saturating_sub(total_elapsed);
                        Some(protocol::TransactionTrace {
                            request_id: rid.clone(),
                            status: if success {
                                protocol::TransactionTraceStatus::Ok
                            } else {
                                protocol::TransactionTraceStatus::Failed
                            },
                            started_at_ms,
                            total_elapsed_ms: total_elapsed,
                            failed_at,
                            commands: results.iter().map(|r| {
                                protocol::TransactionCommandTrace {
                                    index: r.index,
                                    command: r.command.clone(),
                                    started_at_ms,
                                    elapsed_ms: r.elapsed.unwrap_or(0),
                                    before: protocol::UiStateSnapshot::default(),
                                    after: protocol::UiStateSnapshot::default(),
                                    polls: Vec::new(),
                                    error: r.error.clone(),
                                }
                            }).collect(),
                        })
                    } else {
                        None
                    };

                    tracing::info!(
                        category = "AUTOMATION",
                        request_id = %rid,
                        success = success,
                        total_elapsed_ms = total_elapsed,
                        failed_at = ?failed_at,
                        trace_included = include_trace,
                        "automation.batch.completed"
                    );

                    if let Some(ref s) = sender {
                        let _ = s.try_send(Message::batch_result_with_trace(
                            rid.clone(),
                            success,
                            results,
                            failed_at,
                            total_elapsed,
                            trace,
                        ));
                    }
                })
                .detach();
            }

            PromptMessage::ForceSubmit { value } => {
                // Get the current prompt ID and submit the value
                let prompt_id = match &self.current_view {
                    AppView::ArgPrompt { id, .. } => Some(id.clone()),
                    AppView::DivPrompt { id, .. } => Some(id.clone()),
                    AppView::FormPrompt { id, .. } => Some(id.clone()),
                    AppView::TermPrompt { id, .. } => Some(id.clone()),
                    AppView::EditorPrompt { id, .. } => Some(id.clone()),
                    AppView::EmojiPickerView { .. } => None,
                    _ => None,
                };

                if let Some(id) = prompt_id {
                    // Convert serde_json::Value to String for submission
                    let value_str = match &value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Null => String::new(),
                        other => other.to_string(),
                    };

                    self.submit_prompt_response(id, Some(value_str), cx);
                } else {
                    tracing::warn!(
                        category = "WARN",
                        "ForceSubmit received but no active prompt to submit to"
                    );
                }
            }
            // ============================================================
            // NEW PROMPT TYPES (scaffolding - TODO: implement full UI)
            // ============================================================
            PromptMessage::ShowPath {
                id,
                start_path,
                hint,
            } => {
                self.prepare_window_for_prompt("UI", "path", "");

                tracing::info!(
                    category = "UI",
                    id = %id,
                    start_path = ?start_path,
                    hint = ?hint,
                    "Showing path prompt"
                );

                let path_submit_callback = self.make_submit_callback("path");
                let submit_callback: std::sync::Arc<dyn Fn(String, Option<String>) + Send + Sync> =
                    std::sync::Arc::new(move |id, value| {
                        tracing::info!(
                            category = "UI",
                            id = %id,
                            value = ?value,
                            "PathPrompt submit callback called"
                        );
                        path_submit_callback(id, value);
                    });

                // Clone the path_actions_showing and search_text Arcs for header display
                let path_actions_showing = self.path_actions_showing.clone();
                let path_actions_search_text = self.path_actions_search_text.clone();

                let focus_handle = cx.focus_handle();
                let path_prompt = PathPrompt::new(
                    id.clone(),
                    start_path,
                    hint,
                    focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                )
                // Note: Legacy callbacks are no longer needed - we use events now
                // But we still pass the shared state for header display
                .with_actions_showing(path_actions_showing)
                .with_actions_search_text(path_actions_search_text);

                let entity = cx.new(|_| path_prompt);

                // Subscribe to PathPrompt events for actions dialog control
                // This replaces the mutex-polling pattern with event-driven handling
                cx.subscribe(
                    &entity,
                    |this, _entity, event: &PathPromptEvent, cx| match event {
                        PathPromptEvent::ShowActions(path_info) => {
                            tracing::info!(
                                category = "UI",
                                path = %path_info.path,
                                "PathPromptEvent::ShowActions received"
                            );
                            this.handle_show_path_actions(path_info.clone(), cx);
                        }
                        PathPromptEvent::CloseActions => {
                            tracing::info!(
                                category = "UI",
                                "PathPromptEvent::CloseActions received"
                            );
                            this.handle_close_path_actions(cx);
                        }
                    },
                )
                .detach();

                self.current_view = AppView::PathPrompt {
                    id,
                    entity,
                    focus_handle,
                };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::PathPrompt);

                // Reset showing state (no more mutex polling needed)
                if let Ok(mut guard) = self.path_actions_showing.lock() {
                    *guard = false;
                }

                resize_to_view_sync(ViewType::ScriptList, 20);
                cx.notify();
            }
            PromptMessage::ShowEnv {
                id,
                key,
                prompt,
                title,
                secret,
            } => {
                self.prepare_window_for_prompt("UI", "env", "");

                tracing::info!(id, key, ?prompt, ?title, secret, "ShowEnv received");
                tracing::info!(
                    category = "UI",
                    id = %id,
                    key = %key,
                    secret,
                    "ShowEnv prompt received"
                );

                let submit_callback = self.make_submit_callback("env");

                // Check if key already exists in secrets (for UX messaging)
                // Empty values don't count as "existing" - must have actual content
                // Use get_secret_info to get both existence and modification timestamp
                let secret_info = secrets::get_secret_info(&key);
                let exists_in_keyring = secret_info
                    .as_ref()
                    .map(|info| !info.value.is_empty())
                    .unwrap_or(false);
                let modified_at = secret_info.map(|info| info.modified_at);

                // Create EnvPrompt entity
                let focus_handle = self.focus_handle.clone();
                let mut env_prompt = prompts::EnvPrompt::new(
                    id.clone(),
                    key,
                    prompt,
                    title,
                    secret,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                    exists_in_keyring,
                    modified_at,
                );

                // Check keyring first - if value exists and no contextual prompt/title
                // was provided, auto-submit without showing UI. When prompt or title
                // are set, the script wants the user to see the setup context.
                let has_contextual_text = env_prompt.has_prompt_or_title();
                if !has_contextual_text && env_prompt.check_keyring_and_auto_submit() {
                    tracing::info!(
                        category = "UI",
                        "EnvPrompt value found in keyring, auto-submitted"
                    );
                    // Don't switch view, the callback already submitted
                    cx.notify();
                    return;
                }

                let entity = cx.new(|_| env_prompt);
                self.current_view = AppView::EnvPrompt { id, entity };
                self.focused_input = FocusedInput::None; // EnvPrompt has its own focus handling
                self.pending_focus = Some(FocusTarget::EnvPrompt);

                // Resize to standard height for full-window centered layout
                resize_to_view_sync(ViewType::DivPrompt, 0);
                cx.notify();
            }
            PromptMessage::ShowDrop {
                id,
                placeholder,
                hint,
            } => {
                self.prepare_window_for_prompt("UI", "drop", "");

                tracing::info!(id, ?placeholder, ?hint, "ShowDrop received");
                tracing::info!(
                    category = "UI",
                    id = %id,
                    placeholder = ?placeholder,
                    "ShowDrop prompt received"
                );

                let submit_callback = self.make_submit_callback("drop");

                // Create DropPrompt entity
                let focus_handle = self.focus_handle.clone();
                let drop_prompt = prompts::DropPrompt::new(
                    id.clone(),
                    placeholder,
                    hint,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                );

                let entity = cx.new(|_| drop_prompt);
                self.current_view = AppView::DropPrompt { id, entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::DropPrompt);

                resize_to_view_sync(ViewType::DivPrompt, 0);
                cx.notify();
            }
            PromptMessage::ShowTemplate { id, template } => {
                self.prepare_window_for_prompt("UI", "template", "");

                tracing::info!(id, template, "ShowTemplate received");
                tracing::info!(
                    category = "UI",
                    id = %id,
                    template = %template,
                    "ShowTemplate prompt received"
                );

                let submit_callback = self.make_submit_callback("template");

                // Create TemplatePrompt entity
                let focus_handle = self.focus_handle.clone();
                let template_prompt = prompts::TemplatePrompt::new(
                    id.clone(),
                    template,
                    focus_handle,
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                );

                let entity = cx.new(|_| template_prompt);
                self.current_view = AppView::TemplatePrompt { id, entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::TemplatePrompt);

                resize_to_view_sync(ViewType::DivPrompt, 0);
                cx.notify();
            }

            PromptMessage::ShowSelect {
                id,
                placeholder,
                choices,
                multiple,
            } => {
                self.prepare_window_for_prompt("UI", "select", "");

                tracing::info!(
                    id,
                    ?placeholder,
                    choice_count = choices.len(),
                    multiple,
                    "ShowSelect received"
                );
                tracing::info!(
                    category = "UI",
                    id = %id,
                    choice_count = choices.len(),
                    multiple,
                    "ShowSelect prompt received"
                );

                let submit_callback = self.make_submit_callback("select");

                // Create SelectPrompt entity
                let choice_count = choices.len();
                let select_prompt = prompts::SelectPrompt::new(
                    id.clone(),
                    placeholder,
                    choices,
                    multiple,
                    self.focus_handle.clone(),
                    submit_callback,
                    std::sync::Arc::clone(&self.theme),
                );
                let entity = cx.new(|_| select_prompt);
                self.current_view = AppView::SelectPrompt { id, entity };
                self.focused_input = FocusedInput::None; // SelectPrompt has its own focus handling
                self.pending_focus = Some(FocusTarget::SelectPrompt);

                // Resize window based on number of choices
                let view_type = if choice_count == 0 {
                    ViewType::ArgPromptNoChoices
                } else {
                    ViewType::ArgPromptWithChoices
                };
                resize_to_view_sync(view_type, choice_count);
                cx.notify();
            }
            PromptMessage::ShowConfirm {
                id,
                message,
                confirm_text,
                cancel_text,
            } => {
                tracing::info!(
                    category = "CONFIRM",
                    id = %id,
                    message = ?message,
                    "ShowConfirm prompt"
                );

                // Build response callback that sends submit message back to the script
                let response_sender = self.response_sender.clone();
                let prompt_id = id.clone();
                let send_response = {
                    let response_sender = response_sender.clone();
                    let prompt_id = prompt_id.clone();
                    move |confirmed: bool| {
                        tracing::info!(
                            category = "CONFIRM",
                            prompt_id = %prompt_id,
                            confirmed,
                            "User choice received"
                        );
                        if let Some(ref sender) = response_sender {
                            let value = if confirmed {
                                Some("true".to_string())
                            } else {
                                Some("false".to_string())
                            };
                            let response = Message::Submit {
                                id: prompt_id.clone(),
                                value,
                            };
                            match sender.try_send(response) {
                                Ok(()) => {
                                    tracing::info!(category = "CONFIRM", "Submit message sent");
                                }
                                Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                    tracing::warn!(
                                        category = "WARN",
                                        "Response channel full - confirm response dropped"
                                    );
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                                    tracing::info!(
                                        category = "UI",
                                        "Response channel disconnected - script exited"
                                    );
                                }
                            }
                        }
                    }
                };

                let send_confirm = send_response.clone();
                let send_cancel = send_response.clone();
                let send_fallback = send_response;

                // Open parent confirm dialog via shared async helper
                cx.spawn(
                    async move |_this, cx| match crate::confirm::confirm_with_parent_dialog(
                        cx,
                        crate::confirm::ParentConfirmOptions {
                            title: "Confirm".into(),
                            body: gpui::SharedString::from(message),
                            confirm_text: confirm_text
                                .map(gpui::SharedString::from)
                                .unwrap_or("OK".into()),
                            cancel_text: cancel_text
                                .map(gpui::SharedString::from)
                                .unwrap_or("Cancel".into()),
                            ..Default::default()
                        },
                        "prompt_handler_confirm",
                    )
                    .await
                    {
                        Ok(true) => send_confirm(true),
                        Ok(false) => send_cancel(false),
                        Err(error) => {
                            tracing::error!(
                                category = "ERROR",
                                error = %error,
                                "Failed to open confirm dialog window — failing closed"
                            );
                            send_fallback(false);
                        }
                    },
                )
                .detach();

                cx.notify();
            }
            PromptMessage::ShowChat {
                id,
                placeholder,
                messages,
                hint,
                footer,
                actions,
                model,
                models,
                save_history,
                use_builtin_ai,
            } => {
                logging::bench_log("ShowChat_received");

                self.prepare_window_for_prompt("CHAT", "chat", "window_show_requested");

                tracing::info!(
                    id,
                    ?placeholder,
                    message_count = messages.len(),
                    ?model,
                    model_count = models.len(),
                    save_history,
                    use_builtin_ai,
                    "ShowChat received"
                );
                tracing::info!(
                    category = "UI",
                    id = %id,
                    message_count = messages.len(),
                    model_count = models.len(),
                    save_history,
                    use_builtin_ai,
                    "ShowChat prompt received"
                );

                // Store SDK actions for the actions panel (Cmd+K)
                self.sdk_actions = actions;

                // Create submit callback for chat prompt
                let response_sender = self.response_sender.clone();
                let chat_submit_callback: prompts::ChatSubmitCallback =
                    std::sync::Arc::new(move |id, text| {
                        if let Some(ref sender) = response_sender {
                            // Send ChatSubmit message back to SDK
                            let response = Message::ChatSubmit { id, text };
                            match sender.try_send(response) {
                                Ok(()) => {}
                                Err(std::sync::mpsc::TrySendError::Full(_)) => {
                                    tracing::warn!(
                                        category = "WARN",
                                        "Response channel full - chat response dropped"
                                    );
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                                    tracing::info!(
                                        category = "UI",
                                        "Response channel disconnected - script exited"
                                    );
                                }
                            }
                        }
                    });

                // Create ChatPrompt entity with configured models
                let focus_handle = self.focus_handle.clone();
                let mut chat_prompt = prompts::ChatPrompt::new(
                    id.clone(),
                    placeholder,
                    messages,
                    hint,
                    footer,
                    focus_handle,
                    chat_submit_callback,
                    std::sync::Arc::clone(&self.theme),
                );

                // Apply model configuration from SDK
                if !models.is_empty() {
                    chat_prompt = chat_prompt.with_model_names(models);
                }
                if let Some(default_model) = model {
                    chat_prompt = chat_prompt.with_default_model(default_model);
                }

                // Configure history saving
                chat_prompt = chat_prompt.with_save_history(save_history);

                // If SDK requested built-in AI mode, enable it with the app's AI providers
                if use_builtin_ai {
                    use crate::ai::ProviderRegistry;

                    let registry =
                        ProviderRegistry::from_environment_with_config(Some(&self.config));
                    if registry.has_any_provider() {
                        tracing::info!(
                            category = "CHAT",
                            provider_count = registry.provider_ids().len(),
                            "Enabling built-in AI"
                        );
                        chat_prompt = chat_prompt.with_builtin_ai(registry, true);
                        // Auto-respond if there are initial user messages (scriptlets with pre-populated messages)
                        if chat_prompt
                            .messages
                            .iter()
                            .any(|m| m.role == Some(crate::protocol::ChatMessageRole::User))
                        {
                            tracing::info!(
                                category = "CHAT",
                                "Found user messages - enabling needs_initial_response"
                            );
                            chat_prompt = chat_prompt.with_needs_initial_response(true);
                        }
                    } else {
                        tracing::info!(
                            category = "CHAT",
                            "Built-in AI requested but no providers configured"
                        );

                        // Create configure callback that signals via channel
                        let configure_sender = self.inline_chat_configure_sender.clone();
                        let configure_callback: crate::prompts::ChatConfigureCallback =
                            std::sync::Arc::new(move || {
                                tracing::info!(
                                    category = "CHAT",
                                    "Configure callback triggered - sending signal"
                                );
                                let _ = configure_sender.try_send(());
                            });

                        // Create Claude Code callback that signals via channel
                        let claude_code_sender = self.inline_chat_claude_code_sender.clone();
                        let claude_code_callback: crate::prompts::ChatClaudeCodeCallback =
                            std::sync::Arc::new(move || {
                                tracing::info!(
                                    category = "CHAT",
                                    "Claude Code callback triggered - sending signal"
                                );
                                let _ = claude_code_sender.try_send(());
                            });

                        chat_prompt = chat_prompt
                            .with_needs_setup(true)
                            .with_configure_callback(configure_callback)
                            .with_claude_code_callback(claude_code_callback);
                    }
                }

                // Wire on_show_actions so ChatPrompt's internal toggle_actions_menu
                // has a live callback. ⌘K is also intercepted at the parent level.
                logging::bench_log("ChatPrompt_creating");
                let entity = cx.new(|_| chat_prompt);
                let app_weak = cx.entity().downgrade();
                entity.update(cx, |chat, _cx| {
                    chat.set_on_show_actions(std::sync::Arc::new(move |_prompt_id| {
                        tracing::info!(
                            event = "on_show_actions.triggered",
                            source = "sdk-chat",
                            "ChatPrompt requested actions dialog via callback"
                        );
                        let _ = &app_weak;
                    }));
                });
                self.current_view = AppView::ChatPrompt { id, entity };
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::ChatPrompt);
                logging::bench_log("ChatPrompt_created");

                resize_to_view_sync(ViewType::DivPrompt, 0);
                logging::bench_log("resize_queued");
                cx.notify();
                logging::bench_end("hotkey_to_chat_visible");
            }

            PromptMessage::ChatAddMessage { id, message } => {
                tracing::info!(category = "CHAT", id = %id, "ChatAddMessage");
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.add_message(message, cx);
                        });
                    }
                }
            }
            PromptMessage::ChatStreamStart {
                id,
                message_id,
                position,
            } => {
                tracing::info!(
                    category = "CHAT",
                    id = %id,
                    message_id = %message_id,
                    "ChatStreamStart"
                );
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.start_streaming(message_id, position, cx);
                        });
                    }
                }
            }
            PromptMessage::ChatStreamChunk {
                id,
                message_id,
                chunk,
            } => {
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.append_chunk(&message_id, &chunk, cx);
                        });
                    }
                }
            }
            PromptMessage::ChatStreamComplete { id, message_id } => {
                tracing::info!(
                    category = "CHAT",
                    id = %id,
                    message_id = %message_id,
                    "ChatStreamComplete"
                );
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.complete_streaming(&message_id, cx);
                        });
                    }
                }
            }
            PromptMessage::ChatClear { id } => {
                tracing::info!(category = "CHAT", id = %id, "ChatClear");
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.clear_messages(cx);
                        });
                    }
                }
            }
            PromptMessage::ChatSetError {
                id,
                message_id,
                error,
            } => {
                tracing::info!(
                    category = "CHAT",
                    id = %id,
                    message_id = %message_id,
                    error = %error,
                    "ChatSetError"
                );
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.set_message_error(&message_id, error.clone(), cx);
                        });
                    }
                }
            }
            PromptMessage::ChatClearError { id, message_id } => {
                tracing::info!(
                    category = "CHAT",
                    id = %id,
                    message_id = %message_id,
                    "ChatClearError"
                );
                if let AppView::ChatPrompt {
                    id: view_id,
                    entity,
                } = &self.current_view
                {
                    if view_id == &id {
                        entity.update(cx, |chat, cx| {
                            chat.clear_message_error(&message_id, cx);
                        });
                    }
                }
            }
            PromptMessage::ShowHud { text, duration_ms } => {
                self.show_hud(text, duration_ms, cx);
            }
            PromptMessage::SetStatus { status, message } => {
                tracing::info!(
                    category = "STATUS",
                    state = "received",
                    status = %status,
                    has_message = message.is_some(),
                    message = %message.as_deref().unwrap_or(""),
                    "Received setStatus() protocol message"
                );
            }
            PromptMessage::SetInput { text } => {
                self.set_prompt_input(text, cx);
            }
            PromptMessage::SetActions { actions } => {
                tracing::info!(
                    category = "ACTIONS",
                    action_count = actions.len(),
                    "Received setActions"
                );

                self.set_sdk_actions_and_shortcuts(actions.clone(), "ACTIONS", true);

                // Update ActionsDialog if it exists and is open
                if let Some(ref dialog) = self.actions_dialog {
                    dialog.update(cx, |d, _cx| {
                        d.set_sdk_actions(actions);
                    });
                }

                cx.notify();
            }
            PromptMessage::FieldsComingSoon { id, field_count } => {
                tracing::warn!(
                    category = "WARN",
                    prompt = "fields()",
                    id = %id,
                    field_count = field_count,
                    state = "stubbed",
                    "Received unsupported prompt message"
                );
                self.show_prompt_coming_soon_toast("fields()", cx);
            }
            PromptMessage::HotkeyComingSoon { id, placeholder } => {
                tracing::warn!(
                    category = "WARN",
                    prompt = "hotkey()",
                    id = %id,
                    has_placeholder = placeholder.is_some(),
                    state = "stubbed",
                    "Received unsupported prompt message"
                );
                self.show_prompt_coming_soon_toast("hotkey()", cx);
            }
            PromptMessage::WidgetComingSoon { id } => {
                tracing::warn!(
                    category = "WARN",
                    prompt = "widget()",
                    id = %id,
                    state = "stubbed",
                    "Received unsupported prompt message"
                );
                self.show_prompt_coming_soon_toast("widget()", cx);
            }
            PromptMessage::WebcamComingSoon { id } => {
                tracing::warn!(
                    category = "WARN",
                    prompt = "webcam()",
                    id = %id,
                    state = "stubbed",
                    "Received unsupported prompt message"
                );
                self.show_prompt_coming_soon_toast("webcam()", cx);
            }
            PromptMessage::MicComingSoon { id } => {
                tracing::warn!(
                    category = "WARN",
                    prompt = "mic()",
                    id = %id,
                    state = "stubbed",
                    "Received unsupported prompt message"
                );
                self.show_prompt_coming_soon_toast("mic()", cx);
            }
            PromptMessage::AiStartChat {
                request_id,
                message,
                system_prompt,
                image,
                model_id,
                no_response,
                parts,
            } => {
                tracing::info!(
                    category = "AI",
                    request_id = %request_id,
                    message_len = message.len(),
                    has_system_prompt = system_prompt.is_some(),
                    has_image = image.is_some(),
                    model_id = ?model_id,
                    no_response,
                    "AiStartChat request"
                );

                // Open the AI window (creates new if not open, brings to front if open)
                if let Err(e) = crate::ai::open_ai_window(cx) {
                    tracing::error!(
                        category = "ERROR",
                        error = %e,
                        "Failed to open AI window for AiStartChat"
                    );
                    // Still send response so SDK doesn't hang
                    if let Some(ref sender) = self.response_sender {
                        let _ = sender.try_send(Message::AiChatCreated {
                            request_id,
                            chat_id: String::new(),
                            title: String::new(),
                            model_id: model_id.unwrap_or_default(),
                            provider: String::new(),
                            streaming_started: false,
                        });
                    }
                    return;
                }

                // Pre-generate a real ChatId so the SDK gets an actual persistent ID
                let chat_id = crate::ai::ChatId::new();
                let should_submit = !no_response;
                let provider = model_id.as_deref().and_then(|selected_model_id| {
                    let registry = crate::ai::ProviderRegistry::from_environment_with_config(Some(
                        &self.config,
                    ));
                    resolve_ai_start_chat_provider(&registry, selected_model_id)
                });
                let context_parts = parts
                    .into_iter()
                    .map(|part| match part {
                        crate::protocol::AiContextPartInput::ResourceUri { uri, label } => {
                            crate::ai::AiContextPart::ResourceUri { uri, label }
                        }
                        crate::protocol::AiContextPartInput::FilePath { path, label } => {
                            crate::ai::AiContextPart::FilePath { path, label }
                        }
                    })
                    .collect();

                // Queue the StartChat command — the AI window will create the chat,
                // save the user message (with optional image), and optionally stream.
                crate::ai::start_ai_chat(
                    cx,
                    chat_id,
                    &message,
                    context_parts,
                    image.as_deref(),
                    system_prompt.as_deref(),
                    model_id.as_deref(),
                    provider.as_deref(),
                    None,
                    should_submit,
                );

                // Build title from message content
                let title = if message.trim().is_empty() && image.is_some() {
                    "Image attachment".to_string()
                } else {
                    crate::ai::Chat::generate_title_from_content(&message)
                };

                // Send AiChatCreated response with the real chat ID
                if let Some(ref sender) = self.response_sender {
                    let response = Message::AiChatCreated {
                        request_id: request_id.clone(),
                        chat_id: chat_id.as_str(),
                        title,
                        model_id: model_id
                            .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string()),
                        provider: provider.unwrap_or_else(|| "anthropic".to_string()),
                        streaming_started: should_submit,
                    };
                    match sender.try_send(response) {
                        Ok(()) => {
                            tracing::info!(
                                category = "AI",
                                request_id = %request_id,
                                chat_id = %chat_id,
                                "AiChatCreated response sent"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "WARN",
                                "Response channel full - AiChatCreated dropped"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "UI",
                                "Response channel disconnected - script exited"
                            );
                        }
                    }
                }

                cx.notify();
            }
            PromptMessage::AiFocus { request_id } => {
                tracing::info!(category = "AI", request_id = %request_id, "AiFocus request");

                // Check if window was already open before we open/focus it
                let was_open = crate::ai::is_ai_window_open();

                // Open the AI window (creates new if not open, brings to front if open)
                let success = match crate::ai::open_ai_window(cx) {
                    Ok(()) => {
                        tracing::info!(category = "AI", "AI window focused successfully");
                        true
                    }
                    Err(e) => {
                        tracing::error!(
                            category = "ERROR",
                            error = %e,
                            "Failed to focus AI window"
                        );
                        false
                    }
                };

                // Send AiFocusResult response back to SDK
                if let Some(ref sender) = self.response_sender {
                    let response = Message::AiFocusResult {
                        request_id: request_id.clone(),
                        success,
                        was_open,
                    };
                    match sender.try_send(response) {
                        Ok(()) => {
                            tracing::info!(
                                category = "AI",
                                request_id = %request_id,
                                "AiFocusResult sent"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "WARN",
                                "Response channel full - AiFocusResult dropped"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "UI",
                                "Response channel disconnected - script exited"
                            );
                        }
                    }
                }

                cx.notify();
            }
            PromptMessage::ShowGrid { options } => {
                tracing::info!(
                    category = "DEBUG_GRID",
                    grid_size = options.grid_size,
                    show_bounds = options.show_bounds,
                    show_box_model = options.show_box_model,
                    show_alignment_guides = options.show_alignment_guides,
                    "ShowGrid from script"
                );
                self.show_grid(options, cx);
            }
            PromptMessage::HideGrid => {
                tracing::info!(category = "DEBUG_GRID", "HideGrid from script");
                self.hide_grid(cx);
            }
            PromptMessage::SimulateGpuiEvent {
                request_id,
                target,
                event,
            } => {
                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    target = ?target,
                    event = ?event,
                    "gpui_event_simulation.entity_received"
                );

                let result = crate::platform::gpui_event_simulator::dispatch_gpui_event(
                    &request_id,
                    target.as_ref(),
                    &event,
                    cx,
                );

                let response = if result.success {
                    Message::simulate_gpui_event_result_success(
                        request_id,
                        result.dispatch_path,
                        result.resolved_window_id,
                    )
                } else {
                    Message::simulate_gpui_event_result_error(
                        request_id,
                        result.error_code.unwrap_or_else(|| "unknown".to_string()),
                        result.error.unwrap_or_else(|| "Unknown error".to_string()),
                        result.dispatch_path,
                        result.resolved_window_id,
                    )
                };

                if let Some(ref sender) = self.response_sender {
                    if let Err(e) = sender.try_send(response) {
                        tracing::error!(
                            target: "script_kit::automation",
                            error = %e,
                            "Failed to send GPUI event simulation response"
                        );
                    }
                }
            }
        }
    }

    /// Check if a wait condition is currently satisfied.
    fn wait_condition_satisfied(
        &self,
        condition: &protocol::WaitCondition,
        cx: &Context<Self>,
    ) -> bool {
        match condition {
            protocol::WaitCondition::Named(named) => match named {
                protocol::WaitNamedCondition::ChoicesRendered => {
                    let elements = self.collect_visible_elements(100, cx);
                    elements
                        .elements
                        .iter()
                        .any(|el| el.element_type == protocol::ElementType::Choice)
                }
                protocol::WaitNamedCondition::InputEmpty => {
                    let input = self.current_input_value();
                    input.is_empty()
                }
                protocol::WaitNamedCondition::WindowVisible => {
                    script_kit_gpui::is_main_window_visible()
                }
                protocol::WaitNamedCondition::WindowFocused => {
                    let visible = script_kit_gpui::is_main_window_visible();
                    visible && self.focused_input != FocusedInput::None
                }
            },
            protocol::WaitCondition::Detailed(detailed) => match detailed {
                protocol::WaitDetailedCondition::ElementExists { semantic_id }
                | protocol::WaitDetailedCondition::ElementVisible { semantic_id } => {
                    let elements = self.collect_visible_elements(1000, cx);
                    elements
                        .elements
                        .iter()
                        .any(|el| el.semantic_id == *semantic_id)
                }
                protocol::WaitDetailedCondition::ElementFocused { semantic_id } => {
                    let elements = self.collect_visible_elements(1000, cx);
                    elements
                        .elements
                        .iter()
                        .any(|el| el.semantic_id == *semantic_id && el.focused == Some(true))
                }
                protocol::WaitDetailedCondition::StateMatch { state: expected } => {
                    let prompt_type = self.current_prompt_type();
                    let input_value = self.current_input_value();
                    let selected_value = self.current_selected_value();
                    let window_visible = script_kit_gpui::is_main_window_visible();

                    expected
                        .prompt_type
                        .as_deref()
                        .is_none_or(|v| v == prompt_type)
                        && expected
                            .input_value
                            .as_deref()
                            .is_none_or(|v| v == input_value)
                        && expected
                            .selected_value
                            .as_deref()
                            .is_none_or(|v| selected_value.as_deref() == Some(v))
                        && expected.window_visible.is_none_or(|v| v == window_visible)
                }
                // ── ACP-specific wait conditions ────────────────────
                protocol::WaitDetailedCondition::AcpReady => {
                    let state = self.collect_acp_state(cx);
                    state.context_ready && state.status == "idle"
                }
                protocol::WaitDetailedCondition::AcpPickerOpen => {
                    let state = self.collect_acp_state(cx);
                    state.picker.as_ref().is_some_and(|p| p.open)
                }
                protocol::WaitDetailedCondition::AcpPickerClosed => {
                    let state = self.collect_acp_state(cx);
                    state.picker.is_none() || state.picker.as_ref().is_some_and(|p| !p.open)
                }
                protocol::WaitDetailedCondition::AcpItemAccepted => {
                    let state = self.collect_acp_state(cx);
                    state.last_accepted_item.is_some()
                }
                protocol::WaitDetailedCondition::AcpCursorAt { index } => {
                    let state = self.collect_acp_state(cx);
                    state.cursor_index == *index
                }
                protocol::WaitDetailedCondition::AcpStatus { status } => {
                    let state = self.collect_acp_state(cx);
                    state.status == *status
                }
                protocol::WaitDetailedCondition::AcpInputMatch { text } => {
                    let state = self.collect_acp_state(cx);
                    state.input_text == *text
                }
                protocol::WaitDetailedCondition::AcpInputContains { substring } => {
                    let state = self.collect_acp_state(cx);
                    state.input_text.contains(substring.as_str())
                }
                // ── ACP proof wait conditions (test probe) ─────────
                protocol::WaitDetailedCondition::AcpAcceptedViaKey { key } => {
                    let probe = self.collect_acp_test_probe(1, cx);
                    probe
                        .accepted_items
                        .last()
                        .is_some_and(|item| item.accepted_via_key == *key)
                }
                protocol::WaitDetailedCondition::AcpAcceptedLabel { label } => {
                    let probe = self.collect_acp_test_probe(1, cx);
                    probe
                        .accepted_items
                        .last()
                        .is_some_and(|item| item.item_label == *label)
                }
                protocol::WaitDetailedCondition::AcpAcceptedCursorAt { index } => {
                    let probe = self.collect_acp_test_probe(1, cx);
                    probe
                        .accepted_items
                        .last()
                        .is_some_and(|item| item.cursor_after == *index)
                }
                protocol::WaitDetailedCondition::AcpInputLayoutMatch {
                    visible_start,
                    visible_end,
                    cursor_in_window,
                } => {
                    let probe = self.collect_acp_test_probe(1, cx);
                    probe.input_layout.as_ref().is_some_and(|layout| {
                        layout.visible_start == *visible_start
                            && layout.visible_end == *visible_end
                            && layout.cursor_in_window == *cursor_in_window
                    })
                }
                // ── ACP setup wait conditions ─────────────────────
                protocol::WaitDetailedCondition::AcpSetupVisible => {
                    let state = self.collect_acp_state(cx);
                    state.setup.is_some()
                }
                protocol::WaitDetailedCondition::AcpSetupReasonCode { reason_code } => {
                    let state = self.collect_acp_state(cx);
                    state
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.reason_code == *reason_code)
                }
                protocol::WaitDetailedCondition::AcpSetupPrimaryAction { action } => {
                    let state = self.collect_acp_state(cx);
                    state
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.primary_action == *action)
                }
                protocol::WaitDetailedCondition::AcpSetupAgentPickerOpen => {
                    let state = self.collect_acp_state(cx);
                    state.setup.as_ref().is_some_and(|s| s.agent_picker_open)
                }
                protocol::WaitDetailedCondition::AcpSetupSelectedAgent { agent_id } => {
                    let state = self.collect_acp_state(cx);
                    state.setup.as_ref().is_some_and(|s| {
                        s.selected_agent_id
                            .as_ref()
                            .is_some_and(|id| id == agent_id)
                    })
                }
            },
        }
    }

    /// Check if a wait condition is currently satisfied, reading ACP data
    /// from the given detached entity (if provided) instead of the main window.
    ///
    /// Non-ACP conditions always read from the main window regardless.
    fn wait_condition_satisfied_for_target(
        &self,
        condition: &protocol::WaitCondition,
        detached_entity: Option<&gpui::Entity<crate::ai::acp::view::AcpChatView>>,
        cx: &Context<Self>,
    ) -> bool {
        match condition {
            // Non-ACP conditions: delegate to main-window logic
            protocol::WaitCondition::Named(_) => self.wait_condition_satisfied(condition, cx),
            protocol::WaitCondition::Detailed(detailed) => {
                // Check if this is an ACP condition
                let is_acp = matches!(
                    detailed,
                    protocol::WaitDetailedCondition::AcpReady
                        | protocol::WaitDetailedCondition::AcpPickerOpen
                        | protocol::WaitDetailedCondition::AcpPickerClosed
                        | protocol::WaitDetailedCondition::AcpItemAccepted
                        | protocol::WaitDetailedCondition::AcpCursorAt { .. }
                        | protocol::WaitDetailedCondition::AcpStatus { .. }
                        | protocol::WaitDetailedCondition::AcpInputMatch { .. }
                        | protocol::WaitDetailedCondition::AcpInputContains { .. }
                        | protocol::WaitDetailedCondition::AcpAcceptedViaKey { .. }
                        | protocol::WaitDetailedCondition::AcpAcceptedLabel { .. }
                        | protocol::WaitDetailedCondition::AcpAcceptedCursorAt { .. }
                        | protocol::WaitDetailedCondition::AcpInputLayoutMatch { .. }
                        | protocol::WaitDetailedCondition::AcpSetupVisible
                        | protocol::WaitDetailedCondition::AcpSetupReasonCode { .. }
                        | protocol::WaitDetailedCondition::AcpSetupPrimaryAction { .. }
                        | protocol::WaitDetailedCondition::AcpSetupAgentPickerOpen
                        | protocol::WaitDetailedCondition::AcpSetupSelectedAgent { .. }
                );

                if !is_acp || detached_entity.is_none() {
                    return self.wait_condition_satisfied(condition, cx);
                }

                // ACP condition with a detached entity — read from it.
                let state = self.collect_acp_state_for_target(detached_entity, cx);
                let probe_fn = || self.collect_acp_test_probe_for_target(detached_entity, 1, cx);

                match detailed {
                    protocol::WaitDetailedCondition::AcpReady => {
                        state.context_ready && state.status == "idle"
                    }
                    protocol::WaitDetailedCondition::AcpPickerOpen => {
                        state.picker.as_ref().is_some_and(|p| p.open)
                    }
                    protocol::WaitDetailedCondition::AcpPickerClosed => {
                        state.picker.is_none() || state.picker.as_ref().is_some_and(|p| !p.open)
                    }
                    protocol::WaitDetailedCondition::AcpItemAccepted => {
                        state.last_accepted_item.is_some()
                    }
                    protocol::WaitDetailedCondition::AcpCursorAt { index } => {
                        state.cursor_index == *index
                    }
                    protocol::WaitDetailedCondition::AcpStatus { status } => {
                        state.status == *status
                    }
                    protocol::WaitDetailedCondition::AcpInputMatch { text } => {
                        state.input_text == *text
                    }
                    protocol::WaitDetailedCondition::AcpInputContains { substring } => {
                        state.input_text.contains(substring.as_str())
                    }
                    protocol::WaitDetailedCondition::AcpAcceptedViaKey { key } => {
                        let probe = probe_fn();
                        probe
                            .accepted_items
                            .last()
                            .is_some_and(|item| item.accepted_via_key == *key)
                    }
                    protocol::WaitDetailedCondition::AcpAcceptedLabel { label } => {
                        let probe = probe_fn();
                        probe
                            .accepted_items
                            .last()
                            .is_some_and(|item| item.item_label == *label)
                    }
                    protocol::WaitDetailedCondition::AcpAcceptedCursorAt { index } => {
                        let probe = probe_fn();
                        probe
                            .accepted_items
                            .last()
                            .is_some_and(|item| item.cursor_after == *index)
                    }
                    protocol::WaitDetailedCondition::AcpInputLayoutMatch {
                        visible_start,
                        visible_end,
                        cursor_in_window,
                    } => {
                        let probe = probe_fn();
                        probe.input_layout.as_ref().is_some_and(|layout| {
                            layout.visible_start == *visible_start
                                && layout.visible_end == *visible_end
                                && layout.cursor_in_window == *cursor_in_window
                        })
                    }
                    protocol::WaitDetailedCondition::AcpSetupVisible => state.setup.is_some(),
                    protocol::WaitDetailedCondition::AcpSetupReasonCode { reason_code } => state
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.reason_code == *reason_code),
                    protocol::WaitDetailedCondition::AcpSetupPrimaryAction { action } => state
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.primary_action == *action),
                    protocol::WaitDetailedCondition::AcpSetupAgentPickerOpen => {
                        state.setup.as_ref().is_some_and(|s| s.agent_picker_open)
                    }
                    protocol::WaitDetailedCondition::AcpSetupSelectedAgent { agent_id } => {
                        state.setup.as_ref().is_some_and(|s| {
                            s.selected_agent_id
                                .as_ref()
                                .is_some_and(|id| id == agent_id)
                        })
                    }
                    // Non-ACP conditions (already handled above, but required for exhaustiveness)
                    _ => self.wait_condition_satisfied(condition, cx),
                }
            }
        }
    }

    /// Get the current prompt type as a string.
    fn current_prompt_type(&self) -> String {
        match &self.current_view {
            AppView::ScriptList => "none".to_string(),
            AppView::ArgPrompt { .. } => "arg".to_string(),
            AppView::DivPrompt { .. } => "div".to_string(),
            AppView::FormPrompt { .. } => "form".to_string(),
            AppView::EditorPrompt { .. } => "editor".to_string(),
            AppView::TermPrompt { .. } => "term".to_string(),
            AppView::ChatPrompt { .. } => "chat".to_string(),
            AppView::MiniPrompt { .. } => "mini".to_string(),
            AppView::MicroPrompt { .. } => "micro".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Get the current input/filter value.
    fn current_input_value(&self) -> String {
        match &self.current_view {
            AppView::ScriptList => self.filter_text.clone(),
            AppView::ArgPrompt { .. } => self.arg_input.text().to_string(),
            AppView::MiniPrompt { .. } => self.arg_input.text().to_string(),
            AppView::MicroPrompt { .. } => self.arg_input.text().to_string(),
            _ => String::new(),
        }
    }

    /// Get the currently selected value if any.
    fn current_selected_value(&self) -> Option<String> {
        match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                filtered
                    .get(self.arg_selected_index)
                    .map(|c| c.value.clone())
            }
            _ => None,
        }
    }

    /// Collect a machine-readable ACP state snapshot.
    ///
    /// Returns a default (idle, empty) snapshot when the current view is not
    /// `AcpChatView` — callers should check `status == "notAcp"` to detect this.
    fn collect_acp_state(&self, cx: &Context<Self>) -> protocol::AcpStateSnapshot {
        let entity = match &self.current_view {
            AppView::AcpChatView { entity } => entity,
            _ => {
                return protocol::AcpStateSnapshot {
                    status: "notAcp".to_string(),
                    ..Default::default()
                };
            }
        };

        let view = entity.read(cx);

        // Extract state from the ACP view's public API.
        view.collect_acp_state_snapshot(cx)
    }

    /// Collect ACP state from the given detached entity, or fall through to main.
    fn collect_acp_state_for_target(
        &self,
        detached_entity: Option<&gpui::Entity<crate::ai::acp::view::AcpChatView>>,
        cx: &Context<Self>,
    ) -> protocol::AcpStateSnapshot {
        match detached_entity {
            Some(entity) => entity.read(cx).collect_acp_state_snapshot(cx),
            None => self.collect_acp_state(cx),
        }
    }

    /// Collect ACP test probe from the given detached entity, or fall through to main.
    fn collect_acp_test_probe_for_target(
        &self,
        detached_entity: Option<&gpui::Entity<crate::ai::acp::view::AcpChatView>>,
        tail: usize,
        cx: &Context<Self>,
    ) -> protocol::AcpTestProbeSnapshot {
        match detached_entity {
            Some(entity) => entity.read(cx).test_probe_snapshot(tail, cx),
            None => self.collect_acp_test_probe(tail, cx),
        }
    }

    /// Reset the ACP test probe ring buffer.
    fn reset_acp_test_probe(&mut self, cx: &mut Context<Self>) {
        if let AppView::AcpChatView { entity } = &self.current_view {
            entity.update(cx, |view, _cx| {
                view.reset_test_probe();
            });
        }
    }

    /// Collect a bounded ACP test probe snapshot.
    fn collect_acp_test_probe(
        &self,
        tail: usize,
        cx: &Context<Self>,
    ) -> protocol::AcpTestProbeSnapshot {
        let entity = match &self.current_view {
            AppView::AcpChatView { entity } => entity,
            _ => {
                return protocol::AcpTestProbeSnapshot {
                    state: protocol::AcpStateSnapshot {
                        status: "notAcp".to_string(),
                        ..Default::default()
                    },
                    ..Default::default()
                };
            }
        };

        let view = entity.read(cx);
        view.test_probe_snapshot(tail, cx)
    }

    /// Set the input text for the current prompt.
    fn set_input_text(&mut self, text: &str, cx: &mut Context<Self>) {
        match &self.current_view {
            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => {
                self.arg_input.set_text(text);
                self.arg_selected_index = 0;
                cx.notify();
            }
            AppView::ScriptList => {
                self.filter_text = text.to_string();
                self.selected_index = 0;
                cx.notify();
            }
            _ => {
                tracing::warn!(
                    category = "BATCH",
                    "setInput not supported for current view"
                );
            }
        }
    }

    /// Select a choice by its value from the filtered list.
    fn select_choice_by_value(
        &mut self,
        value: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        let choices = match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => choices.clone(),
            _ => anyhow::bail!("selectByValue only supports choice-backed prompts"),
        };

        let filtered = self.get_filtered_arg_choices(&choices);
        let Some(index) = filtered.iter().position(|choice| choice.value == value) else {
            anyhow::bail!("No visible choice matched value '{value}'");
        };

        self.arg_selected_index = index;
        cx.notify();

        let selected = filtered[index].value.clone();

        if submit {
            self.submit_current_value(cx);
        }

        Ok(selected)
    }

    /// Select a choice by semantic ID, optionally submitting.
    fn select_choice_by_semantic_id(
        &mut self,
        semantic_id: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        let choices = match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => choices.clone(),
            _ => anyhow::bail!("selectBySemanticId only supports choice-backed prompts"),
        };

        let filtered = self.get_filtered_arg_choices(&choices);
        let Some(index) = filtered
            .iter()
            .enumerate()
            .position(|(i, choice)| choice.generate_id(i) == semantic_id)
        else {
            anyhow::bail!("No visible choice matched semantic ID '{semantic_id}'");
        };

        self.arg_selected_index = index;
        cx.notify();

        let selected = filtered[index].value.clone();

        if submit {
            self.submit_current_value(cx);
        }

        Ok(selected)
    }

    /// Select the first choice in the filtered list.
    fn select_first_choice(
        &mut self,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Option<String>> {
        let choices = match &self.current_view {
            AppView::ArgPrompt { choices, .. }
            | AppView::MiniPrompt { choices, .. }
            | AppView::MicroPrompt { choices, .. } => choices.clone(),
            _ => anyhow::bail!("selectFirst only supports choice-backed prompts"),
        };

        let filtered = self.get_filtered_arg_choices(&choices);
        if filtered.is_empty() {
            anyhow::bail!("No visible choices to select");
        }

        self.arg_selected_index = 0;
        cx.notify();

        let selected = filtered[0].value.clone();

        if submit {
            self.submit_current_value(cx);
        }

        Ok(Some(selected))
    }

    /// Submit the currently selected value.
    fn submit_current_value(&mut self, cx: &mut Context<Self>) {
        match &self.current_view {
            AppView::ArgPrompt { id, choices, .. }
            | AppView::MiniPrompt { id, choices, .. }
            | AppView::MicroPrompt { id, choices, .. } => {
                let filtered = self.get_filtered_arg_choices(choices);
                let value = if self.arg_selected_index < filtered.len() {
                    filtered[self.arg_selected_index].value.clone()
                } else {
                    self.arg_input.text().to_string()
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(Message::Submit {
                        id: id.clone(),
                        value: Some(value),
                    });
                }
                cx.notify();
            }
            _ => {
                tracing::warn!(category = "BATCH", "submit not supported for current view");
            }
        }
    }
}

/// Get the wire name for a batch command.
fn batch_command_name(cmd: &protocol::BatchCommand) -> String {
    match cmd {
        protocol::BatchCommand::SetInput { .. } => "setInput".to_string(),
        protocol::BatchCommand::ForceSubmit { .. } => "forceSubmit".to_string(),
        protocol::BatchCommand::WaitFor { .. } => "waitFor".to_string(),
        protocol::BatchCommand::SelectByValue { .. } => "selectByValue".to_string(),
        protocol::BatchCommand::SelectBySemanticId { .. } => "selectBySemanticId".to_string(),
        protocol::BatchCommand::FilterAndSelect { .. } => "filterAndSelect".to_string(),
        protocol::BatchCommand::TypeAndSubmit { .. } => "typeAndSubmit".to_string(),
    }
}

// --- merged from part_002.rs ---
#[cfg(test)]
mod prompt_handler_message_tests {
    use super::{
        classify_prompt_message_route, escape_windows_cmd_open_target, prompt_coming_soon_warning,
        resolve_ai_start_chat_provider, should_restore_main_window_after_script_exit,
        unhandled_message_warning, PromptMessageRoute,
    };
    use crate::ai::providers::OpenAiProvider;
    use crate::PromptMessage;

    #[test]
    fn test_handle_prompt_message_routes_confirm_request_to_confirm_window() {
        let message = PromptMessage::ShowConfirm {
            id: "confirm-id".to_string(),
            message: "Continue?".to_string(),
            confirm_text: Some("Yes".to_string()),
            cancel_text: Some("No".to_string()),
        };
        assert_eq!(
            classify_prompt_message_route(&message),
            PromptMessageRoute::ConfirmDialog
        );
    }

    #[test]
    fn test_handle_prompt_message_ignores_unknown_message_without_state_corruption() {
        let message = PromptMessage::UnhandledMessage {
            message_type: "widget".to_string(),
        };
        assert_eq!(
            classify_prompt_message_route(&message),
            PromptMessageRoute::UnhandledWarning
        );

        let warning = unhandled_message_warning("widget");
        assert!(warning.contains("'widget'"));
        assert!(warning.contains("not supported yet"));
    }

    #[test]
    fn test_unhandled_message_warning_includes_recovery_guidance() {
        let message = unhandled_message_warning("widget");
        assert!(message.contains("'widget'"));
        assert!(message.contains("Update the script to a supported message type"));
        assert!(message.contains("update Script Kit GPUI"));
    }

    #[test]
    fn test_prompt_coming_soon_warning_uses_function_style_name() {
        assert_eq!(
            prompt_coming_soon_warning("fields()"),
            "fields() prompt coming soon."
        );
    }

    #[test]
    fn test_truncate_str_chars_returns_valid_utf8_boundary_when_message_is_multibyte() {
        let message = "🙂".repeat(50);
        let truncated = crate::utils::truncate_str_chars(&message, 30);

        assert_eq!(truncated.chars().count(), 30);
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn test_escape_windows_cmd_open_target_escapes_shell_metacharacters() {
        let escaped = escape_windows_cmd_open_target(r#"https://example.com/?x=1&y=2|3"#);
        assert_eq!(escaped, r#"https://example.com/?x=1^&y=2^|3"#);
    }

    #[test]
    fn test_script_exit_restores_hidden_window_only_for_active_follow_up_ui() {
        assert!(should_restore_main_window_after_script_exit(true, true));
        assert!(!should_restore_main_window_after_script_exit(true, false));
        assert!(!should_restore_main_window_after_script_exit(false, true));
    }

    #[test]
    fn test_resolve_ai_start_chat_provider_returns_registered_provider_for_model() {
        let mut registry = crate::ai::ProviderRegistry::new();
        registry.register(std::sync::Arc::new(OpenAiProvider::new("test-key")));

        assert_eq!(
            resolve_ai_start_chat_provider(&registry, "gpt-4o"),
            Some("openai".to_string())
        );
    }

    #[test]
    fn test_resolve_ai_start_chat_provider_returns_none_for_unknown_model() {
        let mut registry = crate::ai::ProviderRegistry::new();
        registry.register(std::sync::Arc::new(OpenAiProvider::new("test-key")));

        assert_eq!(
            resolve_ai_start_chat_provider(&registry, "claude-3-5-sonnet-20241022"),
            None
        );
    }

    #[test]
    fn test_build_script_error_acp_prompt_includes_fix_and_verification_guidance() {
        let prompt = build_script_error_acp_prompt(
            "/tmp/failing-script.ts",
            "ReferenceError: foo is not defined",
            Some(1),
            &["Check the missing symbol".to_string()],
        );

        assert!(prompt.contains("failing-script.ts"));
        assert!(prompt.contains("fix it"));
        assert!(prompt.contains("verify the fix"));
        assert!(prompt.contains("Exit code: 1"));
        assert!(prompt.contains("Check the missing symbol"));
    }

    #[test]
    fn test_build_script_error_report_markdown_includes_all_available_sections() {
        let report = build_script_error_report_markdown(
            "/tmp/failing-script.ts",
            "ReferenceError: foo is not defined",
            Some("stderr line 1\nstderr line 2"),
            Some(1),
            Some("stack line 1\nstack line 2"),
            &["Check the missing symbol".to_string()],
        );

        assert!(report.contains("# Script Failure Report"));
        assert!(report.contains("## Script Path"));
        assert!(report.contains("## Error Summary"));
        assert!(report.contains("## Exit Code"));
        assert!(report.contains("## Suggestions"));
        assert!(report.contains("## Stderr"));
        assert!(report.contains("## Stack Trace"));
    }

    #[test]
    fn test_persist_script_error_acp_context_bundle_writes_snapshot_and_report() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let script_path = temp_dir.path().join("failing-script.ts");
        std::fs::write(&script_path, "throw new Error('boom');").expect("write script");

        let bundle = persist_script_error_acp_context_bundle_in_dir(
            temp_dir.path(),
            script_path.to_str().expect("utf8 path"),
            "ReferenceError: foo is not defined",
            Some("stderr output"),
            Some(1),
            Some("stack trace"),
            &["Check the missing symbol".to_string()],
        )
        .expect("persist ACP context bundle");

        let script_snapshot =
            std::fs::read_to_string(&bundle.script_snapshot_path).expect("read script snapshot");
        let error_report =
            std::fs::read_to_string(&bundle.error_report_path).expect("read error report");

        assert_eq!(bundle.script_snapshot_label, "failing-script.ts");
        assert_eq!(bundle.error_report_label, "failing-script-error-report.md");
        assert_eq!(script_snapshot, "throw new Error('boom');");
        assert!(error_report.contains("ReferenceError: foo is not defined"));
        assert!(error_report.contains("stderr output"));
        assert!(error_report.contains("stack trace"));
    }
}
