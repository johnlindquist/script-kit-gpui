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
                elements,
                focused_semantic_id,
                selected_semantic_id,
            },
        );
    }
}

#[allow(dead_code)] // Binary dictation overlay uses this; lib-only builds do not.
pub(crate) fn remove_dictation_microphone_prompt_popup_snapshot(window_id: &str) {
    if let Ok(mut cache) = prompt_popup_semantic_cache().lock() {
        cache.remove(window_id);
    }
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
        AutomationWindowKind::PromptPopup => collect_cached_prompt_popup_snapshot(&resolved.id)
            .or_else(|| collect_prompt_popup_snapshot(cx))
            .unwrap_or_else(|| {
                panel_only_fallback(
                    "panel:prompt-popup",
                    resolved.title.clone(),
                    "panel_only_prompt_popup",
                )
            }),
        AutomationWindowKind::Dictation => collect_dictation_snapshot(resolved),
        AutomationWindowKind::DevStyleTool => collect_dev_style_tool_snapshot(),
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

#[allow(clippy::vec_init_then_push)]
fn collect_dev_style_tool_snapshot() -> SurfaceElementSnapshot {
    let mut elements = Vec::new();
    elements.push(element(
        "panel:dev-style-tool",
        ElementType::Panel,
        Some("Script Kit Dev Style Tool".to_string()),
        None,
        None,
        None,
        None,
    ));
    elements.push(element(
        "button:dev-style-tool-save",
        ElementType::Button,
        Some("Save".to_string()),
        Some("saveCurrentStyleSettings".to_string()),
        None,
        None,
        Some(0),
    ));
    elements.push(element(
        "button:dev-style-tool-undo",
        ElementType::Button,
        Some("Undo".to_string()),
        Some("undoStyleChange".to_string()),
        None,
        None,
        Some(1),
    ));
    elements.push(element(
        "button:dev-style-tool-redo",
        ElementType::Button,
        Some("Redo".to_string()),
        Some("redoStyleChange".to_string()),
        None,
        None,
        Some(2),
    ));
    elements.push(element(
        "button:dev-style-tool-reset-all",
        ElementType::Button,
        Some("Reset All".to_string()),
        Some("resetStyleControls".to_string()),
        None,
        None,
        Some(3),
    ));
    elements.push(element(
        "button:dev-style-tool-copy-markdown",
        ElementType::Button,
        Some("Copy Markdown".to_string()),
        Some("copySavedStyleMarkdown".to_string()),
        None,
        None,
        Some(4),
    ));
    for (offset, target) in crate::dev_style_tool::DevStyleKitchenSinkTarget::ALL
        .iter()
        .enumerate()
    {
        elements.push(element(
            target.semantic_id(),
            ElementType::Button,
            Some(target.label().to_string()),
            Some(target.action_value().to_string()),
            None,
            None,
            Some(5 + offset),
        ));
    }
    elements.push(element(
        "input:dev-style-tool-saved-markdown",
        ElementType::Input,
        Some("Saved style markdown".to_string()),
        Some("savedStyleMarkdown".to_string()),
        None,
        None,
        Some(10),
    ));
    let mut seen_groups = Vec::new();
    let mut seen_sections = Vec::new();
    let mut next_index = 11usize;
    for (semantic_id, label) in [
        (
            "tab:dev-style-tool:main-window-styling",
            "Main Window Styling",
        ),
        ("tab:dev-style-tool:text-copy", "Text / Copy"),
        (
            "tab:dev-style-tool:actions-popup-styling",
            "Actions Popup Styling",
        ),
        (
            "tab:dev-style-tool:agent-chat-styling",
            "Agent Chat Styling",
        ),
        (
            "tab:dev-style-tool:confirm-modal-styling",
            "Confirm Modal Styling",
        ),
    ] {
        elements.push(element(
            semantic_id,
            ElementType::Button,
            Some(label.to_string()),
            Some(label.to_string()),
            None,
            None,
            Some(next_index),
        ));
        next_index += 1;
    }
    elements.push(element(
        "summary:dev-style-tool-active-scope",
        ElementType::Panel,
        Some("Active style scope".to_string()),
        Some("activeStyleScope".to_string()),
        None,
        None,
        Some(next_index),
    ));
    next_index += 1;
    for control in crate::dev_style_tool::COPY_CONTROLS {
        let control_index = next_index;
        next_index += 3;
        elements.push(element(
            &format!("control:dev-style-tool-copy:{}", control.id.as_str()),
            ElementType::Panel,
            Some(control.label.to_string()),
            Some(control.id.as_str().to_string()),
            None,
            None,
            Some(control_index),
        ));
        elements.push(element(
            &format!("input:dev-style-tool-copy:{}", control.id.as_str()),
            ElementType::Input,
            Some(control.label.to_string()),
            Some(control.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 1),
        ));
        elements.push(element(
            &format!("button:dev-style-tool-copy-reset:{}", control.id.as_str()),
            ElementType::Button,
            Some(format!("Reset {}", control.label)),
            Some(control.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 2),
        ));
    }

    let mut seen_actions_groups = Vec::new();
    for knob in crate::dev_style_tool::ACTIONS_POPUP_KNOBS {
        if !seen_actions_groups.contains(&knob.group) {
            seen_actions_groups.push(knob.group);
            elements.push(element(
                &format!(
                    "tab:dev-style-tool-actions:{}",
                    actions_popup_group_slug(knob.group)
                ),
                ElementType::Button,
                Some(knob.group.label().to_string()),
                Some(actions_popup_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
            elements.push(element(
                &format!(
                    "actions-style-section:{}",
                    actions_popup_group_slug(knob.group)
                ),
                ElementType::Panel,
                Some(knob.group.label().to_string()),
                Some(actions_popup_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
        }
        let control_index = next_index;
        next_index += 3;
        elements.push(element(
            &format!("slider:dev-style-tool-actions:{}", knob.id.as_str()),
            ElementType::Slider,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index),
        ));
        elements.push(element(
            &format!("input:dev-style-tool-actions:{}", knob.id.as_str()),
            ElementType::Input,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 1),
        ));
        elements.push(element(
            &format!("button:dev-style-tool-actions-reset:{}", knob.id.as_str()),
            ElementType::Button,
            Some(format!("Reset {}", knob.label)),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 2),
        ));
    }

    let mut seen_agent_chat_groups = Vec::new();
    for knob in crate::dev_style_tool::AGENT_CHAT_KNOBS {
        if !seen_agent_chat_groups.contains(&knob.group) {
            seen_agent_chat_groups.push(knob.group);
            elements.push(element(
                &format!(
                    "tab:dev-style-tool-agent-chat:{}",
                    agent_chat_group_slug(knob.group)
                ),
                ElementType::Button,
                Some(knob.group.label().to_string()),
                Some(agent_chat_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
            elements.push(element(
                &format!(
                    "agent-chat-style-section:{}",
                    agent_chat_group_slug(knob.group)
                ),
                ElementType::Panel,
                Some(knob.group.label().to_string()),
                Some(agent_chat_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
        }
        let control_index = next_index;
        next_index += 3;
        elements.push(element(
            &format!("slider:dev-style-tool-agent-chat:{}", knob.id.as_str()),
            ElementType::Slider,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index),
        ));
        elements.push(element(
            &format!("input:dev-style-tool-agent-chat:{}", knob.id.as_str()),
            ElementType::Input,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 1),
        ));
        elements.push(element(
            &format!(
                "button:dev-style-tool-agent-chat-reset:{}",
                knob.id.as_str()
            ),
            ElementType::Button,
            Some(format!("Reset {}", knob.label)),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 2),
        ));
    }

    let mut seen_confirm_modal_groups = Vec::new();
    for knob in crate::dev_style_tool::CONFIRM_MODAL_KNOBS {
        if !seen_confirm_modal_groups.contains(&knob.group) {
            seen_confirm_modal_groups.push(knob.group);
            elements.push(element(
                &format!(
                    "tab:dev-style-tool-confirm-modal:{}",
                    confirm_modal_group_slug(knob.group)
                ),
                ElementType::Button,
                Some(knob.group.label().to_string()),
                Some(confirm_modal_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
            elements.push(element(
                &format!(
                    "confirm-modal-style-section:{}",
                    confirm_modal_group_slug(knob.group)
                ),
                ElementType::Panel,
                Some(knob.group.label().to_string()),
                Some(confirm_modal_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
        }
        let control_index = next_index;
        next_index += 3;
        elements.push(element(
            &format!("slider:dev-style-tool-confirm-modal:{}", knob.id.as_str()),
            ElementType::Slider,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index),
        ));
        elements.push(element(
            &format!("input:dev-style-tool-confirm-modal:{}", knob.id.as_str()),
            ElementType::Input,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 1),
        ));
        elements.push(element(
            &format!(
                "button:dev-style-tool-confirm-modal-reset:{}",
                knob.id.as_str()
            ),
            ElementType::Button,
            Some(format!("Reset {}", knob.label)),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 2),
        ));
    }

    for knob in crate::dev_style_tool::STYLE_KNOBS {
        if !seen_groups.contains(&knob.group) {
            seen_groups.push(knob.group);
            elements.push(element(
                &format!("tab:dev-style-tool:{}", dev_style_group_slug(knob.group)),
                ElementType::Button,
                Some(knob.group.label().to_string()),
                Some(dev_style_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
            elements.push(element(
                &format!("style-section:{}", dev_style_group_slug(knob.group)),
                ElementType::Panel,
                Some(knob.group.label().to_string()),
                Some(dev_style_group_slug(knob.group).to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
        }
        let section = crate::dev_style_tool::StyleKnobSection::for_knob(knob);
        if !seen_sections.contains(&(knob.group, section)) {
            seen_sections.push((knob.group, section));
            elements.push(element(
                &format!(
                    "style-subsection:{}:{}",
                    dev_style_group_slug(knob.group),
                    section.slug
                ),
                ElementType::Panel,
                Some(section.label.to_string()),
                Some(section.slug.to_string()),
                None,
                None,
                Some(next_index),
            ));
            next_index += 1;
        }
        let control_index = next_index;
        next_index += 3;
        elements.push(element(
            &format!("slider:dev-style-tool:{}", knob.id.as_str()),
            ElementType::Slider,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index),
        ));
        elements.push(element(
            &format!("input:dev-style-tool:{}", knob.id.as_str()),
            ElementType::Input,
            Some(knob.label.to_string()),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 1),
        ));
        elements.push(element(
            &format!("button:dev-style-tool-reset:{}", knob.id.as_str()),
            ElementType::Button,
            Some(format!("Reset {}", knob.label)),
            Some(knob.id.as_str().to_string()),
            None,
            None,
            Some(control_index + 2),
        ));
    }

    SurfaceElementSnapshot {
        total_count: elements.len(),
        elements,
        focused_semantic_id: None,
        selected_semantic_id: None,
        warnings: Vec::new(),
        quality: SnapshotQuality::Full,
    }
}

fn dev_style_group_slug(group: crate::dev_style_tool::StyleKnobGroup) -> &'static str {
    match group {
        crate::dev_style_tool::StyleKnobGroup::Shell => "shell",
        crate::dev_style_tool::StyleKnobGroup::Search => "search",
        crate::dev_style_tool::StyleKnobGroup::List => "list",
        crate::dev_style_tool::StyleKnobGroup::Row => "row",
        crate::dev_style_tool::StyleKnobGroup::Icon => "icon",
        crate::dev_style_tool::StyleKnobGroup::Metadata => "metadata",
        crate::dev_style_tool::StyleKnobGroup::Typography => "typography",
        crate::dev_style_tool::StyleKnobGroup::Footer => "footer",
        crate::dev_style_tool::StyleKnobGroup::HeaderInfoBar => "header-info-bar",
    }
}

fn agent_chat_group_slug(group: crate::dev_style_tool::AgentChatKnobGroup) -> &'static str {
    match group {
        crate::dev_style_tool::AgentChatKnobGroup::Transcript => "transcript",
        crate::dev_style_tool::AgentChatKnobGroup::Markdown => "markdown",
        crate::dev_style_tool::AgentChatKnobGroup::UserMessage => "user-message",
        crate::dev_style_tool::AgentChatKnobGroup::AssistantMessage => "assistant-message",
        crate::dev_style_tool::AgentChatKnobGroup::CollapsibleBlocks => "collapsible-blocks",
        crate::dev_style_tool::AgentChatKnobGroup::ErrorAndSystem => "error-system",
    }
}

fn actions_popup_group_slug(group: crate::dev_style_tool::ActionsPopupKnobGroup) -> &'static str {
    match group {
        crate::dev_style_tool::ActionsPopupKnobGroup::Shell => "shell",
        crate::dev_style_tool::ActionsPopupKnobGroup::Search => "search",
        crate::dev_style_tool::ActionsPopupKnobGroup::List => "list",
        crate::dev_style_tool::ActionsPopupKnobGroup::Row => "row",
        crate::dev_style_tool::ActionsPopupKnobGroup::Section => "section",
        crate::dev_style_tool::ActionsPopupKnobGroup::ContextHeader => "context-header",
    }
}

fn confirm_modal_group_slug(group: crate::dev_style_tool::ConfirmModalKnobGroup) -> &'static str {
    match group {
        crate::dev_style_tool::ConfirmModalKnobGroup::Shell => "shell",
        crate::dev_style_tool::ConfirmModalKnobGroup::Header => "header",
        crate::dev_style_tool::ConfirmModalKnobGroup::Anatomy => "anatomy",
        crate::dev_style_tool::ConfirmModalKnobGroup::Actions => "actions",
    }
}

fn collect_dictation_snapshot(resolved: &AutomationWindowInfo) -> SurfaceElementSnapshot {
    let state = crate::dictation::snapshot_overlay_state().unwrap_or_default();
    let phase = format!("{:?}", state.phase);
    let target = state.target.overlay_label().to_string();

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

    let mut target_badge = element(
        "button:dictation-target",
        ElementType::Button,
        Some(target),
        None,
        None,
        None,
        Some(0),
    );
    target_badge.role = Some("target".to_string());
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

    Some(SurfaceElementSnapshot {
        elements: vec![
            element(
                "panel:notes-window",
                ElementType::Panel,
                resolved.title.clone(),
                None,
                None,
                None,
                None,
            ),
            editor,
        ],
        total_count: 2,
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
                    Some(dialog.selected_index == visual_index),
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
                let is_selected = dialog.selected_index == visual_index;
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

/// Collect semantic elements from a known prompt popup type.
///
/// Tries each known popup kind in order and returns the first match.
/// Returns `None` if no known popup is open, causing the caller to fall
/// back to `panel_only_prompt_popup`.
fn collect_prompt_popup_snapshot(cx: &gpui::App) -> Option<SurfaceElementSnapshot> {
    if let Some(snapshot) = collect_history_popup_snapshot(cx) {
        return Some(snapshot);
    }
    if let Some(snapshot) = collect_confirm_popup_snapshot(cx) {
        return Some(snapshot);
    }
    None
}

fn collect_cached_prompt_popup_snapshot(window_id: &str) -> Option<SurfaceElementSnapshot> {
    let cached = prompt_popup_semantic_cache()
        .lock()
        .ok()?
        .get(window_id)
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
