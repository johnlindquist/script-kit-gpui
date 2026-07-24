//! Shared visual anatomy and keyboard policy for AI recovery.

use std::rc::Rc;

use gpui::{div, prelude::*, px, rgb, rgba, AnyElement, App, Div, FontWeight, Window};
use sk_protocol::ai_reliability::{AiRecoveryAction, DisabledReason, RecoveryRole};

use crate::ai::reliability::{
    AiRecoveryCardSpec, AiRecoveryLayout, AiRecoveryTone, AI_RECOVERY_BODY_ID,
    AI_RECOVERY_DISMISS_ID, AI_RECOVERY_PROGRESS_ID, AI_RECOVERY_TITLE_ID,
};
use crate::components::{
    info_metrics, info_palette, Button, ButtonColors, ButtonVariant, InfoStateDensity,
    INFO_SPACING, INFO_TYPE_SCALE,
};
use crate::theme::{AppChromeColors, SemanticChipColors, Theme};

pub type AiRecoveryActionHandler = Rc<dyn Fn(AiRecoveryAction, &mut Window, &mut App) + 'static>;
pub type AiRecoveryDismissHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

#[derive(Clone)]
pub struct AiRecoveryCardHandlers {
    pub on_action: AiRecoveryActionHandler,
    pub on_dismiss: Option<AiRecoveryDismissHandler>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRecoverySemanticNode {
    pub semantic_id: &'static str,
    pub role: &'static str,
    pub enabled: bool,
    pub disabled_reason: Option<DisabledReason>,
}

pub fn recovery_semantic_tree(spec: &AiRecoveryCardSpec) -> Vec<AiRecoverySemanticNode> {
    let mut nodes = vec![
        AiRecoverySemanticNode {
            semantic_id: spec.semantic_id,
            role: "card",
            enabled: true,
            disabled_reason: None,
        },
        AiRecoverySemanticNode {
            semantic_id: AI_RECOVERY_TITLE_ID,
            role: "title",
            enabled: true,
            disabled_reason: None,
        },
        AiRecoverySemanticNode {
            semantic_id: AI_RECOVERY_BODY_ID,
            role: "body",
            enabled: true,
            disabled_reason: None,
        },
    ];
    if spec.progress.is_some() {
        nodes.push(AiRecoverySemanticNode {
            semantic_id: AI_RECOVERY_PROGRESS_ID,
            role: "progress",
            enabled: true,
            disabled_reason: None,
        });
    }
    nodes.extend(spec.actions.iter().map(|action| AiRecoverySemanticNode {
        semantic_id: action.semantic_id,
        role: match action.role {
            RecoveryRole::Primary => "primary-action",
            RecoveryRole::Secondary => "secondary-action",
            RecoveryRole::Diagnostic => "diagnostic-action",
        },
        enabled: action.enabled,
        disabled_reason: action.disabled_reason.clone(),
    }));
    if spec.dismissible {
        nodes.push(AiRecoverySemanticNode {
            semantic_id: AI_RECOVERY_DISMISS_ID,
            role: "dismiss",
            enabled: true,
            disabled_reason: None,
        });
    }
    nodes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRecoveryKey {
    Tab { shift: bool },
    Enter,
    Space,
    Escape,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRecoveryFocusTarget {
    Action(usize),
    Dismiss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiRecoveryKeyDecision {
    Focus(AiRecoveryFocusTarget),
    Activate(AiRecoveryAction),
    Dismiss,
    Ignore,
}

pub fn decide_recovery_key(
    spec: &AiRecoveryCardSpec,
    focused: Option<AiRecoveryFocusTarget>,
    key: AiRecoveryKey,
) -> AiRecoveryKeyDecision {
    let mut targets = spec
        .actions
        .iter()
        .enumerate()
        .filter(|(_, action)| action.enabled)
        .map(|(index, _)| AiRecoveryFocusTarget::Action(index))
        .collect::<Vec<_>>();
    if spec.dismissible {
        targets.push(AiRecoveryFocusTarget::Dismiss);
    }

    match key {
        AiRecoveryKey::Tab { shift } => {
            if targets.is_empty() {
                return AiRecoveryKeyDecision::Ignore;
            }
            let current = focused
                .and_then(|focused| targets.iter().position(|target| *target == focused))
                .unwrap_or_else(|| if shift { 0 } else { targets.len() - 1 });
            let next = if shift {
                current.checked_sub(1).unwrap_or(targets.len() - 1)
            } else {
                (current + 1) % targets.len()
            };
            AiRecoveryKeyDecision::Focus(targets[next])
        }
        AiRecoveryKey::Enter | AiRecoveryKey::Space => match focused {
            Some(AiRecoveryFocusTarget::Action(index)) => spec
                .actions
                .get(index)
                .filter(|action| action.enabled)
                .map(|action| AiRecoveryKeyDecision::Activate(action.action.clone()))
                .unwrap_or(AiRecoveryKeyDecision::Ignore),
            Some(AiRecoveryFocusTarget::Dismiss) if spec.dismissible => {
                AiRecoveryKeyDecision::Dismiss
            }
            Some(AiRecoveryFocusTarget::Dismiss) | None => AiRecoveryKeyDecision::Ignore,
        },
        AiRecoveryKey::Escape if spec.dismissible => AiRecoveryKeyDecision::Dismiss,
        AiRecoveryKey::Escape | AiRecoveryKey::Other => AiRecoveryKeyDecision::Ignore,
    }
}

pub fn render_ai_recovery_card(
    spec: AiRecoveryCardSpec,
    theme: &Theme,
    handlers: AiRecoveryCardHandlers,
) -> AnyElement {
    let palette = info_palette(theme);
    let metrics = info_metrics(match spec.layout {
        AiRecoveryLayout::ComposerInline | AiRecoveryLayout::DeskRow => InfoStateDensity::Compact,
        AiRecoveryLayout::TranscriptCard | AiRecoveryLayout::BlockingPanel => {
            InfoStateDensity::Comfortable
        }
    });
    let tone_color = match spec.tone {
        AiRecoveryTone::Warning => theme.colors.ui.warning,
        AiRecoveryTone::Error => theme.colors.ui.error,
        AiRecoveryTone::Progress => theme.colors.accent.selected,
        AiRecoveryTone::Success => theme.colors.ui.success,
    };
    let tone_chip = AppChromeColors::from_theme(theme).semantic_chip_colors(theme, tone_color);
    let button_colors = ButtonColors::from_theme(theme);
    let mut action_elements = Vec::new();
    for action in &spec.actions {
        let recovery_action = action.action.clone();
        let handler = handlers.on_action.clone();
        let mut button = Button::new(action.label.clone(), button_colors)
            .id(action.semantic_id)
            .variant(match action.role {
                RecoveryRole::Primary => ButtonVariant::Primary,
                RecoveryRole::Secondary | RecoveryRole::Diagnostic => ButtonVariant::Ghost,
            })
            .disabled(!action.enabled);
        if action.enabled {
            button = button.on_click(Box::new(move |_, window, cx| {
                handler(recovery_action.clone(), window, cx);
            }));
        }
        action_elements.push(button.into_any_element());
    }

    let mut header = div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(INFO_SPACING.sm))
        .child(recovery_tone_icon(tone_chip).child(match spec.tone {
            AiRecoveryTone::Warning => "!",
            AiRecoveryTone::Error => "×",
            AiRecoveryTone::Progress => "…",
            AiRecoveryTone::Success => "✓",
        }))
        .child(
            div()
                .min_w(px(0.0))
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(INFO_SPACING.xxs))
                .child(
                    div()
                        .id(AI_RECOVERY_TITLE_ID)
                        .text_size(px(INFO_TYPE_SCALE.title.size))
                        .line_height(px(INFO_TYPE_SCALE.title.line))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(palette.title)
                        .child(spec.title.clone()),
                )
                .child(
                    div()
                        .id(AI_RECOVERY_BODY_ID)
                        .text_size(px(INFO_TYPE_SCALE.body.size))
                        .line_height(px(INFO_TYPE_SCALE.body.line))
                        .text_color(palette.body)
                        .child(spec.body.clone()),
                ),
        );

    if spec.dismissible {
        let mut dismiss = Button::new("Dismiss", button_colors)
            .id(AI_RECOVERY_DISMISS_ID)
            .variant(ButtonVariant::Icon)
            .disabled(handlers.on_dismiss.is_none());
        if let Some(on_dismiss) = handlers.on_dismiss.clone() {
            dismiss = dismiss.on_click(Box::new(move |_, window, cx| {
                on_dismiss(window, cx);
            }));
        }
        header = header.child(dismiss);
    }

    let mut body = div()
        .id(spec.semantic_id)
        .w_full()
        .max_w(px(metrics.max_width))
        .flex()
        .flex_col()
        .gap(px(metrics.item_gap))
        .px(px(metrics.pad_x))
        .py(px(metrics.pad_y))
        .rounded(px(metrics.radius))
        .border_1()
        .border_color(palette.border)
        .bg(palette.panel)
        .child(header);

    if let Some(note) = spec.preservation_note {
        body = body.child(
            div()
                .text_size(px(INFO_TYPE_SCALE.caption.size))
                .line_height(px(INFO_TYPE_SCALE.caption.line))
                .text_color(palette.hint)
                .child(note),
        );
    }
    if let Some(progress) = spec.progress {
        body = body.child(
            div()
                .id(AI_RECOVERY_PROGRESS_ID)
                .text_size(px(INFO_TYPE_SCALE.caption.size))
                .line_height(px(INFO_TYPE_SCALE.caption.line))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(tone_color))
                .child(progress.label),
        );
    }
    if !action_elements.is_empty() {
        body = body.child(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(INFO_SPACING.xs))
                .children(action_elements),
        );
    }
    body.into_any_element()
}

fn recovery_tone_icon(colors: SemanticChipColors) -> Div {
    div()
        .size(px(INFO_SPACING.xl))
        .rounded(px(INFO_SPACING.sm))
        .border_1()
        .border_color(rgba(colors.border_rgba))
        .bg(rgba(colors.bg_rgba))
        .text_color(rgb(colors.text_hex))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(INFO_TYPE_SCALE.subhead.size))
        .font_weight(FontWeight::SEMIBOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::reliability::{
        AiRecoveryActionSpec, AiRecoveryProgress, AiRecoveryTone, AI_RECOVERY_CARD_ID,
    };
    use sk_protocol::ai_reliability::{AiRecoveryAction, DisabledReason, RecoveryRole};
    use std::collections::HashSet;

    fn spec() -> AiRecoveryCardSpec {
        AiRecoveryCardSpec {
            semantic_id: AI_RECOVERY_CARD_ID,
            layout: AiRecoveryLayout::ComposerInline,
            tone: AiRecoveryTone::Error,
            title: "Connection stopped".into(),
            body: "Your work is saved.".into(),
            preservation_note: Some("Your question is saved.".into()),
            progress: Some(AiRecoveryProgress {
                label: "Waiting".into(),
            }),
            actions: vec![
                AiRecoveryActionSpec {
                    semantic_id: "ai-recovery-retry",
                    label: "Try again".into(),
                    action: AiRecoveryAction::Retry,
                    role: RecoveryRole::Primary,
                    enabled: true,
                    disabled_reason: None,
                },
                AiRecoveryActionSpec {
                    semantic_id: "ai-recovery-copy-details",
                    label: "Copy details".into(),
                    action: AiRecoveryAction::CopyDetails,
                    role: RecoveryRole::Diagnostic,
                    enabled: false,
                    disabled_reason: Some(DisabledReason::UnsupportedBySurface),
                },
            ],
            dismissible: true,
        }
    }

    #[test]
    fn semantic_tree_has_unique_stable_ids_and_disabled_reasons() {
        let tree = recovery_semantic_tree(&spec());
        let ids = tree
            .iter()
            .map(|node| node.semantic_id)
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), tree.len());
        assert!(tree
            .iter()
            .any(|node| node.semantic_id == AI_RECOVERY_TITLE_ID));
        assert!(tree.iter().any(|node| {
            node.semantic_id == "ai-recovery-copy-details"
                && node.disabled_reason == Some(DisabledReason::UnsupportedBySurface)
        }));
    }

    #[test]
    fn tab_and_shift_tab_cycle_only_enabled_targets() {
        let spec = spec();
        assert_eq!(
            decide_recovery_key(&spec, None, AiRecoveryKey::Tab { shift: false }),
            AiRecoveryKeyDecision::Focus(AiRecoveryFocusTarget::Action(0))
        );
        assert_eq!(
            decide_recovery_key(
                &spec,
                Some(AiRecoveryFocusTarget::Action(0)),
                AiRecoveryKey::Tab { shift: false }
            ),
            AiRecoveryKeyDecision::Focus(AiRecoveryFocusTarget::Dismiss)
        );
        assert_eq!(
            decide_recovery_key(
                &spec,
                Some(AiRecoveryFocusTarget::Action(0)),
                AiRecoveryKey::Tab { shift: true }
            ),
            AiRecoveryKeyDecision::Focus(AiRecoveryFocusTarget::Dismiss)
        );
    }

    #[test]
    fn enter_space_and_escape_have_deterministic_actions() {
        let spec = spec();
        for key in [AiRecoveryKey::Enter, AiRecoveryKey::Space] {
            assert_eq!(
                decide_recovery_key(&spec, Some(AiRecoveryFocusTarget::Action(0)), key),
                AiRecoveryKeyDecision::Activate(AiRecoveryAction::Retry)
            );
        }
        assert_eq!(
            decide_recovery_key(&spec, None, AiRecoveryKey::Escape),
            AiRecoveryKeyDecision::Dismiss
        );
    }
}
