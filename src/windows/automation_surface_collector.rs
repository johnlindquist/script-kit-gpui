//! Secondary-surface semantic element collectors.
//!
//! Provides [`collect_surface_snapshot`] which returns semantic elements for
//! non-main automation windows (Notes, AgentChatDetached, ActionsDialog, PromptPopup).
//!
//! Used by both `getElements` and `inspectAutomationWindow` so agents see one
//! consistent semantic model regardless of which protocol command they use.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::protocol;
use crate::protocol::{
    AutomationWindowInfo, AutomationWindowKind, ElementInfo, ElementStyleInfo, ElementType,
};

fn paint_measurement_component_type(stable_id: &str) -> protocol::LayoutComponentType {
    use protocol::LayoutComponentType;

    if stable_id.starts_with("list-row:")
        || stable_id.contains("transcript-row-")
        || stable_id.starts_with("dictation-history-row-")
        || stable_id.starts_with("chat-transcript-user-")
        || stable_id.starts_with("chat-transcript-response-")
        || stable_id.starts_with("actions-row:")
        || stable_id.starts_with("agent_chat-history-popup-row-")
        || stable_id.starts_with("dictation-microphone-popup-row-")
    {
        return LayoutComponentType::ListItem;
    }

    if stable_id == crate::components::builtin_leading_separator::BUILTIN_LEADING_SEPARATOR_ID {
        return LayoutComponentType::Header;
    }

    match stable_id {
        "main-view-context-cwd-button"
        | "main-view-context-model-button"
        | "agent-chat-send-button"
        | "hud:primary-action"
        | "confirm-ok-button"
        | "confirm-secondary-button"
        | "confirm-cancel-button" => LayoutComponentType::Button,
        "main-view-input-shell"
        | "main-view-input-body"
        | "focused-text-mini-input-row"
        | "focused-text-mini-scope-row"
        | "actions-search"
        | "notes-editor-input-viewport" => LayoutComponentType::Input,
        "agent-chat-transcript-viewport"
        | "chat-transcript-viewport"
        | "actions-list-viewport"
        | "agent_chat-history-popup-list"
        | "dictation-microphone-popup-list" => LayoutComponentType::List,
        "native-main-window-footer-spacer" | "hud-pill" | "snap-overlay-root" => {
            LayoutComponentType::Panel
        }
        "main-view-header"
        | "notes-titlebar"
        | "confirm-modal-header"
        | "actions-context-header"
        | "agent_chat-history-popup-header"
        | "dictation-header" => LayoutComponentType::Header,
        "main-view-shell"
        | "main-view-context-zone"
        | "main-view-main"
        | "notes-window-root"
        | "actions-window-root"
        | "actions-dialog-root"
        | "confirm-popup-root"
        | "confirm-modal-stack"
        | "confirm-modal-action-row"
        | "dictation-overlay"
        | "dictation-content" => LayoutComponentType::Container,
        _ => LayoutComponentType::Other,
    }
}

#[cfg(test)]
mod paint_measurement_component_type_tests {
    #[test]
    fn dictation_history_row_selectors_are_list_items() {
        assert_eq!(
            super::paint_measurement_component_type("dictation-history-row-entry-123"),
            crate::protocol::LayoutComponentType::ListItem
        );
    }

    #[gpui::test]
    fn standalone_snap_layout_uses_actual_paint_and_rejects_replaced_generation(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::window_control::snap_session::{SnapOverlayModel, SnapOverlayTarget};
        use gpui::{point, px, size, AppContext as _};
        let handle = cx.update(|cx| {
            cx.open_window(
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                        point(px(0.), px(0.)),
                        size(px(400.), px(240.)),
                    ))),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                |_, cx| cx.new(|_| crate::window_control::SnapOverlayView::new()),
            )
            .unwrap()
        });
        cx.update(|cx| {
            handle
                .update(cx, |view, _window, cx| {
                    let display = crate::window_control::Bounds {
                        x: 0,
                        y: 0,
                        width: 400,
                        height: 240,
                    };
                    view.set_model(
                        Some(SnapOverlayModel {
                            display_bounds: display,
                            mode: crate::window_control::SnapMode::Simple,
                            is_dominant: true,
                            targets: vec![SnapOverlayTarget {
                                tile: crate::window_control::TilePosition::LeftHalf,
                                bounds: crate::window_control::Bounds {
                                    width: 200,
                                    ..display
                                },
                                active: true,
                            }],
                        }),
                        cx,
                    );
                })
                .unwrap()
        });
        cx.update(|cx| {
            gpui::AnyWindowHandle::from(handle)
                .update(cx, |_, window, cx| window.draw(cx).clear())
                .unwrap()
        });
        let info = cx.update(|cx| {
            crate::windows::register_runtime_window_instance(
                super::AutomationWindowInfo {
                    id: "test:standalone-snap-layout".into(),
                    kind: super::AutomationWindowKind::SnapOverlay,
                    title: Some("Snap".into()),
                    focused: false,
                    visible: false,
                    semantic_surface: Some("snapOverlay".into()),
                    bounds: None,
                    parent_window_id: None,
                    parent_window_generation: None,
                    parent_kind: None,
                    pid: Some(std::process::id()),
                    generation: None,
                },
                handle.into(),
                cx,
            )
            .unwrap()
        });
        let layout = cx
            .update(|cx| super::collect_registered_surface_layout(&info, cx))
            .unwrap();
        let target = layout
            .components
            .iter()
            .find(|component| component.name == "snap:target:LeftHalf")
            .expect("real target paint");
        assert_eq!(target.bounds.width, 200.0);
        assert_eq!(target.bounds.height, 240.0);
        assert!(target
            .measurement_frame_generation
            .is_some_and(|frame| frame > 0));
        let mut stale = info.clone();
        stale.generation = info.generation.map(|generation| generation + 1);
        assert!(cx
            .update(|cx| super::collect_registered_surface_layout(&stale, cx))
            .is_err());
        cx.update(|cx| {
            handle
                .update(cx, |_, window, _| {
                    window.refresh();
                    assert!(!window.rendered_frame_is_current());
                })
                .unwrap();
            let error = super::collect_registered_surface_layout(&info, cx).unwrap_err();
            assert!(error
                .to_string()
                .contains("surface_layout_unavailable:unpainted_or_stale"));
        });
        crate::windows::remove_runtime_window_instance(&info.id, info.generation.unwrap());
        cx.update(|cx| {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .unwrap()
        });
    }
}
/// Machine-readable indicator of the semantic element quality level.
///
/// Mirrors [`crate::protocol::SemanticQuality`] at the collector layer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SnapshotQuality {
    /// Full semantic elements collected.
    #[default]
    Full,
    /// Only a panel-level element was collected (entity unavailable).
    PanelOnly,
}

impl SnapshotQuality {
    pub const fn projection_quality(self) -> crate::protocol::ProjectionQuality {
        match self {
            Self::Full => crate::protocol::ProjectionQuality::Complete,
            Self::PanelOnly => crate::protocol::ProjectionQuality::Partial,
        }
    }

    pub fn reason_codes(self) -> Vec<crate::protocol::ProjectionReason> {
        match self {
            Self::Full => Vec::new(),
            Self::PanelOnly => vec![crate::protocol::ProjectionReason::PanelOnly],
        }
    }
}

/// Lightweight snapshot of semantic elements from a non-main surface.
#[derive(Clone, Debug, Default)]
pub struct SurfaceElementSnapshot {
    pub elements: Vec<ElementInfo>,
    pub total_count: usize,
    pub focused_semantic_id: Option<String>,
    pub selected_semantic_id: Option<String>,
    pub warnings: Vec<String>,
    /// Semantic quality level of this snapshot.
    pub quality: SnapshotQuality,
}

#[derive(Clone, Debug, Default)]
struct PromptPopupElementSnapshot {
    generation: Option<u64>,
    elements: Vec<ElementInfo>,
    focused_semantic_id: Option<String>,
    selected_semantic_id: Option<String>,
}

fn prompt_popup_semantic_cache() -> &'static Mutex<HashMap<String, PromptPopupElementSnapshot>> {
    static CACHE: OnceLock<Mutex<HashMap<String, PromptPopupElementSnapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn actions_dialog_semantic_cache() -> &'static Mutex<HashMap<String, PromptPopupElementSnapshot>> {
    static CACHE: OnceLock<Mutex<HashMap<String, PromptPopupElementSnapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[allow(dead_code)] // Binary app_impl uses this; lib-only builds do not.
pub(crate) fn upsert_actions_dialog_snapshot(
    window_id: &str,
    dialog_entity: &gpui::Entity<crate::actions::ActionsDialog>,
    cx: &gpui::App,
) {
    let Some(generation) =
        crate::windows::automation_window_by_id(window_id).and_then(|info| info.generation)
    else {
        return;
    };
    let snapshot = collect_actions_dialog_elements(dialog_entity, 1000, cx);
    if let Ok(mut cache) = actions_dialog_semantic_cache().lock() {
        cache.insert(
            window_id.to_string(),
            PromptPopupElementSnapshot {
                generation: Some(generation),
                elements: snapshot.elements,
                focused_semantic_id: snapshot.focused_semantic_id,
                selected_semantic_id: snapshot.selected_semantic_id,
            },
        );
    }
}

#[allow(dead_code)] // Binary app_impl uses this; lib-only builds do not.
pub(crate) fn remove_actions_dialog_snapshot(window_id: &str) {
    if let Ok(mut cache) = actions_dialog_semantic_cache().lock() {
        cache.remove(window_id);
    }
}

#[allow(dead_code)] // Binary dictation overlay uses this; lib-only builds do not.
pub(crate) fn upsert_dictation_microphone_prompt_popup_snapshot(
    window_id: &str,
    generation: u64,
    snapshot: &crate::dictation::DictationMicrophonePopupSnapshot,
) {
    let mut elements = Vec::new();
    elements.push(element(
        "panel:dictation-microphone-popup",
        ElementType::Panel,
        Some("Dictation Microphones".to_string()),
        None,
        None,
        None,
        None,
    ));
    elements.push(element(
        "list:dictation-microphones",
        ElementType::List,
        Some(format!("{} rows", snapshot.rows.len())),
        None,
        None,
        None,
        None,
    ));

    let mut selected_semantic_id = None;
    for (idx, row) in snapshot.rows.iter().enumerate() {
        let is_selected = snapshot.selected_row_id.as_deref() == Some(row.row_id.as_str());
        if is_selected {
            selected_semantic_id = Some(row.semantic_id.clone());
        }

        let mut info = element(
            &row.semantic_id,
            ElementType::Choice,
            Some(row.title.clone()),
            Some(row.row_id.clone()),
            Some(is_selected),
            None,
            Some(idx),
        );
        info.role = Some(row.subtitle.clone());
        info.kind = Some("DictationMicrophone".to_string());
        info.selectable = Some(true);
        elements.push(info);
    }

    let focused_semantic_id = selected_semantic_id.clone();
    if let Ok(mut cache) = prompt_popup_semantic_cache().lock() {
        cache.insert(
            window_id.to_string(),
            PromptPopupElementSnapshot {
                generation: Some(generation),
                elements,
                focused_semantic_id,
                selected_semantic_id,
            },
        );
    }
}

#[allow(dead_code)] // Binary dictation overlay uses this; lib-only builds do not.
pub(crate) fn remove_dictation_microphone_prompt_popup_snapshot_if_generation(
    window_id: &str,
    generation: u64,
) -> bool {
    if let Ok(mut cache) = prompt_popup_semantic_cache().lock() {
        if cache
            .get(window_id)
            .is_some_and(|snapshot| snapshot.generation == Some(generation))
        {
            cache.remove(window_id);
            return true;
        }
    }
    false
}

impl SurfaceElementSnapshot {
    /// Returns semantic fallback warnings relevant to popup capture receipts.
    ///
    /// These are the `panel_only_*` warnings that indicate the surface could
    /// not be fully introspected and only a panel-level element was collected.
    /// Agents use these to know when semantic receipts are degraded for a
    /// popup surface.
    pub fn popup_semantic_warnings(&self) -> Vec<String> {
        self.warnings
            .iter()
            .filter(|w| w.starts_with("panel_only_"))
            .cloned()
            .collect()
    }
}

fn element(
    semantic_id: &str,
    element_type: ElementType,
    text: Option<String>,
    value: Option<String>,
    selected: Option<bool>,
    focused: Option<bool>,
    index: Option<usize>,
) -> ElementInfo {
    ElementInfo {
        semantic_id: semantic_id.to_string(),
        element_type,
        text,
        value,
        content: None,
        selected,
        focused,
        index,
        role: None,
        kind: None,
        source: None,
        source_name: None,
        selectable: None,
        status_kind: None,
        action_disabled: None,
        style: None,
    }
}

fn conversation_semantic_role(
    role: crate::components::main_view_chrome::SemanticChipRole,
) -> crate::protocol::ConversationSemanticRole {
    use crate::components::main_view_chrome::SemanticChipRole;
    use crate::protocol::ConversationSemanticRole;

    match role {
        SemanticChipRole::ContextAttachment => ConversationSemanticRole::ContextChip,
        SemanticChipRole::Identity => ConversationSemanticRole::IdentityBadge,
        SemanticChipRole::DestinationSelector => ConversationSemanticRole::DestinationSelector,
    }
}

fn conversation_semantic_action(
    role: crate::components::main_view_chrome::SemanticChipRole,
    action: crate::components::main_view_chrome::SemanticChipAction,
) -> crate::protocol::ConversationSemanticAction {
    use crate::components::main_view_chrome::{SemanticChipAction, SemanticChipRole};
    use crate::protocol::ConversationSemanticAction;

    match (role, action) {
        (SemanticChipRole::ContextAttachment, SemanticChipAction::OpenDetails) => {
            ConversationSemanticAction::OpenContextDetails
        }
        (SemanticChipRole::ContextAttachment, SemanticChipAction::RemoveContext) => {
            ConversationSemanticAction::RemoveContext
        }
        (SemanticChipRole::Identity, SemanticChipAction::OpenSelector) => {
            ConversationSemanticAction::OpenIdentitySelector
        }
        (
            SemanticChipRole::Identity,
            SemanticChipAction::OpenDetails | SemanticChipAction::OpenSurface,
        ) => ConversationSemanticAction::OpenIdentityDetails,
        (SemanticChipRole::DestinationSelector, SemanticChipAction::SelectDestination) => {
            ConversationSemanticAction::SelectDestination
        }
        _ => unreachable!("SemanticChipSpec rejects role/action mismatches"),
    }
}

fn semantic_chip_action_element(
    chip: &crate::components::main_view_chrome::SemanticChipSpec,
    action: Option<crate::components::main_view_chrome::SemanticChipAction>,
    trailing: bool,
) -> ElementInfo {
    let action = action.map(|action| conversation_semantic_action(chip.role, action));
    let semantic_id = if trailing {
        action.map_or_else(
            || format!("{}:unavailable", chip.semantic_id),
            |action| format!("{}:{}", chip.semantic_id, action.as_str()),
        )
    } else {
        chip.semantic_id.to_string()
    };

    ElementInfo {
        semantic_id,
        element_type: if chip.enabled && action.is_some() {
            ElementType::Button
        } else {
            ElementType::Panel
        },
        text: Some(if trailing {
            format!("Remove {}", chip.label)
        } else {
            chip.label.to_string()
        }),
        value: (!trailing && !chip.shortcut_tokens.is_empty())
            .then(|| chip.shortcut_tokens.join("")),
        content: None,
        selected: None,
        focused: None,
        index: None,
        role: Some(conversation_semantic_role(chip.role).as_str().to_string()),
        kind: action.map(|action| action.as_str().to_string()),
        source: Some("ConversationSemanticChip".to_string()),
        source_name: None,
        selectable: Some(chip.enabled && action.is_some()),
        status_kind: None,
        action_disabled: chip.disabled_reason.as_ref().map(ToString::to_string),
        style: None,
    }
}

/// Project the same typed semantic chip specification used by renderers into
/// the element protocol. Every main and secondary conversational collector
/// goes through this function, so role/action strings cannot drift by host.
pub(crate) fn collect_semantic_chip_element(
    chip: &crate::components::main_view_chrome::SemanticChipSpec,
) -> ElementInfo {
    semantic_chip_action_element(chip, chip.body_action, false)
}

/// Project every executable region of a chip. Context attachments may expose
/// both body details and a separately-addressable trailing remove action.
pub(crate) fn collect_semantic_chip_elements(
    chip: &crate::components::main_view_chrome::SemanticChipSpec,
) -> Vec<ElementInfo> {
    let mut elements = vec![collect_semantic_chip_element(chip)];
    if let Some(action) = chip.trailing_action {
        elements.push(semantic_chip_action_element(chip, Some(action), true));
    }
    elements
}

pub(crate) fn collect_conversation_command_elements<Handler>(
    commands: &[crate::components::conversation_actions::BoundConversationCommand<Handler>],
) -> Vec<ElementInfo> {
    commands
        .iter()
        .map(|command| {
            let action = command.descriptor.command_action();
            ElementInfo {
                semantic_id: action.id.clone(),
                element_type: ElementType::Button,
                text: Some(action.title),
                value: action.shortcut,
                content: None,
                selected: Some(false),
                focused: Some(false),
                index: None,
                role: Some("conversationCommand".to_string()),
                kind: Some(action.id),
                source: Some("ConversationCommandDescriptor".to_string()),
                source_name: None,
                selectable: Some(action.availability.is_executable()),
                status_kind: action
                    .requires_confirmation
                    .then(|| "confirmationRequired".to_string()),
                // Preserve the host's exact reviewed copy; the shared action
                // owns availability while the typed host reason owns wording.
                action_disabled: command
                    .descriptor
                    .availability
                    .disabled_reason()
                    .map(str::to_string),
                style: None,
            }
        })
        .collect()
}

fn cancellation_semantic_kind(
    kind: sk_protocol::ai_reliability::CancellationKind,
) -> (&'static str, &'static str) {
    use sk_protocol::ai_reliability::CancellationKind;
    match kind {
        CancellationKind::UserStopped => ("userStopped", "Stopped"),
        CancellationKind::UserCancelled => ("userCancelled", "Cancelled"),
        CancellationKind::AppShutdown => ("appShutdown", "Stopped when the app closed"),
    }
}

fn project_agent_chat_conversation_elements(
    assistant_messages: &[(u64, &str)],
    cancellation: Option<sk_protocol::ai_reliability::CancellationKind>,
    command_status: Option<&str>,
) -> Vec<ElementInfo> {
    let mut elements = Vec::new();
    if let Some(status) = command_status {
        elements.push(ElementInfo {
            semantic_id: "conversation.commandStatus".to_string(),
            element_type: ElementType::Panel,
            text: Some(status.to_string()),
            value: None,
            content: None,
            selected: Some(false),
            focused: Some(false),
            index: None,
            role: Some("conversationCommandStatus".to_string()),
            kind: Some("disabledAcknowledgement".to_string()),
            source: Some("ConversationCommandExecution".to_string()),
            source_name: None,
            selectable: Some(false),
            status_kind: Some("disabled".to_string()),
            action_disabled: Some(status.to_string()),
            style: None,
        });
    }

    for (message_id, _) in assistant_messages
        .iter()
        .filter(|(_, body)| !body.trim().is_empty())
    {
        elements.push(ElementInfo {
            semantic_id: format!("conversation.copyTurn:{message_id}"),
            element_type: ElementType::Button,
            text: Some("Copy Response".to_string()),
            value: None,
            content: None,
            selected: Some(false),
            focused: Some(false),
            index: None,
            role: Some("conversationCommand".to_string()),
            kind: Some("copyTurn".to_string()),
            source: Some("AgentChatThreadMessage".to_string()),
            source_name: Some(message_id.to_string()),
            selectable: Some(true),
            status_kind: None,
            action_disabled: None,
            style: None,
        });
    }

    if let Some(cancellation) = cancellation {
        let (kind, text) = cancellation_semantic_kind(cancellation);
        let turn_id = assistant_messages
            .last()
            .map(|(message_id, _)| message_id.to_string())
            .unwrap_or_else(|| "latest".to_string());
        elements.push(ElementInfo {
            semantic_id: format!("conversation.turnStatus:{turn_id}"),
            element_type: ElementType::Panel,
            text: Some(text.to_string()),
            value: None,
            content: None,
            selected: Some(false),
            focused: Some(false),
            index: None,
            role: Some("conversationTurnStatus".to_string()),
            kind: Some(kind.to_string()),
            source: Some("AiOperationState".to_string()),
            source_name: None,
            selectable: Some(false),
            status_kind: Some("cancelled".to_string()),
            action_disabled: None,
            style: None,
        });
    }

    elements
}

fn collect_agent_chat_thread_conversation_elements(
    thread: &crate::ai::agent_chat::ui::thread::AgentChatThread,
    command_status: Option<&str>,
) -> Vec<ElementInfo> {
    use crate::ai::agent_chat::ui::thread::AgentChatThreadMessageRole;
    use sk_protocol::ai_reliability::AiPhase;

    let assistant_messages: Vec<_> = thread
        .messages
        .iter()
        .filter(|message| matches!(message.role, AgentChatThreadMessageRole::Assistant))
        .map(|message| (message.id, message.body.as_ref()))
        .collect();
    let cancellation = match &thread.reliability_state().phase {
        AiPhase::Cancelled { kind, .. } => Some(*kind),
        _ => None,
    };
    project_agent_chat_conversation_elements(&assistant_messages, cancellation, command_status)
}

pub(crate) fn collect_agent_chat_conversation_elements(
    entity: &gpui::Entity<crate::ai::agent_chat::ui::view::AgentChatView>,
    cx: &gpui::App,
) -> Vec<ElementInfo> {
    let view = entity.read(cx);
    let command_status = view.command_status_text();
    match &view.session {
        crate::ai::agent_chat::ui::AgentChatSession::Live(thread) if !view.shows_setup_card(cx) => {
            let mut elements =
                collect_agent_chat_thread_conversation_elements(thread.read(cx), command_status);
            elements.extend(view.spine_hint_semantic_elements());
            elements
        }
        _ => {
            let mut elements = view.setup_semantic_elements(cx);
            if let Some(status) = command_status {
                let mut element = ElementInfo::panel("conversation.commandStatus");
                element.text = Some(status.to_string());
                element.role = Some("conversationCommandStatus".to_string());
                element.kind = Some("disabledAcknowledgement".to_string());
                element.source = Some("ConversationCommandExecution".to_string());
                element.selectable = Some(false);
                element.status_kind = Some("disabled".to_string());
                element.action_disabled = Some(status.to_string());
                elements.push(element);
            }
            elements
        }
    }
}

/// Resolve only the requested runtime lifetime, never a singleton replacement.
pub(crate) fn current_surface_metadata(
    resolved: &AutomationWindowInfo,
) -> Option<AutomationWindowInfo> {
    let generation = resolved.generation?;
    let current = crate::windows::automation_window_by_id(&resolved.id)?;
    if generation == 0 || current.generation != Some(generation) || current.kind != resolved.kind {
        return None;
    }
    crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)?;
    if let Some(parent_id) = &current.parent_window_id {
        crate::windows::get_runtime_window_handle_for_generation(
            parent_id,
            current.parent_window_generation?,
        )?;
    }
    Some(current)
}

/// Read an exact root without re-entering a window already borrowed by the caller.
pub(crate) fn read_window_root<T: gpui::Render + 'static, R>(
    handle: gpui::WindowHandle<T>,
    window: Option<&gpui::Window>,
    cx: &gpui::App,
    read: impl FnOnce(&T, &gpui::App) -> R,
) -> anyhow::Result<R> {
    if let Some(window) = window {
        anyhow::ensure!(
            window.window_handle() == handle.into(),
            "window_root_owner_mismatch"
        );
        let root = window
            .root::<T>()
            .flatten()
            .ok_or_else(|| anyhow::anyhow!("window_root_type_mismatch"))?;
        Ok(read(root.read(cx), cx))
    } else {
        handle.read_with(cx, read)
    }
}

fn exact_root<T: 'static>(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<gpui::Entity<T>> {
    let current = current_surface_metadata(resolved)?;
    crate::windows::get_runtime_window_handle_for_generation(&current.id, current.generation?)?
        .read(cx, |entity: gpui::Entity<T>, _| entity)
        .ok()
}

pub(crate) fn exact_actions_dialog_entity(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<gpui::Entity<crate::actions::ActionsDialog>> {
    let root = exact_root::<crate::actions::ActionsWindow>(resolved, cx)?;
    Some(root.read(cx).dialog.clone())
}

/// Append the same frame-coherent measured nodes used by Main, without Main ownership.
pub fn append_window_paint_measurements(layout: &mut protocol::LayoutInfo, window: &gpui::Window) {
    let mut measurements: Vec<_> = window.debug_bounds_entries().iter().collect();
    measurements.sort_by(|left, right| left.selector.cmp(&right.selector));
    let frame = window.rendered_frame_generation();
    for component in &mut layout.components {
        if component.measurement_frame_generation.is_none() {
            component.measurement_frame_generation = Some(frame);
        }
    }
    for measurement in measurements {
        let bounds = measurement.bounds;
        let visible = measurement.visible_bounds;
        let clip = measurement.clip_bounds;
        layout.components.push(
            protocol::LayoutComponentInfo::new(
                measurement.selector.clone(),
                paint_measurement_component_type(&measurement.selector),
            )
            .with_bounds(
                bounds.origin.x.as_f32(),
                bounds.origin.y.as_f32(),
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32(),
            )
            .with_measurement("paint-time", "window")
            .with_paint_visibility(
                visible.origin.x.as_f32(),
                visible.origin.y.as_f32(),
                visible.size.width.as_f32(),
                visible.size.height.as_f32(),
                clip.origin.x.as_f32(),
                clip.origin.y.as_f32(),
                clip.size.width.as_f32(),
                clip.size.height.as_f32(),
            )
            .with_measurement_frame(frame),
        );
    }
}

/// Project an exact production root and its completed paint without constructing a launcher.
pub fn collect_registered_surface_layout(
    resolved: &AutomationWindowInfo,
    cx: &mut gpui::App,
) -> anyhow::Result<protocol::LayoutInfo> {
    collect_registered_surface_layout_inner(resolved, None, cx)
}

/// Atomic evaluator-only layout read, qualified by the published render store
/// rather than by a caller's assertion that a dirty window is safe to inspect.
#[cfg(all(target_os = "macos", any(test, feature = "owned-ui-evaluation")))]
pub fn collect_registered_surface_layout_for_completed_frame(
    resolved: &AutomationWindowInfo,
    completed: &protocol::CompletedFrameIdentity,
    cx: &mut gpui::App,
) -> anyhow::Result<protocol::LayoutInfo> {
    anyhow::ensure!(
        resolved.id == completed.target.window_id
            && resolved.generation == completed.target.window_generation,
        "surface_layout_completed_target_mismatch"
    );
    crate::computer_use::owned_render_capture::with_owned_completed_frame(
        completed,
        cx,
        |published, cx| collect_registered_surface_layout_inner(resolved, Some(published), cx),
    )
}

fn layout_frame_matches(
    window: &gpui::Window,
    completed: Option<&protocol::CompletedFrameIdentity>,
) -> bool {
    match completed {
        Some(published) => {
            Some(window.rendered_frame_generation()) == published.target.frame_generation
        }
        None => window.rendered_frame_is_current(),
    }
}

fn collect_registered_surface_layout_inner(
    resolved: &AutomationWindowInfo,
    completed: Option<&protocol::CompletedFrameIdentity>,
    cx: &mut gpui::App,
) -> anyhow::Result<protocol::LayoutInfo> {
    let mut target = current_surface_metadata(resolved)
        .ok_or_else(|| anyhow::anyhow!("surface_layout_target_stale"))?;
    anyhow::ensure!(
        target.kind != AutomationWindowKind::Main,
        "surface_layout_main_requires_main_owner"
    );
    let generation = target
        .generation
        .ok_or_else(|| anyhow::anyhow!("surface_layout_generation_missing"))?;
    let host_policy = crate::windows::runtime_window_host_policy(&target.id, generation)?;
    host_policy.validate()?;
    let handle = crate::windows::get_runtime_window_handle_for_generation(&target.id, generation)
        .ok_or_else(|| anyhow::anyhow!("surface_layout_target_stale"))?;
    let (bounds, viewport, frame) = handle.update(cx, |_, window, _| {
        anyhow::ensure!(
            window.is_owned_hidden() == host_policy.is_hidden(),
            "surface_layout_host_mismatch"
        );
        anyhow::ensure!(
            layout_frame_matches(window, completed),
            "surface_layout_unavailable:unpainted_or_stale"
        );
        if host_policy.is_hidden() {
            anyhow::ensure!(
                !target.visible && !target.focused,
                "surface_layout_hidden_metadata_mismatch"
            );
        }
        Ok((
            window.bounds(),
            window.viewport_size(),
            window.rendered_frame_generation(),
        ))
    })??;
    target.bounds = Some(protocol::AutomationWindowBounds {
        x: bounds.origin.x.as_f32() as f64,
        y: bounds.origin.y.as_f32() as f64,
        width: viewport.width.as_f32() as f64,
        height: viewport.height.as_f32() as f64,
    });
    let mut required = Vec::<String>::new();
    let mut layout = match target.kind {
        AutomationWindowKind::Notes => {
            let (entity, _) =
                crate::notes::get_notes_app_entity_and_handle_for_generation(generation, cx)
                    .ok_or_else(|| anyhow::anyhow!("notes_layout_owner_missing"))?;
            required.extend(["notes-window-root".into(), "notes-titlebar".into()]);
            entity.read(cx).automation_layout_info(&target)
        }
        AutomationWindowKind::AgentChatDetached => {
            let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                &target.id, generation,
            )
            .ok_or_else(|| anyhow::anyhow!("chat_layout_owner_missing"))?;
            entity.read(cx).automation_layout_info(&target, cx)
        }
        AutomationWindowKind::ActionsDialog => {
            let entity = exact_actions_dialog_entity(&target, cx)
                .ok_or_else(|| anyhow::anyhow!("actions_layout_owner_missing"))?;
            let dialog = entity.read(cx);
            required.extend([
                "actions-window-root".into(),
                "actions-dialog-root".into(),
                "actions-list-viewport".into(),
            ]);
            if dialog.search_is_visible() {
                required.push("actions-search".into());
            }
            dialog.automation_layout_info(&target, cx)
        }
        AutomationWindowKind::Dictation => {
            anyhow::ensure!(
                crate::dictation::get_dictation_overlay_state_for_instance(generation, cx)
                    .is_some(),
                "dictation_layout_owner_missing"
            );
            required.extend(["dictation-overlay".into(), "dictation-content".into()]);
            crate::dictation::automation_layout_info(&target)
        }
        AutomationWindowKind::Hud => {
            let entity = exact_root::<crate::hud_manager::HudView>(&target, cx)
                .ok_or_else(|| anyhow::anyhow!("hud_layout_owner_missing"))?;
            required.extend(["hud-pill".into(), "hud-pill-label-ellipsis".into()]);
            if entity.read(cx).semantic_state().1.is_some() {
                required.push("hud:primary-action".into());
            }
            protocol::LayoutInfo {
                prompt_type: "hud".into(),
                ..Default::default()
            }
        }
        AutomationWindowKind::SnapOverlay => {
            let entity = exact_root::<crate::window_control::SnapOverlayView>(&target, cx)
                .ok_or_else(|| anyhow::anyhow!("snap_layout_owner_missing"))?;
            required.push("snap-overlay-root".into());
            if let Some(model) = entity.read(cx).model() {
                required.extend(
                    model
                        .targets
                        .iter()
                        .filter(|target| target.active)
                        .map(|target| format!("snap:target:{:?}", target.tile)),
                );
            }
            protocol::LayoutInfo {
                prompt_type: "snapOverlay".into(),
                ..Default::default()
            }
        }
        AutomationWindowKind::PromptPopup => {
            match target.id.as_str() {
                "confirm-popup" => {
                    let snapshot = crate::confirm::get_confirm_popup_snapshot(cx, generation, None).ok_or_else(|| anyhow::anyhow!("confirm_layout_owner_missing"))?;
                    required.extend(["confirm-popup-root", "confirm-modal-header", "confirm-modal-stack", "confirm-modal-action-row", "confirm-ok-button", "confirm-cancel-button"].into_iter().map(str::to_string));
                    if snapshot.secondary_text.is_some() { required.push("confirm-secondary-button".into()); }
                    if !snapshot.body.trim().is_empty() { required.push("confirm-modal-body".into()); }
                }
                crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID => {
                    anyhow::ensure!(crate::ai::agent_chat::ui::history_popup::get_history_popup_snapshot_for_generation(generation, cx).is_some(), "history_layout_owner_missing");
                    required.extend(["agent_chat-history-popup", "agent_chat-history-popup-header", "agent_chat-history-popup-list"].into_iter().map(str::to_string));
                }
                crate::dictation::DICTATION_MICROPHONE_POPUP_AUTOMATION_ID => {
                    anyhow::ensure!(crate::dictation::dictation_microphone_popup_revision_facts(generation, None, cx).is_some(), "microphone_layout_owner_missing");
                    required.extend(["dictation-microphone-popup".into(), "dictation-microphone-popup-list".into()]);
                }
                _ => anyhow::bail!("surface_layout_owner_unknown"),
            }
            protocol::LayoutInfo {
                prompt_type: target
                    .semantic_surface
                    .clone()
                    .unwrap_or_else(|| "promptPopup".into()),
                ..Default::default()
            }
        }
        _ => anyhow::bail!("surface_layout_owner_unknown"),
    };
    layout.window_width = viewport.width.as_f32();
    layout.window_height = viewport.height.as_f32();
    handle.update(cx, |_, window, _| {
        anyhow::ensure!(
            layout_frame_matches(window, completed) && window.rendered_frame_generation() == frame,
            "surface_layout_frame_changed"
        );
        let entries = window.debug_bounds_entries();
        anyhow::ensure!(
            !entries.is_empty(),
            "surface_layout_unavailable:no_painted_selectors"
        );
        for selector in &required {
            let mut matching = entries.iter().filter(|entry| &entry.selector == selector);
            let entry = matching
                .next()
                .ok_or_else(|| anyhow::anyhow!("surface_layout_unavailable:{selector}"))?;
            anyhow::ensure!(
                matching.next().is_none(),
                "surface_layout_ambiguous:{selector}"
            );
            let bounds = entry.bounds;
            anyhow::ensure!(
                [
                    bounds.origin.x.as_f32(),
                    bounds.origin.y.as_f32(),
                    bounds.size.width.as_f32(),
                    bounds.size.height.as_f32()
                ]
                .into_iter()
                .all(f32::is_finite)
                    && bounds.size.width.as_f32() > 0.0
                    && bounds.size.height.as_f32() > 0.0,
                "surface_layout_invalid_bounds:{selector}"
            );
        }
        append_window_paint_measurements(&mut layout, window);
        Ok::<_, anyhow::Error>(())
    })??;
    anyhow::ensure!(
        current_surface_metadata(&target).is_some(),
        "surface_layout_target_changed"
    );
    layout.timestamp = chrono::Utc::now().to_rfc3339();
    Ok(layout)
}

/// Revisions come from the exact production owner's mutations and applied publication.
pub fn surface_revision_facts(
    resolved: &AutomationWindowInfo,
    window: Option<&gpui::Window>,
    cx: &gpui::App,
) -> Option<(u64, u64, u64, u64)> {
    current_surface_metadata(resolved)?;
    let generation = resolved.generation?;
    let handle =
        crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)?;
    if window.is_some_and(|window| window.window_handle() != handle) {
        return None;
    }
    match resolved.kind {
        AutomationWindowKind::Notes => {
            let (entity, handle) =
                crate::notes::get_notes_app_entity_and_handle_for_generation(generation, cx)?;
            let layer_revision =
                read_window_root(handle, window, cx, |root, _| root.layer_revision()).ok()?;
            let view = entity.read(cx);
            Some((
                view.document_revision().checked_add(layer_revision)?,
                view.semantic_revision(cx).checked_add(layer_revision)?,
                view.applied_theme_revision(),
                view.applied_theme_revision(),
            ))
        }
        AutomationWindowKind::AgentChatDetached => {
            let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                &resolved.id,
                generation,
            )?;
            let view = entity.read(cx);
            let theme = view.applied_theme_revision()?;
            let revision = view.semantic_revision(cx);
            Some((revision, revision, theme, theme))
        }
        AutomationWindowKind::Dictation => {
            crate::dictation::dictation_overlay_revision_facts(generation, window, cx)
        }
        AutomationWindowKind::ActionsDialog => read_window_root(
            handle.downcast::<crate::actions::ActionsWindow>()?,
            window,
            cx,
            |root, cx| root.dialog.read(cx).revision_facts(),
        )
        .ok(),
        AutomationWindowKind::Hud => read_window_root(
            handle.downcast::<crate::hud_manager::HudView>()?,
            window,
            cx,
            |view, _| view.revision_facts(),
        )
        .ok(),
        AutomationWindowKind::SnapOverlay => read_window_root(
            handle.downcast::<crate::window_control::SnapOverlayView>()?,
            window,
            cx,
            |view, _| view.revision_facts(),
        )
        .ok(),
        AutomationWindowKind::PromptPopup => {
            if let Some(state) = crate::footer_popup::footer_runtime_state(&resolved.id, generation)
            {
                return Some((
                    state.semantic_revision,
                    state.semantic_revision,
                    state.presentation_revision,
                    state.applied_theme_revision,
                ));
            }
            match resolved.id.as_str() {
                "confirm-popup" => Some(crate::confirm::get_confirm_popup_snapshot(cx, generation, window)?.revisions),
                crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID => crate::ai::agent_chat::ui::history_popup::history_popup_revision_facts(generation, window, cx),
                crate::dictation::DICTATION_MICROPHONE_POPUP_AUTOMATION_ID => crate::dictation::dictation_microphone_popup_revision_facts(generation, window, cx),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Request the real owner's teardown. Callers wait for exact runtime invalidation.
pub fn close_owned_registered_surface(
    resolved: &AutomationWindowInfo,
    cx: &mut gpui::App,
) -> anyhow::Result<()> {
    let resolved = current_surface_metadata(resolved)
        .ok_or_else(|| anyhow::anyhow!("surface_target_stale"))?;
    let generation = resolved
        .generation
        .ok_or_else(|| anyhow::anyhow!("surface_generation_missing"))?;
    anyhow::ensure!(
        crate::windows::runtime_window_host_policy(&resolved.id, generation)?.is_hidden(),
        "owned_surface_required"
    );
    let handle = crate::windows::get_runtime_window_handle_for_generation(&resolved.id, generation)
        .ok_or_else(|| anyhow::anyhow!("surface_target_stale"))?;
    anyhow::ensure!(
        handle.update(cx, |_, window, _| window.is_owned_hidden())?,
        "owned_native_host_required"
    );
    match resolved.kind {
        AutomationWindowKind::Notes => {
            crate::notes::close_owned_notes_window(generation, cx).map_err(anyhow::Error::msg)?
        }
        AutomationWindowKind::AgentChatDetached => {
            anyhow::ensure!(
                crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
                    &resolved.id,
                    generation
                )
                .is_some(),
                "chat_target_stale"
            );
            crate::ai::agent_chat::ui::chat_window::close_chat_window(cx);
        }
        AutomationWindowKind::Dictation => {
            anyhow::ensure!(
                crate::dictation::get_dictation_overlay_state_for_instance(generation, cx)
                    .is_some(),
                "dictation_target_stale"
            );
            crate::dictation::close_dictation_overlay(cx)?;
        }
        AutomationWindowKind::ActionsDialog => {
            anyhow::ensure!(
                exact_actions_dialog_entity(&resolved, cx).is_some(),
                "actions_target_stale"
            );
            crate::actions::close_actions_window(cx);
        }
        AutomationWindowKind::Hud => {
            crate::hud_manager::dismiss_owned_hud_instance(&resolved.id, generation, cx)?
        }
        AutomationWindowKind::SnapOverlay => {
            let handle = handle
                .downcast::<crate::window_control::SnapOverlayView>()
                .ok_or_else(|| anyhow::anyhow!("snap_root_mismatch"))?;
            handle.update(cx, |view, window, _| view.close_owned(window))??;
        }
        AutomationWindowKind::PromptPopup => match resolved.id.as_str() {
            "confirm-popup" => crate::confirm::close_owned_confirm_window(generation, cx)?,
            crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID => {
                anyhow::ensure!(crate::ai::agent_chat::ui::history_popup::get_history_popup_snapshot_for_generation(generation, cx).is_some(), "history_target_stale");
                crate::ai::agent_chat::ui::history_popup::close_history_popup_window_for_owner_loss(
                    cx,
                );
            }
            crate::dictation::DICTATION_MICROPHONE_POPUP_AUTOMATION_ID => {
                crate::dictation::close_dictation_microphone_popup_window_for_owner_loss(cx)
            }
            _ => anyhow::bail!("surface_close_owner_unknown"),
        },
        _ => anyhow::bail!("surface_close_owner_unknown"),
    }
    Ok(())
}

include!("automation_surface_collector_overlays.rs");

/// Collect semantic elements for a resolved non-main automation window.
///
/// Returns `None` for window kinds that do not yet have a collector.
pub fn collect_surface_snapshot(
    resolved: &AutomationWindowInfo,
    limit: usize,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let current = current_surface_metadata(resolved)?;
    let resolved = &current;
    let mut snapshot = match resolved.kind {
        AutomationWindowKind::Notes => collect_notes_snapshot(resolved, cx)?,
        AutomationWindowKind::AgentChatDetached => {
            collect_agent_chat_detached_snapshot(resolved, cx)?
        }
        AutomationWindowKind::ActionsDialog => collect_actions_dialog_snapshot(resolved, cx)?,
        AutomationWindowKind::PromptPopup => collect_exact_prompt_popup_snapshot(resolved, cx)?,
        AutomationWindowKind::Dictation => collect_dictation_snapshot(resolved, cx)?,
        AutomationWindowKind::Hud => collect_hud_snapshot(resolved, cx)?,
        AutomationWindowKind::SnapOverlay => collect_snap_snapshot(resolved, cx)?,
        _ => return None,
    };

    snapshot.total_count = snapshot.elements.len();
    if snapshot.elements.len() > limit {
        snapshot.elements.truncate(limit);
    }

    tracing::info!(
        target: "script_kit::automation",
        window_id = %resolved.id,
        kind = ?resolved.kind,
        element_count = snapshot.elements.len(),
        total_count = snapshot.total_count,
        warning_count = snapshot.warnings.len(),
        focused_semantic_id = ?snapshot.focused_semantic_id,
        selected_semantic_id = ?snapshot.selected_semantic_id,
        "automation.surface.snapshot_collected"
    );

    Some(snapshot)
}

// ---------------------------------------------------------------------------
// Notes collector
// ---------------------------------------------------------------------------

fn collect_notes_snapshot(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let (entity, handle) =
        crate::notes::get_notes_app_entity_and_handle_for_generation(resolved.generation?, cx)?;
    let layers = handle
        .read_with(cx, |root, cx| root.layer_snapshot(cx))
        .ok()?;
    let mut snapshot = collect_notes_elements(&entity, usize::MAX, cx);
    append_root_layer_elements(&mut snapshot, &layers);
    Some(snapshot)
}

/// Project retained Root layers, never synthetic dialogs or notification counts.
pub(crate) fn append_root_layer_elements(
    snapshot: &mut SurfaceElementSnapshot,
    layers: &gpui_component::RootLayerSnapshot,
) {
    for (index, dialog) in layers.dialogs.iter().enumerate() {
        let mut item = element(
            &format!(
                "root-dialog:{}:{}",
                dialog.root_entity_id, dialog.generation
            ),
            ElementType::Panel,
            None,
            Some(dialog.generation.to_string()),
            Some(index + 1 == layers.dialogs.len()),
            None,
            Some(index),
        );
        item.kind = Some("rootDialog".into());
        item.source = Some("gpui-component.Root".into());
        item.status_kind = Some(
            if index + 1 == layers.dialogs.len() {
                "current"
            } else {
                "stacked"
            }
            .into(),
        );
        item.role = Some("dismissDialog".into());
        item.selectable = Some(index + 1 == layers.dialogs.len());
        item.action_disabled = (index + 1 != layers.dialogs.len())
            .then(|| "Only the current dialog can be dismissed".into());
        snapshot.elements.push(item);
    }
    for notification in &layers.notifications {
        let mut item = element(
            &format!("root-notification:{}", notification.entity_id),
            ElementType::Panel,
            notification.title.clone(),
            notification.message.clone(),
            None,
            None,
            None,
        )
        .redact_content(crate::protocol::ElementContentKind::UserContent);
        item.kind = Some("rootNotification".into());
        item.source = Some("gpui-component.Notification".into());
        item.status_kind = Some(
            if notification.closing {
                "closing"
            } else {
                "visible"
            }
            .into(),
        );
        item.role = Some("dismissNotification".into());
        item.selectable = Some(!notification.closing);
        item.action_disabled = notification
            .closing
            .then(|| "Notification is already closing".into());
        snapshot.elements.push(item);
    }
    if !layers.dialogs.is_empty() {
        snapshot.focused_semantic_id = None;
    }
    snapshot.total_count = snapshot.elements.len();
}

#[cfg(test)]
mod root_layer_projection_tests {
    #[test]
    fn projection_retains_dialog_lifetimes_and_notification_closing_state() {
        let layers = gpui_component::RootLayerSnapshot {
            revision: 7,
            dialogs: vec![
                gpui_component::RootDialogId {
                    root_entity_id: 12,
                    generation: 3,
                },
                gpui_component::RootDialogId {
                    root_entity_id: 12,
                    generation: 6,
                },
            ],
            notifications: vec![gpui_component::notification::NotificationLayerSnapshot {
                entity_id: 24,
                closing: true,
                title: Some("Saved".into()),
                message: None,
            }],
            notifications_expanded: false,
        };
        let mut snapshot = super::SurfaceElementSnapshot {
            focused_semantic_id: Some("input:notes-editor".into()),
            ..Default::default()
        };
        super::append_root_layer_elements(&mut snapshot, &layers);
        assert_eq!(snapshot.elements[0].semantic_id, "root-dialog:12:3");
        assert_eq!(snapshot.elements[0].status_kind.as_deref(), Some("stacked"));
        assert_eq!(snapshot.elements[1].semantic_id, "root-dialog:12:6");
        assert_eq!(snapshot.elements[1].status_kind.as_deref(), Some("current"));
        assert_eq!(snapshot.elements[2].semantic_id, "root-notification:24");
        assert_eq!(snapshot.elements[2].status_kind.as_deref(), Some("closing"));
        assert_eq!(snapshot.elements[0].selectable, Some(false));
        assert_eq!(snapshot.elements[1].selectable, Some(true));
        assert_eq!(snapshot.elements[1].role.as_deref(), Some("dismissDialog"));
        assert_eq!(snapshot.elements[2].selectable, Some(false));
        assert_eq!(
            snapshot.elements[2].role.as_deref(),
            Some("dismissNotification")
        );
        assert_eq!(snapshot.total_count, 3);
        assert!(snapshot.focused_semantic_id.is_none());
        assert_eq!(layers.revision, 7);
    }
}

pub(crate) fn collect_notes_elements(
    entity: &gpui::Entity<crate::notes::NotesApp>,
    limit: usize,
    cx: &gpui::App,
) -> SurfaceElementSnapshot {
    let notes = entity.read(cx);
    let text = notes.editor_text(cx);
    let metrics = crate::notes::window::style::adopted_metrics();
    let theme = crate::theme::get_cached_theme();
    let editor_surface =
        crate::components::notes_editor::NotesEditorSurfaceStyle::from_theme(&theme);
    let editor_runtime = notes.editor_runtime_info(cx);
    let mut editor = element(
        "input:notes-editor",
        ElementType::Input,
        None,
        Some(text),
        None,
        Some(true),
        None,
    )
    .redact_content(crate::protocol::ElementContentKind::UserContent);
    editor.style = Some(ElementStyleInfo {
        owner: editor_surface.owner.to_string(),
        input_render_path: Some(editor_surface.input_render_path.to_string()),
        editor_runtime: Some(editor_runtime),
        surface_background_rgb: Some(editor_surface.background_rgb),
        occlusion_rgba: Some(editor_surface.occlusion_rgba),
        padding_x: Some(metrics.editor_padding_x),
        padding_y: Some(metrics.editor_padding_y),
        font_family_source: Some("theme.mono_font_family".to_string()),
        text_size_source: Some("theme.mono_font_size".to_string()),
    });

    // Titlebar commands are projected from the same mode-sensitive
    // NotesAction descriptors as the renderer, Actions, and keyboard router.
    let titlebar_actions = notes.titlebar_action_descriptors();
    let titlebar_elements = titlebar_actions
        .into_iter()
        .enumerate()
        .map(|(index, descriptor)| {
            let mut action = element(
                &descriptor.semantic_action_id,
                ElementType::Button,
                Some(descriptor.label.to_string()),
                descriptor.shortcut.map(str::to_string),
                None,
                None,
                Some(index),
            );
            action.role = Some(
                if descriptor.action == crate::notes::NotesAction::SendToAi {
                    "handoff"
                } else {
                    "action"
                }
                .to_string(),
            );
            action.kind = Some(
                if descriptor.action == crate::notes::NotesAction::SendToAi {
                    "MainAgentChat".to_string()
                } else {
                    format!("{:?}", descriptor.action)
                },
            );
            action.selectable = Some(descriptor.availability.is_enabled());
            action.action_disabled = descriptor.disabled_reason().map(str::to_string);
            action
        });

    let identity = collect_semantic_chip_element(&notes.document_identity_spec());
    let mut elements = vec![element(
        "panel:notes-window",
        ElementType::Panel,
        Some("Notes".to_string()),
        None,
        None,
        None,
        None,
    )];
    elements.extend(titlebar_elements);
    elements.push(editor);
    elements.push(identity);
    let total_count = elements.len();
    elements.truncate(limit);

    SurfaceElementSnapshot {
        elements,
        total_count,
        focused_semantic_id: Some("input:notes-editor".to_string()),
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

// ---------------------------------------------------------------------------
// Detached Agent Chat collector
// ---------------------------------------------------------------------------

fn collect_agent_chat_detached_snapshot(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let entity = crate::ai::agent_chat::ui::chat_window::get_agent_chat_view_for_instance(
        &resolved.id,
        resolved.generation?,
    )?;
    Some(collect_agent_chat_detached_elements(
        &entity,
        usize::MAX,
        cx,
    ))
}

/// Collect semantic elements from a live detached Agent Chat entity.
///
/// Shared by the surface snapshot path (`getElements`) and the
/// [`DetachedAgentChatTransactionProvider`](super::automation_transaction_provider::DetachedAgentChatTransactionProvider)
/// so both see the same semantic model.
pub(crate) fn collect_agent_chat_detached_elements(
    entity: &gpui::Entity<crate::ai::agent_chat::ui::view::AgentChatView>,
    limit: usize,
    cx: &gpui::App,
) -> SurfaceElementSnapshot {
    if entity.read(cx).shows_setup_card(cx) {
        let mut elements = collect_agent_chat_conversation_elements(entity, cx);
        let total_count = elements.len();
        elements.truncate(limit);
        return SurfaceElementSnapshot {
            total_count,
            elements,
            focused_semantic_id: None,
            selected_semantic_id: None,
            warnings: Vec::new(),
            quality: SnapshotQuality::Full,
        };
    }

    let state = entity.read(cx).collect_agent_chat_state_snapshot(cx);

    let picker_open = state.picker.as_ref().map(|p| p.open).unwrap_or(false);

    let mut elements = vec![
        element(
            "panel:agent_chat-detached",
            ElementType::Panel,
            None,
            None,
            None,
            None,
            None,
        ),
        element(
            "input:agent_chat-composer",
            ElementType::Input,
            None,
            Some(state.input_text.clone()),
            None,
            Some(true),
            None,
        )
        .redact_content(crate::protocol::ElementContentKind::UserContent),
        element(
            "list:agent_chat-messages",
            ElementType::List,
            Some(format!("{} messages", state.message_count)),
            None,
            None,
            None,
            None,
        ),
    ];

    if let Some(session) = &entity.read(cx).composer_picker_session {
        for (index, item) in session.items.iter().enumerate() {
            elements.push(element(
                &format!("choice:{index}:{}", item.id),
                ElementType::Choice,
                Some(item.label.to_string()),
                Some(item.id.to_string()),
                Some(index == session.selected_index),
                None,
                Some(index),
            ));
        }
    }
    if picker_open {
        elements.push(element(
            "panel:agent_chat-picker",
            ElementType::Panel,
            Some("open".to_string()),
            None,
            None,
            None,
            None,
        ));
    }

    elements.extend(
        entity
            .read(cx)
            .conversation_semantic_chip_specs(cx)
            .iter()
            .flat_map(collect_semantic_chip_elements),
    );
    elements.extend(collect_conversation_command_elements(
        &entity.read(cx).conversation_command_bindings(cx),
    ));
    elements.extend(collect_agent_chat_conversation_elements(entity, cx));

    let total_count = elements.len();
    if elements.len() > limit {
        elements.truncate(limit);
    }

    SurfaceElementSnapshot {
        total_count,
        elements,
        focused_semantic_id: Some("input:agent_chat-composer".to_string()),
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

// ---------------------------------------------------------------------------
// Actions dialog collector
// ---------------------------------------------------------------------------

/// Collect semantic elements from the live ActionsDialog entity.
///
/// Returns `None` when the exact Actions root no longer exists.
fn collect_actions_dialog_snapshot(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let entity = exact_actions_dialog_entity(resolved, cx)?;
    Some(collect_actions_dialog_elements(&entity, usize::MAX, cx))
}

#[cfg(test)]
fn collect_cached_actions_dialog_snapshot(
    window_id: &str,
    generation: u64,
) -> Option<SurfaceElementSnapshot> {
    let cached = actions_dialog_semantic_cache()
        .lock()
        .ok()
        .and_then(|cache| {
            cache
                .get(window_id)
                .filter(|snapshot| snapshot.generation == Some(generation))
                .cloned()
        })?;
    Some(SurfaceElementSnapshot {
        total_count: cached.elements.len(),
        elements: cached.elements,
        focused_semantic_id: cached.focused_semantic_id,
        selected_semantic_id: cached.selected_semantic_id,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    })
}

/// Collect semantic elements from a live ActionsDialog entity.
///
/// Shared by the surface snapshot path (`getElements`) and the
/// [`ActionsDialogTransactionProvider`](super::automation_transaction_provider::ActionsDialogTransactionProvider)
/// so both see the same semantic model.
pub(crate) fn collect_actions_dialog_elements(
    dialog_entity: &gpui::Entity<crate::actions::ActionsDialog>,
    limit: usize,
    cx: &gpui::App,
) -> SurfaceElementSnapshot {
    let dialog = dialog_entity.read(cx);

    let mut elements = Vec::new();

    // Search input
    let search_focused = !dialog.hide_search;
    elements.push(element(
        "input:actions-search",
        ElementType::Input,
        None,
        Some(dialog.search_text.clone()),
        None,
        Some(search_focused),
        None,
    ));

    // List of filtered actions
    let action_count = dialog.filtered_actions.len();
    elements.push(element(
        "list:actions",
        ElementType::List,
        Some(format!("{action_count} actions")),
        None,
        None,
        None,
        None,
    ));

    // Individual action choices. Use grouped visual indexes so semantic ids
    // match rowGeometry, which also includes section headers.
    let mut selected_semantic_id = None;

    for (visual_index, grouped_item) in dialog.grouped_items.iter().enumerate() {
        match grouped_item {
            crate::actions::GroupedActionItem::SectionHeader(label) => {
                elements.push(element(
                    &format!("section:{visual_index}"),
                    ElementType::Panel,
                    Some(label.clone()),
                    None,
                    Some(dialog.selected_index == Some(visual_index)),
                    None,
                    Some(visual_index),
                ));
            }
            crate::actions::GroupedActionItem::Item(filter_idx) => {
                let Some(&action_idx) = dialog.filtered_actions.get(*filter_idx) else {
                    continue;
                };
                let Some(action) = dialog.actions.get(action_idx) else {
                    continue;
                };
                let is_selected = dialog.selected_index == Some(visual_index);
                let semantic_id = format!("choice:{visual_index}:{}", action.id);

                if is_selected {
                    selected_semantic_id = Some(semantic_id.clone());
                }

                elements.push(element(
                    &semantic_id,
                    ElementType::Choice,
                    Some(action.title.clone()),
                    Some(action.id.clone()),
                    Some(is_selected),
                    None,
                    Some(visual_index),
                ));
                if let Some(reason) = action.disabled_reason() {
                    elements.push(element(
                        &format!("disabled-reason:{}", action.id),
                        ElementType::Panel,
                        Some(reason.to_string()),
                        None,
                        None,
                        None,
                        Some(visual_index),
                    ));
                }
            }
        }
    }

    let focused_semantic_id = if search_focused {
        Some("input:actions-search".to_string())
    } else {
        selected_semantic_id.clone()
    };

    let total_count = elements.len();
    if elements.len() > limit {
        elements.truncate(limit);
    }

    SurfaceElementSnapshot {
        total_count,
        elements,
        focused_semantic_id,
        selected_semantic_id,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_semantic_chip_element, collect_semantic_chip_elements,
        project_agent_chat_conversation_elements, prompt_popup_semantic_cache,
        remove_dictation_microphone_prompt_popup_snapshot_if_generation,
        PromptPopupElementSnapshot,
    };

    #[test]
    fn conversation_semantic_projection_exposes_every_role_safe_action() {
        use crate::components::main_view_chrome::{SemanticChipAction, SemanticChipSpec};

        let context = SemanticChipSpec::context_attachment("context:one", "File", true);
        let context_elements = collect_semantic_chip_elements(&context);
        assert_eq!(context_elements.len(), 2);
        assert_eq!(context_elements[0].role.as_deref(), Some("contextChip"));
        assert_eq!(
            context_elements[0].kind.as_deref(),
            Some("openContextDetails")
        );
        assert_eq!(context_elements[1].kind.as_deref(), Some("removeContext"));
        assert_ne!(
            context_elements[0].semantic_id,
            context_elements[1].semantic_id
        );

        let identity = SemanticChipSpec::enabled_identity(
            "identity:one",
            "Agent",
            SemanticChipAction::OpenSelector,
            "⇧⇥",
        );
        let identity = collect_semantic_chip_element(&identity);
        assert_eq!(identity.role.as_deref(), Some("identityBadge"));
        assert_eq!(identity.kind.as_deref(), Some("openIdentitySelector"));

        let destination = SemanticChipSpec::destination_selector("destination:one", "Today");
        let destination = collect_semantic_chip_element(&destination);
        assert_eq!(destination.role.as_deref(), Some("destinationSelector"));
        assert_eq!(destination.kind.as_deref(), Some("selectDestination"));
    }

    #[test]
    fn agent_chat_conversation_projection_exposes_safe_status_and_copy_eligibility() {
        use sk_protocol::ai_reliability::CancellationKind;

        let elements = project_agent_chat_conversation_elements(
            &[(7, "  exact bytes  "), (8, " \n\t ")],
            Some(CancellationKind::UserStopped),
            Some("Stop the current response first"),
        );

        let copy = elements
            .iter()
            .filter(|element| element.kind.as_deref() == Some("copyTurn"))
            .collect::<Vec<_>>();
        assert_eq!(copy.len(), 1, "whitespace-only turns expose no copy action");
        assert_eq!(copy[0].semantic_id, "conversation.copyTurn:7");
        assert_eq!(copy[0].source.as_deref(), Some("AgentChatThreadMessage"));

        let status = elements
            .iter()
            .find(|element| element.semantic_id == "conversation.commandStatus")
            .expect("blocked command acknowledgement");
        assert_eq!(status.status_kind.as_deref(), Some("disabled"));
        assert_eq!(
            status.action_disabled.as_deref(),
            Some("Stop the current response first")
        );

        let cancellation = elements
            .iter()
            .find(|element| element.status_kind.as_deref() == Some("cancelled"))
            .expect("safe cancellation status");
        assert_eq!(cancellation.kind.as_deref(), Some("userStopped"));
        assert_eq!(cancellation.semantic_id, "conversation.turnStatus:8");
        assert!(cancellation.value.is_none());
    }

    #[test]
    fn prompt_popup_cache_cleanup_requires_exact_generation() {
        let id = "test:prompt-popup-cache-generation";
        prompt_popup_semantic_cache().lock().unwrap().insert(
            id.to_string(),
            PromptPopupElementSnapshot {
                generation: Some(14),
                elements: Vec::new(),
                focused_semantic_id: None,
                selected_semantic_id: None,
            },
        );

        assert!(!remove_dictation_microphone_prompt_popup_snapshot_if_generation(id, 13));
        assert!(prompt_popup_semantic_cache()
            .lock()
            .unwrap()
            .contains_key(id));
        assert!(remove_dictation_microphone_prompt_popup_snapshot_if_generation(id, 14));
        assert!(!prompt_popup_semantic_cache()
            .lock()
            .unwrap()
            .contains_key(id));
    }

    #[test]
    fn actions_cache_never_answers_for_a_different_lifetime() {
        let id = "test:actions-cache-exact-lifetime";
        super::actions_dialog_semantic_cache()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                id.into(),
                PromptPopupElementSnapshot {
                    generation: Some(9),
                    elements: Vec::new(),
                    focused_semantic_id: None,
                    selected_semantic_id: None,
                },
            );
        assert!(super::collect_cached_actions_dialog_snapshot(id, 8).is_none());
        assert!(super::collect_cached_actions_dialog_snapshot(id, 9).is_some());
        super::remove_actions_dialog_snapshot(id);
        assert!(super::collect_cached_actions_dialog_snapshot(id, 9).is_none());
    }
}
