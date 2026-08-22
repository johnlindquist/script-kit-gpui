//! Pure launcher interaction planning over the existing window orchestrator.

/// Snapshot of the interaction layers above the root launcher. Facts are
/// supplied by the owning host; no focus/window side effects occur here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LauncherEscapeFacts {
    pub actions_open: bool,
    pub attachment_portal_open: bool,
    pub object_selector_open: bool,
    pub trigger_picker_open: bool,
    pub trigger_picker_filter_only: bool,
    pub visible_filter: bool,
    pub return_to_origin: bool,
}

/// Exactly one layer may close or transition per physical or simulated Escape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherEscapeDecision {
    CloseActions,
    CancelAttachmentPortal,
    CloseObjectSelector,
    CloseTriggerPicker,
    ClearVisibleFilter,
    ReturnToOrigin,
    DismissMain,
}

/// Popup dismissal has one cross-host grammar: Escape moves back one nested
/// route, while the explicit Actions toggle closes the entire overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayDismissTrigger {
    Escape,
    ActionsToggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayDismissDecision {
    PopRoute,
    CloseOverlay,
}

/// A destructive confirmation authorizes only its exact, still-selected target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedDestructiveActionDecision {
    ExecuteCapturedTarget,
    RejectUnconfirmed,
    RejectMissingTarget,
    RejectChangedTarget,
}

pub fn plan_confirmed_destructive_action<T: PartialEq>(
    confirmation_granted: bool,
    captured_target: &T,
    current_target: Option<&T>,
) -> ConfirmedDestructiveActionDecision {
    if !confirmation_granted {
        return ConfirmedDestructiveActionDecision::RejectUnconfirmed;
    }

    match current_target {
        Some(current_target) if current_target == captured_target => {
            ConfirmedDestructiveActionDecision::ExecuteCapturedTarget
        }
        Some(_) => ConfirmedDestructiveActionDecision::RejectChangedTarget,
        None => ConfirmedDestructiveActionDecision::RejectMissingTarget,
    }
}

pub const fn plan_overlay_dismiss(
    route_depth: usize,
    trigger: OverlayDismissTrigger,
) -> OverlayDismissDecision {
    match trigger {
        OverlayDismissTrigger::Escape if route_depth > 1 => OverlayDismissDecision::PopRoute,
        OverlayDismissTrigger::Escape | OverlayDismissTrigger::ActionsToggle => {
            OverlayDismissDecision::CloseOverlay
        }
    }
}

pub const fn plan_launcher_escape(facts: LauncherEscapeFacts) -> LauncherEscapeDecision {
    if facts.actions_open {
        LauncherEscapeDecision::CloseActions
    } else if facts.attachment_portal_open {
        LauncherEscapeDecision::CancelAttachmentPortal
    } else if facts.object_selector_open {
        LauncherEscapeDecision::CloseObjectSelector
    } else if facts.trigger_picker_open {
        if facts.trigger_picker_filter_only {
            LauncherEscapeDecision::ClearVisibleFilter
        } else {
            LauncherEscapeDecision::CloseTriggerPicker
        }
    } else if facts.visible_filter {
        LauncherEscapeDecision::ClearVisibleFilter
    } else if facts.return_to_origin {
        LauncherEscapeDecision::ReturnToOrigin
    } else {
        LauncherEscapeDecision::DismissMain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DestructiveAppIdentity {
        name: &'static str,
        bundle_id: Option<&'static str>,
        path: &'static str,
    }

    #[test]
    fn destructive_confirmation_executes_only_exact_captured_identity() {
        let captured = DestructiveAppIdentity {
            name: "Example",
            bundle_id: Some("com.example.app"),
            path: "/Applications/Example.app",
        };

        assert_eq!(
            plan_confirmed_destructive_action(true, &captured, Some(&captured)),
            ConfirmedDestructiveActionDecision::ExecuteCapturedTarget
        );
    }

    #[test]
    fn destructive_confirmation_rejects_changed_name_bundle_or_path() {
        let captured = DestructiveAppIdentity {
            name: "Example",
            bundle_id: Some("com.example.app"),
            path: "/Applications/Example.app",
        };

        for changed in [
            DestructiveAppIdentity {
                name: "Different",
                ..captured.clone()
            },
            DestructiveAppIdentity {
                bundle_id: Some("com.example.other"),
                ..captured.clone()
            },
            DestructiveAppIdentity {
                path: "/Applications/Other.app",
                ..captured.clone()
            },
        ] {
            assert_eq!(
                plan_confirmed_destructive_action(true, &captured, Some(&changed)),
                ConfirmedDestructiveActionDecision::RejectChangedTarget
            );
        }
    }

    #[test]
    fn destructive_confirmation_rejects_cancellation_and_missing_selection() {
        let captured = DestructiveAppIdentity {
            name: "Example",
            bundle_id: None,
            path: "/Applications/Example.app",
        };

        assert_eq!(
            plan_confirmed_destructive_action(false, &captured, Some(&captured)),
            ConfirmedDestructiveActionDecision::RejectUnconfirmed
        );
        assert_eq!(
            plan_confirmed_destructive_action(true, &captured, None),
            ConfirmedDestructiveActionDecision::RejectMissingTarget
        );
    }

    #[test]
    fn destructive_confirmation_requires_exact_conversation_session() {
        let captured = "conversation-owner-a".to_string();
        let replacement = "conversation-owner-b".to_string();

        assert_eq!(
            plan_confirmed_destructive_action(true, &captured, Some(&captured)),
            ConfirmedDestructiveActionDecision::ExecuteCapturedTarget
        );
        assert_eq!(
            plan_confirmed_destructive_action(true, &captured, Some(&replacement)),
            ConfirmedDestructiveActionDecision::RejectChangedTarget
        );
        assert_eq!(
            plan_confirmed_destructive_action(false, &captured, Some(&captured)),
            ConfirmedDestructiveActionDecision::RejectUnconfirmed
        );
    }

    #[test]
    fn each_escape_closes_exactly_one_topmost_layer() {
        let all = LauncherEscapeFacts {
            actions_open: true,
            attachment_portal_open: true,
            object_selector_open: true,
            trigger_picker_open: true,
            trigger_picker_filter_only: false,
            visible_filter: true,
            return_to_origin: true,
        };
        assert_eq!(
            plan_launcher_escape(all),
            LauncherEscapeDecision::CloseActions
        );
        let portal = LauncherEscapeFacts {
            actions_open: false,
            ..all
        };
        assert_eq!(
            plan_launcher_escape(portal),
            LauncherEscapeDecision::CancelAttachmentPortal
        );
        let object = LauncherEscapeFacts {
            attachment_portal_open: false,
            ..portal
        };
        assert_eq!(
            plan_launcher_escape(object),
            LauncherEscapeDecision::CloseObjectSelector
        );
        let picker = LauncherEscapeFacts {
            object_selector_open: false,
            ..object
        };
        assert_eq!(
            plan_launcher_escape(picker),
            LauncherEscapeDecision::CloseTriggerPicker
        );
        let filter = LauncherEscapeFacts {
            trigger_picker_open: false,
            ..picker
        };
        assert_eq!(
            plan_launcher_escape(filter),
            LauncherEscapeDecision::ClearVisibleFilter
        );
        let origin = LauncherEscapeFacts {
            visible_filter: false,
            ..filter
        };
        assert_eq!(
            plan_launcher_escape(origin),
            LauncherEscapeDecision::ReturnToOrigin
        );
        assert_eq!(
            plan_launcher_escape(LauncherEscapeFacts {
                return_to_origin: false,
                ..origin
            }),
            LauncherEscapeDecision::DismissMain,
        );
    }

    #[test]
    fn filter_only_picker_clears_its_visible_filter_without_closing_the_launcher() {
        assert_eq!(
            plan_launcher_escape(LauncherEscapeFacts {
                trigger_picker_open: true,
                trigger_picker_filter_only: true,
                visible_filter: true,
                ..Default::default()
            }),
            LauncherEscapeDecision::ClearVisibleFilter,
        );
    }

    #[test]
    fn every_fact_combination_has_one_deterministic_safe_decision() {
        for mask in 0u8..128 {
            let facts = LauncherEscapeFacts {
                actions_open: mask & 1 != 0,
                attachment_portal_open: mask & 2 != 0,
                object_selector_open: mask & 4 != 0,
                trigger_picker_open: mask & 8 != 0,
                trigger_picker_filter_only: mask & 16 != 0,
                visible_filter: mask & 32 != 0,
                return_to_origin: mask & 64 != 0,
            };
            assert_eq!(plan_launcher_escape(facts), plan_launcher_escape(facts));
            if facts.actions_open {
                assert_eq!(
                    plan_launcher_escape(facts),
                    LauncherEscapeDecision::CloseActions
                );
            }
        }
    }

    #[test]
    fn popup_escape_pops_one_route_but_actions_toggle_closes_any_depth() {
        for depth in 0..16 {
            let expected_escape = if depth > 1 {
                OverlayDismissDecision::PopRoute
            } else {
                OverlayDismissDecision::CloseOverlay
            };
            assert_eq!(
                plan_overlay_dismiss(depth, OverlayDismissTrigger::Escape),
                expected_escape
            );
            assert_eq!(
                plan_overlay_dismiss(depth, OverlayDismissTrigger::ActionsToggle),
                OverlayDismissDecision::CloseOverlay
            );
        }
    }
}
