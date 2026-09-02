use crate::flows::session::FLOW_STOPPED_CAPTION;

/// One display projection for a settled turn, used by BOTH live finalization
/// and restore (Oracle 2026-07-21, WP-A3/A4): raw engine output decorated
/// from the structured outcome. `SessionTurn.assistant` itself stays raw —
/// the caption never enters persistence or the engine rollup.
pub(crate) fn flow_turn_display_assistant(turn: &crate::flows::session::SessionTurn) -> String {
    match turn.outcome {
        crate::flows::session::PersistedTurnOutcome::Ok
        | crate::flows::session::PersistedTurnOutcome::Failed => turn.assistant.clone(),
        crate::flows::session::PersistedTurnOutcome::Stopped => {
            let raw = turn.assistant.as_str();
            // Whitespace-aware caption join: reuse an existing trailing break
            // rather than stacking a second blank line after item boundaries.
            if raw.is_empty() {
                FLOW_STOPPED_CAPTION.to_string()
            } else if raw.ends_with("\n\n") {
                format!("{raw}{FLOW_STOPPED_CAPTION}")
            } else if raw.ends_with('\n') {
                format!("{raw}\n{FLOW_STOPPED_CAPTION}")
            } else {
                format!("{raw}\n\n{FLOW_STOPPED_CAPTION}")
            }
        }
    }
}

/// A settled turn plus the exact suffix the live streaming row still needs.
/// The finalizer computes the display projection ONCE and hands the caller
/// the literal delta, so there is no hidden "finalization only appends"
/// prefix invariant to violate later (Oracle 2026-07-21).
struct FinalizedFlowTurn {
    turn: crate::flows::session::SessionTurn,
    live_suffix: String,
}

fn finalize_flow_session_turn(
    active: crate::flows::session::ActiveTurn,
    outcome: FlowTurnOutcome,
) -> FinalizedFlowTurn {
    use crate::flows::session::PersistedTurnOutcome;

    let (outcome, failure) = match outcome {
        FlowTurnOutcome::Ok => (PersistedTurnOutcome::Ok, None),
        FlowTurnOutcome::Stopped => (PersistedTurnOutcome::Stopped, None),
        FlowTurnOutcome::Failed(record) => (
            PersistedTurnOutcome::Failed,
            Some(crate::flows::session::PersistedAiFailure::from_record(
                &record,
            )),
        ),
    };
    let turn = crate::flows::session::SessionTurn {
        user: active.user_text,
        assistant: active.assistant_acc,
        outcome,
        failure,
    };
    let display = flow_turn_display_assistant(&turn);
    let live_suffix = display[turn.assistant.len()..].to_string();
    FinalizedFlowTurn { turn, live_suffix }
}

/// Footer grammar for a flow session (Oracle audit 2026-07-21, Footer-A):
/// idle = `↵ Send · ⌘K Actions · Esc Background`; working = `⌘. Stop · ⌘K Actions ·
/// Esc Background`. Terminate Runtime is Actions-only: no hidden destructive
/// shortcut is advertised or handled.
///
/// The working row shows Stop because the status text says only THAT the
/// session is busy, never how to make it stop. `⌘.` is already bound
/// ([`FlowSessionKeyAction::Stop`]) and cancels the in-flight turn while the
/// conversation survives — but it was the one live binding the footer never
/// named, so a user watching a runaway turn had `Esc Background` (leave it running)
/// or nothing. Agent Chat has always shown its stop affordance (`Esc Stop`);
/// this closes half of that gap. The two surfaces still stop on DIFFERENT
/// keys — see `docs/specs/one-conversation-experience.md` §2.1.
/// Test-only view of [`flow_session_footer_hints`] so the native footer test
/// can assert the two renderings of this grammar have not drifted apart.
#[cfg(test)]
pub(crate) fn flow_session_footer_hints_for_tests(working: bool) -> Vec<gpui::SharedString> {
    flow_session_footer_hints(working)
}

fn flow_session_footer_hints(working: bool) -> Vec<gpui::SharedString> {
    use crate::components::conversation_actions::{
        flow_conversation_commands, FlowConversationCommand,
    };
    let commands = flow_conversation_commands(working);
    let mut hints = Vec::with_capacity(3);
    for handler in [
        if working {
            FlowConversationCommand::Stop
        } else {
            FlowConversationCommand::Send
        },
        FlowConversationCommand::Background,
    ] {
        if let Some(command) = commands.iter().find(|command| {
            command.handler == handler && command.descriptor.availability.is_enabled()
        }) {
            if let Some(shortcut) = command.descriptor.shortcut {
                hints.push(gpui::SharedString::from(format!(
                    "{shortcut} {}",
                    command.descriptor.label
                )));
            }
        }
    }
    hints.insert(1, gpui::SharedString::from("⌘K Actions"));
    hints
}
