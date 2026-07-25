//! Shared per-turn conversation action affordances.
//!
//! Owns the response **copy** control that both Flow's `ChatPrompt` and Agent
//! Chat's transcript render. Flow had this control; Agent Chat did not. Rather
//! than author a second one, the existing Flow implementation was extracted
//! here verbatim (metrics now live in
//! [`crate::components::conversation_style::ConversationActionStyle`]) and both
//! surfaces render through it.
//!
//! ## Why eligibility is a pure function
//!
//! Whether a row gets a copy button depends on the message role, whether the
//! body is empty, and whether the turn is still streaming. Encoding that in
//! renderer `if` chains is how the two surfaces drifted in the first place, and
//! it cannot be unit-tested. [`turn_copy_eligibility`] is a total function over
//! those inputs, so every case — including the ones that must NOT show a button
//! — is enumerable in a test without constructing a window.

use gpui::{div, prelude::*, px, rgb, svg, Animation, AnimationExt as _, SharedString};

use crate::components::conversation_style::ConversationStyleDef;
use crate::designs::icon_variations::IconName;

/// What a conversation row should do about a per-turn copy control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnCopyEligibility {
    /// No control at all — there is nothing meaningful to copy yet.
    Absent,
    /// Control shown and clickable.
    Enabled,
    /// Control shown and clickable, with the streaming activity dot.
    EnabledStreaming,
}

impl TurnCopyEligibility {
    pub(crate) fn is_present(self) -> bool {
        !matches!(self, Self::Absent)
    }

    pub(crate) fn shows_activity_dot(self) -> bool {
        matches!(self, Self::EnabledStreaming)
    }
}

/// Role of the conversation row being considered, reduced to the only
/// distinction the copy affordance cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnCopyRole {
    /// An ordinary assistant answer row.
    Assistant,
    /// Thought, tool, system, and error rows. These carry diagnostics or
    /// internal state, not an answer the user asked for, so they never get a
    /// response-copy control.
    NonAnswer,
}

/// Decide whether a conversation row shows a per-turn copy control.
///
/// Total over its inputs so the negative cases are testable. In particular an
/// empty pending assistant row must be `Absent`: a visible copy button that
/// silently copies nothing is worse than no button, and it is the state a row
/// sits in for the entire gap between submit and first token.
pub(crate) fn turn_copy_eligibility(
    role: TurnCopyRole,
    body_is_empty: bool,
    is_streaming: bool,
) -> TurnCopyEligibility {
    match role {
        TurnCopyRole::NonAnswer => TurnCopyEligibility::Absent,
        TurnCopyRole::Assistant => {
            if body_is_empty {
                // Nothing to put on the pasteboard yet, streaming or not.
                TurnCopyEligibility::Absent
            } else if is_streaming {
                // A partial answer IS copyable — the user may want the part
                // already on screen — and the dot signals more is coming.
                TurnCopyEligibility::EnabledStreaming
            } else {
                TurnCopyEligibility::Enabled
            }
        }
    }
}

/// Everything a surface must supply to render the shared copy control.
pub(crate) struct ConversationCopyButtonSpec {
    /// GPUI element id. Must be stable across frames for the same row.
    pub id: SharedString,
    /// Semantic id projected for `getElements`/probes.
    pub fidelity_id: SharedString,
    /// Semantic id for the streaming dot, projected separately so a probe can
    /// distinguish "copy present" from "copy present and still streaming".
    pub activity_fidelity_id: SharedString,
    pub eligibility: TurnCopyEligibility,
    /// Animation key discriminator, so two rows' dots animate independently.
    pub animation_index: usize,
}

/// Render the shared per-turn copy control.
///
/// Returns `None` when the row is not eligible, so callers cannot accidentally
/// paint a disabled-looking button for a row that has nothing to copy.
pub(crate) fn render_conversation_copy_button(
    spec: ConversationCopyButtonSpec,
    style: &ConversationStyleDef,
    theme: &crate::theme::Theme,
    on_copy: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Option<gpui::Stateful<gpui::Div>> {
    if !spec.eligibility.is_present() {
        return None;
    }

    let actions = style.actions;
    let theme_colors = &theme.colors;
    let hover_bg = crate::theme::hover_overlay_bg(theme, actions.button_hover_bg_alpha as u8);
    let hover_opacity = actions.button_hover_opacity;
    let accent = theme_colors.accent.selected;
    let icon_color = theme_colors.text.secondary;
    let activity_fidelity_id = spec.activity_fidelity_id.clone();
    let animation_index = spec.animation_index;

    let control = div()
        .id(spec.id)
        .debug_selector(move || spec.fidelity_id.to_string())
        .relative()
        .flex()
        .items_center()
        .justify_center()
        .w(px(actions.button_size))
        .h(px(actions.button_size))
        .rounded(px(actions.button_radius))
        .cursor_pointer()
        .opacity(actions.button_opacity)
        .hover(move |s| s.opacity(hover_opacity).bg(hover_bg))
        .child(
            svg()
                .path(IconName::Copy.asset_path())
                .size(px(actions.icon_size))
                .text_color(rgb(icon_color)),
        )
        .when(spec.eligibility.shows_activity_dot(), move |slot| {
            slot.child(
                div()
                    .debug_selector(move || activity_fidelity_id.to_string())
                    .absolute()
                    .right(px(actions.activity_dot_inset))
                    .bottom(px(actions.activity_dot_inset))
                    .size(px(actions.activity_dot_size))
                    .rounded(px(999.0))
                    .bg(rgb(accent))
                    .with_animation(
                        ("conversation-turn-streaming-dot-pulse", animation_index),
                        Animation::new(std::time::Duration::from_millis(actions.activity_pulse_ms))
                            .repeat(),
                        |style, delta| {
                            let sine = (delta * std::f32::consts::PI * 2.0).sin();
                            style.opacity(0.65 + (0.35 * ((sine + 1.0) / 2.0)))
                        },
                    ),
            )
        })
        .on_click(move |event, window, cx| on_copy(event, window, cx));

    Some(control)
}

#[cfg(test)]
mod conversation_actions_tests {
    use super::*;

    #[test]
    fn completed_assistant_answer_has_turn_copy() {
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, false, false),
            TurnCopyEligibility::Enabled
        );
    }

    #[test]
    fn streaming_partial_answer_has_turn_copy_with_activity_dot() {
        let eligibility = turn_copy_eligibility(TurnCopyRole::Assistant, false, true);
        assert_eq!(eligibility, TurnCopyEligibility::EnabledStreaming);
        assert!(eligibility.is_present());
        assert!(eligibility.shows_activity_dot());
    }

    /// The row sits in this state for the whole gap between submit and first
    /// token. A button here would copy an empty string while looking like it
    /// worked.
    #[test]
    fn empty_pending_assistant_row_has_no_turn_copy() {
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, true, true),
            TurnCopyEligibility::Absent
        );
        assert_eq!(
            turn_copy_eligibility(TurnCopyRole::Assistant, true, false),
            TurnCopyEligibility::Absent
        );
    }

    #[test]
    fn thought_tool_system_and_error_rows_have_no_turn_copy() {
        for streaming in [true, false] {
            for empty in [true, false] {
                assert_eq!(
                    turn_copy_eligibility(TurnCopyRole::NonAnswer, empty, streaming),
                    TurnCopyEligibility::Absent,
                    "non-answer rows never expose response copy \
                     (empty={empty}, streaming={streaming})"
                );
            }
        }
    }

    /// Exhaustive: every (role, empty, streaming) combination is decided, and
    /// exactly the two non-empty assistant cases are present.
    #[test]
    fn eligibility_is_total_and_only_non_empty_assistant_rows_are_present() {
        let mut present = 0;
        for role in [TurnCopyRole::Assistant, TurnCopyRole::NonAnswer] {
            for empty in [true, false] {
                for streaming in [true, false] {
                    if turn_copy_eligibility(role, empty, streaming).is_present() {
                        present += 1;
                        assert_eq!(role, TurnCopyRole::Assistant);
                        assert!(!empty);
                    }
                }
            }
        }
        assert_eq!(present, 2, "only streaming + settled non-empty assistant");
    }

    #[test]
    fn action_metrics_come_from_the_shared_style_owner() {
        let style = crate::components::conversation_style::production_conversation_style();
        // Lifted verbatim from Flow's original control so the port cannot
        // silently resize the hit target.
        assert_eq!(style.actions.button_size, 24.0);
        assert_eq!(style.actions.button_radius, 4.0);
        assert_eq!(style.actions.button_opacity, 0.7);
        assert_eq!(style.actions.icon_size, 16.0);
        assert_eq!(style.actions.activity_dot_size, 7.0);
    }
}
