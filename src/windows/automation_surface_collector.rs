//! Secondary-surface semantic element collectors.
//!
//! Provides [`collect_surface_snapshot`] which returns semantic elements for
//! non-main automation windows (Notes, AgentChatDetached, ActionsDialog, PromptPopup).
//!
//! Used by both `getElements` and `inspectAutomationWindow` so agents see one
//! consistent semantic model regardless of which protocol command they use.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::protocol::{
    AutomationWindowInfo, AutomationWindowKind, ElementInfo, ElementStyleInfo, ElementType,
};

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
    let snapshot = collect_actions_dialog_elements(dialog_entity, 1000, cx);
    if let Ok(mut cache) = actions_dialog_semantic_cache().lock() {
        cache.insert(
            window_id.to_string(),
            PromptPopupElementSnapshot {
                generation: None,
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
        format!(
            "{}:{}",
            chip.semantic_id,
            action
                .expect("trailing semantic actions are present")
                .as_str()
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
    use crate::components::conversation_actions::ConfirmPolicy;

    commands
        .iter()
        .map(|command| ElementInfo {
            semantic_id: command.descriptor.semantic_action_id.to_string(),
            element_type: ElementType::Button,
            text: Some(command.descriptor.label.to_string()),
            value: command.descriptor.shortcut.map(str::to_string),
            selected: Some(false),
            focused: Some(false),
            index: None,
            role: Some("conversationCommand".to_string()),
            kind: Some(command.descriptor.semantic_action_id.to_string()),
            source: Some("ConversationCommandDescriptor".to_string()),
            source_name: None,
            selectable: Some(command.descriptor.availability.is_enabled()),
            status_kind: (command.descriptor.confirmation == ConfirmPolicy::Required)
                .then(|| "confirmationRequired".to_string()),
            action_disabled: command
                .descriptor
                .availability
                .disabled_reason()
                .map(str::to_string),
            style: None,
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
        crate::ai::agent_chat::ui::AgentChatSession::Setup(_) => command_status
            .map(|status| {
                let mut element = ElementInfo::panel("conversation.commandStatus");
                element.text = Some(status.to_string());
                element.role = Some("conversationCommandStatus".to_string());
                element.kind = Some("disabledAcknowledgement".to_string());
                element.source = Some("ConversationCommandExecution".to_string());
                element.selectable = Some(false);
                element.status_kind = Some("disabled".to_string());
                element.action_disabled = Some(status.to_string());
                element
            })
            .into_iter()
            .collect(),
        crate::ai::agent_chat::ui::AgentChatSession::Live(thread) => {
            collect_agent_chat_thread_conversation_elements(thread.read(cx), command_status)
        }
    }
}

/// Collect semantic elements for a resolved non-main automation window.
///
/// Returns `None` for window kinds that do not yet have a collector.
pub fn collect_surface_snapshot(
    resolved: &AutomationWindowInfo,
    limit: usize,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let mut snapshot = match resolved.kind {
        AutomationWindowKind::Notes => collect_notes_snapshot(resolved, cx).unwrap_or_else(|| {
            panel_only_fallback(
                "panel:notes-window",
                resolved.title.clone(),
                "panel_only_notes",
            )
        }),
        AutomationWindowKind::AgentChatDetached => {
            collect_agent_chat_detached_snapshot(resolved, cx).unwrap_or_else(|| {
                panel_only_fallback(
                    "panel:agent_chat-detached",
                    resolved.title.clone(),
                    "panel_only_agent_chat_detached",
                )
            })
        }
        AutomationWindowKind::ActionsDialog => collect_actions_dialog_snapshot(cx)
            .or_else(|| collect_cached_actions_dialog_snapshot(&resolved.id))
            .unwrap_or_else(|| {
                panel_only_fallback(
                    "panel:actions-dialog",
                    resolved.title.clone(),
                    "panel_only_actions_dialog",
                )
            }),
        AutomationWindowKind::PromptPopup => collect_exact_prompt_popup_snapshot(resolved, cx)?,
        AutomationWindowKind::Dictation => collect_dictation_snapshot(resolved),
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

fn collect_dictation_snapshot(resolved: &AutomationWindowInfo) -> SurfaceElementSnapshot {
    let state = crate::dictation::snapshot_overlay_state().unwrap_or_default();
    let phase = format!("{:?}", state.phase);
    let destination = crate::dictation::destination_selector_spec(state.target);

    let mut panel = element(
        "panel:dictation-overlay",
        ElementType::Panel,
        resolved
            .title
            .clone()
            .or_else(|| Some("Dictation".to_string())),
        None,
        None,
        Some(resolved.focused),
        None,
    );
    panel.kind = Some("overlay".to_string());
    panel.status_kind = Some(phase.clone());

    let mut signal = element(
        "panel:dictation-signal-band",
        ElementType::Panel,
        Some(phase),
        None,
        None,
        None,
        None,
    );
    signal.kind = Some("signal".to_string());

    let mut target_badge = collect_semantic_chip_element(&destination);
    target_badge.selectable = Some(crate::dictation::can_cycle_dictation_target());

    SurfaceElementSnapshot {
        elements: vec![panel, signal, target_badge],
        total_count: 3,
        focused_semantic_id: Some("panel:dictation-overlay".to_string()),
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

/// Fallback for surfaces that cannot be fully introspected.
fn panel_only_fallback(
    panel_id: &str,
    title: Option<String>,
    warning: &str,
) -> SurfaceElementSnapshot {
    SurfaceElementSnapshot {
        elements: vec![element(
            panel_id,
            ElementType::Panel,
            title,
            None,
            None,
            Some(true),
            None,
        )],
        total_count: 1,
        focused_semantic_id: Some(panel_id.to_string()),
        selected_semantic_id: None,
        warnings: vec![warning.to_string()],
        quality: SnapshotQuality::PanelOnly,
    }
}

// ---------------------------------------------------------------------------
// Notes collector
// ---------------------------------------------------------------------------

fn collect_notes_snapshot(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let text = crate::notes::get_notes_editor_text(cx)?;
    let metrics = crate::notes::window::style::adopted_metrics();
    let theme = crate::theme::get_cached_theme();
    let editor_surface =
        crate::components::notes_editor::NotesEditorSurfaceStyle::from_theme(&theme);
    let editor_runtime = crate::notes::get_notes_editor_runtime_info(cx)?;
    let mut editor = element(
        "input:notes-editor",
        ElementType::Input,
        None,
        Some(text),
        None,
        Some(true),
        None,
    );
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

    // The one-shot titlebar Ask AI command (replaces the removed Notes/Agent
    // mode switcher). The element id mirrors the renderer's GPUI id; its
    // role/kind advertise that activation hands off to the MAIN window's
    // Agent Chat rather than changing any Notes-local mode.
    let mut ask_ai = element(
        "button:notes-ask-ai",
        ElementType::Button,
        Some("Ask AI".to_string()),
        None,
        None,
        None,
        Some(0),
    );
    ask_ai.role = Some("handoff".to_string());
    ask_ai.kind = Some("MainAgentChat".to_string());
    ask_ai.selectable = Some(true);

    let identity = crate::notes::get_notes_document_identity_spec(cx)
        .map(|spec| collect_semantic_chip_element(&spec));
    let mut elements = vec![
        element(
            "panel:notes-window",
            ElementType::Panel,
            resolved.title.clone(),
            None,
            None,
            None,
            None,
        ),
        ask_ai,
        editor,
    ];
    elements.extend(identity);
    let total_count = elements.len();

    Some(SurfaceElementSnapshot {
        elements,
        total_count,
        focused_semantic_id: Some("input:notes-editor".to_string()),
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    })
}

// ---------------------------------------------------------------------------
// Detached Agent Chat collector
// ---------------------------------------------------------------------------

fn collect_agent_chat_detached_snapshot(
    _resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    let entity = crate::ai::agent_chat::ui::chat_window::get_detached_agent_chat_view_entity()?;
    Some(collect_agent_chat_detached_elements(&entity, 1000, cx))
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
        ),
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

    if elements.len() > limit {
        elements.truncate(limit);
    }

    SurfaceElementSnapshot {
        total_count: elements.len(),
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
/// Returns `None` if the actions window is not open or its entity cannot be
/// read, causing the caller to fall back to `panel_only_actions_dialog`.
fn collect_actions_dialog_snapshot(cx: &gpui::App) -> Option<SurfaceElementSnapshot> {
    let dialog_entity = crate::actions::get_actions_dialog_entity(cx)?;
    Some(collect_actions_dialog_elements(&dialog_entity, 1000, cx))
}

fn collect_cached_actions_dialog_snapshot(window_id: &str) -> Option<SurfaceElementSnapshot> {
    let cached = actions_dialog_semantic_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(window_id).cloned())?;
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

    if elements.len() > limit {
        elements.truncate(limit);
    }

    SurfaceElementSnapshot {
        total_count: elements.len(),
        elements,
        focused_semantic_id,
        selected_semantic_id,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

// ---------------------------------------------------------------------------
// Prompt popup collector (composer picker, history popup, confirm)
// ---------------------------------------------------------------------------

/// Collect semantic elements only from the exact registered PromptPopup
/// subtype and lifetime. There is deliberately no "whichever popup is open"
/// fallback: a stale or mismatched target must fail closed.
fn collect_exact_prompt_popup_snapshot(
    resolved: &AutomationWindowInfo,
    cx: &gpui::App,
) -> Option<SurfaceElementSnapshot> {
    match resolved.id.as_str() {
        crate::ai::agent_chat::ui::history_popup::AGENT_CHAT_HISTORY_POPUP_AUTOMATION_ID => {
            let generation = resolved.generation?;
            if crate::ai::agent_chat::ui::history_popup::history_popup_generation()
                != Some(generation)
            {
                return None;
            }
            collect_history_popup_snapshot(cx)
        }
        crate::dictation::DICTATION_MICROPHONE_POPUP_AUTOMATION_ID => {
            collect_cached_prompt_popup_snapshot(&resolved.id, resolved.generation?)
        }
        "confirm-popup" if resolved.generation.is_none() => collect_confirm_popup_snapshot(cx),
        _ => None,
    }
}

fn collect_cached_prompt_popup_snapshot(
    window_id: &str,
    generation: u64,
) -> Option<SurfaceElementSnapshot> {
    let cached = prompt_popup_semantic_cache()
        .lock()
        .ok()?
        .get(window_id)
        .filter(|snapshot| snapshot.generation == Some(generation))
        .cloned()?;
    Some(SurfaceElementSnapshot {
        total_count: cached.elements.len(),
        elements: cached.elements,
        focused_semantic_id: cached.focused_semantic_id,
        selected_semantic_id: cached.selected_semantic_id,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    })
}

fn collect_history_popup_snapshot(cx: &gpui::App) -> Option<SurfaceElementSnapshot> {
    let snap = crate::ai::agent_chat::ui::history_popup::get_history_popup_snapshot(cx)?;

    let mut elements = vec![element(
        "panel:history-popup",
        ElementType::Panel,
        Some(snap.title.to_string()),
        Some(snap.query.to_string()),
        None,
        None,
        None,
    )];

    let entry_count = snap.entries.len();
    elements.push(element(
        "list:history-entries",
        ElementType::List,
        Some(format!("{entry_count} sessions")),
        None,
        None,
        None,
        None,
    ));

    let mut selected_semantic_id = None;
    for (idx, entry) in snap.entries.iter().enumerate() {
        let is_selected = idx == snap.selected_index;
        let semantic_id = format!("choice:{}:{}", idx, entry.hit.entry.session_id);

        if is_selected {
            selected_semantic_id = Some(semantic_id.clone());
        }

        elements.push(element(
            &semantic_id,
            ElementType::Choice,
            Some(entry.title.to_string()),
            Some(entry.hit.entry.session_id.clone()),
            Some(is_selected),
            None,
            Some(idx),
        ));
    }

    Some(SurfaceElementSnapshot {
        total_count: elements.len(),
        elements,
        focused_semantic_id: selected_semantic_id.clone(),
        selected_semantic_id,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    })
}

fn collect_confirm_popup_snapshot(cx: &gpui::App) -> Option<SurfaceElementSnapshot> {
    let snap = crate::confirm::get_confirm_popup_snapshot(cx)?;

    let confirm_focused = snap.focused_button == "confirm";
    let secondary_focused = snap.focused_button == "secondary";
    let cancel_focused = snap.focused_button == "cancel";
    let has_secondary = snap.secondary_text.is_some();

    let mut elements = vec![
        element(
            "panel:confirm-dialog",
            ElementType::Panel,
            Some(snap.title),
            Some(snap.body),
            None,
            None,
            None,
        ),
        element(
            "button:0:confirm",
            ElementType::Button,
            Some(snap.confirm_text),
            Some("confirm".to_string()),
            None,
            Some(confirm_focused),
            Some(0),
        ),
    ];
    if let Some(secondary_text) = snap.secondary_text {
        elements.push(element(
            "button:1:secondary",
            ElementType::Button,
            Some(secondary_text),
            Some("secondary".to_string()),
            None,
            Some(secondary_focused),
            Some(1),
        ));
    }
    let cancel_index = if has_secondary { 2 } else { 1 };
    let cancel_semantic_id = if has_secondary {
        "button:2:cancel"
    } else {
        "button:1:cancel"
    };
    elements.push(element(
        cancel_semantic_id,
        ElementType::Button,
        Some(snap.cancel_text),
        Some("cancel".to_string()),
        None,
        Some(cancel_focused),
        Some(cancel_index),
    ));

    let focused_semantic_id = if confirm_focused {
        "button:0:confirm"
    } else if secondary_focused {
        "button:1:secondary"
    } else {
        cancel_semantic_id
    };

    Some(SurfaceElementSnapshot {
        total_count: elements.len(),
        elements,
        focused_semantic_id: Some(focused_semantic_id.to_string()),
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    })
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
}
