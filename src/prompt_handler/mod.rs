// Prompt message handling methods - extracted from app_impl.rs
// This file is included via include!() macro in main.rs

// --- merged from part_000.rs ---
use crate::design_evaluation::prompt_fixtures::*;
use crate::prompt_completion::{PromptCompletionBinding, PromptOutcome};
include!("registered_surface.rs");
include!("batch_transport.rs");
include!("gpui_dispatch.rs");

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

include!("script_error_context.rs");

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

fn resolve_get_state_target(
    target: Option<&crate::protocol::AutomationWindowTarget>,
) -> Result<crate::protocol::AutomationWindowInfo, String> {
    let target = target.unwrap_or(&crate::protocol::AutomationWindowTarget::Main);
    let resolved = crate::windows::resolve_automation_window(Some(target))
        .map_err(|error| error.to_string())?;
    if resolved.kind == protocol::AutomationWindowKind::Main {
        let handle = resolved.generation.and_then(|generation| {
            crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)
        });
        if handle.is_none() || handle != crate::get_main_window_handle() {
            return Err("main_target_owner_mismatch".to_string());
        }
    }
    Ok(resolved)
}

/// Which window an Agent Chat read should target.
#[derive(Clone)]
enum AgentChatReadTarget {
    /// Read from the main window's Agent Chat view (current behavior).
    Main {
        info: Option<crate::protocol::AutomationWindowInfo>,
    },
    /// Read from the detached Agent Chat chat window's entity.
    Detached {
        info: crate::protocol::AutomationWindowInfo,
        entity: gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PromptPopupSubtype {
    AgentChatHistory,
    DictationMicrophone,
    Confirm,
}

fn resolve_prompt_popup_subtype(
    resolved: &crate::protocol::AutomationWindowInfo,
) -> Result<PromptPopupSubtype, crate::protocol::TransactionError> {
    let subtype = match resolved.id.as_str() {
        crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID => {
            PromptPopupSubtype::AgentChatHistory
        }
        crate::dictation::DICTATION_MICROPHONE_POPUP_AUTOMATION_ID => {
            PromptPopupSubtype::DictationMicrophone
        }
        "confirm-popup" => PromptPopupSubtype::Confirm,
        other => {
            return Err(crate::protocol::TransactionError::action_failed(format!(
                "PromptPopup target {other} is not a batch-addressable popup subtype"
            )));
        }
    };

    let is_open = match subtype {
        PromptPopupSubtype::AgentChatHistory => {
            resolved.generation.is_some()
                && crate::ai::agent_chat::ui::history_popup::is_history_popup_window_open()
        }
        PromptPopupSubtype::DictationMicrophone => {
            resolved.generation.is_some()
                && crate::dictation::is_dictation_microphone_popup_window_open()
        }
        PromptPopupSubtype::Confirm => crate::confirm::is_confirm_popup_window_open(),
    };
    if !is_open {
        return Err(crate::protocol::TransactionError::action_failed(format!(
            "PromptPopup target {} does not match a live {:?} lifetime",
            resolved.id, subtype
        )));
    }

    Ok(subtype)
}

fn revalidate_prompt_popup_target(
    expected: &crate::protocol::AutomationWindowInfo,
    subtype: PromptPopupSubtype,
) -> Result<(), crate::protocol::TransactionError> {
    let exact_target = match expected.generation {
        Some(generation) => crate::protocol::AutomationWindowTarget::Instance {
            id: expected.id.clone(),
            generation,
        },
        None => crate::protocol::AutomationWindowTarget::Id {
            id: expected.id.clone(),
        },
    };
    let current =
        crate::windows::resolve_automation_window(Some(&exact_target)).map_err(|error| {
            crate::protocol::TransactionError::action_failed(format!(
                "PromptPopup target {} became stale: {error}",
                expected.id
            ))
        })?;
    let current_subtype = resolve_prompt_popup_subtype(&current)?;
    if current_subtype != subtype || current.generation != expected.generation {
        return Err(crate::protocol::TransactionError::action_failed(format!(
            "PromptPopup target {} changed lifetime before command execution",
            expected.id
        )));
    }
    Ok(())
}

/// A resolved batch/wait target. Secondary operations retain the exact registry identity.
#[derive(Clone)]
enum AutomationReadTarget {
    Main {
        info: Option<crate::protocol::AutomationWindowInfo>,
    },
    Registered {
        info: crate::protocol::AutomationWindowInfo,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AutomationBatchTargetKind {
    Main,
    AgentChatDetached,
    Notes,
    ActionsDialog,
    PromptPopup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BatchTargetCapabilities {
    display_name: &'static str,
    unsupported_target_name: &'static str,
    supported_commands: &'static [&'static str],
    concise_unsupported_message: bool,
}

impl BatchTargetCapabilities {
    fn for_target(kind: AutomationBatchTargetKind) -> Self {
        match kind {
            AutomationBatchTargetKind::Main => Self {
                display_name: "Main",
                unsupported_target_name: "main",
                supported_commands: &[
                    "setInput",
                    "forceSubmit",
                    "waitFor",
                    "openActions",
                    "selectByValue",
                    "selectBySemanticId",
                    "filterAndSelect",
                    "typeAndSubmit",
                ],
                concise_unsupported_message: true,
            },
            AutomationBatchTargetKind::AgentChatDetached => Self {
                display_name: "Detached Agent Chat",
                unsupported_target_name: "detached Agent Chat",
                supported_commands: &["setInput", "waitFor", "selectByValue", "selectBySemanticId"],
                concise_unsupported_message: true,
            },
            AutomationBatchTargetKind::Notes => Self {
                display_name: "Notes",
                unsupported_target_name: "Notes",
                supported_commands: &[
                    "setInput",
                    "openActions",
                    "togglePreview",
                    "selectBySemanticId",
                    "waitFor",
                ],
                concise_unsupported_message: true,
            },
            AutomationBatchTargetKind::ActionsDialog => Self {
                display_name: "ActionsDialog",
                unsupported_target_name: "ActionsDialog",
                supported_commands: &["setInput", "selectByValue", "selectBySemanticId", "waitFor"],
                concise_unsupported_message: false,
            },
            AutomationBatchTargetKind::PromptPopup => Self {
                display_name: "PromptPopup",
                unsupported_target_name: "PromptPopup",
                supported_commands: &[
                    "setInput",
                    "selectByValue",
                    "selectBySemanticId",
                    "setThemeControl",
                    "waitFor",
                ],
                concise_unsupported_message: false,
            },
        }
    }
}

fn registered_batch_target_kind(
    target: &protocol::AutomationWindowInfo,
) -> AutomationBatchTargetKind {
    match target.kind {
        protocol::AutomationWindowKind::Main => AutomationBatchTargetKind::Main,
        protocol::AutomationWindowKind::AgentChatDetached => {
            AutomationBatchTargetKind::AgentChatDetached
        }
        protocol::AutomationWindowKind::Notes => AutomationBatchTargetKind::Notes,
        protocol::AutomationWindowKind::ActionsDialog => AutomationBatchTargetKind::ActionsDialog,
        _ => AutomationBatchTargetKind::PromptPopup,
    }
}

fn batch_target_kind_for_resolved_target(
    target: &AutomationReadTarget,
) -> AutomationBatchTargetKind {
    match target {
        AutomationReadTarget::Main { .. } => AutomationBatchTargetKind::Main,
        AutomationReadTarget::Registered { info } => registered_batch_target_kind(info),
    }
}

fn supported_batch_commands_for_target(kind: AutomationBatchTargetKind) -> &'static [&'static str] {
    BatchTargetCapabilities::for_target(kind).supported_commands
}

fn unsupported_batch_command_error(
    kind: AutomationBatchTargetKind,
    cmd: &protocol::BatchCommand,
) -> protocol::TransactionError {
    let command = batch_command_name(cmd);
    let capabilities = BatchTargetCapabilities::for_target(kind);
    let supported = supported_batch_commands_for_target(kind).join(", ");
    let message = match kind {
        AutomationBatchTargetKind::ActionsDialog => {
            format!("ActionsDialog batch supports: {supported}. Got: {command}")
        }
        AutomationBatchTargetKind::PromptPopup => {
            format!("PromptPopup batch supports: {supported}. Got: {command}")
        }
        _ => format!(
            "{} is not supported for {} batch targets",
            command, capabilities.unsupported_target_name
        ),
    };
    let suggestion = if capabilities.concise_unsupported_message {
        format!(
            "{} batch supports: {}.",
            capabilities.display_name, supported
        )
    } else {
        format!(
            "Use a supported command for {} targets.",
            capabilities.display_name
        )
    };

    protocol::TransactionError {
        code: protocol::TransactionErrorCode::UnsupportedCommand,
        message,
        suggestion: Some(suggestion),
    }
}

fn is_agent_chat_wait_condition(condition: &protocol::WaitCondition) -> bool {
    matches!(
        condition,
        protocol::WaitCondition::Detailed(
            protocol::WaitDetailedCondition::AgentChatReady
                | protocol::WaitDetailedCondition::AgentChatPickerOpen
                | protocol::WaitDetailedCondition::AgentChatPickerClosed
                | protocol::WaitDetailedCondition::AgentChatItemAccepted
                | protocol::WaitDetailedCondition::AgentChatCursorAt { .. }
                | protocol::WaitDetailedCondition::AgentChatStatus { .. }
                | protocol::WaitDetailedCondition::AgentChatInputMatch { .. }
                | protocol::WaitDetailedCondition::AgentChatInputContains { .. }
                | protocol::WaitDetailedCondition::AgentChatAcceptedViaKey { .. }
                | protocol::WaitDetailedCondition::AgentChatAcceptedLabel { .. }
                | protocol::WaitDetailedCondition::AgentChatAcceptedCursorAt { .. }
                | protocol::WaitDetailedCondition::AgentChatInputLayoutMatch { .. }
                | protocol::WaitDetailedCondition::AgentChatSetupVisible
                | protocol::WaitDetailedCondition::AgentChatSetupReasonCode { .. }
                | protocol::WaitDetailedCondition::AgentChatSetupPrimaryAction { .. }
                | protocol::WaitDetailedCondition::AgentChatSetupAgentPickerOpen
                | protocol::WaitDetailedCondition::AgentChatSetupSelectedAgent { .. }
        )
    )
}

fn resolve_automation_read_target(
    request_id: &str,
    op: &'static str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
    cx: &gpui::App,
) -> Result<AutomationReadTarget, crate::protocol::TransactionError> {
    let Some(target) = target else {
        return Ok(AutomationReadTarget::Main { info: None });
    };
    let resolved = crate::windows::resolve_automation_window(Some(target)).map_err(|error| {
        tracing::warn!(request_id, op, error = %error, "automation.target.resolve_failed");
        protocol::TransactionError::action_failed(format!("{op} target resolution failed: {error}"))
    })?;
    if resolved.kind == protocol::AutomationWindowKind::Main {
        let handle = resolved.generation.and_then(|generation| {
            crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)
        });
        if handle.is_none() || handle != crate::get_main_window_handle() {
            return Err(protocol::TransactionError::action_failed(
                "main_target_owner_mismatch",
            ));
        }
        return Ok(AutomationReadTarget::Main {
            info: Some(resolved),
        });
    }
    registered_surface_ui_snapshot(&resolved, cx).map_err(registered_surface_transaction_error)?;
    Ok(AutomationReadTarget::Registered { info: resolved })
}

/// Resolve an automation target for Agent Chat read operations (getAgentChatState, getAgentChatTestProbe).
///
/// Allows `Main` and `AgentChatDetached` kinds. Rejects all other secondary targets
/// with a structured error. For `AgentChatDetached`, returns the live entity from the
/// detached chat window (or errors if no detached window is open).
fn resolve_agent_chat_read_target(
    request_id: &str,
    op: &'static str,
    target: Option<&crate::protocol::AutomationWindowTarget>,
) -> Result<AgentChatReadTarget, crate::protocol::TransactionError> {
    // No explicit target → default to main window (preserves existing behavior).
    let Some(target) = target else {
        return Ok(AgentChatReadTarget::Main { info: None });
    };

    let resolved = crate::windows::resolve_automation_window(Some(target)).map_err(|err| {
        tracing::warn!(
            target: "script_kit::automation",
            request_id = %request_id,
            op = op,
            error = %err,
            "automation.agent_chat_target.resolve_failed"
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
                "automation.agent_chat_target.main"
            );
            Ok(AgentChatReadTarget::Main {
                info: Some(resolved),
            })
        }
        crate::protocol::AutomationWindowKind::AgentChatDetached => {
            registered_surface_target(&resolved).map_err(registered_surface_transaction_error)?;
            match resolved.generation.and_then(|generation| {
                crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                    &resolved.id,
                    generation,
                )
            }) {
                Some(entity) => {
                    tracing::info!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        op = op,
                        window_id = %resolved.id,
                        kind = ?resolved.kind,
                        "automation.agent_chat_target.detached_resolved"
                    );
                    Ok(AgentChatReadTarget::Detached {
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
                        "automation.agent_chat_target.detached_no_entity"
                    );
                    Err(crate::protocol::TransactionError::action_failed(format!(
                        "{op} resolved detached Agent Chat target {} but no live view entity is available \
                         (window may be a placeholder or closed)",
                        resolved.id
                    )))
                }
            }
        }
        crate::protocol::AutomationWindowKind::Notes => {
            // The Notes window no longer hosts an Agent Chat surface: every
            // notes→AI affordance stages into the MAIN window's Agent Chat.
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                op = op,
                window_id = %resolved.id,
                "automation.agent_chat_target.notes_not_a_chat_host"
            );
            Err(crate::protocol::TransactionError::action_failed(format!(
                "{op} resolved Notes target {}, but Notes no longer hosts an Agent Chat surface; target the main window instead",
                resolved.id
            )))
        }
        other_kind => {
            tracing::warn!(
                target: "script_kit::automation",
                request_id = %request_id,
                op = op,
                window_id = %resolved.id,
                kind = ?other_kind,
                "automation.agent_chat_target.non_agent_chat_rejected"
            );
            Err(crate::protocol::TransactionError::action_failed(format!(
                "{op} supports only Main, Ai, and AgentChatDetached targets; resolved {} ({:?})",
                resolved.id, other_kind
            )))
        }
    }
}

/// Build an `AgentChatResolvedTarget` from a resolved `AgentChatReadTarget` and emit
/// a structured `agent_chat_target_resolved` log line.
fn build_agent_chat_resolved_target(
    request_id: &str,
    op: &'static str,
    agent_chat_target: &AgentChatReadTarget,
) -> Option<crate::protocol::AgentChatResolvedTarget> {
    let (window_id, window_kind, title) = match agent_chat_target {
        AgentChatReadTarget::Main { info } => {
            if let Some(info) = info {
                (
                    info.id.clone(),
                    info.kind.as_camel_case().to_string(),
                    info.title.clone(),
                )
            } else {
                (
                    "main".to_string(),
                    crate::protocol::AutomationWindowKind::Main
                        .as_camel_case()
                        .to_string(),
                    Some("Script Kit".to_string()),
                )
            }
        }
        AgentChatReadTarget::Detached { info, .. } => (
            info.id.clone(),
            info.kind.as_camel_case().to_string(),
            info.title.clone(),
        ),
    };

    tracing::info!(
        target: "script_kit::automation",
        event = "agent_chat_target_resolved",
        request_id = %request_id,
        window_id = %window_id,
        kind = %window_kind,
        title = ?title,
        op = op,
    );

    Some(crate::protocol::AgentChatResolvedTarget {
        window_id,
        window_kind,
        title,
    })
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

include!("message_route.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DevtoolsSelectionState {
    MainMenuScriptList,
    ChoiceBackedPrompt,
    UnsupportedPrompt,
}

// --- merged from part_001.rs ---
impl ScriptListApp {
    pub(crate) fn build_automation_inspect_snapshot(
        &self,
        request_id: &str,
        target: Option<&protocol::AutomationWindowTarget>,
        hi_dpi: Option<bool>,
        probes: &[protocol::PixelProbe],
        cx: &Context<Self>,
    ) -> protocol::AutomationInspectSnapshot {
        tracing::info!(
            target: "script_kit::automation",
            request_id = %request_id,
            target = ?target,
            probe_count = probes.len(),
            "automation.inspect.request"
        );

        // Step 1: Resolve the automation window target.
        let resolved = match crate::windows::resolve_automation_window(target) {
            Ok(info) => info,
            Err(err) => {
                return protocol::AutomationInspectSnapshot {
                    schema_version: protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
                    window_id: String::new(),
                    window_kind: "unknown".to_string(),
                    surface_kind: None,
                    app_view_variant: None,
                    native_footer_surface: None,
                    target_generation: None,
                    surface_generation: None,
                    data_generation: None,
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
                    pid: None,
                };
            }
        };
        let main_target_owned = resolved.kind == protocol::AutomationWindowKind::Main
            && resolved
                .generation
                .and_then(|generation| {
                    crate::windows::get_runtime_window_handle_for_generation(
                        &resolved.id,
                        generation,
                    )
                })
                .is_some_and(|handle| Some(handle) == crate::get_main_window_handle());

        // Step 2: Capture RGBA image for dimensions and pixel probes.
        let hi_dpi_mode = hi_dpi.unwrap_or(false);
        let rgba_result = match crate::runtime_policy::check(
            crate::runtime_policy::ExternalEffect::ScreenCapture,
        ) {
            Ok(()) => crate::platform::capture_targeted_rgba_image(target, hi_dpi_mode),
            Err(error) => Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
        };

        let (shot_w, shot_h, probe_results, mut warnings) = match rgba_result {
            Ok(ref rgba_image) => {
                let w = rgba_image.width();
                let h = rgba_image.height();
                let mut results = Vec::with_capacity(probes.len());
                for probe in probes {
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
        let (surface_snapshot, semantic_quality) = if main_target_owned {
            let outcome = self.collect_visible_elements(200, cx);
            (
                crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                    total_count: outcome.total_count,
                    focused_semantic_id: outcome.focused_semantic_id(),
                    selected_semantic_id: outcome.selected_semantic_id(),
                    warnings: outcome.warnings.clone(),
                    elements: outcome.elements,
                    quality: crate::windows::automation_surface_collector::SnapshotQuality::Full,
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
        let os_window_id = if crate::runtime_policy::is_owned_evaluation() {
            None
        } else {
            crate::platform::resolve_targeted_os_window_id(target)
        };

        // Step 5: Compute screenshot-relative geometry for the target surface.
        let target_bounds_in_screenshot = protocol::target_bounds_in_screenshot(&resolved);
        let surface_hit_point = target_bounds_in_screenshot
            .as_ref()
            .map(protocol::default_surface_hit_point);
        let suggested_hit_points =
            protocol::default_suggested_hit_points(&resolved, target_bounds_in_screenshot.as_ref());

        tracing::info!(
            target: "script_kit::automation",
            request_id = %request_id,
            window_id = %resolved.id,
            target_bounds_in_screenshot = ?target_bounds_in_screenshot,
            suggested_hit_count = suggested_hit_points.len(),
            "automation.inspect.geometry_computed"
        );

        let surface_kind =
            main_target_owned.then(|| format!("{:?}", self.current_view.surface_kind()));
        let app_view_variant =
            main_target_owned.then(|| self.current_view.app_view_variant().to_string());
        let native_footer_surface = main_target_owned
            .then(|| {
                self.current_view
                    .native_footer_surface()
                    .map(str::to_string)
            })
            .flatten();
        let target_generation = resolved.generation.and_then(|generation| {
            crate::windows::automation_registry::automation_target_revision(
                &resolved.id,
                generation,
            )
        });
        let revision_facts = if main_target_owned {
            let facts = self.owned_revision_facts();
            Some((
                facts.surface_generation,
                facts.data_generation,
                facts.presentation_revision,
                crate::theme::service::theme_revision(),
            ))
        } else {
            crate::windows::automation_surface_collector::surface_revision_facts(
                &resolved, None, cx,
            )
        };

        let snapshot = protocol::AutomationInspectSnapshot {
            schema_version: protocol::AUTOMATION_INSPECT_SCHEMA_VERSION,
            window_id: resolved.id.clone(),
            window_kind: format!("{:?}", resolved.kind),
            surface_kind,
            app_view_variant,
            native_footer_surface,
            target_generation,
            surface_generation: revision_facts.map(|facts| facts.0),
            data_generation: revision_facts.map(|facts| facts.1),
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
            pid: resolved.pid,
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

        snapshot
    }

    pub(crate) fn handle_stdin_protocol_message(
        &mut self,
        message: crate::protocol::Message,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(prompt_message) = prompt_message_from_protocol_message(message.clone()) {
            self.handle_prompt_message_in_window(prompt_message, window, cx);
            return;
        }

        match message {
            Message::CaptureScreenshot {
                request_id,
                hi_dpi,
                target,
            } => {
                let hi_dpi_mode = hi_dpi.unwrap_or(false);
                let response = match crate::platform::capture_targeted_screenshot(
                    target.as_ref(),
                    hi_dpi_mode,
                ) {
                    Ok((png_data, width, height)) => {
                        use base64::Engine;
                        let base64_data =
                            base64::engine::general_purpose::STANDARD.encode(&png_data);
                        tracing::info!(
                            category = "STDIN",
                            request_id = %request_id,
                            width,
                            height,
                            hi_dpi = hi_dpi_mode,
                            data_len = base64_data.len(),
                            "captureScreenshot receipt"
                        );
                        Message::screenshot_result(request_id, base64_data, width, height)
                    }
                    Err(e) => {
                        tracing::error!(
                            category = "STDIN",
                            request_id = %request_id,
                            error = %e,
                            "captureScreenshot failed"
                        );
                        Message::screenshot_error(request_id, e.to_string())
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for captureScreenshot"
                    );
                }
            }
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
            Message::GetLogs {
                request_id,
                limit,
                level,
                target,
                contains,
            } => {
                let limit = limit.unwrap_or(100);
                let (entries, matched) = crate::logging::query_log_ring(
                    limit,
                    level.as_deref(),
                    target.as_deref(),
                    contains.as_deref(),
                );
                let entries = entries
                    .into_iter()
                    .filter_map(|entry| serde_json::to_value(entry).ok())
                    .collect();
                let response = Message::LogsResult {
                    request_id,
                    entries,
                    matched,
                    capacity: crate::logging::LOG_RING_CAPACITY,
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for getLogs"
                    );
                }
            }
            Message::CheckAccessibility { request_id } => {
                let granted = crate::permissions_wizard::check_accessibility_permission();
                tracing::info!(
                    category = "STDIN",
                    event_type = "check_accessibility_result",
                    request_id = %request_id,
                    granted,
                    "checkAccessibility receipt"
                );
                let response = Message::accessibility_status(granted, request_id);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for checkAccessibility"
                    );
                }
            }
            Message::GetWindowBounds { request_id } => {
                let bounds = crate::windows::list_automation_windows()
                    .into_iter()
                    .find(|w| w.id == "main")
                    .and_then(|w| w.bounds);
                let (x, y, width, height, bounds_available) = match bounds {
                    Some(b) => (b.x, b.y, b.width, b.height, true),
                    None => (0.0, 0.0, 0.0, 0.0, false),
                };
                tracing::info!(
                    category = "STDIN",
                    event_type = "get_window_bounds_result",
                    request_id = %request_id,
                    x,
                    y,
                    width,
                    height,
                    bounds_available,
                    "getWindowBounds receipt"
                );
                let response = Message::window_bounds(x, y, width, height, request_id);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for getWindowBounds"
                    );
                }
            }
            Message::FrontmostWindow { request_id } => {
                let (window_opt, error_opt) =
                    match crate::window_control::get_frontmost_window_of_previous_app() {
                        Ok(Some(window)) => {
                            let window_info = crate::protocol::SystemWindowInfo {
                                window_id: window.id,
                                title: window.title,
                                app_name: window.app,
                                bounds: Some(crate::protocol::TargetWindowBounds {
                                    x: window.bounds.x,
                                    y: window.bounds.y,
                                    width: window.bounds.width,
                                    height: window.bounds.height,
                                }),
                                is_minimized: None,
                                is_active: Some(true),
                            };
                            (Some(window_info), None)
                        }
                        Ok(None) => (None, Some("No frontmost window found".to_string())),
                        Err(e) => (None, Some(e.to_string())),
                    };
                tracing::info!(
                    category = "STDIN",
                    event_type = "frontmost_window_result",
                    request_id = %request_id,
                    window_present = window_opt.is_some(),
                    error_present = error_opt.is_some(),
                    "frontmostWindow receipt"
                );
                let response = Message::frontmost_window_result(request_id, window_opt, error_opt);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for frontmostWindow"
                    );
                }
            }
            Message::GetSelectedText { request_id } => {
                let (text, error_present) = match crate::selected_text::get_selected_text() {
                    Ok(text) => (text, false),
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            request_id = %request_id,
                            error = %e,
                            "getSelectedText probe failed; returning empty text"
                        );
                        (String::new(), true)
                    }
                };
                tracing::info!(
                    category = "STDIN",
                    event_type = "get_selected_text_result",
                    request_id = %request_id,
                    text_len = text.len(),
                    error_present,
                    "getSelectedText receipt"
                );
                let response = Message::selected_text_response(text, request_id);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for getSelectedText"
                    );
                }
            }
            Message::CaptureFocusedText { request_id } => {
                let response = match crate::platform::accessibility::capture_focused_text_field(
                    crate::platform::accessibility::CaptureFocusedTextOptions::default(),
                ) {
                    Ok(snapshot) => {
                        tracing::info!(
                            category = "STDIN",
                            event_type = "capture_focused_text_result",
                            request_id = %request_id,
                            text_len = snapshot.text.len(),
                            char_count = snapshot.metrics.chars,
                            app_name = %snapshot.app.name,
                            success = true,
                            "captureFocusedText receipt"
                        );
                        Message::focused_text_snapshot_response(
                            serde_json::json!({
                                "sessionId": snapshot.session_id.to_string(),
                                "capturedAtMs": snapshot.captured_at_ms,
                                "app": {
                                    "name": snapshot.app.name,
                                    "bundleId": snapshot.app.bundle_id,
                                    "processId": snapshot.app.process_id,
                                },
                                "text": snapshot.text,
                                "metrics": {
                                    "bytes": snapshot.metrics.bytes,
                                    "chars": snapshot.metrics.chars,
                                    "utf16Units": snapshot.metrics.utf16_units,
                                    "lines": snapshot.metrics.lines,
                                    "estimatedTokens": snapshot.metrics.estimated_tokens,
                                },
                                "capabilities": {
                                    "canReplace": snapshot.capabilities.can_replace,
                                    "canAppend": snapshot.capabilities.can_append,
                                    "canCopy": snapshot.capabilities.can_copy,
                                }
                            }),
                            request_id,
                        )
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            event_type = "capture_focused_text_result",
                            request_id = %request_id,
                            success = false,
                            error = %e,
                            "captureFocusedText probe failed"
                        );
                        Message::focused_text_snapshot_error(e.to_string(), request_id)
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for captureFocusedText"
                    );
                }
            }
            Message::RequestAccessibility { request_id } => {
                let granted = crate::permissions_wizard::request_accessibility_permission();
                tracing::info!(
                    category = "STDIN",
                    event_type = "request_accessibility_result",
                    request_id = %request_id,
                    granted,
                    "requestAccessibility receipt"
                );
                let response = Message::accessibility_status(granted, request_id);
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for requestAccessibility"
                    );
                }
            }
            Message::SetSelectedText { text, request_id } => {
                let text_len = text.len();
                let response = match crate::selected_text::set_selected_text(&text) {
                    Ok(()) => {
                        tracing::info!(
                            category = "STDIN",
                            event_type = "set_selected_text_result",
                            request_id = %request_id,
                            text_len,
                            success = true,
                            "setSelectedText receipt"
                        );
                        Message::text_set_success(request_id)
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            event_type = "set_selected_text_result",
                            request_id = %request_id,
                            text_len,
                            success = false,
                            error = %e,
                            "setSelectedText probe failed"
                        );
                        Message::text_set_error(e.to_string(), request_id)
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for setSelectedText"
                    );
                }
            }
            Message::ReplaceFocusedText {
                session_id,
                text,
                request_id,
            } => {
                let text_len = text.len();
                let response = match crate::platform::accessibility::replace_focused_text(
                    crate::platform::accessibility::FocusedTextSessionId(session_id),
                    &text,
                    crate::platform::accessibility::TextMutationOptions::default(),
                ) {
                    Ok(_) => {
                        tracing::info!(
                            category = "STDIN",
                            event_type = "replace_focused_text_result",
                            request_id = %request_id,
                            text_len,
                            success = true,
                            "replaceFocusedText receipt"
                        );
                        Message::focused_text_mutation_response("replace".to_string(), request_id)
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            event_type = "replace_focused_text_result",
                            request_id = %request_id,
                            text_len,
                            success = false,
                            error = %e,
                            "replaceFocusedText failed"
                        );
                        Message::focused_text_mutation_error(
                            "replace".to_string(),
                            e.to_string(),
                            request_id,
                        )
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for replaceFocusedText"
                    );
                }
            }
            Message::AppendFocusedText {
                session_id,
                text,
                request_id,
            } => {
                let text_len = text.len();
                let response = match crate::platform::accessibility::append_focused_text(
                    crate::platform::accessibility::FocusedTextSessionId(session_id),
                    &text,
                    crate::platform::accessibility::TextMutationOptions::default(),
                ) {
                    Ok(_) => {
                        tracing::info!(
                            category = "STDIN",
                            event_type = "append_focused_text_result",
                            request_id = %request_id,
                            text_len,
                            success = true,
                            "appendFocusedText receipt"
                        );
                        Message::focused_text_mutation_response("append".to_string(), request_id)
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            event_type = "append_focused_text_result",
                            request_id = %request_id,
                            text_len,
                            success = false,
                            error = %e,
                            "appendFocusedText failed"
                        );
                        Message::focused_text_mutation_error(
                            "append".to_string(),
                            e.to_string(),
                            request_id,
                        )
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for appendFocusedText"
                    );
                }
            }
            Message::CopyFocusedTextOutput { text, request_id } => {
                let text_len = text.len();
                let response = match crate::platform::accessibility::copy_text_output(&text) {
                    Ok(_) => {
                        tracing::info!(
                            category = "STDIN",
                            event_type = "copy_focused_text_output_result",
                            request_id = %request_id,
                            text_len,
                            success = true,
                            "copyFocusedTextOutput receipt"
                        );
                        Message::focused_text_mutation_response("copy".to_string(), request_id)
                    }
                    Err(e) => {
                        tracing::warn!(
                            category = "STDIN",
                            event_type = "copy_focused_text_output_result",
                            request_id = %request_id,
                            text_len,
                            success = false,
                            error = %e,
                            "copyFocusedTextOutput failed"
                        );
                        Message::focused_text_mutation_error(
                            "copy".to_string(),
                            e.to_string(),
                            request_id,
                        )
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                } else {
                    tracing::warn!(
                        category = "STDIN",
                        "No response sender available for copyFocusedTextOutput"
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

    /// Reveal and size only the interactive host after successful construction.
    fn prepare_constructed_sdk_prompt(
        &mut self,
        kind: &str,
        deferred: bool,
        cx: &mut Context<Self>,
    ) {
        if crate::runtime_policy::is_owned_evaluation() {
            return;
        }
        self.prepare_window_for_prompt("UI", kind, "");
        if deferred {
            let expected = self
                .prompt_completion
                .as_ref()
                .map(|binding| binding.instance().clone());
            cx.spawn(async move |this, cx| {
                let target = this
                    .update(cx, |app, cx| {
                        if app
                            .prompt_completion
                            .as_ref()
                            .map(|binding| binding.instance())
                            == expected.as_ref()
                        {
                            app.calculate_window_size_params_with_app(Some(cx))
                        } else {
                            None
                        }
                    })
                    .ok()
                    .flatten();
                if let Some((view_type, item_count)) = target {
                    resize_to_view_sync(view_type, item_count);
                }
            })
            .detach();
        } else if let Some((view_type, item_count)) =
            self.calculate_window_size_params_with_app(Some(cx))
        {
            resize_to_view_sync(view_type, item_count);
        }
    }

    pub(crate) fn make_submit_callback(
        &self,
        dropped_label: &'static str,
    ) -> Arc<dyn Fn(String, Option<String>) + Send + Sync> {
        if let Some(binding) = self
            .prompt_completion
            .as_ref()
            .filter(|binding| !binding.observation().retired)
        {
            return binding.submit_callback();
        }
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
        if crate::runtime_policy::is_owned_evaluation() {
            return;
        }
        // Clear NEEDS_RESET when receiving a UI prompt from an active script.
        // This prevents the window from resetting when shown.
        if NEEDS_RESET.swap(false, Ordering::SeqCst) {
            tracing::info!(
                category = log_target,
                prompt_kind = %prompt_kind,
                "Cleared NEEDS_RESET - script is showing prompt UI"
            );
        }
        clear_main_state_restore_after_focus_loss();

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

    fn show_prompt_coming_soon_toast(&mut self, prompt_name: &str, cx: &mut Context<Self>) {
        let toast = Toast::warning(prompt_coming_soon_warning(prompt_name), &self.theme)
            .duration_ms(Some(TOAST_WARNING_MS));
        self.toast_manager.push(toast);
        cx.notify();
    }

    /// Handle a prompt message from the script.
    fn handle_prompt_message(&mut self, msg: PromptMessage, cx: &mut Context<Self>) {
        self.handle_prompt_message_with_window(msg, None, cx);
    }

    /// Handle a prompt message while the caller already owns the main window update.
    ///
    /// Stdin protocol requests run inside `window_for_stdin.update`, so attempting
    /// to re-enter that window through its global handle fails. Carrying the live
    /// window reference lets layout receipts read the completed paint frame without
    /// a nested update or an asynchronous response race.
    fn handle_prompt_message_in_window(
        &mut self,
        msg: PromptMessage,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.handle_prompt_message_with_window(msg, Some(window), cx);
    }

    #[tracing::instrument(skip(self, current_window, cx), fields(msg_type = ?msg))]
    fn handle_prompt_message_with_window(
        &mut self,
        msg: PromptMessage,
        current_window: Option<&mut gpui::Window>,
        cx: &mut Context<Self>,
    ) {
        let route = classify_prompt_message_route(&msg);
        tracing::debug!(target: "prompt_handler", ?route, "Routing prompt message");

        match msg {
            PromptMessage::ShowArg {
                id,
                placeholder,
                choices,
                actions,
            } => {
                let seed = PromptSeed::Arg(ChoicePromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    placeholder,
                    choices,
                    input: String::new(),
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("arg", false, cx);
            }
            PromptMessage::ShowMini {
                id,
                placeholder,
                choices,
            } => {
                let seed = PromptSeed::Mini(ChoicePromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    placeholder,
                    choices,
                    input: String::new(),
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("mini", false, cx);
            }
            PromptMessage::ShowMicro {
                id,
                placeholder,
                choices,
            } => {
                let seed = PromptSeed::Micro(ChoicePromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    placeholder,
                    choices,
                    input: String::new(),
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("micro", false, cx);
            }
            PromptMessage::ShowDiv {
                id,
                html,
                container_classes,
                actions,
                placeholder: _placeholder, // KNOWN: Not rendered; wiring requires DivPrompt render-surface changes.
                hint: _hint, // KNOWN: Not rendered; wiring requires DivPrompt render-surface changes.
                footer: _footer, // KNOWN: Not rendered; wiring requires DivPrompt render-surface changes.
                container_bg,
                container_padding,
                opacity,
            } => {
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

                let seed = PromptSeed::Div(DivPromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    html,
                    options: container_options,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("div", false, cx);
            }
            PromptMessage::ShowForm { id, html, actions } => {
                let seed = PromptSeed::Form(FormPromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    html,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("form", false, cx);
            }
            PromptMessage::ShowFields {
                id,
                fields,
                actions,
            } => {
                let seed = PromptSeed::Fields(FieldsPromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    fields,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("fields", false, cx);
            }
            PromptMessage::ShowTerm {
                id,
                command,
                actions,
            } => {
                if let Err(error) =
                    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process)
                {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                let terminal = match command {
                    Some(command) => crate::terminal::TerminalHandle::with_command_and_theme(
                        &command,
                        80,
                        24,
                        &self.theme,
                    ),
                    None => crate::terminal::TerminalHandle::new_with_theme(80, 24, &self.theme),
                };
                let terminal = match terminal {
                    Ok(terminal) => terminal,
                    Err(error) => {
                        self.show_error_toast(error.to_string(), cx);
                        return;
                    }
                };
                let seed = PromptSeed::Term(TerminalPromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    terminal,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("term", true, cx);
            }
            PromptMessage::ShowEditor {
                id,
                content,
                language,
                template,
                actions,
            } => {
                let seed = PromptSeed::Editor(EditorPromptSeed {
                    common: PromptSeedCommon::sdk(id, actions, self.response_sender.clone()),
                    content: content.unwrap_or_default(),
                    language: language.unwrap_or_else(|| "markdown".to_string()),
                    template,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("editor", true, cx);
            }

            PromptMessage::ScriptExit => {
                tracing::info!(
                    category = "VISIBILITY",
                    "=== ScriptExit message received ==="
                );
                if let Some(binding) = &self.prompt_completion {
                    binding.retire();
                }
                if crate::runtime_policy::is_owned_evaluation() {
                    self.reset_to_script_list(cx);
                    return;
                }

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
                let keep_agent_chat_open =
                    matches!(self.current_view, AppView::AgentChatView { .. });

                if keep_tab_ai_save_offer_open {
                    tracing::info!(
                        category = "VISIBILITY",
                        keep_tab_ai_save_offer_open,
                        keep_agent_chat_open,
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
                } else if keep_agent_chat_open {
                    tracing::info!(
                        category = "VISIBILITY",
                        keep_tab_ai_save_offer_open,
                        keep_agent_chat_open,
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
                let error_diagnostic = crate::ai::reliability::redact_diagnostic(&error_message);
                tracing::error!(
                    category = "ERROR",
                    diagnostic_fingerprint = %error_diagnostic.fingerprint.0,
                    exit_code = ?exit_code,
                    script_path = %script_path,
                    "Script error received"
                );
                if let Some(ref stderr) = stderr_output {
                    let diagnostic = crate::ai::reliability::redact_diagnostic(stderr);
                    tracing::error!(
                        category = "ERROR",
                        script_path = %script_path,
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        diagnostic_bytes = stderr.len(),
                        "Script stderr output"
                    );
                }
                if let Some(ref trace) = stack_trace {
                    let diagnostic = crate::ai::reliability::redact_diagnostic(trace);
                    tracing::error!(
                        category = "ERROR",
                        script_path = %script_path,
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        diagnostic_bytes = trace.len(),
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
                let log_path = script_kit_gpui::scriptlet_cache::get_log_file_path();
                let toast = build_script_error_toast(
                    error_message.clone(),
                    details_text,
                    &script_path,
                    &log_path,
                    &self.theme,
                );

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

                self.route_script_error_to_agent_chat(
                    ScriptErrorAgentChatContext {
                        script_path: &script_path,
                        error_message: &error_message,
                        stderr_output: stderr_output.as_deref(),
                        exit_code,
                        stack_trace: stack_trace.as_deref(),
                        suggestions: &suggestions,
                    },
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
                        "copy-details",
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

            PromptMessage::GetState {
                request_id,
                target,
                summary_only,
            } => {
                tracing::info!(
                    category = "UI",
                    request_id = %request_id,
                    target = ?target,
                    "Collecting state for request"
                );

                let main_target =
                    match resolve_get_state_target(target.as_ref()).and_then(|resolved| {
                        if resolved.kind == protocol::AutomationWindowKind::Main {
                            return Ok(Some(resolved));
                        }
                        let response = registered_surface_state_result(&request_id, &resolved, cx)
                            .map_err(|error| error.to_string())?;
                        if let Some(sender) = &self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        Ok(None)
                    }) {
                        Ok(Some(resolved)) => resolved,
                        Ok(None) => return,
                        Err(error) => {
                            if let Some(ref sender) = self.response_sender {
                                let _ = sender.try_send(Message::state_result(
                                    request_id.clone(),
                                    "target_resolution_failed".to_string(),
                                    Some(format!("target_error:{}", error)),
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    String::new(),
                                    0,
                                    0,
                                    -1,
                                    None,
                                    false,
                                    false,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                    None,
                                ));
                            }
                            return;
                        }
                    };

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
                        let (visible_rows, selected_row_index) =
                            self.script_list_visible_row_labels_from_cache();
                        let selected_value =
                            selected_row_index.and_then(|index| visible_rows.get(index).cloned());
                        let selected_grouped_index = self
                            .resolved_main_menu_selected_subject()
                            .map(|subject| match subject {
                                ResolvedMainMenuSelection::SearchResult { row, .. }
                                | ResolvedMainMenuSelection::Calculator { row, .. } => {
                                    row.grouped_index
                                }
                            });
                        (
                            "none".to_string(),
                            None,
                            None,
                            self.filter_text.clone(),
                            self.scripts.len()
                                + self.scriptlets.len()
                                + self.builtin_entries.len()
                                + self.apps.len()
                                + self.skills.len()
                                + self.flow_desk_corpus().len(),
                            visible_rows.len(),
                            selected_grouped_index.map_or(-1, |index| index as i32),
                            selected_value,
                        )
                    }
                    AppView::About { .. } => (
                        "about".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
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
                    AppView::FormPrompt { id, entity } => {
                        let prompt_type = entity.read(cx).prompt_type().to_string();
                        (
                            prompt_type,
                            Some(id.clone()),
                            None,
                            String::new(),
                            0,
                            0,
                            -1,
                            None,
                        )
                    }
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
                    AppView::HotkeyPrompt { id, .. } => (
                        "hotkey".to_string(),
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
                        let (dataset_count, visible_count) =
                            self.clipboard_history_dataset_and_visible_counts(filter);
                        let selected_value = self
                            .clipboard_history_selected_visible_row(filter, *selected_index)
                            .map(|(_, entry)| entry.text_preview);
                        (
                            "clipboardHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            dataset_count,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::AgentChatHistoryView {
                        filter,
                        selected_index,
                    } => {
                        let (dataset_count, visible_count) =
                            Self::agent_chat_history_dataset_and_visible_counts(filter);
                        let selected_value =
                            Self::agent_chat_history_selected_visible_row(filter, *selected_index)
                                .map(|entry| entry.title_display().to_string());
                        (
                            "agent_chatHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            dataset_count,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::BrowserHistoryView {
                        filter,
                        selected_index,
                    } => {
                        let filtered_entries: Vec<crate::browser_history::BrowserHistoryEntry> =
                            crate::browser_history::fuzzy_search_browser_history(
                                &self.cached_browser_history,
                                filter,
                            )
                            .into_iter()
                            .map(|entry| entry.entry)
                            .collect();
                        let selected_value = filtered_entries
                            .get(*selected_index)
                            .map(|entry| entry.display_title().to_string());
                        (
                            "browserHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            self.cached_browser_history.len(),
                            filtered_entries.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::DictationHistoryView {
                        filter,
                        selected_index,
                        visible_limit,
                    } => {
                        let (dataset_count, visible_count) = self
                            .dictation_history_dataset_and_visible_counts(filter, *visible_limit);
                        let selected_value = self
                            .dictation_history_selected_visible_row(
                                filter,
                                *selected_index,
                                *visible_limit,
                            )
                            .map(|entry| entry.preview);
                        (
                            "dictationHistory".to_string(),
                            None,
                            None,
                            filter.clone(),
                            dataset_count,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::NotesBrowseView { search } => {
                        let (dataset_count, visible_count) =
                            Self::notes_browse_dataset_and_visible_counts(search);
                        let selected_value = Self::notes_browse_selected_visible_row(search)
                            .map(|entry| entry.title);
                        (
                            "notesBrowse".to_string(),
                            None,
                            None,
                            search.query.clone(),
                            dataset_count,
                            visible_count,
                            search.selected_index() as i32,
                            selected_value,
                        )
                    }
                    // P0 FIX: View state only - data comes from self.apps
                    AppView::AppLauncherView {
                        filter,
                        selected_index,
                    } => {
                        let (dataset_count, visible_count) =
                            self.app_launcher_dataset_and_visible_counts(filter);
                        (
                            "appLauncher".to_string(),
                            None,
                            None,
                            filter.clone(),
                            dataset_count,
                            visible_count,
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
                    AppView::BrowserTabsView {
                        filter,
                        selected_index,
                    } => {
                        let (dataset_count, visible_count) =
                            self.browser_tabs_dataset_and_visible_counts(filter);
                        let selected_value = self
                            .browser_tabs_selected_visible_row(filter, *selected_index)
                            .map(|tab| tab.display_title().to_string());
                        (
                            "browserTabs".to_string(),
                            None,
                            None,
                            filter.clone(),
                            dataset_count,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
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
                    AppView::FlowSessionView { .. } => (
                        "flowSession".to_string(),
                        Some("flow-session".to_string()),
                        None,
                        // The shared MAIN input is the composer; its draft is
                        // the session's observable input value.
                        self.filter_text.clone(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::FileSearchView {
                        ref query,
                        selected_index,
                        ..
                    } => {
                        let selection = self.file_search_selection_binding(*selected_index);
                        (
                            "fileSearch".to_string(),
                            Some("file-search".to_string()),
                            None,
                            query.clone(),
                            self.cached_file_results.len(),
                            self.file_search_display_indices.len(),
                            selection
                                .projection
                                .map(|projection| projection.display_index as i32)
                                .unwrap_or(-1),
                            selection.file.as_ref().map(|file| file.name.clone()),
                        )
                    }
                    AppView::ProfileSearchView {
                        filter,
                        selected_index,
                    } => {
                        let results = self.profile_search_results_for_filter(filter);
                        let selected_value = results
                            .get(*selected_index)
                            .map(|result| result.profile.name.clone());
                        (
                            "profileSearch".to_string(),
                            Some("profile-search".to_string()),
                            Some("Search profiles...".to_string()),
                            filter.clone(),
                            results.len(),
                            results.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ThemeChooserView {
                        filter,
                        selected_index,
                    } => {
                        let catalog = Self::theme_chooser_catalog();
                        let filtered =
                            Self::theme_chooser_catalog_filtered_indices(filter, &catalog);
                        let selected_name = filtered
                            .get(*selected_index)
                            .and_then(|idx| catalog.get(*idx))
                            .map(|entry| entry.name.clone());
                        (
                            "themeChooser".to_string(),
                            Some("theme-chooser".to_string()),
                            None,
                            filter.clone(),
                            // choice_count = total dataset, visible_choice_count = filter-aware
                            // (same contract as the EmojiPickerView arm below; these were
                            // swapped, so visibleChoiceCount froze at the catalog total).
                            catalog.len(),
                            filtered.len(),
                            *selected_index as i32,
                            selected_name,
                        )
                    }
                    AppView::EmojiPickerView {
                        filter,
                        selected_index,
                        selected_category,
                    } => {
                        let dataset_count = crate::emoji::EMOJIS
                            .iter()
                            .filter(|emoji| {
                                selected_category
                                    .map(|category| emoji.category == category)
                                    .unwrap_or(true)
                            })
                            .count();
                        let visible_count = crate::emoji::search_emojis(filter)
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
                            dataset_count,
                            visible_count,
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
                    } => {
                        let (total, visible_count) =
                            Self::kit_store_browse_dataset_and_visible_counts(results);
                        let selected_value = Self::kit_store_browse_selected_visible_result(
                            results,
                            *selected_index,
                        )
                        .map(|result| result.full_name);
                        (
                            "browseKits".to_string(),
                            None,
                            None,
                            query.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::MigrateV1View {
                        filter,
                        selected_index,
                        board,
                    } => {
                        let total = board.rows.len();
                        let visible = Self::migrate_visible_rows(&board.rows, filter);
                        let selected_value = visible
                            .get(*selected_index)
                            .and_then(|row_ix| board.rows.get(*row_ix))
                            .map(|row| row.file.clone());
                        (
                            "migrateV1".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::InstalledKitsView {
                        filter,
                        selected_index,
                        kits,
                    } => {
                        let (total, visible_count) =
                            Self::kit_store_installed_dataset_and_visible_counts(kits, filter);
                        let selected_value = Self::kit_store_installed_selected_visible_kit(
                            kits,
                            filter,
                            *selected_index,
                        )
                        .map(|kit| kit.name);
                        (
                            "installedKits".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ProcessManagerView {
                        filter,
                        selected_index,
                    } => {
                        let (total, visible_count) =
                            self.process_manager_dataset_and_visible_counts(filter);
                        let selected_value =
                            self.process_manager_selected_visible_row_name(filter, *selected_index);
                        (
                            "processManager".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::FlowUxView {
                        filter,
                        selected_index,
                        ..
                    } => {
                        let cwd = self.flow_ux_cwd();
                        let roster = crate::flows::catalog::flow_catalog().roster_for(&cwd);
                        let filtered = crate::flows::catalog::filter_flows(&roster.flows, filter);
                        let selected_value =
                            filtered.get(*selected_index).map(|flow| flow.id.clone());
                        (
                            "flowUx".to_string(),
                            None,
                            None,
                            filter.clone(),
                            roster.flows.len(),
                            filtered.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::CurrentAppCommandsView {
                        filter,
                        selected_index,
                    } => {
                        let (total, visible_count) =
                            self.current_app_commands_dataset_and_visible_counts(filter);
                        let selected_value = self.current_app_commands_selected_visible_row_name(
                            filter,
                            *selected_index,
                        );
                        (
                            "currentAppCommands".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::SearchAiPresetsView {
                        filter,
                        selected_index,
                    } => {
                        let total = Self::ai_preset_search_visible_row_labels("").len();
                        let rows = Self::ai_preset_search_visible_row_labels(filter);
                        let selected_value = rows.get(*selected_index).cloned();
                        (
                            "searchAiPresets".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            rows.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
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
                    } => {
                        let (total, visible_count) =
                            self.settings_dataset_and_visible_counts(filter);
                        let selected_value =
                            self.settings_selected_visible_row_name(filter, *selected_index);
                        (
                            "settings".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::PermissionsWizardView { selected_index } => {
                        let kinds = crate::permissions_wizard::PermissionKind::all();
                        let selected_value = kinds
                            .get(*selected_index)
                            .map(|kind| kind.name().to_string());
                        (
                            "permissionsWizard".to_string(),
                            None,
                            None,
                            String::new(),
                            kinds.len(),
                            kinds.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::FavoritesBrowseView {
                        filter,
                        selected_index,
                    } => {
                        let total = self.filtered_favorite_ids_for_filter("").len();
                        let rows = self.filtered_favorite_ids_for_filter(filter);
                        let selected_value = rows.get(*selected_index).cloned();
                        (
                            "favorites".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            rows.len(),
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::AgentChatView { .. } => (
                        "agentChatChat".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::DayPage { entity } => {
                        let content = entity.read(cx).notes_editor.read(cx).content(cx);
                        ("dayPage".to_string(), None, None, content, 0, 0, -1, None)
                    }
                    AppView::ScriptIssuesView { .. } => (
                        "scriptIssues".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        None,
                    ),
                    AppView::SdkReferenceView {
                        filter,
                        selected_index,
                        entries,
                    } => {
                        let (total, visible_count) =
                            crate::mcp_resources::sdk_reference_dataset_and_visible_counts(
                                entries, filter,
                            );
                        let selected_value =
                            crate::mcp_resources::sdk_reference_selected_visible_entry(
                                entries,
                                filter,
                                *selected_index,
                            )
                            .map(|row| row.entry.name.clone());
                        (
                            "sdkReference".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::TipsView {
                        filter,
                        selected_index,
                        entries,
                    } => {
                        let query = filter.trim().to_lowercase();
                        let visible: Vec<_> = entries
                            .iter()
                            .filter(|tip| {
                                query.is_empty()
                                    || tip.title.to_lowercase().contains(&query)
                                    || tip.hint.to_lowercase().contains(&query)
                                    || tip
                                        .keywords
                                        .iter()
                                        .any(|keyword| keyword.to_lowercase().contains(&query))
                            })
                            .collect();
                        (
                            "tips".to_string(),
                            None,
                            None,
                            filter.clone(),
                            entries.len(),
                            visible.len(),
                            *selected_index as i32,
                            visible.get(*selected_index).map(|tip| tip.title.clone()),
                        )
                    }
                    AppView::ScriptTemplateCatalogView {
                        filter,
                        selected_index,
                        templates,
                    } => {
                        let (total, visible_count) = crate::mcp_resources::
                            script_template_catalog_dataset_and_visible_counts(templates, filter);
                        let selected_value = crate::mcp_resources::
                            script_template_catalog_selected_visible_template(
                                templates,
                                filter,
                                *selected_index,
                            )
                            .map(|row| row.template.id.clone());
                        (
                            "scriptTemplateCatalog".to_string(),
                            None,
                            None,
                            filter.clone(),
                            total,
                            visible_count,
                            *selected_index as i32,
                            selected_value,
                        )
                    }
                    AppView::ConfirmPrompt { options, .. } => (
                        "confirmPrompt".to_string(),
                        None,
                        None,
                        String::new(),
                        0,
                        0,
                        -1,
                        Some(options.title.to_string()),
                    ),
                };
                let observation = self.prompt_observation(cx);
                let (input_value, choice_count, visible_choice_count, selected_index) = observation
                    .as_ref()
                    .map(|state| {
                        (
                            state.input.clone(),
                            state.choice_count,
                            state.choice_count,
                            state.selected_index.map(|index| index as i32).unwrap_or(-1),
                        )
                    })
                    .unwrap_or((
                        input_value,
                        choice_count,
                        visible_choice_count,
                        selected_index,
                    ));
                let selected_value = match &self.current_view {
                    AppView::SelectPrompt { entity, .. } => {
                        let prompt = entity.read(cx);
                        prompt
                            .filtered_choices
                            .get(prompt.focused_index)
                            .and_then(|index| prompt.choices.get(*index))
                            .map(|choice| choice.value.clone())
                    }
                    AppView::PathPrompt { entity, .. } => {
                        let prompt = entity.read(cx);
                        prompt
                            .filtered_entries
                            .get(prompt.selected_index)
                            .map(|entry| entry.path.clone())
                    }
                    _ => selected_value,
                };

                // Report the resolved owner, never a process-wide visibility proxy.
                let window_visible = main_target.visible;
                let is_focused = main_target.focused;
                let filter_input_decorations = {
                    let input_state = self.gpui_input_state.read(cx);
                    let input_text = input_state.value().to_string();
                    let use_canonical_filter =
                        self.pending_filter_sync && !self.filter_text.is_empty();
                    let text = if use_canonical_filter
                        || (input_text.is_empty() && !self.filter_text.is_empty())
                    {
                        self.filter_text.clone()
                    } else {
                        input_text
                    };
                    let object_refs_by_range =
                        menu_syntax_object_refs_by_range_for_filter(&text, &self.scripts);
                    let roles = input_state.highlight_range_roles();
                    let mut chips = if use_canonical_filter {
                        Vec::new()
                    } else {
                        input_state
                            .highlight_ranges()
                            .iter()
                            .enumerate()
                            .filter_map(|(index, (range, _color))| {
                                let chip_text = text.get(range.clone())?.to_string();
                                let role = roles
                                    .get(index)
                                    .cloned()
                                    .unwrap_or_else(|| "highlight".to_string());
                                let mut chip = serde_json::json!({
                                    "content": crate::protocol::RedactedElementContent::new(
                                        crate::protocol::ElementContentKind::UserContent,
                                        &chip_text,
                                    ),
                                    "range": [range.start, range.end],
                                    "role": role,
                                });
                                if role == "objectRef" {
                                    if let Some(object_ref) =
                                        object_refs_by_range.get(&(range.start, range.end))
                                    {
                                        chip["kind"] = serde_json::json!(object_ref.kind.as_str());
                                        chip["idContent"] = serde_json::json!(
                                            crate::protocol::RedactedElementContent::new(
                                                crate::protocol::ElementContentKind::ExternalContent,
                                                &object_ref.id,
                                            )
                                        );
                                        chip["labelContent"] = serde_json::json!(
                                            crate::protocol::RedactedElementContent::new(
                                                crate::protocol::ElementContentKind::ExternalContent,
                                                &object_ref.label,
                                            )
                                        );
                                        if let Some(deeplink) = object_ref.deeplink.as_ref() {
                                            chip["deeplinkContent"] = serde_json::json!(
                                                crate::protocol::RedactedElementContent::new(
                                                    crate::protocol::ElementContentKind::FilePath,
                                                    deeplink,
                                                )
                                            );
                                        }
                                    }
                                }
                                Some(chip)
                            })
                            .collect::<Vec<_>>()
                    };
                    if chips.is_empty() && !text.is_empty() {
                        let capture_targets =
                            crate::menu_syntax::registered_capture_targets_from_scripts(
                                &self.scripts,
                            );
                        chips = crate::menu_syntax::input_spans_for_input_with_targets(
                            &text,
                            &capture_targets,
                        )
                        .into_iter()
                        .filter(|span| {
                            span.role != crate::menu_syntax::MenuSyntaxFragmentRole::Subject
                        })
                        .filter_map(|span| {
                            let chip_text = text.get(span.range.clone())?.to_string();
                            let role = crate::menu_syntax::input_span_role_name(span.role);
                            let mut chip = serde_json::json!({
                                "text": chip_text,
                                "range": [span.range.start, span.range.end],
                                "role": role,
                            });
                            if role == "objectRef" {
                                if let Some(object_ref) =
                                    object_refs_by_range.get(&(span.range.start, span.range.end))
                                {
                                    chip["kind"] = serde_json::json!(object_ref.kind.as_str());
                                    chip["id"] = serde_json::json!(object_ref.id);
                                    chip["label"] = serde_json::json!(object_ref.label);
                                    if let Some(deeplink) = object_ref.deeplink.as_ref() {
                                        chip["deeplink"] = serde_json::json!(deeplink);
                                    }
                                }
                            }
                            Some(chip)
                        })
                        .collect();
                    }
                    Some(serde_json::json!({
                        "content": crate::protocol::RedactedElementContent::new(
                            crate::protocol::ElementContentKind::UserContent,
                            &text,
                        ),
                        "chips": chips,
                    }))
                };

                let menu_syntax_main_hint =
                    if !summary_only && matches!(self.current_view, AppView::ScriptList) {
                        // Run 12 — also treat the empty-result gate as true when
                        // the parser returns Incomplete but the user is mid-typing
                        // a non-source head (`has:`, `:type:`, etc.). Source heads
                        // stay tied to visible rows so `c: sub` does not report a
                        // no-match hint beside real Clipboard History results.
                        let parser_thinks_empty = self
                            .menu_syntax_mode
                            .advanced_query_for(&self.filter_text)
                            .is_some()
                            && visible_choice_count == 0;
                        let detector_owns_head =
                            crate::menu_syntax::main_hint::has_active_head(&self.filter_text);
                        let source_head_has_results =
                            crate::menu_syntax::main_hint::active_head_is_source_filter(
                                &self.filter_text,
                            ) && visible_choice_count > 0;
                        let advanced_query_has_results = (self
                            .menu_syntax_mode
                            .advanced_query_for(&self.filter_text)
                            .is_some()
                            || crate::menu_syntax::query::parse_filter_query(&self.filter_text)
                                .is_some())
                            && visible_choice_count > 0;
                        let advanced_query_results_empty = parser_thinks_empty
                            || (detector_owns_head
                                && !source_head_has_results
                                && !advanced_query_has_results);
                        self.menu_syntax_main_hint_snapshot(
                            &self.filter_text,
                            advanced_query_results_empty,
                        )
                    } else {
                        None
                    };

                // Story D slice 2: compute the capture-history popup
                // snapshot from the current filter text. Returns None
                // when the cursor is not on a slot trigger or no
                // history exists for the active target.
                //
                // Run 14 Pass 19: route through the schema-aware
                // variant. Run 14 Pass 21: the closure now collects
                // every loaded script's `capture.v1` handler specs and
                // calls `capture_kv_enum_values_for_specs` to find the
                // first matching `kv_enums[key]` for the active
                // target. Scripts that declare nothing → empty Vec →
                // legacy fall-through with `source: None`. Scripts
                // that DO declare enums → schema rows ranked first
                // with `Some(SchemaEnum)` discriminators.
                let capture_history_picker = if summary_only {
                    None
                } else {
                    crate::menu_syntax::capture_history_picker::snapshot_from_filter_text_with_overrides(
                        &self.filter_text,
                        &crate::menu_syntax::history::HistoryStore::from_env(),
                        |target, key| {
                            let specs: Vec<_> = self
                                .scripts
                                .iter()
                                .flat_map(|s| crate::menu_syntax::script_menu_syntax_specs(s).into_iter())
                                .collect();
                            let refs: Vec<&crate::menu_syntax::MenuSyntaxHandlerSpec> = specs.iter().collect();
                            crate::menu_syntax::capture_kv_enum_values_for_specs(target, key, &refs)
                        },
                    )
                };
                let script_list_active = matches!(self.current_view, AppView::ScriptList);
                let main_window_preflight = if !summary_only && script_list_active {
                    self.rebuild_main_window_preflight_if_needed();
                    self.cached_main_window_preflight
                        .as_ref()
                        .and_then(|receipt| serde_json::to_value(receipt).ok())
                } else {
                    None
                };
                let root_file_search = if script_list_active {
                    let root_file_query_intent = self
                        .menu_syntax_mode
                        .advanced_query_for(&self.computed_filter_text)
                        .filter(|advanced_query| {
                            advanced_query
                                .source_filters
                                .includes(crate::menu_syntax::RootUnifiedSourceFilter::Files)
                        })
                        .map(|_| crate::file_search::RootFileQueryIntent::ExplicitFilesSourceFilter)
                        .unwrap_or(crate::file_search::RootFileQueryIntent::OrdinaryRoot);
                    let root_file_match_mode =
                        crate::file_search::root_file_inline_match_mode_for_query(
                            &self.root_search.root_file_search_query,
                            root_file_query_intent,
                        );
                    let root_file_section_label = root_file_match_mode
                        .map(crate::file_search::RootFileInlineMatchMode::section_label);
                    let root_file_handoff_subtitle = root_file_match_mode
                        .map(crate::file_search::RootFileInlineMatchMode::handoff_subtitle);
                    Some(serde_json::json!({
                        "query": self.root_search.root_file_search_query,
                        "mode": self.root_search.root_file_search_mode.map(|mode| format!("{:?}", mode)),
                        "matchMode": root_file_match_mode.map(crate::file_search::RootFileInlineMatchMode::receipt_name),
                        "sectionLabel": root_file_section_label,
                        "handoffVisible": root_file_match_mode.is_some(),
                        "handoffSubtitle": root_file_handoff_subtitle,
                        "loading": self.root_search.root_file_provider_loading,
                        "providerLoading": self.root_search.root_file_provider_loading,
                        "visibleLoading": self.visible_root_file_search_loading(),
                        "generation": self.root_search.root_file_search_generation,
                        "visibleResultCount": self.root_search.root_file_results.len(),
                        "visibleRootFileCount": self.root_search.root_file_results.len(),
                        "loadedFileCount": self.root_search.root_file_results.len(),
                        "recentSeedCount": self.root_search.root_recent_file_results.len(),
                        "cacheEntryCount": self.root_search.root_file_result_cache.len(),
                        "cacheResultCount": self.active_root_file_cache_result_count(),
                    }))
                } else {
                    None
                };
                let filter_input_diagnostics = if script_list_active {
                    Some(serde_json::json!({
                        "canonicalFilterText": self.filter_text,
                        "computedFilterText": self.computed_filter_text,
                        "rawVisualInputValue": self.gpui_input_state.read(cx).value().to_string(),
                        "pendingFilterSync": self.pending_filter_sync,
                    }))
                } else {
                    None
                };
                let main_list_scroll = if script_list_active {
                    Some(self.main_list_scroll_receipt())
                } else {
                    None
                };
                let active_list_scroll = main_list_scroll
                    .clone()
                    .or_else(|| self.active_builtin_list_scroll_receipt(cx));
                let actions_dialog =
                    if self.show_actions_popup || crate::actions::is_actions_window_open() {
                        self.actions_dialog
                        .clone()
                        .or_else(|| crate::actions::get_actions_dialog_entity(cx))
                        .map(|dialog| {
                        let dialog = dialog.read(cx);
                        let visible_actions = dialog
                            .filtered_actions
                            .iter()
                            .filter_map(|action_idx| dialog.actions.get(*action_idx))
                            .map(|action| {
                                let canonical_shortcut = action
                                    .shortcut
                                    .as_deref()
                                    .map(crate::components::hint_strip::canonical_shortcut_hint);
                                serde_json::json!({
                                    "id": action.id,
                                    "label": action.title,
                                    "section": action.section,
                                    "shortcut": action.shortcut,
                                    "canonicalShortcut": canonical_shortcut,
                                    "destructive": crate::actions::is_destructive_action(action),
                                    "enabled": true,
                                })
                            })
                            .collect::<Vec<_>>();
                        let detailed_state = dialog.automation_state("actionsDialog", cx);
                        let shortcut_parity = detailed_state
                            .get("actions")
                            .and_then(|actions| actions.get("shortcutParity"))
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        let subject = self.pending_root_unified_actions_subject.as_ref();
                        let context_title = subject.map(|subject| subject.context_title());
                        let context_stable_key = subject.and_then(|subject| subject.stable_key());
                        let context_source = subject.map(|subject| subject.source_name());
                        serde_json::json!({
                            "open": true,
                            "host": self.current_actions_host().map(|host| format!("{:?}", host)),
                            "contextTitle": context_title,
                            "contextStableKey": context_stable_key,
                            "contextSource": context_source,
                            "selectedActionId": dialog.get_selected_action_id(),
                            "visibleActions": visible_actions,
                            "shortcutParity": shortcut_parity,
                        })
                    })
                    } else {
                        None
                    };
                let drop_state = match &self.current_view {
                    AppView::DropPrompt { entity, .. } => {
                        let drop_prompt = entity.read(cx);
                        Some(serde_json::json!({
                            "fileCount": drop_prompt.dropped_files.len(),
                            "files": drop_prompt
                                .dropped_files
                                .iter()
                                .enumerate()
                                .map(|(index, file)| file.automation_metadata(index))
                                .collect::<Vec<_>>(),
                        }))
                    }
                    _ => None,
                };
                let path_state = match &self.current_view {
                    AppView::PathPrompt { entity, .. } => {
                        let path_prompt = entity.read(cx);
                        Some(path_prompt.automation_state())
                    }
                    _ => None,
                };
                let dictation_state = Some(crate::dictation::automation_state());
                let day_page_state = match &self.current_view {
                    AppView::DayPage { entity } => Some(entity.read(cx).automation_state(cx)),
                    _ => None,
                };

                // Create the response
                let response = Message::state_result(
                    request_id.clone(),
                    prompt_type,
                    prompt_id,
                    Some(self.current_surface_contract_snapshot(&main_target, cx)),
                    self.active_popup_contract_snapshot(),
                    Some(self.active_footer_snapshot(&main_target)),
                    self.submit_diagnostics_snapshot(),
                    placeholder,
                    input_value,
                    choice_count,
                    visible_choice_count,
                    selected_index,
                    selected_value,
                    is_focused,
                    window_visible,
                    Some(self.mini_ai_state_snapshot(cx)),
                    None,
                    filter_input_decorations,
                    filter_input_diagnostics,
                    menu_syntax_main_hint,
                    capture_history_picker,
                    main_window_preflight,
                    actions_dialog,
                    root_file_search,
                    main_list_scroll,
                    active_list_scroll,
                    crate::ai::harness::screenshot_files::current_screenshot_identity(),
                    drop_state,
                    path_state,
                    None,
                    day_page_state,
                    dictation_state,
                    self.ghost_prediction.as_ref().map(|p| {
                        serde_json::json!({
                            "query": p.query,
                            "fullLabel": p.full_label,
                            "ghostSuffix": p.ghost_suffix,
                            "confidence": p.confidence,
                            "kind": p.kind_label(),
                            "acceptsTab": p.accepts_tab(),
                        })
                    }),
                    Some(self.flow_ux_automation_snapshot(cx)),
                    Some(self.conversations.snapshot()),
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

            PromptMessage::GetAgentChatState { request_id, target } => {
                tracing::info!(
                    category = "AGENT_CHAT_STATE",
                    request_id = %request_id,
                    target = ?target,
                    "agent_chat_state.request"
                );

                // Resolve target: Main → main window, AgentChatDetached → detached entity,
                // anything else → structured error.
                let agent_chat_target = match resolve_agent_chat_read_target(
                    &request_id,
                    "getAgentChatState",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let state = protocol::AgentChatStateSnapshot {
                            warnings: vec![format!("target_unsupported: {}", error.message)],
                            ..Default::default()
                        };
                        let response = Message::agent_chat_state_result(request_id.clone(), state);
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let resolved_target = build_agent_chat_resolved_target(
                    &request_id,
                    "getAgentChatState",
                    &agent_chat_target,
                );

                let mut state = match &agent_chat_target {
                    AgentChatReadTarget::Main { .. } => self.collect_agent_chat_state(cx),
                    AgentChatReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.collect_agent_chat_state_snapshot(cx)
                    }
                };
                state.resolved_target = resolved_target;
                let reliability_window_id = match &agent_chat_target {
                    AgentChatReadTarget::Main { info } => {
                        info.as_ref().map(|info| info.id.as_str()).unwrap_or("main")
                    }
                    AgentChatReadTarget::Detached { info, .. } => info.id.as_str(),
                };
                if let Some(fixture) =
                    crate::ai::reliability::ai_reliability_fixture_for_target(reliability_window_id)
                {
                    state.reliability = Some(fixture);
                }

                tracing::info!(
                    target: "script_kit::agent_chat_telemetry",
                    category = "AGENT_CHAT_STATE",
                    request_id = %request_id,
                    status = %state.status,
                    cursor_index = state.cursor_index,
                    picker_open = state.picker.as_ref().is_some_and(|p| p.open),
                    message_count = state.message_count,
                    context_ready = state.context_ready,
                    "agent_chat_state.result"
                );

                let response = Message::agent_chat_state_result(request_id.clone(), state);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "AGENT_CHAT_STATE",
                                request_id = %request_id,
                                "agent_chat_state.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "AGENT_CHAT_STATE",
                                request_id = %request_id,
                                "agent_chat_state.response_channel_disconnected"
                            );
                        }
                    }
                } else {
                    tracing::error!(
                        category = "AGENT_CHAT_STATE",
                        request_id = %request_id,
                        "agent_chat_state.no_response_sender"
                    );
                }
            }

            PromptMessage::GetAiReliabilityState { request_id, target } => {
                let state = match crate::windows::resolve_automation_window(target.as_ref()) {
                    Ok(resolved) => crate::ai::reliability::ai_reliability_snapshot_for_target(
                        &resolved.id,
                        resolved.kind,
                    ),
                    Err(error) => {
                        let mut state = protocol::AiReliabilityStateSnapshot::ready("unresolved");
                        state.last_transition.invalid_transition =
                            Some(format!("target_resolution_failed:{error}"));
                        state
                    }
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(Message::AiReliabilityStateResult {
                        request_id: request_id.clone(),
                        state,
                    });
                }
            }

            PromptMessage::SetAiReliabilityTestFixture {
                request_id,
                fixture_id,
                target,
            } => {
                let result = crate::windows::resolve_automation_window(target.as_ref())
                    .map_err(|error| error.to_string())
                    .and_then(|resolved| {
                        crate::ai::reliability::set_ai_reliability_fixture(resolved.id, &fixture_id)
                    });
                let (success, error, state) = match result {
                    Ok(state) => (true, None, Some(state)),
                    Err(error) => (false, Some(error), None),
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(Message::AiReliabilityTestFixtureResult {
                        request_id: request_id.clone(),
                        success,
                        error,
                        state,
                    });
                }
            }

            PromptMessage::PerformAgentChatSetupAction {
                request_id,
                action,
                agent_id,
                target,
            } => {
                tracing::info!(
                    category = "AGENT_CHAT_SETUP_ACTION",
                    request_id = %request_id,
                    action = ?action,
                    agent_id = ?agent_id,
                    target = ?target,
                    "agent_chat_setup_action.request"
                );

                // Resolve the Agent Chat target — now accepts both Main and AgentChatDetached.
                let agent_chat_target = match resolve_agent_chat_read_target(
                    &request_id,
                    "performAgentChatSetupAction",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let response = Message::agent_chat_setup_action_result_error(
                            request_id.clone(),
                            error.message,
                        );
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                // For Main targets, verify the main window is actually showing AgentChatView.
                if matches!(agent_chat_target, AgentChatReadTarget::Main { .. })
                    && !matches!(self.current_view, AppView::AgentChatView { .. })
                {
                    tracing::warn!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        "automation.agent_chat_action_target_main_view_missing"
                    );
                    let response = Message::agent_chat_setup_action_result_error(
                            request_id.clone(),
                            "performAgentChatSetupAction resolved the main Agent Chat target but the main window is not currently showing AgentChatView".to_string(),
                        );
                    if let Some(ref sender) = self.response_sender {
                        let _ = sender.try_send(response);
                    }
                    return;
                }

                tracing::info!(
                    target: "script_kit::automation",
                    request_id = %request_id,
                    resolved_target = match &agent_chat_target {
                        AgentChatReadTarget::Main { .. } => "main",
                        AgentChatReadTarget::Detached { .. } => "detached",
                    },
                    "automation.agent_chat_action_target_resolved"
                );

                let resolved_target = build_agent_chat_resolved_target(
                    &request_id,
                    "performAgentChatSetupAction",
                    &agent_chat_target,
                );

                // Dispatch the action to the resolved Agent Chat view.
                let result = match agent_chat_target.clone() {
                    AgentChatReadTarget::Main { .. } => match &self.current_view {
                        AppView::AgentChatView { entity } => entity.update(cx, |view, cx| {
                            view.perform_setup_automation_action(action, agent_id.as_deref(), cx)
                        }),
                        _ => Err("current main view is not AgentChatView".to_string()),
                    },
                    AgentChatReadTarget::Detached { entity, .. } => {
                        entity.update(cx, |view, cx| {
                            view.perform_setup_automation_action(action, agent_id.as_deref(), cx)
                        })
                    }
                };

                let mut state = match &agent_chat_target {
                    AgentChatReadTarget::Main { .. } => self.collect_agent_chat_state(cx),
                    AgentChatReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.collect_agent_chat_state_snapshot(cx)
                    }
                };
                state.resolved_target = resolved_target;

                let response = match result {
                    Ok(()) => {
                        Message::agent_chat_setup_action_result_success(request_id.clone(), state)
                    }
                    Err(error_msg) => {
                        tracing::warn!(
                            category = "AGENT_CHAT_SETUP_ACTION",
                            request_id = %request_id,
                            error = %error_msg,
                            "agent_chat_setup_action.failed"
                        );
                        Message::AgentChatSetupActionResult {
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

            PromptMessage::InspectContextPreparation {
                request_id,
                fixture_id,
            } => {
                use crate::ai::message_parts::{
                    prepare_user_message, AiContextPart, ContextPreparationItem,
                };

                if std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() != Some("1") {
                    let response = Message::ContextPreparationProbeResult {
                        request_id,
                        receipt: serde_json::json!({
                            "schemaVersion": 1,
                            "fixtureId": fixture_id,
                            "classification": "fixtureUnavailable",
                        }),
                    };
                    if let Some(ref sender) = self.response_sender {
                        let _ = sender.try_send(response);
                    }
                    return;
                }

                let base64_canary = "SAFE001_BASE64_CANARY".repeat(10_000);
                let nonbinary_canary = "SAFE001_NONBINARY_SURVIVES";
                let (authored, items) = match fixture_id.as_str() {
                    "acceptedOversizedJson" => (
                        "Inspect the attached synthetic context.",
                        vec![ContextPreparationItem::supplemental(
                            AiContextPart::TextBlock {
                                label: "Synthetic oversized JSON".to_string(),
                                source: "fixture://safe001/oversized".to_string(),
                                text: serde_json::json!({
                                    "selectedText": nonbinary_canary,
                                    "nested": {
                                        "image": {
                                            "mimeType": "image/png",
                                            "base64Data": base64_canary,
                                        }
                                    }
                                })
                                .to_string(),
                                mime_type: Some("text/plain".to_string()),
                            },
                        )],
                    ),
                    "missingPrimary" => (
                        "Authored text must not bypass required context.",
                        vec![ContextPreparationItem::primary(AiContextPart::FilePath {
                            path: "/missing/SAFE001_RAW_PATH_CANARY".to_string(),
                            label: "Required context".to_string(),
                        })],
                    ),
                    "missingSupplemental" => (
                        "The authored message remains sendable.",
                        vec![ContextPreparationItem::supplemental(
                            AiContextPart::FilePath {
                                path: "/missing/SAFE001_RAW_PATH_CANARY".to_string(),
                                label: "Optional attachment".to_string(),
                            },
                        )],
                    ),
                    _ => {
                        let response = Message::ContextPreparationProbeResult {
                            request_id,
                            receipt: serde_json::json!({
                                "schemaVersion": 1,
                                "fixtureId": fixture_id,
                                "classification": "fixtureUnknown",
                            }),
                        };
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let prepared = prepare_user_message(authored, &items, &[], &[]);
                let final_content = &prepared.final_user_content;
                let serialized_receipt =
                    serde_json::to_string(&prepared.receipt).unwrap_or_else(|_| "{}".to_string());
                let private_checks = serde_json::json!({
                    "payloadChars": final_content.chars().count(),
                    "nonbinaryFieldPreserved": final_content.contains(nonbinary_canary),
                    "base64CanaryAbsent": !final_content.contains("SAFE001_BASE64_CANARY"),
                    "binaryOmissionMarkerPresent": final_content.contains("[binary omitted:"),
                    "receiptRawCanariesAbsent": !serialized_receipt.contains("SAFE001_RAW_PATH_CANARY")
                        && !serialized_receipt.contains("SAFE001_BASE64_CANARY")
                        && !serialized_receipt.contains(nonbinary_canary),
                    "canSendMessage": prepared.can_send_message(),
                    "unresolvedCount": prepared.unresolved_parts().len(),
                });
                let response = Message::ContextPreparationProbeResult {
                    request_id,
                    receipt: serde_json::json!({
                        "schemaVersion": 1,
                        "fixtureId": fixture_id,
                        "classification": "runtimeConfirmed",
                        "preparation": prepared.receipt,
                        "privateChecks": private_checks,
                    }),
                };
                if let Some(ref sender) = self.response_sender {
                    let _ = sender.try_send(response);
                }
            }

            PromptMessage::ResetAgentChatTestProbe { request_id, target } => {
                tracing::info!(
                    category = "AGENT_CHAT_PROBE",
                    request_id = %request_id,
                    target = ?target,
                    "agent_chat_test_probe.reset"
                );

                // Resolve target: Main → main window, AgentChatDetached → detached entity,
                // anything else → structured error.
                let agent_chat_target = match resolve_agent_chat_read_target(
                    &request_id,
                    "resetAgentChatTestProbe",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let probe = protocol::AgentChatTestProbeSnapshot {
                            warnings: vec![format!("target_unsupported: {}", error.message)],
                            ..Default::default()
                        };
                        let response =
                            Message::agent_chat_test_probe_result(request_id.clone(), probe);
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let resolved_target = build_agent_chat_resolved_target(
                    &request_id,
                    "resetAgentChatTestProbe",
                    &agent_chat_target,
                );

                match &agent_chat_target {
                    AgentChatReadTarget::Main { .. } => {
                        self.reset_agent_chat_test_probe(cx);
                    }
                    AgentChatReadTarget::Detached { entity, .. } => {
                        entity.update(cx, |view, _cx| {
                            view.reset_test_probe();
                        });
                    }
                };

                // Respond with the current (now-empty) probe snapshot.
                let mut probe = match &agent_chat_target {
                    AgentChatReadTarget::Main { .. } => self.collect_agent_chat_test_probe(
                        protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS,
                        cx,
                    ),
                    AgentChatReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.test_probe_snapshot(protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS, cx)
                    }
                };
                probe.state.resolved_target = resolved_target;
                let response = Message::agent_chat_test_probe_result(request_id.clone(), probe);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "AGENT_CHAT_PROBE",
                                request_id = %request_id,
                                "agent_chat_test_probe.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "AGENT_CHAT_PROBE",
                                request_id = %request_id,
                                "agent_chat_test_probe.response_channel_disconnected"
                            );
                        }
                    }
                }
            }

            PromptMessage::GetAgentChatTestProbe {
                request_id,
                tail,
                target,
            } => {
                let tail = tail
                    .unwrap_or(protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS)
                    .clamp(1, protocol::AGENT_CHAT_TEST_PROBE_MAX_EVENTS);
                tracing::info!(
                    category = "AGENT_CHAT_PROBE",
                    request_id = %request_id,
                    tail,
                    target = ?target,
                    "agent_chat_test_probe.request"
                );

                // Resolve target: Main → main window, AgentChatDetached → detached entity,
                // anything else → structured error.
                let agent_chat_target = match resolve_agent_chat_read_target(
                    &request_id,
                    "getAgentChatTestProbe",
                    target.as_ref(),
                ) {
                    Ok(t) => t,
                    Err(error) => {
                        let probe = protocol::AgentChatTestProbeSnapshot {
                            warnings: vec![format!("target_unsupported: {}", error.message)],
                            ..Default::default()
                        };
                        let response =
                            Message::agent_chat_test_probe_result(request_id.clone(), probe);
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(response);
                        }
                        return;
                    }
                };

                let resolved_target = build_agent_chat_resolved_target(
                    &request_id,
                    "getAgentChatTestProbe",
                    &agent_chat_target,
                );

                let mut probe = match &agent_chat_target {
                    AgentChatReadTarget::Main { .. } => {
                        self.collect_agent_chat_test_probe(tail, cx)
                    }
                    AgentChatReadTarget::Detached { entity, .. } => {
                        let view = entity.read(cx);
                        view.test_probe_snapshot(tail, cx)
                    }
                };
                probe.state.resolved_target = resolved_target;
                let response = Message::agent_chat_test_probe_result(request_id.clone(), probe);

                if let Some(ref sender) = self.response_sender {
                    match sender.try_send(response) {
                        Ok(()) => {}
                        Err(std::sync::mpsc::TrySendError::Full(_)) => {
                            tracing::warn!(
                                category = "AGENT_CHAT_PROBE",
                                request_id = %request_id,
                                "agent_chat_test_probe.response_channel_full"
                            );
                        }
                        Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                            tracing::info!(
                                category = "AGENT_CHAT_PROBE",
                                request_id = %request_id,
                                "agent_chat_test_probe.response_channel_disconnected"
                            );
                        }
                    }
                }
            }

            PromptMessage::GetElements {
                request_id,
                limit,
                target,
                include_headers,
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

                let (
                    snapshot,
                    semantic_surface,
                    projection_version,
                    projection_quality,
                    reason_codes,
                ) = match resolved_target {
                    Some(Ok(ref resolved))
                        if resolved.kind != protocol::AutomationWindowKind::Main =>
                    {
                        let snapshot = crate::windows::automation_surface_collector::collect_surface_snapshot(
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
                        });
                        let quality = snapshot.quality;
                        let semantic_surface = resolved
                            .semantic_surface
                            .clone()
                            .unwrap_or_else(|| format!("{:?}", resolved.kind));
                        (
                            snapshot,
                            semantic_surface,
                            1,
                            quality.projection_quality(),
                            quality.reason_codes(),
                        )
                    }
                    Some(Err(ref err)) => {
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(Message::elements_result_with_projection(
                                request_id.clone(),
                                "unresolvedTarget".to_string(),
                                1,
                                protocol::ProjectionQuality::Unsupported,
                                vec![protocol::ProjectionReason::TargetResolutionFailed],
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
                        if matches!(self.current_view, AppView::ScriptList) {
                            self.get_grouped_results_cached();
                        }
                        let outcome = self.collect_visible_elements_with_headers(
                            max_elements,
                            include_headers,
                            cx,
                        );
                        let semantic_surface = outcome.semantic_surface.clone();
                        let projection_version = outcome.version;
                        let projection_quality = outcome.projection_quality;
                        let reason_codes = outcome.reason_codes.clone();
                        let snapshot = crate::windows::automation_surface_collector::SurfaceElementSnapshot {
                            total_count: outcome.total_count,
                            focused_semantic_id: outcome.focused_semantic_id(),
                            selected_semantic_id: outcome.selected_semantic_id(),
                            warnings: outcome.warnings.clone(),
                            elements: outcome.elements,
                            quality: match projection_quality {
                                protocol::ProjectionQuality::Complete => crate::windows::automation_surface_collector::SnapshotQuality::Full,
                                protocol::ProjectionQuality::Partial
                                | protocol::ProjectionQuality::Unsupported => crate::windows::automation_surface_collector::SnapshotQuality::PanelOnly,
                            },
                        };
                        (
                            snapshot,
                            semantic_surface,
                            projection_version,
                            projection_quality,
                            reason_codes,
                        )
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

                let response = Message::elements_result_with_projection(
                    request_id.clone(),
                    semantic_surface,
                    projection_version,
                    projection_quality,
                    reason_codes,
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

                if target.is_some() {
                    match crate::windows::resolve_automation_window(target.as_ref()) {
                        Ok(resolved) if resolved.kind != protocol::AutomationWindowKind::Main => {
                            let layout = if resolved.semantic_surface.as_deref()
                                == Some("footerOverlay")
                            {
                                resolved
                                    .generation
                                    .ok_or_else(|| anyhow::anyhow!("footer_generation_missing"))
                                    .and_then(|generation| {
                                        crate::footer_popup::footer_fixture_layout(
                                            &resolved.id,
                                            generation,
                                            cx,
                                        )
                                    })
                            } else if resolved.id == "shortcut-recorder-popup" {
                                resolved
                                    .generation
                                    .ok_or_else(|| anyhow::anyhow!("shortcut_generation_missing"))
                                    .and_then(|generation| {
                                        crate::shortcut_recorder::shortcut_fixture_layout(
                                            &resolved.id,
                                            generation,
                                            cx,
                                        )
                                    })
                            } else {
                                crate::windows::automation_surface_collector::collect_registered_surface_layout(&resolved, cx)
                            };
                            let layout = layout.unwrap_or_else(|error| {
                                tracing::warn!(request_id = %request_id, error = %error, "getLayoutInfo: target layout unavailable");
                                protocol::LayoutInfo::default()
                            });
                            if let Some(ref sender) = self.response_sender {
                                let _ = sender.try_send(Message::layout_info_result(
                                    request_id.clone(),
                                    layout,
                                ));
                            }
                            return;
                        }
                        Ok(_) => { /* main window — proceed */ }
                        Err(error) => {
                            tracing::warn!(
                                target: "script_kit::automation",
                                request_id = %request_id,
                                error = %error,
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
                let actual_window_size = current_window
                    .as_deref()
                    .map(|window| {
                        let size = window.viewport_size();
                        (f32::from(size.width), f32::from(size.height))
                    })
                    .or_else(|| {
                        crate::windows::list_automation_windows()
                            .into_iter()
                            .find(|window| window.id == "main")
                            .and_then(|window| window.bounds)
                            .map(|bounds| (bounds.width as f32, bounds.height as f32))
                    });
                let mut layout_info = self.build_layout_info(actual_window_size, cx);
                if let Some(window) = current_window.as_deref() {
                    Self::append_paint_measurements(&mut layout_info, window);
                } else if let Some(main_window) = crate::get_main_window_handle() {
                    if let Err(error) = main_window.update(cx, |_root, window, _cx| {
                        Self::append_paint_measurements(&mut layout_info, window);
                    }) {
                        tracing::warn!(
                            target: "script_kit::automation",
                            request_id = %request_id,
                            error = %error,
                            "getLayoutInfo: paint measurement window update failed"
                        );
                    }
                } else {
                    tracing::warn!(
                        target: "script_kit::automation",
                        request_id = %request_id,
                        "getLayoutInfo: main window unavailable for paint measurements"
                    );
                }

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
                let snapshot = self.build_automation_inspect_snapshot(
                    &request_id,
                    target.as_ref(),
                    hi_dpi,
                    &probes,
                    cx,
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
                let poll_ms = poll_interval.unwrap_or(25).clamp(1, 1_000);
                let rid = request_id.clone();
                let wait_command = protocol::BatchCommand::WaitFor {
                    condition: condition.clone(),
                    timeout: Some(timeout_ms),
                    poll_interval: Some(poll_ms),
                };
                let wait_options = protocol::BatchOptions {
                    stop_on_error: true,
                    rollback_on_error: false,
                    timeout: timeout_ms.max(1),
                };
                let (command_fingerprint, dispatch_guard) = match prepare_transaction_transport(
                    &rid,
                    std::slice::from_ref(&wait_command),
                    &wait_options,
                    target.as_ref(),
                    cx,
                ) {
                    Ok(Some(prepared)) => prepared,
                    Ok(None) => return,
                    Err(error) => {
                        if let Some(sender) = &self.response_sender {
                            let _ = sender.try_send(Message::wait_for_result(
                                rid,
                                false,
                                0,
                                Some(error),
                            ));
                        }
                        return;
                    }
                };

                let resolved_target =
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
                    };
                let secondary_target = match &resolved_target {
                    AutomationReadTarget::Registered { info } => Some(info.clone()),
                    AutomationReadTarget::Main { .. } => None,
                };

                tracing::info!(
                    category = "AUTOMATION",
                    request_id = %rid,
                    timeout_ms = timeout_ms,
                    poll_ms = poll_ms,
                    trace_mode = ?trace_mode,
                    "automation.wait_for.started"
                );

                let already_satisfied = match secondary_target.as_ref() {
                    Some(info) => registered_surface_wait_satisfied(info, &condition, cx),
                    None => Ok(self.wait_condition_satisfied(&condition, cx)),
                };
                let already_satisfied = match already_satisfied {
                    Ok(satisfied) => satisfied,
                    Err(error) => {
                        if let Some(ref sender) = self.response_sender {
                            let _ = sender.try_send(Message::wait_for_result(
                                request_id.clone(),
                                false,
                                0,
                                Some(registered_surface_transaction_error(error)),
                            ));
                        }
                        return;
                    }
                };
                if already_satisfied {
                    let include_trace =
                        protocol::transaction_trace::should_include_trace(trace_mode, true);
                    let trace = if include_trace {
                        let snapshot = match secondary_target.as_ref() {
                            Some(info) => registered_surface_ui_snapshot(info, cx),
                            None => Ok(self.build_main_ui_snapshot(cx)),
                        };
                        let snapshot = match snapshot {
                            Ok(snapshot) => snapshot,
                            Err(error) => {
                                if let Some(ref sender) = self.response_sender {
                                    let _ = sender.try_send(Message::wait_for_result(
                                        request_id.clone(),
                                        false,
                                        0,
                                        Some(registered_surface_transaction_error(error)),
                                    ));
                                }
                                return;
                            }
                        };
                        let started_at_ms = protocol::transaction_trace::now_epoch_ms();
                        Some(protocol::TransactionTrace {
                            schema_version: protocol::TRANSACTION_TRACE_SCHEMA_VERSION,
                            request_id: rid.clone(),
                            command_fingerprint: command_fingerprint.clone(),
                            status: protocol::TransactionTraceStatus::Ok,
                            started_at_ms,
                            total_elapsed_ms: 0,
                            failed_at: None,
                            commands: vec![protocol::TransactionCommandTrace {
                                index: 0,
                                command: "waitFor".to_string(),
                                command_payload: None,
                                started_at_ms,
                                elapsed_ms: 0,
                                before: snapshot.clone(),
                                after: snapshot.clone(),
                                polls: vec![protocol::WaitPollObservation {
                                    attempt: 1,
                                    elapsed_ms: 0,
                                    condition_satisfied: true,
                                    snapshot,
                                    matched_semantic_ids: Vec::new(),
                                }],
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
                    cx.spawn(async move |this, cx| {
                        if let Err(error) = dispatch_guard.validate() {
                            if let Some(sender) = &sender {
                                let _ = sender.try_send(Message::wait_for_result(
                                    rid,
                                    false,
                                    0,
                                    Some(protocol::TransactionError::action_failed(error)),
                                ));
                            }
                            return;
                        }
                        let started_at_ms = protocol::transaction_trace::now_epoch_ms();
                        let start = std::time::Instant::now();
                        let timeout_dur = std::time::Duration::from_millis(timeout_ms);
                        let poll_dur = std::time::Duration::from_millis(poll_ms);

                        // Capture `before` once at entry so callers can diff against
                        // the state the poll loop saw when it began.
                        let before_snapshot = match secondary_target.as_ref() {
                            Some(info) => cx.update(|cx| registered_surface_ui_snapshot(info, cx)),
                            None => this.update(cx, |this, cx| this.build_main_ui_snapshot(cx)),
                        };
                        let before_snapshot = match before_snapshot {
                            Ok(snapshot) => snapshot,
                            Err(error) => {
                                if let Some(ref sender) = sender {
                                    let _ = sender.try_send(Message::wait_for_result(
                                        rid.clone(),
                                        false,
                                        0,
                                        Some(registered_surface_transaction_error(error)),
                                    ));
                                }
                                return;
                            }
                        };

                        let mut polls: Vec<protocol::WaitPollObservation> = Vec::new();
                        let mut last_snapshot = before_snapshot.clone();

                        loop {
                            cx.background_executor()
                                .timer(poll_dur.min(timeout_dur.saturating_sub(start.elapsed())))
                                .await;
                            if start.elapsed() >= timeout_dur || polls.len() >= MAX_WAIT_POLLS {
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
                                        schema_version: protocol::TRANSACTION_TRACE_SCHEMA_VERSION,
                                        request_id: rid.clone(),
                                        command_fingerprint: command_fingerprint.clone(),
                                        status: protocol::TransactionTraceStatus::Timeout,
                                        started_at_ms,
                                        total_elapsed_ms: elapsed_ms,
                                        failed_at: Some(0),
                                        commands: vec![protocol::TransactionCommandTrace {
                                            index: 0,
                                            command: "waitFor".to_string(),
                                            command_payload: None,
                                            started_at_ms,
                                            elapsed_ms,
                                            before: before_snapshot.clone(),
                                            after: last_snapshot.clone(),
                                            polls: polls.clone(),
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
                            if let Err(error) = dispatch_guard.validate() {
                                if let Some(sender) = &sender {
                                    let _ = sender.try_send(Message::wait_for_result(
                                        rid.clone(),
                                        false,
                                        start.elapsed().as_millis() as u64,
                                        Some(protocol::TransactionError::action_failed(error)),
                                    ));
                                }
                                break;
                            }
                            // Capture condition_satisfied + a fresh snapshot in the
                            // same `this.update(...)` closure so both reflect the
                            // same tick of state.
                            let poll_result = match secondary_target.as_ref() {
                                Some(info) => cx.update(|cx| {
                                    let satisfied =
                                        registered_surface_wait_satisfied(info, &condition, cx)?;
                                    let snapshot = registered_surface_ui_snapshot(info, cx)?;
                                    Ok((satisfied, snapshot))
                                }),
                                None => this.update(cx, |this, cx| {
                                    (
                                        this.wait_condition_satisfied(&condition, cx),
                                        this.build_main_ui_snapshot(cx),
                                    )
                                }),
                            };
                            match poll_result {
                                Ok((condition_satisfied, snapshot)) => {
                                    let elapsed_ms = start.elapsed().as_millis() as u64;
                                    last_snapshot = snapshot.clone();
                                    polls.push(protocol::WaitPollObservation {
                                        attempt: polls.len() + 1,
                                        elapsed_ms,
                                        condition_satisfied,
                                        snapshot,
                                        matched_semantic_ids: Vec::new(),
                                    });
                                    if condition_satisfied {
                                        let include_trace =
                                            protocol::transaction_trace::should_include_trace(
                                                trace_mode, true,
                                            );
                                        let trace = if include_trace {
                                            Some(protocol::TransactionTrace {
                                                schema_version:
                                                    protocol::TRANSACTION_TRACE_SCHEMA_VERSION,
                                                request_id: rid.clone(),
                                                command_fingerprint: command_fingerprint.clone(),
                                                status: protocol::TransactionTraceStatus::Ok,
                                                started_at_ms,
                                                total_elapsed_ms: elapsed_ms,
                                                failed_at: None,
                                                commands: vec![protocol::TransactionCommandTrace {
                                                    index: 0,
                                                    command: "waitFor".to_string(),
                                                    command_payload: None,
                                                    started_at_ms,
                                                    elapsed_ms,
                                                    before: before_snapshot.clone(),
                                                    after: last_snapshot.clone(),
                                                    polls: polls.clone(),
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
                                            let _ =
                                                s.try_send(Message::wait_for_result_with_trace(
                                                    rid.clone(),
                                                    true,
                                                    elapsed_ms,
                                                    None::<crate::protocol::TransactionError>,
                                                    trace,
                                                ));
                                        }
                                        break;
                                    }
                                    continue;
                                }
                                Err(error) => {
                                    let elapsed_ms = start.elapsed().as_millis() as u64;
                                    let error = registered_surface_transaction_error(error);
                                    let include_trace =
                                        protocol::transaction_trace::should_include_trace(
                                            trace_mode, false,
                                        );
                                    let trace = if include_trace {
                                        Some(protocol::TransactionTrace {
                                            schema_version:
                                                protocol::TRANSACTION_TRACE_SCHEMA_VERSION,
                                            request_id: rid.clone(),
                                            command_fingerprint: command_fingerprint.clone(),
                                            status: protocol::TransactionTraceStatus::Failed,
                                            started_at_ms,
                                            total_elapsed_ms: elapsed_ms,
                                            failed_at: Some(0),
                                            commands: vec![protocol::TransactionCommandTrace {
                                                index: 0,
                                                command: "waitFor".to_string(),
                                                command_payload: None,
                                                started_at_ms,
                                                elapsed_ms,
                                                before: before_snapshot.clone(),
                                                after: last_snapshot.clone(),
                                                polls: polls.clone(),
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
                expected,
            } => {
                let opts = options.unwrap_or(protocol::BatchOptions {
                    stop_on_error: true,
                    rollback_on_error: false,
                    timeout: 5_000,
                });
                let rid = request_id.clone();
                let sender = self.response_sender.clone();
                let batch_start = std::time::Instant::now();
                let batch_timeout = std::time::Duration::from_millis(opts.timeout);
                let (command_fingerprint, dispatch_guard) = match prepare_transaction_transport(
                    &rid,
                    &commands,
                    &opts,
                    target.as_ref(),
                    cx,
                ) {
                    Ok(Some(prepared)) => prepared,
                    Ok(None) => return, // The original in-flight/terminal request owns its sole reply.
                    Err(error) => {
                        if let Some(sender) = &self.response_sender {
                            let _ = sender.try_send(Message::batch_result(
                                rid,
                                false,
                                vec![protocol::BatchResultEntry {
                                    index: 0,
                                    success: false,
                                    command: "batch".into(),
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
                };

                // Resolve target: accept Main, AgentChatDetached, and Notes.
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
                    AutomationReadTarget::Main {
                        info: Some(dispatch_guard.info.clone()),
                    }
                };
                let batch_target_kind = batch_target_kind_for_resolved_target(&batch_target);

                tracing::info!(
                    category = "AUTOMATION",
                    request_id = %rid,
                    command_count = commands.len(),
                    trace_mode = ?trace_mode,
                    target = ?target,
                    "automation.batch.started"
                );

                let main_batch_window_handle = crate::get_main_window_handle();

                cx.spawn(async move |this, cx| {
                    // Secondary commands use the same exact production owners as the evaluator.
                    if let AutomationReadTarget::Registered { info } = &batch_target {
                        let batch_started_at_ms = protocol::transaction_trace::now_epoch_ms();
                        let mut results = Vec::new();
                        for (index, cmd) in commands.iter().enumerate() {
                            if let Err(error) = dispatch_guard.validate() {
                                results.push(protocol::BatchResultEntry {
                                    index, success: false, command: batch_command_name(cmd),
                                    elapsed: Some(0), value: None,
                                    error: Some(protocol::TransactionError::action_failed(error)),
                                });
                                break;
                            }
                            if batch_start.elapsed() >= batch_timeout {
                                results.push(protocol::BatchResultEntry {
                                    index, success: false, command: batch_command_name(cmd),
                                    elapsed: Some(0), value: None,
                                    error: Some(protocol::TransactionError::wait_timeout("Batch timeout exceeded")),
                                });
                                break;
                            }
                            let cmd_start = std::time::Instant::now();
                            let result = if let protocol::BatchCommand::WaitFor { condition, timeout, poll_interval } = cmd {
                                let (wait_timeout, wait_poll) = bounded_batch_wait(
                                    *timeout, *poll_interval, batch_timeout.saturating_sub(batch_start.elapsed()),
                                );
                                let mut polls = 0;
                                loop {
                                    let remaining = wait_timeout.saturating_sub(cmd_start.elapsed())
                                        .min(batch_timeout.saturating_sub(batch_start.elapsed()));
                                    if remaining.is_zero() || polls >= MAX_WAIT_POLLS {
                                        break Err(protocol::TransactionError::wait_timeout("Batch wait deadline or poll budget exceeded"));
                                    }
                                    if let Err(error) = dispatch_guard.validate() {
                                        break Err(protocol::TransactionError::action_failed(error));
                                    }
                                    polls += 1;
                                    match cx.update(|cx| registered_surface_wait_satisfied(info, condition, cx)) {
                                        Ok(true) => break Ok(None),
                                        Err(error) => break Err(registered_surface_transaction_error(error)),
                                        Ok(false) => {}
                                    }
                                    cx.background_executor().timer(wait_poll.min(remaining)).await;
                                }
                            } else {
                                cx.update(|cx| {
                                    validate_batch_app_effect(expected.as_ref(), &dispatch_guard, &this, cx)?;
                                    apply_registered_surface_command(info, cmd, cx)
                                })
                                    .map_err(registered_surface_transaction_error)
                            };
                            let (value, error) = match result {
                                Ok(value) => (value, None),
                                Err(error) => (None, Some(error)),
                            };
                            let success = error.is_none();
                            results.push(protocol::BatchResultEntry {
                                index, success, command: batch_command_name(cmd),
                                elapsed: Some(cmd_start.elapsed().as_millis() as u64), value, error,
                            });
                            if !success && opts.stop_on_error { break; }
                        }
                        let total_elapsed = batch_start.elapsed().as_millis() as u64;
                        let failed_at = results.iter().position(|result| !result.success);
                        let success = failed_at.is_none();
                        let trace = match protocol::transaction_trace::maybe_persist_batch_trace_from_results(
                            trace_mode,
                            rid.clone(),
                            command_fingerprint.clone(),
                            batch_started_at_ms,
                            total_elapsed,
                            success,
                            failed_at,
                            &commands,
                            &results,
                            None,
                        ) {
                            Ok(trace) => trace,
                            Err(error) => {
                                tracing::warn!(
                                    target: "script_kit::transaction",
                                    request_id = %rid,
                                    error = %error,
                                    "batch trace persistence failed"
                                );
                                if let Some(ref s) = sender {
                                    let _ = s.try_send(Message::batch_result(
                                        rid.clone(),
                                        false,
                                        vec![protocol::BatchResultEntry {
                                            index: 0,
                                            success: false,
                                            command: "trace".to_string(),
                                            elapsed: Some(total_elapsed),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!(
                                                "failed to persist transaction trace: {error}"
                                            ))),
                                        }],
                                        Some(0),
                                        total_elapsed,
                                    ));
                                }
                                return;
                            }
                        };

                        tracing::info!(
                            category = "AUTOMATION",
                            request_id = %rid,
                            success,
                            total_elapsed_ms = total_elapsed,
                            failed_at = ?failed_at,
                            target = %info.id,
                            trace_included = trace.is_some(),
                            "automation.batch.secondary.completed"
                        );

                        if let Some(ref s) = sender {
                            let _ = s.try_send(Message::batch_result_with_trace(
                                rid.clone(), success, results, failed_at, total_elapsed, trace,
                            ));
                        }
                        return;
                    }

                    // ── Main-window batch path (existing) ────────────────
                    let batch_started_at_ms = protocol::transaction_trace::now_epoch_ms();
                    let mut results: Vec<protocol::BatchResultEntry> = Vec::new();
                    let mut failed = false;

                    for (index, cmd) in commands.iter().enumerate() {
                        if let Err(error) = dispatch_guard.validate() {
                            results.push(protocol::BatchResultEntry {
                                index, success: false, command: batch_command_name(cmd), elapsed: Some(0),
                                value: None, error: Some(protocol::TransactionError::action_failed(error)),
                            });
                            failed = true;
                            break;
                        }
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
                                match set_main_window_input_text_for_batch(
                                    &this,
                                    main_batch_window_handle,
                                    expected.as_ref(),
                                    &dispatch_guard,
                                    text,
                                    cx,
                                ) {
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
                                    validate_batch_main_effect(this, expected.as_ref(), &dispatch_guard, cx)?;
                                    this.select_choice_by_value(&value, submit, cx)
                                }) {
                                    Ok(Ok(v)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectByValue", value_bytes = v.len(), "batch.step.ok");
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
                                match select_main_window_semantic_id_for_batch(
                                    &this,
                                    main_batch_window_handle,
                                    match &batch_target {
                                        AutomationReadTarget::Main { info } => info.as_ref(),
                                        AutomationReadTarget::Registered { .. } => None,
                                    },
                                    expected.as_ref(),
                                    &dispatch_guard,
                                    &semantic_id,
                                    submit,
                                    cx,
                                ) {
                                    Ok(v) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectBySemanticId", value_bytes = v.len(), "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "selectBySemanticId".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: Some(v),
                                            error: None,
                                        });
                                    }
                                    Err(e) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "selectBySemanticId", error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "selectBySemanticId".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(e.downcast::<protocol::TransactionError>().unwrap_or_else(|error| protocol::TransactionError::selection_not_found(error.to_string()))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::SetThemeControl { control, value } => {
                                let control = control.clone();
                                let value = value.clone();
                                match this.update(cx, |this, cx| {
                                    validate_batch_main_effect(this, expected.as_ref(), &dispatch_guard, cx)?;
                                    if !matches!(
                                        this.current_view,
                                        AppView::ThemeChooserView { .. }
                                    ) {
                                        return Err(anyhow::anyhow!(
                                            "setThemeControl requires ThemeChooserView"
                                        ));
                                    }
                                    this.set_theme_chooser_control_from_devtools(
                                        &control,
                                        &value,
                                        cx,
                                    )
                                }) {
                                    Ok(Ok(v)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "setThemeControl", control = %control, value_bytes = value.len(), "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "setThemeControl".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: Some(v),
                                            error: None,
                                        });
                                    }
                                    Ok(Err(e)) | Err(e) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "setThemeControl", control = %control, value_bytes = value.len(), error = %e, "batch.step.error");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "setThemeControl".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(format!("{e}"))),
                                        });
                                        failed = true;
                                        if opts.stop_on_error { break; }
                                    }
                                }
                            }
                            protocol::BatchCommand::UndoStyleChange => {
                                let command = batch_command_name(cmd);
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command,
                                    elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                    value: None,
                                    error: Some(unsupported_batch_command_error(
                                        batch_target_kind,
                                        cmd,
                                    )),
                                });
                                failed = true;
                                if opts.stop_on_error {
                                    break;
                                }
                            }
                            protocol::BatchCommand::RedoStyleChange => {
                                let command = batch_command_name(cmd);
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command,
                                    elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                    value: None,
                                    error: Some(unsupported_batch_command_error(
                                        batch_target_kind,
                                        cmd,
                                    )),
                                });
                                failed = true;
                                if opts.stop_on_error {
                                    break;
                                }
                            }
                            protocol::BatchCommand::ResetStyleControls => {
                                let command = batch_command_name(cmd);
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command,
                                    elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                    value: None,
                                    error: Some(unsupported_batch_command_error(
                                        batch_target_kind,
                                        cmd,
                                    )),
                                });
                                failed = true;
                                if opts.stop_on_error {
                                    break;
                                }
                            }
                            protocol::BatchCommand::SaveCurrentStyleSettings => {
                                let command = batch_command_name(cmd);
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command,
                                    elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                    value: None,
                                    error: Some(unsupported_batch_command_error(
                                        batch_target_kind,
                                        cmd,
                                    )),
                                });
                                failed = true;
                                if opts.stop_on_error {
                                    break;
                                }
                            }
                            protocol::BatchCommand::FilterAndSelect {
                                filter,
                                select_first,
                                submit,
                            } => {
                                let filter = filter.clone();
                                let select_first = *select_first;
                                let submit = *submit;
                                match set_main_window_input_text_for_batch(
                                    &this,
                                    main_batch_window_handle,
                                    expected.as_ref(),
                                    &dispatch_guard,
                                    &filter,
                                    cx,
                                )
                                .and_then(|_| {
                                    this.update(cx, |this, cx| {
                                        validate_batch_main_effect(this, expected.as_ref(), &dispatch_guard, cx)?;
                                        if select_first {
                                            this.select_first_choice(submit, cx)
                                        } else {
                                            Ok(None)
                                        }
                                    })
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
                                match set_main_window_input_text_for_batch(
                                    &this,
                                    main_batch_window_handle,
                                    expected.as_ref(),
                                    &dispatch_guard,
                                    &text,
                                    cx,
                                )
                                .and_then(|_| {
                                    this.update(cx, |this, cx| {
                                        validate_batch_main_effect(this, expected.as_ref(), &dispatch_guard, cx)?;
                                        this.submit_current_value(cx)
                                    })?
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
                            protocol::BatchCommand::OpenActions => {
                                let result = if let Some(window_handle) =
                                    crate::get_main_window_handle()
                                {
                                    window_handle.update(cx, |_, window, cx| {
                                        validate_batch_window_effect(expected.as_ref(), &dispatch_guard, &this, window, cx)?;
                                        this.update(cx, |this, cx| {
                                            this.dispatch_actions_toggle_for_current_view(
                                                window,
                                                cx,
                                                "devtools_batch_open_actions",
                                            )
                                        })
                                    })
                                } else {
                                    Err(anyhow::anyhow!("Main window handle is not available"))
                                };

                                match result {
                                    Ok(Ok(true)) => {
                                        tracing::info!(category = "BATCH", request_id = %rid, index = index, command = "openActions", "batch.step.ok");
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: true,
                                            command: "openActions".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: None,
                                        });
                                    }
                                    Ok(Ok(false)) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "openActions".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(
                                                "Current main view does not expose actions",
                                            )),
                                        });
                                        failed = true;
                                        if opts.stop_on_error {
                                            break;
                                        }
                                    }
                                    Ok(Err(e)) | Err(e) => {
                                        results.push(protocol::BatchResultEntry {
                                            index,
                                            success: false,
                                            command: "openActions".to_string(),
                                            elapsed: Some(cmd_start.elapsed().as_millis() as u64),
                                            value: None,
                                            error: Some(protocol::TransactionError::action_failed(
                                                format!("{e}"),
                                            )),
                                        });
                                        failed = true;
                                        if opts.stop_on_error {
                                            break;
                                        }
                                    }
                                }
                            }
                            protocol::BatchCommand::TogglePreview => {
                                let command = batch_command_name(cmd);
                                results.push(protocol::BatchResultEntry {
                                    index,
                                    success: false,
                                    command,
                                    elapsed: Some(0),
                                    value: None,
                                    error: Some(unsupported_batch_command_error(
                                        AutomationBatchTargetKind::Main,
                                        cmd,
                                    )),
                                });
                                failed = true;
                                if opts.stop_on_error {
                                    break;
                                }
                            }
                            protocol::BatchCommand::ForceSubmit { value } => {
                                let value = value.clone();
                                match this.update(cx, |this, cx| {
                                    validate_batch_main_effect(this, expected.as_ref(), &dispatch_guard, cx)?;
                                    let prompt_id = match &this.current_view {
                                        AppView::ArgPrompt { id, .. } => Some(id.clone()),
                                        AppView::DivPrompt { id, .. } => Some(id.clone()),
                                        AppView::FormPrompt { id, .. } => Some(id.clone()),
                                        AppView::TermPrompt { id, .. } => Some(id.clone()),
                                        AppView::EditorPrompt { id, .. } => Some(id.clone()),
                                        AppView::TemplatePrompt { id, .. } => Some(id.clone()),
                                        _ => None,
                                    };
                                    if let Some(id) = prompt_id {
                                        let value_str = match &value {
                                            serde_json::Value::String(s) => s.clone(),
                                            serde_json::Value::Null => String::new(),
                                            other => other.to_string(),
                                        };
                                        this.record_submit_diagnostic(
                                            "protocol",
                                            "forceSubmit",
                                            Some(id.as_str()),
                                            Some(value_str.as_str()),
                                            false,
                                        );
                                        this.submit_prompt_response(id, Some(value_str.clone()), cx);
                                        if let Some(error) = this.prompt_completion.as_ref().and_then(|binding| binding.observation().error) {
                                            return Err(anyhow::anyhow!(error));
                                        }
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
                                let (wait_timeout, wait_poll) = bounded_batch_wait(
                                    *timeout, *poll_interval, batch_timeout.saturating_sub(batch_start.elapsed()),
                                );
                                let wait_start = std::time::Instant::now();
                                let mut polls = 0;
                                let wait_result = loop {
                                    let remaining = wait_timeout.saturating_sub(wait_start.elapsed())
                                        .min(batch_timeout.saturating_sub(batch_start.elapsed()));
                                    if remaining.is_zero() || polls >= MAX_WAIT_POLLS {
                                        break Err(protocol::TransactionError::wait_timeout("Batch wait deadline or poll budget exceeded"));
                                    }
                                    if let Err(error) = dispatch_guard.validate() {
                                        break Err(protocol::TransactionError::action_failed(error));
                                    }
                                    polls += 1;
                                    match this.update(cx, |this, cx| this.wait_condition_satisfied(condition, cx)) {
                                        Ok(true) => break Ok(()),
                                        Ok(false) => {}
                                        Err(error) => break Err(protocol::TransactionError::action_failed(error.to_string())),
                                    }
                                    cx.background_executor().timer(wait_poll.min(remaining)).await;
                                };
                                let error = wait_result.err();
                                let success = error.is_none();
                                results.push(protocol::BatchResultEntry {
                                    index, success, command: "waitFor".into(),
                                    elapsed: Some(wait_start.elapsed().as_millis() as u64), value: None, error,
                                });
                                if !success {
                                    failed = true;
                                    if opts.stop_on_error { break; }
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

                    let trace = match protocol::transaction_trace::maybe_persist_batch_trace_from_results(
                        trace_mode,
                        rid.clone(),
                        command_fingerprint.clone(),
                        batch_started_at_ms,
                        total_elapsed,
                        success,
                        failed_at,
                        &commands,
                        &results,
                        None,
                    ) {
                        Ok(trace) => trace,
                        Err(error) => {
                            tracing::warn!(
                                target: "script_kit::transaction",
                                request_id = %rid,
                                error = %error,
                                "batch trace persistence failed"
                            );
                            if let Some(ref s) = sender {
                                let _ = s.try_send(Message::batch_result(
                                    rid.clone(),
                                    false,
                                    vec![protocol::BatchResultEntry {
                                        index: 0,
                                        success: false,
                                        command: "trace".to_string(),
                                        elapsed: Some(total_elapsed),
                                        value: None,
                                        error: Some(protocol::TransactionError::action_failed(format!(
                                            "failed to persist transaction trace: {error}"
                                        ))),
                                    }],
                                    Some(0),
                                    total_elapsed,
                                ));
                            }
                            return;
                        }
                    };

                    tracing::info!(
                        category = "AUTOMATION",
                        request_id = %rid,
                        success = success,
                        total_elapsed_ms = total_elapsed,
                        failed_at = ?failed_at,
                        trace_included = trace.is_some(),
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
                    AppView::TemplatePrompt { id, .. } => Some(id.clone()),
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
            // Additional prompt types
            // ============================================================
            PromptMessage::ShowPath {
                id,
                start_path,
                hint,
            } => {
                let seed = PromptSeed::Path(PathPromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    source: PathSource::Production(start_path),
                    hint,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("path", false, cx);
            }
            PromptMessage::ShowEnv {
                id,
                key,
                prompt,
                title,
                secret,
            } => {
                if let Err(error) =
                    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Credentials)
                {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }

                tracing::info!(
                    category = "UI",
                    id = %id,
                    key = %key,
                    secret,
                    "ShowEnv prompt received"
                );

                // Check if key already exists in secrets (for UX messaging). Missing
                // keys stay distinct from storage/decrypt/parse failures.
                let (exists_in_keyring, modified_at, stored_secret_value, secret_store_error) =
                    match secrets::get_secret_info_result(&key) {
                        Ok(secret_info) => {
                            let exists = secret_info
                                .as_ref()
                                .map(|info| !info.value.is_empty())
                                .unwrap_or(false);
                            let modified_at = secret_info.as_ref().map(|info| info.modified_at);
                            let value = secret_info.map(|info| info.value);
                            (exists, modified_at, value, None)
                        }
                        Err(error) => {
                            tracing::warn!(
                                category = "UI",
                                key = %key,
                                kind = error.kind_str(),
                                "EnvPrompt secret store unavailable"
                            );
                            (false, None, None, Some(error))
                        }
                    };

                let previous_view = self.current_view.clone();
                let previous_focus = self.pending_focus;
                let previous_input = self.focused_input;
                let common = PromptSeedCommon::sdk(id, None, self.response_sender.clone());
                let completion = common.completion.clone();
                let seed = PromptSeed::Env(EnvPromptSeed {
                    common,
                    key,
                    prompt,
                    title,
                    secret,
                    facts: EnvSecretFacts {
                        exists: exists_in_keyring,
                        modified_at,
                        stored_value: stored_secret_value,
                        error: secret_store_error,
                    },
                    local_storage: None,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                let auto_submitted = if let AppView::EnvPrompt { entity, .. } = &self.current_view {
                    entity.update(cx, |prompt, _cx| {
                        !prompt.has_prompt_or_title() && prompt.check_keyring_and_auto_submit()
                    })
                } else {
                    false
                };
                if auto_submitted {
                    let completion = completion.observation();
                    if let Some(error) = completion.error {
                        self.show_error_toast(error.to_string(), cx);
                    } else if completion.completed {
                        self.transition_current_view_and_rekey_main_automation_surface(
                            previous_view,
                        );
                        self.pending_focus = previous_focus;
                        self.focused_input = previous_input;
                        if matches!(self.current_view, AppView::ScriptList) {
                            self.flush_pending_main_menu_query(cx);
                        }
                        cx.notify();
                        return;
                    }
                }
                self.prepare_constructed_sdk_prompt("env", false, cx);
            }
            PromptMessage::ShowDrop {
                id,
                placeholder,
                hint,
            } => {
                let seed = PromptSeed::Drop(DropPromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    placeholder,
                    hint,
                    owned_files: None,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("drop", false, cx);
            }
            PromptMessage::ShowTemplate { id, template } => {
                let seed = PromptSeed::Template(TemplatePromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    template,
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("template", false, cx);
            }

            PromptMessage::ShowSelect {
                id,
                placeholder,
                choices,
                multiple,
            } => {
                let seed = PromptSeed::Select(SelectPromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    placeholder,
                    choices,
                    multiple,
                    disabled: Vec::new(),
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("select", false, cx);
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
                    message_bytes = message.len(),
                    "ShowConfirm prompt"
                );

                let options = crate::confirm::ParentConfirmOptions {
                    title: "Confirm".into(),
                    body: gpui::SharedString::from(message),
                    confirm_text: confirm_text
                        .map(gpui::SharedString::from)
                        .unwrap_or("OK".into()),
                    cancel_text: cancel_text
                        .map(gpui::SharedString::from)
                        .unwrap_or("Cancel".into()),
                    ..Default::default()
                };

                if let Some(window) =
                    current_window.filter(|_| !crate::runtime_policy::is_owned_evaluation())
                {
                    let binding = PromptCompletionBinding::sdk(id, self.response_sender.clone());
                    if let Some(previous) = self.prompt_completion.replace(binding.clone()) {
                        previous.retire();
                    }
                    let send_confirm = binding.clone();
                    let confirm_app = cx.entity().downgrade();
                    let cancel_app = confirm_app.clone();
                    self.prepare_window_for_prompt("UI", "confirm", "");
                    crate::confirm::open_parent_confirm_dialog(
                        window,
                        cx,
                        options,
                        move |_window, cx| {
                            if let Err(error) =
                                send_confirm.try_complete(PromptOutcome::Confirmed(true))
                            {
                                let _ = confirm_app.update(cx, |app, cx| {
                                    app.show_error_toast(error.to_string(), cx)
                                });
                            }
                        },
                        move |_window, cx| {
                            if let Err(error) =
                                binding.try_complete(PromptOutcome::Confirmed(false))
                            {
                                let _ = cancel_app.update(cx, |app, cx| {
                                    app.show_error_toast(error.to_string(), cx)
                                });
                            }
                        },
                    );
                } else {
                    let seed = PromptSeed::Confirm(ConfirmPromptSeed {
                        common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                        options,
                    });
                    if let Err(error) = self.construct_prompt_seed(seed, cx) {
                        self.show_error_toast(error.to_string(), cx);
                        return;
                    }
                    self.prepare_constructed_sdk_prompt("confirm", false, cx);
                }

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

                if use_builtin_ai {
                    if let Err(error) = crate::runtime_policy::check(
                        crate::runtime_policy::ExternalEffect::Provider,
                    ) {
                        self.show_error_toast(error.to_string(), cx);
                        return;
                    }
                }
                if save_history {
                    if let Err(error) = crate::runtime_policy::check(
                        crate::runtime_policy::ExternalEffect::ExternalStorage,
                    ) {
                        self.show_error_toast(error.to_string(), cx);
                        return;
                    }
                }

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

                self.sdk_actions = None;
                self.action_shortcuts.clear();
                if let Some(actions) = actions {
                    self.set_sdk_actions_and_shortcuts(actions, "CHAT", false);
                }
                let binding =
                    PromptCompletionBinding::sdk(id.clone(), self.response_sender.clone());
                if let Some(previous) = self.prompt_completion.replace(binding.clone()) {
                    previous.retire();
                }
                let dismiss_binding = binding.clone();

                let dismiss_sender = self.inline_chat_escape_sender.clone();
                let dismiss_main_window_mode = self.main_window_mode;
                let dismiss_callback: crate::prompts::ChatPromptDismissCallback =
                    std::sync::Arc::new(move |request| {
                        tracing::info!(
                            target: "script_kit::mini_ai",
                            event = "mini_ai_window_close_requested",
                            prompt_id = %request.prompt_id,
                            main_window_mode = ?dismiss_main_window_mode,
                            trigger = ?request.trigger,
                            "SDK ChatPrompt dismiss requested"
                        );
                        if let Err(error) = dismiss_binding.try_complete(PromptOutcome::Cancelled) {
                            tracing::warn!(%error, "SDK Chat cancellation refused");
                            return;
                        }
                        if let Err(error) = dismiss_sender.try_send(()) {
                            tracing::warn!(%error, "SDK Chat dismissal route unavailable");
                        }
                    });

                let chat_submit_callback = binding.chat_submit_callback();

                // Create ChatPrompt entity with configured models
                let focus_handle = self.focus_handle.clone();
                let mut chat_prompt = prompts::ChatPrompt::new_sdk(
                    id.clone(),
                    placeholder,
                    messages,
                    hint,
                    footer,
                    focus_handle,
                    Some(chat_submit_callback),
                    save_history,
                    std::sync::Arc::clone(&self.theme),
                )
                .with_dismiss_binding(crate::prompts::ChatPromptDismissBinding {
                    route: crate::prompts::ChatPromptDismissRoute::Back,
                    active_work: crate::components::conversation_actions::ActiveWorkDismissal::RequiresExplicitStop,
                    callback: dismiss_callback,
                })
                .with_mini_mode(self.main_window_mode == MainWindowMode::Mini);

                // Apply model configuration from SDK
                if !models.is_empty() {
                    chat_prompt = chat_prompt.with_model_names(models);
                }
                if let Some(default_model) = model {
                    chat_prompt = chat_prompt.with_default_model(default_model);
                }

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
                let actions_sender = self.inline_chat_actions_sender.clone();
                entity.update(cx, |chat, _cx| {
                    chat.set_on_show_actions(std::sync::Arc::new(move |prompt_id| {
                        tracing::info!(
                            target: "script_kit::mini_ai",
                            event = "on_show_actions.triggered",
                            source = "sdk-chat",
                            prompt_id = %prompt_id,
                            "ChatPrompt requested actions dialog via callback"
                        );
                        let _ = actions_sender.try_send(MiniAiUiRequest::ToggleActions {
                            prompt_id: prompt_id.to_string(),
                            source: "sdk_chat",
                        });
                    }));
                });
                self.transition_current_view_and_rekey_main_automation_surface(
                    AppView::ChatPrompt { id, entity },
                );
                self.focused_input = FocusedInput::None;
                self.pending_focus = Some(FocusTarget::ChatPrompt);
                self.bind_owned_surface_revision_observers(cx);
                logging::bench_log("ChatPrompt_created");

                self.prepare_constructed_sdk_prompt("chat", false, cx);
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
                // The raw SDK error string is classified at the prompt
                // boundary (`set_message_error`); log only its size so raw
                // provider payloads never reach the log stream.
                tracing::info!(
                    category = "CHAT",
                    id = %id,
                    message_id = %message_id,
                    error_len = error.len(),
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
                if script_kit_gpui::script_requested_hide() {
                    script_kit_gpui::set_script_requested_hide(false);
                    tracing::info!(
                        category = "VISIBILITY",
                        "HUD consumed script-requested hide without restoring main window"
                    );
                }
                self.show_hud(text, duration_ms, cx);
            }
            PromptMessage::SetStatus { status, message } => {
                tracing::info!(
                    category = "STATUS",
                    state = "received",
                    status = %status,
                    has_message = message.is_some(),
                    message_bytes = message.as_ref().map_or(0, String::len),
                    "Received setStatus() protocol message"
                );
            }
            PromptMessage::SetInput { text } => {
                if let Some(window) = current_window {
                    if let Err(error) = self.set_input_text_in_window(&text, window, cx) {
                        self.show_error_toast(error.to_string(), cx);
                    }
                } else if matches!(
                    self.current_view,
                    AppView::EditorPrompt { .. } | AppView::ScratchPadView { .. }
                ) {
                    self.show_error_toast("prompt_input_window_required".to_string(), cx);
                } else {
                    self.set_prompt_input(text, cx);
                }
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
            PromptMessage::ShowHotkey { id, placeholder } => {
                let seed = PromptSeed::Hotkey(HotkeyPromptSeed {
                    common: PromptSeedCommon::sdk(id, None, self.response_sender.clone()),
                    description: placeholder.unwrap_or_else(|| "Press a shortcut".into()),
                });
                if let Err(error) = self.construct_prompt_seed(seed, cx) {
                    self.show_error_toast(error.to_string(), cx);
                    return;
                }
                self.prepare_constructed_sdk_prompt("hotkey", false, cx);
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

                // Open Agent Chat (creates new if not open, brings to front if open)
                if let Err(e) = crate::ai::agent_chat::ui::chat_window::open_chat_window(cx) {
                    tracing::error!(
                        category = "ERROR",
                        error = %e,
                        "Failed to open Agent Chat for AiStartChat"
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
                let context_parts: Vec<crate::ai::AiContextPart> = parts
                    .into_iter()
                    .map(|part| match part {
                        crate::protocol::AiContextPartInput::ResourceUri { uri, label } => {
                            crate::ai::AiContextPart::ResourceUri { uri, label }
                        }
                        crate::protocol::AiContextPartInput::FilePath { path, label } => {
                            crate::ai::AiContextPart::FilePath { path, label }
                        }
                        crate::protocol::AiContextPartInput::TextBlock {
                            label,
                            source,
                            text,
                            mime_type,
                        } => crate::ai::AiContextPart::TextBlock {
                            label,
                            source,
                            text,
                            mime_type,
                        },
                    })
                    .collect();

                if system_prompt.is_some() || image.is_some() {
                    tracing::warn!(
                        category = "AI",
                        request_id = %request_id,
                        has_system_prompt = system_prompt.is_some(),
                        has_image = image.is_some(),
                        "AiStartChat system prompt and image are not carried into Agent Chat"
                    );
                }

                // Stage the message in the Agent Chat composer once the view is
                // ready, then optionally submit it as the first turn.
                let message_for_chat = message.clone();
                cx.spawn(async move |_this, cx| {
                    let mut waits_completed = 0usize;
                    loop {
                        let ready = cx.update(|_cx| {
                            crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()
                                .is_some()
                        });
                        if ready || waits_completed >= 8 {
                            break;
                        }
                        waits_completed += 1;
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(16))
                            .await;
                    }
                    cx.update(|cx| {
                        let Some(entity) = crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity() else {
                            tracing::error!(
                                category = "ERROR",
                                "Agent Chat view unavailable for AiStartChat handoff"
                            );
                            return;
                        };
                        entity.update(cx, |chat, cx| {
                            if chat.is_setup_mode() {
                                tracing::error!(
                                    category = "ERROR",
                                    "Agent Chat is in setup mode; cannot start chat"
                                );
                                return;
                            }
                            chat.live_thread().update(cx, |thread, cx| {
                                for part in context_parts {
                                    thread.add_context_part(part, cx);
                                }
                                thread.set_input(message_for_chat, cx);
                                if should_submit {
                                    if let Err(error) = thread.submit_input(cx) {
                                        tracing::error!(
                                            category = "ERROR",
                                            error = %error,
                                            "AiStartChat submit failed"
                                        );
                                    }
                                }
                            });
                        });
                    });
                })
                .detach();

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

                // Check if the chat window was already open before we open/focus it
                let was_open = crate::ai::agent_chat::ui::chat_window::is_chat_window_open();

                // Open Agent Chat (creates new if not open, brings to front if open)
                let success = match crate::ai::agent_chat::ui::chat_window::open_chat_window(cx) {
                    Ok(()) => {
                        tracing::info!(category = "AI", "Agent Chat focused successfully");
                        true
                    }
                    Err(e) => {
                        tracing::error!(
                            category = "ERROR",
                            error = %e,
                            "Failed to focus Agent Chat"
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
            PromptMessage::SimulateGpuiEvent { message } => {
                // A scheduling ticket is internal. The shared producer emits exactly one
                // correlated terminal reply after execution or cancellation.
                if let Some(sender) = self.response_sender.clone() {
                    let mut message = *message;
                    let precondition =
                        gpui_dispatch_precondition(&mut message, cx.entity().downgrade());
                    crate::platform::gpui_event_simulator::handle_gpui_event_message(
                        message,
                        sender,
                        precondition,
                        cx,
                    );
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
                    let input = self.current_input_value(cx);
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
                    let snapshot = self.build_main_ui_snapshot(cx);
                    crate::protocol::transaction_executor::matches_state_spec(&snapshot, expected)
                }
                detailed => protocol::transaction_executor::matches_agent_chat_wait_condition(
                    detailed,
                    &self.collect_agent_chat_state(cx),
                    || self.collect_agent_chat_test_probe(1, cx),
                )
                .unwrap_or(false),
            },
        }
    }

    /// Get the current prompt type as a string.
    fn current_prompt_type(&self, cx: &App) -> String {
        match &self.current_view {
            AppView::ScriptList => "none".to_string(),
            AppView::ArgPrompt { .. } => "arg".to_string(),
            AppView::DivPrompt { .. } => "div".to_string(),
            AppView::FormPrompt { entity, .. } => entity.read(cx).prompt_type().to_string(),
            AppView::EditorPrompt { .. } => "editor".to_string(),
            AppView::TermPrompt { .. } => "term".to_string(),
            AppView::HotkeyPrompt { .. } => "hotkey".to_string(),
            AppView::ChatPrompt { .. } => "chat".to_string(),
            AppView::MiniPrompt { .. } => "mini".to_string(),
            AppView::MicroPrompt { .. } => "micro".to_string(),
            AppView::DayPage { .. } => "dayPage".to_string(),
            _ => "unknown".to_string(),
        }
    }

    /// Get the active launcher surface contract for `getState.surfaceContract`.
    fn surface_presentation_snapshot(
        view: &AppView,
    ) -> crate::protocol::SurfacePresentationSnapshot {
        use crate::protocol::SurfaceRowPrimitive;

        let contract = view.surface_contract();
        let header = view.main_view_header_input_policy();
        let (shell_owner, intentional_divergence) = match header {
            crate::MainViewHeaderInputPolicy::ViewOwnedCanonicalInput
            | crate::MainViewHeaderInputPolicy::ViewOwnedCanonicalMultilineInput
            | crate::MainViewHeaderInputPolicy::ViewOwnedContextOnly => {
                ("components::main_view_chrome", None)
            }
            crate::MainViewHeaderInputPolicy::RootContextOnly => {
                ("components::prompt_layout_shell", None)
            }
            crate::MainViewHeaderInputPolicy::ViewOwnedIntentionalCompact => (
                "focused_text::compact_shell",
                Some("Focused Text Mini intentionally owns a compact 44px input shell."),
            ),
        };
        let input_owner = match contract.vocabulary.input_ownership {
            crate::LauncherSurfaceInputOwnership::LauncherFilter => "components::text_input",
            crate::LauncherSurfaceInputOwnership::PromptEntity => "prompt_entity::shared_input",
            crate::LauncherSurfaceInputOwnership::ChildView => "child_view::owned_input",
            crate::LauncherSurfaceInputOwnership::NoEditableInput => "none",
        };
        let row_primitive = match view {
            AppView::SelectPrompt { .. } => SurfaceRowPrimitive::UnifiedListItem,
            AppView::AgentChatView { .. }
            | AppView::ChatPrompt { .. }
            | AppView::FlowSessionView { .. } => SurfaceRowPrimitive::ConversationTurn,
            AppView::ScriptList | AppView::ActionsDialog | AppView::ArgPrompt { .. } => {
                SurfaceRowPrimitive::LegacyListItem
            }
            _ => match contract.vocabulary.family {
                crate::LauncherSurfaceFamily::MainMenu
                | crate::LauncherSurfaceFamily::FilterableLauncherList
                | crate::LauncherSurfaceFamily::AttachmentPortal => {
                    SurfaceRowPrimitive::LegacyListItem
                }
                crate::LauncherSurfaceFamily::AssistantWorkspace
                | crate::LauncherSurfaceFamily::UtilityWorkspace => {
                    SurfaceRowPrimitive::SpecializedContent
                }
                crate::LauncherSurfaceFamily::ScriptPrompt
                | crate::LauncherSurfaceFamily::FeedbackSurface => SurfaceRowPrimitive::None,
            },
        };
        let actions_owner = match contract.actions_policy {
            crate::LauncherSurfaceActionsPolicy::MainMenuActions
            | crate::LauncherSurfaceActionsPolicy::HostRowActions
            | crate::LauncherSurfaceActionsPolicy::ActionsDialogActions => Some("actions::dialog"),
            crate::LauncherSurfaceActionsPolicy::PromptEntityActions => {
                Some("prompt_entity::actions")
            }
            crate::LauncherSurfaceActionsPolicy::ChildViewActions => Some("child_view::actions"),
            crate::LauncherSurfaceActionsPolicy::NoSurfaceActions => None,
        };

        crate::protocol::SurfacePresentationSnapshot {
            shell_owner: shell_owner.to_string(),
            input_owner: input_owner.to_string(),
            row_primitive,
            footer_owner: view
                .native_footer_surface()
                .map(|_| "footer_popup::native_footer".to_string()),
            actions_owner: actions_owner.map(str::to_string),
            theme_owner: "theme::AppChromeColors + ui::chrome::tokens".to_string(),
            intentional_divergence: intentional_divergence.map(str::to_string),
        }
    }

    fn current_surface_contract_snapshot(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
        cx: &gpui::App,
    ) -> crate::protocol::LauncherSurfaceContractSnapshot {
        let contract = self.current_view.surface_contract();
        let surface_kind = format!("{:?}", self.current_view.surface_kind());
        let app_view_variant = self.current_view.app_view_variant().to_string();
        let native_footer_surface = self
            .current_view
            .native_footer_surface()
            .map(str::to_string);
        let target_identity = Some(target.clone()).and_then(|window| {
            let facts = self.owned_revision_facts();
            let target_generation = window.generation.and_then(|generation| {
                crate::windows::automation_registry::automation_target_revision(
                    &window.id, generation,
                )
            })?;

            Some(crate::protocol::AutomationTargetIdentitySnapshot {
                window_id: window.id,
                window_generation: window.generation,
                app_view_variant: app_view_variant.clone(),
                target_generation,
                surface_generation: facts.surface_generation,
                data_generation: facts
                    .data_generation
                    .strict_add(self.gpui_input_state.read(cx).revision())
                    .strict_add(self.arg_input.revision())
                    .strict_add(self.owned_child_semantic_revision(cx)),
                presentation_revision: Some(facts.presentation_revision),
                theme_revision: Some(crate::theme::service::theme_revision()),
                frame_generation: None,
            })
        });
        crate::protocol::LauncherSurfaceContractSnapshot {
            schema_version: crate::protocol::LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION,
            surface_kind,
            family: format!("{:?}", contract.vocabulary.family),
            input_ownership: format!("{:?}", contract.vocabulary.input_ownership),
            preview_role: format!("{:?}", contract.vocabulary.preview_role),
            focus_policy: format!("{:?}", contract.focus_policy),
            keyboard_policy: format!("{:?}", contract.keyboard_policy),
            actions_policy: format!("{:?}", contract.actions_policy),
            proof_policy: format!("{:?}", contract.proof_policy),
            visual_policy: format!("{:?}", contract.visual_policy),
            automation_semantic_surface: contract.automation_semantic_surface.to_string(),
            native_footer_surface,
            target_identity,
            presentation: Some(Self::surface_presentation_snapshot(&self.current_view)),
        }
    }

    /// Get the active popup surface contract for `getState.activePopupContract`.
    fn active_popup_contract_snapshot(
        &self,
    ) -> Option<crate::protocol::LauncherSurfaceContractSnapshot> {
        if !(self.show_actions_popup || self.actions_dialog.is_some()) {
            return None;
        }
        let contract = AppView::ActionsDialog.surface_contract();
        Some(crate::protocol::LauncherSurfaceContractSnapshot {
            schema_version: crate::protocol::LAUNCHER_SURFACE_CONTRACT_SCHEMA_VERSION,
            surface_kind: "ActionsDialog".to_string(),
            family: format!("{:?}", contract.vocabulary.family),
            input_ownership: format!("{:?}", contract.vocabulary.input_ownership),
            preview_role: format!("{:?}", contract.vocabulary.preview_role),
            focus_policy: format!("{:?}", contract.focus_policy),
            keyboard_policy: format!("{:?}", contract.keyboard_policy),
            actions_policy: format!("{:?}", contract.actions_policy),
            proof_policy: format!("{:?}", contract.proof_policy),
            visual_policy: format!("{:?}", contract.visual_policy),
            automation_semantic_surface: contract.automation_semantic_surface.to_string(),
            native_footer_surface: AppView::ActionsDialog
                .native_footer_surface()
                .map(str::to_string),
            target_identity: None,
            presentation: Some(Self::surface_presentation_snapshot(&AppView::ActionsDialog)),
        })
    }

    fn footer_action_name(action: crate::footer_popup::FooterAction) -> String {
        match action {
            crate::footer_popup::FooterAction::Run => "run",
            crate::footer_popup::FooterAction::Actions => "actions",
            crate::footer_popup::FooterAction::Ai => "ai",
            crate::footer_popup::FooterAction::Apply => "apply",
            crate::footer_popup::FooterAction::Replace => "replace",
            crate::footer_popup::FooterAction::Append => "append",
            crate::footer_popup::FooterAction::Copy => "copy",
            crate::footer_popup::FooterAction::Expand => "expand",
            crate::footer_popup::FooterAction::Retry => "retry",
            crate::footer_popup::FooterAction::Close => "close",
            crate::footer_popup::FooterAction::Stop => "stop",
            crate::footer_popup::FooterAction::PasteResponse => "pasteResponse",
            crate::footer_popup::FooterAction::Cwd => "cwd",
            crate::footer_popup::FooterAction::AgentModel => "agentModel",
            crate::footer_popup::FooterAction::Tips => "tips",
        }
        .to_string()
    }

    fn active_footer_button_snapshot(
        button: &crate::footer_popup::FooterButtonConfig,
    ) -> crate::protocol::ActiveFooterButtonSnapshot {
        crate::protocol::ActiveFooterButtonSnapshot {
            id: button.id.to_string(),
            action: Self::footer_action_name(button.action),
            key: button.key.to_string(),
            shortcut_tokens: button.shortcut_tokens.clone(),
            canonical_shortcut: button.canonical_shortcut.clone(),
            shortcut_routable: button.shortcut_routable,
            label: button.label.to_string(),
            enabled: button.enabled,
            selected: button.selected,
            placement: match button.placement {
                crate::footer_popup::FooterPlacement::Leading => "leading",
                crate::footer_popup::FooterPlacement::Trailing => "trailing",
            }
            .to_string(),
            action_disabled: button.disabled_reason.as_ref().map(ToString::to_string),
        }
    }

    fn active_footer_dot_status_name(status: crate::footer_popup::FooterDotStatus) -> &'static str {
        match status {
            crate::footer_popup::FooterDotStatus::Hidden => "hidden",
            crate::footer_popup::FooterDotStatus::Streaming => "streaming",
            crate::footer_popup::FooterDotStatus::WaitingForPermission => "waitingForPermission",
            crate::footer_popup::FooterDotStatus::Idle => "idle",
            crate::footer_popup::FooterDotStatus::Error => "error",
        }
    }

    pub(crate) fn active_footer_snapshot(
        &self,
        target: &crate::protocol::AutomationWindowInfo,
    ) -> crate::protocol::ActiveFooterSnapshot {
        let expected_surface = self.current_view.native_footer_surface();
        let state = target.generation.and_then(|generation| {
            crate::footer_popup::footer_runtime_state(&target.id, generation)
        });
        let host = state.as_ref().map(|state| state.host).unwrap_or_default();
        let popup_open = self.show_actions_popup || self.actions_dialog.is_some();
        let config = state.map(|state| state.config);
        let slot_model = config.as_ref().map(|cfg| cfg.slot_model());
        let native_buttons: Vec<_> = config
            .as_ref()
            .map(|cfg| {
                cfg.buttons
                    .iter()
                    .map(Self::active_footer_button_snapshot)
                    .collect()
            })
            .unwrap_or_default();
        let left_info = config.as_ref().and_then(|cfg| {
            cfg.left_info
                .as_ref()
                .map(|info| crate::protocol::ActiveFooterLeftInfoSnapshot {
                    dot_status: Self::active_footer_dot_status_name(info.dot_status).to_string(),
                    model_name: info.model_name.clone(),
                    profile_name: info.profile_name.clone(),
                    icon_token: info.icon_token.clone(),
                    keycap: info.keycap.clone(),
                    action: info.action.map(Self::footer_action_name),
                    selected: info.selected,
                    cwd_chip: info.cwd_chip.as_ref().map(|chip| {
                        crate::protocol::ActiveFooterCwdChipSnapshot {
                            label: chip.label.clone(),
                            icon_token: chip.icon_token.clone(),
                        }
                    }),
                })
        });

        let native_ready = expected_surface.is_some()
            && host.native_host_installed
            && host.installed_surface == expected_surface;
        let agent_chat_footer_hidden = matches!(self.current_view, AppView::AgentChatView { .. })
            && expected_surface.is_some()
            && config.is_none();

        let prompt_owned = matches!(
            self.current_view,
            AppView::TermPrompt { .. }
                | AppView::SdkReferenceView { .. }
                | AppView::ScriptTemplateCatalogView { .. }
        );
        let content_owned = matches!(self.current_view, AppView::About { .. });
        let owner = if popup_open {
            "popup"
        } else if agent_chat_footer_hidden {
            "none"
        } else if native_ready {
            "native"
        } else if expected_surface.is_some() || prompt_owned {
            "prompt"
        } else if content_owned {
            "content"
        } else {
            "none"
        };

        let buttons = match owner {
            "native" | "prompt" if expected_surface.is_some() => native_buttons,
            "prompt" => [
                crate::footer_popup::FooterButtonConfig::new(
                    crate::footer_popup::FooterAction::Actions,
                    "⌘K",
                    "Actions",
                ),
                crate::footer_popup::FooterButtonConfig::new(
                    crate::footer_popup::FooterAction::Close,
                    "Esc",
                    "Close",
                ),
            ]
            .iter()
            .map(Self::active_footer_button_snapshot)
            .collect(),
            _ => Vec::new(),
        };
        let (
            action_slot_count,
            context_chip_count,
            duplicate_action_ids,
            duplicate_shortcut_keys,
            slot_contract_violation,
        ) = if let Some(model) = slot_model.as_ref() {
            (
                model.action_slot_count,
                model.context_chip_count,
                model.duplicate_action_ids.clone(),
                model.duplicate_shortcut_keys.clone(),
                model.violation.map(str::to_string),
            )
        } else {
            (
                buttons.len(),
                0,
                Vec::new(),
                Vec::new(),
                (buttons.len() > crate::footer_popup::MAIN_WINDOW_FOOTER_MAX_ACTION_SLOTS)
                    .then_some("too_many_action_slots".to_string()),
            )
        };

        let mismatch = match (expected_surface, host.installed_surface) {
            (Some(expected), Some(active)) if expected != active => {
                Some(format!("expected:{expected};active:{active}"))
            }
            (Some(expected), None) if host.requested_surface == Some(expected) => {
                Some(format!("native_host_missing:{expected}"))
            }
            _ => None,
        };

        crate::protocol::ActiveFooterSnapshot {
            schema_version: crate::protocol::ACTIVE_FOOTER_SCHEMA_VERSION,
            owner: owner.to_string(),
            expected_surface: expected_surface.map(str::to_string),
            requested_surface: host.requested_surface.map(str::to_string),
            active_surface: host.installed_surface.map(str::to_string),
            native_footer_host_installed: native_ready,
            gpui_fallback_visible: crate::footer_popup::main_footer_gpui_overlay_visible(),
            left_info,
            button_count: buttons.len(),
            action_slot_count,
            context_chip_count,
            duplicate_action_ids,
            duplicate_shortcut_keys,
            slot_contract_violation,
            buttons,
            mismatch,
        }
    }

    /// Get the current input/filter value.
    ///
    /// Verbatim-echo contract: this is the sole reader that produces
    /// `getState.inputValue`. For ScriptList, it returns
    /// `self.filter_text.clone()` unconditionally — no length cap, no
    /// truncation, no transformation. See
    /// `set_filter_text_immediate` at
    /// `src/app_impl/filter_input_updates.rs` for the companion writer
    /// and the full contract (stdin line cap `MAX_STDIN_COMMAND_BYTES`
    /// = 16 KiB is the only bound). Pinned by
    /// `tests/stdin_setfilter_input_value_verbatim_contract.rs`.
    fn current_input_value(&self, cx: &App) -> String {
        match &self.current_view {
            AppView::ScriptList => self.filter_text.clone(),
            AppView::ArgPrompt { .. } => self.arg_input.text().to_string(),
            AppView::MiniPrompt { .. } => self.arg_input.text().to_string(),
            AppView::MicroPrompt { .. } => self.arg_input.text().to_string(),
            AppView::DayPage { entity } => entity.read(cx).automation_input_value(cx),
            _ => String::new(),
        }
    }

    /// Get the currently selected value if any.
    fn current_selected_value(&self) -> Option<String> {
        match &self.current_view {
            AppView::ScriptList => {
                if let Some(value) = self
                    .menu_syntax_object_selector_state
                    .snapshot
                    .as_ref()
                    .filter(|_| self.menu_syntax_object_selector_state.owns_main_list())
                    .and_then(|snapshot| {
                        self.menu_syntax_object_selector_state
                            .selected_row_id
                            .as_deref()
                            .and_then(|id| snapshot.rows.iter().find(|row| row.id == id))
                    })
                    .map(|row| row.token.clone().unwrap_or_else(|| row.id.clone()))
                {
                    return Some(value);
                }
                self.menu_syntax_trigger_picker_state
                    .snapshot
                    .as_ref()
                    .filter(|_| self.menu_syntax_trigger_picker_state.owns_main_list())
                    .and_then(|snapshot| {
                        self.menu_syntax_trigger_picker_state
                            .selected_row_id
                            .as_deref()
                            .and_then(|id| snapshot.rows.iter().find(|row| row.id == id))
                    })
                    .map(|row| row.token.clone().unwrap_or_else(|| row.id.clone()))
            }
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

    /// Build a UI state snapshot for the Main launcher window.
    ///
    /// Used by waitFor polling to populate `before` / `after` / `polls[*].snapshot`
    /// in [`TransactionCommandTrace`](protocol::TransactionCommandTrace). Mirrors
    /// the fields populated by [`getState`]-handling earlier in this file.
    fn build_main_ui_snapshot(&self, cx: &Context<Self>) -> protocol::UiStateSnapshot {
        let window_visible = script_kit_gpui::is_main_window_visible();
        let window_focused = window_visible && self.focused_input != FocusedInput::None;
        let input_value = self.current_input_value(cx);
        let selected_value = self.current_selected_value();
        let outcome = self.collect_visible_elements(200, cx);
        let focused_semantic_id = outcome.focused_semantic_id();
        let visible_semantic_ids = outcome
            .elements
            .iter()
            .map(|el| el.semantic_id.clone())
            .collect();
        let choice_count = outcome
            .elements
            .iter()
            .filter(|el| el.element_type == protocol::ElementType::Choice)
            .count();
        protocol::UiStateSnapshot {
            window_visible,
            window_focused,
            prompt_type: Some(self.current_prompt_type(cx)),
            input_value: if input_value.is_empty() {
                None
            } else {
                Some(input_value)
            },
            selected_value,
            focused_semantic_id,
            visible_semantic_ids,
            choice_count,
            ..Default::default()
        }
    }

    /// Collect a machine-readable Agent Chat state snapshot.
    ///
    /// Returns a default (idle, empty) snapshot when the current view is not
    /// `AgentChatView` — callers should check `status == "notAgentChat"` to detect this.
    fn collect_agent_chat_state(&self, cx: &Context<Self>) -> protocol::AgentChatStateSnapshot {
        let entity = match &self.current_view {
            AppView::AgentChatView { entity } => entity,
            _ => {
                return protocol::AgentChatStateSnapshot {
                    status: "notAgentChat".to_string(),
                    ..Default::default()
                };
            }
        };

        let view = entity.read(cx);

        // Extract state from the Agent Chat view's public API.
        view.collect_agent_chat_state_snapshot(cx)
    }

    /// Collect Agent Chat state from the given detached entity, or fall through to main.
    fn collect_agent_chat_state_for_target(
        &self,
        detached_entity: Option<&gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>>,
        cx: &Context<Self>,
    ) -> protocol::AgentChatStateSnapshot {
        match detached_entity {
            Some(entity) => entity.read(cx).collect_agent_chat_state_snapshot(cx),
            None => self.collect_agent_chat_state(cx),
        }
    }

    /// Collect Agent Chat test probe from the given detached entity, or fall through to main.
    fn collect_agent_chat_test_probe_for_target(
        &self,
        detached_entity: Option<&gpui::Entity<crate::ai::agent_chat::ui::AgentChatView>>,
        tail: usize,
        cx: &Context<Self>,
    ) -> protocol::AgentChatTestProbeSnapshot {
        match detached_entity {
            Some(entity) => entity.read(cx).test_probe_snapshot(tail, cx),
            None => self.collect_agent_chat_test_probe(tail, cx),
        }
    }

    /// Reset the Agent Chat test probe ring buffer.
    fn reset_agent_chat_test_probe(&mut self, cx: &mut Context<Self>) {
        if let AppView::AgentChatView { entity } = &self.current_view {
            entity.update(cx, |view, _cx| {
                view.reset_test_probe();
            });
        }
    }

    /// Collect a bounded Agent Chat test probe snapshot.
    fn collect_agent_chat_test_probe(
        &self,
        tail: usize,
        cx: &Context<Self>,
    ) -> protocol::AgentChatTestProbeSnapshot {
        let entity = match &self.current_view {
            AppView::AgentChatView { entity } => entity,
            _ => {
                return protocol::AgentChatTestProbeSnapshot {
                    state: protocol::AgentChatStateSnapshot {
                        status: "notAgentChat".to_string(),
                        ..Default::default()
                    },
                    ..Default::default()
                };
            }
        };

        let view = entity.read(cx);
        view.test_probe_snapshot(tail, cx)
    }

    fn set_input_text_in_window(
        &mut self,
        text: &str,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        match &self.current_view {
            AppView::EditorPrompt { entity, .. } | AppView::ScratchPadView { entity, .. } => {
                let entity = entity.clone();
                entity.update(cx, |editor, cx| {
                    editor.set_input(text.to_string(), window, cx)
                });
                self.mark_main_data_changed();
                cx.notify();
            }
            AppView::ScriptList => {
                self.menu_syntax_form_input_active = false;
                self.menu_syntax_form_draft_field_id = None;
                self.menu_syntax_form_draft_value.clear();
                self.menu_syntax_form_suggestion_field_id = None;
                self.menu_syntax_form_suggestion_selected_index = None;
                self.set_filter_text_immediate(text.to_string(), window, cx);
                cx.notify();
            }
            AppView::DayPage { entity } => {
                let entity = entity.clone();
                entity.update(cx, |view, cx| {
                    view.set_input(text.to_string(), window, cx);
                });
                cx.notify();
            }
            // Both surfaces use the shared main input and its production query setter.
            AppView::FlowSessionView { .. } | AppView::FileSearchView { .. } => {
                self.set_filter_text_immediate(text.to_string(), window, cx);
                cx.notify();
            }
            _ => self.set_input_text(text, cx)?,
        }
        Ok(())
    }

    /// Set the input text for the current prompt.
    fn set_input_text(&mut self, text: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        match &self.current_view {
            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => {
                self.arg_input.set_text(text);
                self.filter_text = text.to_string();
                self.pending_filter_sync = true;
                self.mark_main_data_changed();
                self.set_arg_selected_index(0);
                cx.notify();
            }
            AppView::ScriptList => {
                let text = text.to_string();
                self.filter_text = text.clone();
                self.selected_index = 0;
                self.queue_filter_compute(text, cx);
                cx.notify();
            }
            AppView::AgentChatView { entity } => {
                let entity = entity.clone();
                entity.update(cx, |view, cx| view.set_input(text.to_string(), cx));
                cx.notify();
            }
            AppView::ChatPrompt { entity, .. } => {
                let entity = entity.clone();
                entity.update(cx, |prompt, cx| prompt.set_input(text.to_string(), cx));
                cx.notify();
            }
            AppView::SelectPrompt { entity, .. } => {
                let entity = entity.clone();
                entity.update(cx, |prompt, cx| prompt.set_input(text.to_string(), cx));
                cx.notify();
            }
            AppView::TemplatePrompt { entity, .. } => {
                let entity = entity.clone();
                entity.update(cx, |prompt, cx| prompt.set_input(text.to_string(), cx));
                cx.notify();
            }
            AppView::FormPrompt { entity, .. } => {
                let entity = entity.clone();
                entity.update(cx, |prompt, cx| prompt.set_input(text.to_string(), cx));
                cx.notify();
            }
            AppView::QuickTerminalView { entity } => {
                let entity = entity.clone();
                let payload = text.to_string();
                entity.update(cx, |term, cx| {
                    term.send_raw_input(&payload)?;
                    cx.notify();
                    anyhow::Ok(())
                })?;
            }
            // FlowSessionView and FileSearchView use the window-aware main-input
            // path above; they never reach this fallback.
            _ => anyhow::bail!("set_input_not_supported_for_current_view"),
        }
        Ok(())
    }

    /// Select a choice by its value from the filtered list.
    fn select_choice_by_value(
        &mut self,
        value: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        match self.devtools_selection_state() {
            DevtoolsSelectionState::MainMenuScriptList => {
                self.select_main_menu_choice_by_value(value, submit, cx)
            }
            DevtoolsSelectionState::ChoiceBackedPrompt => {
                self.select_prompt_choice_by_value(value, submit, cx)
            }
            DevtoolsSelectionState::UnsupportedPrompt => {
                anyhow::bail!("selectByValue only supports visible choice surfaces")
            }
        }
    }

    /// Select a choice by semantic ID, optionally submitting.
    fn select_choice_by_semantic_id(
        &mut self,
        semantic_id: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        if semantic_id == "button:dictation-history-load-more" {
            let AppView::DictationHistoryView { visible_limit, .. } = &mut self.current_view else {
                anyhow::bail!("Dictation History Load More is not visible");
            };
            if submit {
                *visible_limit =
                    visible_limit.saturating_add(crate::dictation::DICTATION_HISTORY_PAGE_SIZE);
                cx.notify();
            }
            return Ok(semantic_id.to_string());
        }

        if semantic_id == "footer:native:close"
            && matches!(self.current_view, AppView::QuickTerminalView { .. })
        {
            if submit {
                self.close_quick_terminal_main_window_state_first(cx);
            }
            return Ok(semantic_id.to_string());
        }

        if semantic_id == "footer:quick_terminal:ai" || semantic_id == "footer:prompt:ai" {
            let AppView::QuickTerminalView { entity } = &self.current_view else {
                anyhow::bail!("Quick Terminal Agent footer is only available in QuickTerminalView");
            };
            if submit {
                self.open_agent_chat_with_quick_terminal_output(entity.clone(), cx);
            }
            return Ok(semantic_id.to_string());
        }

        if let AppView::FormPrompt { entity, .. } = &self.current_view {
            let entity = entity.clone();
            let selected = entity.update(cx, |form, cx| {
                let selected = form.focus_field_by_semantic_id(semantic_id);
                cx.notify();
                selected
            });

            if let Some(selected) = selected {
                if submit {
                    self.submit_current_value(cx)?;
                }
                return Ok(selected);
            }

            anyhow::bail!("No form field matched semantic ID '{semantic_id}'");
        }

        match self.devtools_selection_state() {
            DevtoolsSelectionState::MainMenuScriptList => {
                self.select_main_menu_choice_by_semantic_id(semantic_id, submit, cx)
            }
            DevtoolsSelectionState::ChoiceBackedPrompt => {
                self.select_prompt_choice_by_semantic_id(semantic_id, submit, cx)
            }
            DevtoolsSelectionState::UnsupportedPrompt => {
                anyhow::bail!("selectBySemanticId only supports visible choice surfaces")
            }
        }
    }

    /// Select the first choice in the filtered list.
    fn select_first_choice(
        &mut self,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Option<String>> {
        match self.devtools_selection_state() {
            DevtoolsSelectionState::MainMenuScriptList => {
                self.select_first_main_menu_choice(submit, cx)
            }
            DevtoolsSelectionState::ChoiceBackedPrompt => {
                self.select_first_prompt_choice(submit, cx)
            }
            DevtoolsSelectionState::UnsupportedPrompt => {
                anyhow::bail!("selectFirst only supports visible choice surfaces")
            }
        }
    }

    fn devtools_selection_state(&self) -> DevtoolsSelectionState {
        match &self.current_view {
            AppView::ScriptList => DevtoolsSelectionState::MainMenuScriptList,
            AppView::ArgPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => DevtoolsSelectionState::ChoiceBackedPrompt,
            _ => DevtoolsSelectionState::UnsupportedPrompt,
        }
    }

    fn select_main_menu_choice_by_value(
        &mut self,
        value: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        anyhow::ensure!(
            self.root_search.query_is_current(),
            "main_menu_query_pending"
        );
        let mut matches = self
            .main_menu_committed_rows()
            .iter()
            .filter(|row| row.eligibility.selectable)
            .filter_map(|row| {
                let matched = match row.subject {
                    MainMenuRowSubject::SearchResult { flat_index } => {
                        let result = self.main_menu_committed_results().get(flat_index)?;
                        result.launcher_command_id().as_deref() == Some(value)
                            || result.launcher_command_name() == value
                    }
                    MainMenuRowSubject::Calculator => self
                        .main_menu_committed_calculator()
                        .is_some_and(|calculator| calculator.formatted == value),
                };
                matched.then_some(row.grouped_index)
            });
        let grouped_index = matches
            .next()
            .ok_or_else(|| anyhow::anyhow!("main_menu_choice_not_found"))?;
        anyhow::ensure!(matches.next().is_none(), "ambiguous_main_menu_value");
        self.apply_main_menu_selection(grouped_index, submit, cx)?;
        Ok(value.to_string())
    }

    fn select_main_menu_choice_by_semantic_id(
        &mut self,
        semantic_id: &str,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<String> {
        let row = self
            .resolve_main_menu_semantic_row(semantic_id)
            .ok_or_else(|| anyhow::anyhow!("main_menu_semantic_target_not_current"))?;
        let grouped_index = row.grouped_index;
        let selected = match row.subject {
            MainMenuRowSubject::SearchResult { flat_index } => {
                self.main_menu_committed_results()[flat_index].launcher_command_name()
            }
            MainMenuRowSubject::Calculator => self
                .main_menu_committed_calculator()
                .ok_or_else(|| anyhow::anyhow!("main_menu_calculator_missing"))?
                .formatted
                .clone(),
        };
        self.apply_main_menu_selection(grouped_index, submit, cx)?;
        Ok(selected)
    }

    fn select_first_main_menu_choice(
        &mut self,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<Option<String>> {
        let Some(semantic_id) = self
            .main_menu_committed_rows()
            .iter()
            .find(|row| row.eligibility.selectable)
            .map(|row| row.semantic_id.clone())
        else {
            return Ok(None);
        };
        self.select_main_menu_choice_by_semantic_id(&semantic_id, submit, cx)
            .map(Some)
    }

    fn apply_main_menu_selection(
        &mut self,
        grouped_index: usize,
        submit: bool,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.select_main_menu_row(grouped_index, MainMenuSelectionOrigin::Agent, cx),
            "main_menu_choice_not_selectable"
        );
        self.hovered_index = None;
        self.last_scrolled_index = None;
        self.reveal_main_list_selection_above_footer("devtools main-menu selection");
        // Deep selections on long unmeasured lists converge over a few frames;
        // keep retrying until the model confirms safe-viewport placement
        // [PF-008].
        self.schedule_main_list_selection_reveal_above_footer("devtools main-menu selection", cx);
        cx.notify();

        if submit {
            self.submit_current_value(cx)?;
        }
        Ok(())
    }

    fn select_prompt_choice_by_value(
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

        let selected = filtered[index].value.clone();
        self.set_arg_selected_index(index);
        cx.notify();

        if submit {
            self.submit_current_value(cx)?;
        }

        Ok(selected)
    }

    fn select_prompt_choice_by_semantic_id(
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

        let selected = filtered[index].value.clone();
        self.set_arg_selected_index(index);
        cx.notify();

        if submit {
            self.submit_current_value(cx)?;
        }

        Ok(selected)
    }

    fn select_first_prompt_choice(
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

        let selected = filtered[0].value.clone();
        self.set_arg_selected_index(0);
        cx.notify();

        if submit {
            self.submit_current_value(cx)?;
        }

        Ok(Some(selected))
    }

    /// Submit the currently selected value.
    fn submit_current_value(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let binding = self.prompt_completion.clone();
        let before = binding
            .as_ref()
            .and_then(|binding| binding.observation().receipt)
            .map(|receipt| receipt.sequence);
        match self.current_view.clone() {
            AppView::ScriptList => {
                self.submit_main_menu_selected_subject(cx)?;
                return Ok(());
            }
            AppView::ArgPrompt { id, .. }
            | AppView::MiniPrompt { id, .. }
            | AppView::MicroPrompt { id, .. } => {
                self.submit_arg_prompt_from_current_state(&id, cx);
            }
            AppView::FormPrompt { id, entity } => {
                let value = entity
                    .update(cx, |form, cx| form.validated_submit_value(cx))
                    .ok_or_else(|| anyhow::anyhow!("invalid_form_submission"))?;
                self.submit_prompt_response(id, Some(value), cx);
            }
            AppView::EditorPrompt { entity, .. } | AppView::ScratchPadView { entity, .. } => {
                entity.update(cx, |editor, cx| editor.submit(cx))
            }
            AppView::EnvPrompt { entity, .. } => entity.update(cx, |prompt, cx| prompt.submit(cx)),
            AppView::DropPrompt { entity, .. } => entity.update(cx, |prompt, _cx| prompt.submit()),
            AppView::TemplatePrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.submit(cx))
            }
            AppView::PathPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.handle_enter(cx))
            }
            AppView::SelectPrompt { entity, .. } => {
                anyhow::ensure!(
                    entity.update(cx, |prompt, cx| prompt.submit(cx)),
                    "invalid_select_submission"
                );
            }
            AppView::NamingPrompt { entity, .. } => {
                entity.update(cx, |prompt, cx| prompt.submit(cx))
            }
            AppView::ChatPrompt { entity, .. } => entity.update(cx, |prompt, cx| prompt.submit(cx)),
            AppView::DivPrompt { entity, .. } => entity.update(cx, |prompt, _cx| prompt.submit()),
            AppView::TermPrompt { entity, .. } | AppView::QuickTerminalView { entity } => {
                entity.update(cx, |prompt, cx| {
                    if crate::runtime_policy::is_owned_evaluation() {
                        prompt.terminal.finish_fixture(0)?;
                    } else {
                        prompt.send_raw_input("\r")?;
                    }
                    cx.notify();
                    anyhow::Ok(())
                })?;
                return Ok(()); // The real terminal exit timer delivers completion.
            }
            _ => anyhow::bail!("submit_not_supported_for_current_view"),
        }
        self.mark_main_data_changed();
        cx.notify();
        let completion = binding
            .ok_or_else(|| anyhow::anyhow!("prompt_completion_missing"))?
            .observation();
        if let Some(error) = completion.error {
            return Err(error.into());
        }
        anyhow::ensure!(
            completion.receipt.as_ref().map(|receipt| receipt.sequence) != before,
            "prompt_submission_not_delivered"
        );
        Ok(())
    }
}

/// Get the wire name for a batch command.
fn batch_command_name(cmd: &protocol::BatchCommand) -> String {
    match cmd {
        protocol::BatchCommand::SetInput { .. } => "setInput".to_string(),
        protocol::BatchCommand::OpenActions => "openActions".to_string(),
        protocol::BatchCommand::TogglePreview => "togglePreview".to_string(),
        protocol::BatchCommand::ForceSubmit { .. } => "forceSubmit".to_string(),
        protocol::BatchCommand::WaitFor { .. } => "waitFor".to_string(),
        protocol::BatchCommand::SelectByValue { .. } => "selectByValue".to_string(),
        protocol::BatchCommand::SelectBySemanticId { .. } => "selectBySemanticId".to_string(),
        protocol::BatchCommand::SetThemeControl { .. } => "setThemeControl".to_string(),
        protocol::BatchCommand::UndoStyleChange => "undoStyleChange".to_string(),
        protocol::BatchCommand::RedoStyleChange => "redoStyleChange".to_string(),
        protocol::BatchCommand::ResetStyleControls => "resetStyleControls".to_string(),
        protocol::BatchCommand::SaveCurrentStyleSettings => "saveCurrentStyleSettings".to_string(),
        protocol::BatchCommand::FilterAndSelect { .. } => "filterAndSelect".to_string(),
        protocol::BatchCommand::TypeAndSubmit { .. } => "typeAndSubmit".to_string(),
    }
}

fn menu_syntax_object_refs_by_range_for_filter(
    text: &str,
    scripts: &[std::sync::Arc<crate::scripts::Script>],
) -> std::collections::HashMap<(usize, usize), crate::menu_syntax::CaptureObjectRef> {
    let capture_targets = crate::menu_syntax::registered_capture_targets_from_scripts(scripts);
    let invocation = match crate::menu_syntax::parse_with_capture_targets(text, &capture_targets) {
        crate::menu_syntax::MenuSyntaxParse::Capture(invocation) => invocation,
        _ => return std::collections::HashMap::new(),
    };
    crate::menu_syntax::object_refs_for_raw_capture(&invocation.target, &invocation.raw)
        .into_iter()
        .filter(|object_ref| object_ref.resolved)
        .filter_map(|object_ref| object_ref.range.map(|range| (range, object_ref)))
        .collect()
}

#[cfg(test)]
include!("tests.rs");
