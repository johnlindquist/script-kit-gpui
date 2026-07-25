//! Where an AI recovery action is allowed to appear.
//!
//! # Why this module exists
//!
//! Every AI surface used to render its recovery actions as loose buttons in
//! the middle of the window, painted by the shared card itself. Nothing else
//! in the app does that. The two sanctioned homes for a button are:
//!
//! 1. a **modal**, for a decision the user must make before continuing;
//! 2. a **floating footer or the actions menu**, for everything else.
//!
//! A recovery card is a *message*. The message says what broke and what was
//! preserved. Its actions are ordinary affordances and belong in the ordinary
//! places. This module is the single pure rule that decides which home each
//! action gets, so no surface can quietly grow a third answer.
//!
//! # The rule
//!
//! - The **primary** action becomes the footer's primary affordance. There is
//!   at most one, guaranteed upstream by `normalize_action_order`.
//! - **Secondary** and **diagnostic** actions go to the actions menu. They are
//!   real actions, but they must not compete with the primary one for space.
//! - **Dismiss** is Escape, in the footer, when the card is dismissible.
//! - A **blocking** surface is the only thing that may raise a modal, because
//!   a blocking panel is by definition a decision you must make.
//!
//! Nothing here performs effects, touches GPUI, or reads a clock. The plan is
//! a value, so a surface can be tested for what it *would* show without a
//! window.

use sk_protocol::ai_reliability::{AiRecoveryAction, DisabledReason, RecoveryRole};

use super::presentation::{AiRecoveryActionSpec, AiRecoveryCardSpec, AiRecoveryLayout};

/// The sanctioned homes for a recovery action.
///
/// This enum is deliberately closed and has no `Inline` variant. Adding one
/// back is the change that reintroduces the defect, and it cannot be done
/// without editing this doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryActionHome {
    /// The floating footer's primary affordance.
    Footer,
    /// The actions menu, reached with the actions chord.
    ActionsMenu,
    /// A modal the user must answer before continuing.
    Modal,
}

/// One recovery action, resolved to a home and ready for a surface to mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedRecoveryAction {
    pub semantic_id: &'static str,
    pub label: gpui::SharedString,
    pub action: AiRecoveryAction,
    pub home: RecoveryActionHome,
    pub enabled: bool,
    pub disabled_reason: Option<DisabledReason>,
}

/// Everything a surface needs to render one recovery state, split by home.
///
/// The message half is what the card still draws. The other halves are what
/// the surface hands to its footer, its actions menu, and — only when the
/// surface blocks — a modal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveryPresentationPlan {
    /// The single footer affordance, if the card has a primary action.
    pub footer: Option<PlacedRecoveryAction>,
    /// Secondary and diagnostic actions, in their normalized order.
    pub menu: Vec<PlacedRecoveryAction>,
    /// Actions that must be answered before continuing. Empty unless the
    /// surface uses a blocking layout.
    pub modal: Vec<PlacedRecoveryAction>,
    /// Whether the surface should offer Escape as a dismiss affordance.
    pub dismissible: bool,
}

impl RecoveryPresentationPlan {
    /// Every placed action, regardless of home.
    ///
    /// Used by the element collector so a probe can see the same set the
    /// renderer used. Order is footer, then menu, then modal.
    pub fn all(&self) -> impl Iterator<Item = &PlacedRecoveryAction> {
        self.footer
            .iter()
            .chain(self.menu.iter())
            .chain(self.modal.iter())
    }

    /// True when the plan places nothing anywhere.
    pub fn is_empty(&self) -> bool {
        self.footer.is_none() && self.menu.is_empty() && self.modal.is_empty()
    }
}

/// Resolve the home for a single action under a given layout.
///
/// Total over its inputs, so every case is testable without a window.
pub fn home_for_action(layout: AiRecoveryLayout, role: RecoveryRole) -> RecoveryActionHome {
    // A blocking panel is the one surface that is already a "you must decide"
    // moment, so its actions are the only ones allowed to become a modal.
    if layout == AiRecoveryLayout::BlockingPanel {
        return RecoveryActionHome::Modal;
    }
    match role {
        RecoveryRole::Primary => RecoveryActionHome::Footer,
        RecoveryRole::Secondary | RecoveryRole::Diagnostic => RecoveryActionHome::ActionsMenu,
    }
}

fn place(spec: &AiRecoveryActionSpec, home: RecoveryActionHome) -> PlacedRecoveryAction {
    PlacedRecoveryAction {
        semantic_id: spec.semantic_id,
        label: spec.label.clone(),
        action: spec.action.clone(),
        home,
        enabled: spec.enabled,
        disabled_reason: spec.disabled_reason.clone(),
    }
}

/// Split a recovery card spec into the parts each home renders.
///
/// The card keeps title, body, preservation note, and progress. Everything
/// clickable leaves.
pub fn plan_recovery_presentation(spec: &AiRecoveryCardSpec) -> RecoveryPresentationPlan {
    let mut plan = RecoveryPresentationPlan {
        dismissible: spec.dismissible,
        ..Default::default()
    };

    for action in &spec.actions {
        let home = home_for_action(spec.layout, action.role);
        let placed = place(action, home);
        match home {
            // `normalize_action_order` guarantees at most one primary, but a
            // future caller could build a spec by hand. Keep the first and
            // demote the rest rather than silently dropping them: a lost
            // action is exactly the class of bug this work exists to stop.
            RecoveryActionHome::Footer => {
                if plan.footer.is_none() {
                    plan.footer = Some(placed);
                } else {
                    plan.menu.push(PlacedRecoveryAction {
                        home: RecoveryActionHome::ActionsMenu,
                        ..placed
                    });
                }
            }
            RecoveryActionHome::ActionsMenu => plan.menu.push(placed),
            RecoveryActionHome::Modal => plan.modal.push(placed),
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::reliability::{AiRecoveryTone, AI_RECOVERY_CARD_ID};

    fn action(
        semantic_id: &'static str,
        role: RecoveryRole,
        enabled: bool,
    ) -> AiRecoveryActionSpec {
        AiRecoveryActionSpec {
            semantic_id,
            label: semantic_id.into(),
            action: AiRecoveryAction::Retry,
            role,
            enabled,
            disabled_reason: None,
        }
    }

    fn spec(layout: AiRecoveryLayout, actions: Vec<AiRecoveryActionSpec>) -> AiRecoveryCardSpec {
        AiRecoveryCardSpec {
            semantic_id: AI_RECOVERY_CARD_ID,
            layout,
            tone: AiRecoveryTone::Error,
            title: "Connection stopped".into(),
            body: "Your work is saved.".into(),
            preservation_note: None,
            progress: None,
            actions,
            dismissible: true,
        }
    }

    #[test]
    fn no_action_is_ever_placed_inline() {
        // The whole point. Every role, on every layout, must land in one of
        // the three sanctioned homes — there is no fourth answer.
        for layout in [
            AiRecoveryLayout::ComposerInline,
            AiRecoveryLayout::TranscriptCard,
            AiRecoveryLayout::BlockingPanel,
            AiRecoveryLayout::DeskRow,
        ] {
            for role in [
                RecoveryRole::Primary,
                RecoveryRole::Secondary,
                RecoveryRole::Diagnostic,
            ] {
                let home = home_for_action(layout, role);
                assert!(
                    matches!(
                        home,
                        RecoveryActionHome::Footer
                            | RecoveryActionHome::ActionsMenu
                            | RecoveryActionHome::Modal
                    ),
                    "{layout:?}/{role:?} resolved outside the sanctioned homes"
                );
            }
        }
    }

    #[test]
    fn the_primary_action_becomes_the_footer_affordance() {
        let plan = plan_recovery_presentation(&spec(
            AiRecoveryLayout::ComposerInline,
            vec![
                action("ai-recovery-retry", RecoveryRole::Primary, true),
                action("ai-recovery-copy-details", RecoveryRole::Diagnostic, true),
            ],
        ));

        let footer = plan.footer.expect("primary action must reach the footer");
        assert_eq!(footer.semantic_id, "ai-recovery-retry");
        assert_eq!(footer.home, RecoveryActionHome::Footer);
        assert_eq!(plan.menu.len(), 1);
        assert_eq!(plan.menu[0].semantic_id, "ai-recovery-copy-details");
        assert!(plan.modal.is_empty());
    }

    #[test]
    fn secondary_and_diagnostic_actions_go_to_the_actions_menu() {
        let plan = plan_recovery_presentation(&spec(
            AiRecoveryLayout::TranscriptCard,
            vec![
                action("ai-recovery-sign-in", RecoveryRole::Secondary, true),
                action("ai-recovery-copy-details", RecoveryRole::Diagnostic, true),
            ],
        ));

        assert!(
            plan.footer.is_none(),
            "no primary means no footer affordance"
        );
        assert_eq!(
            plan.menu
                .iter()
                .map(|placed| placed.semantic_id)
                .collect::<Vec<_>>(),
            vec!["ai-recovery-sign-in", "ai-recovery-copy-details"],
        );
    }

    #[test]
    fn only_a_blocking_layout_may_raise_a_modal() {
        let blocking = plan_recovery_presentation(&spec(
            AiRecoveryLayout::BlockingPanel,
            vec![action("ai-recovery-retry", RecoveryRole::Primary, true)],
        ));
        assert_eq!(blocking.modal.len(), 1);
        assert!(blocking.footer.is_none());

        for layout in [
            AiRecoveryLayout::ComposerInline,
            AiRecoveryLayout::TranscriptCard,
            AiRecoveryLayout::DeskRow,
        ] {
            let plan = plan_recovery_presentation(&spec(
                layout,
                vec![action("ai-recovery-retry", RecoveryRole::Primary, true)],
            ));
            assert!(
                plan.modal.is_empty(),
                "{layout:?} must not interrupt with a modal"
            );
        }
    }

    #[test]
    fn a_second_primary_is_demoted_rather_than_dropped() {
        // A hand-built spec can carry two primaries. Losing one silently is
        // the failure mode this whole change exists to prevent, so the extra
        // one must still be reachable — just not in the footer.
        let plan = plan_recovery_presentation(&spec(
            AiRecoveryLayout::ComposerInline,
            vec![
                action("ai-recovery-retry", RecoveryRole::Primary, true),
                action("ai-recovery-choose-model", RecoveryRole::Primary, true),
            ],
        ));

        assert_eq!(
            plan.footer.as_ref().map(|placed| placed.semantic_id),
            Some("ai-recovery-retry")
        );
        assert_eq!(plan.menu.len(), 1);
        assert_eq!(plan.menu[0].semantic_id, "ai-recovery-choose-model");
        assert_eq!(plan.menu[0].home, RecoveryActionHome::ActionsMenu);
    }

    #[test]
    fn a_disabled_action_keeps_its_reason_through_placement() {
        // A recovery action a surface cannot perform is never rendered
        // enabled. Placement must carry the reason, or the footer would show
        // an enabled-looking affordance for something that cannot run.
        let mut disabled = action("ai-recovery-retry", RecoveryRole::Primary, false);
        disabled.disabled_reason = Some(DisabledReason::UnsupportedBySurface);

        let plan =
            plan_recovery_presentation(&spec(AiRecoveryLayout::ComposerInline, vec![disabled]));

        let footer = plan.footer.expect("action must still be placed");
        assert!(!footer.enabled);
        assert_eq!(
            footer.disabled_reason,
            Some(DisabledReason::UnsupportedBySurface)
        );
    }

    #[test]
    fn an_empty_spec_places_nothing_anywhere() {
        let plan = plan_recovery_presentation(&spec(AiRecoveryLayout::ComposerInline, vec![]));
        assert!(plan.is_empty());
        assert_eq!(plan.all().count(), 0);
    }

    #[test]
    fn every_placed_action_is_visible_to_the_collector() {
        // `all()` feeds the element collector. If it ever misses a home, a
        // probe would report an action the user can actually reach as absent.
        let plan = plan_recovery_presentation(&spec(
            AiRecoveryLayout::ComposerInline,
            vec![
                action("ai-recovery-retry", RecoveryRole::Primary, true),
                action("ai-recovery-sign-in", RecoveryRole::Secondary, true),
                action("ai-recovery-copy-details", RecoveryRole::Diagnostic, true),
            ],
        ));

        assert_eq!(
            plan.all()
                .map(|placed| placed.semantic_id)
                .collect::<Vec<_>>(),
            vec![
                "ai-recovery-retry",
                "ai-recovery-sign-in",
                "ai-recovery-copy-details"
            ],
        );
    }
}
