#[cfg(test)]
mod flow_desk_state {
    use super::{resolve_flow_desk_state, FlowDeskState};
    use crate::flows::catalog::RosterStatus;

    fn failure() -> crate::ai::reliability::AppFailureRecord {
        crate::ai::reliability::process_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            crate::ai::reliability::ProcessFailureFacts::RuntimeClosed,
        )
    }

    #[test]
    fn all_seven_states_have_distinct_typed_resolutions() {
        assert_eq!(
            resolve_flow_desk_state(true, RosterStatus::Loading, None, false, 0),
            FlowDeskState::Loading
        );
        assert_eq!(
            resolve_flow_desk_state(false, RosterStatus::Error, Some(failure()), false, 0),
            FlowDeskState::MdflowMissing,
            "missing binary wins over a stale roster error"
        );
        assert_eq!(
            resolve_flow_desk_state(true, RosterStatus::Legacy, None, false, 0),
            FlowDeskState::MdflowIncompatible
        );
        assert!(matches!(
            resolve_flow_desk_state(true, RosterStatus::Error, Some(failure()), false, 0,),
            FlowDeskState::RosterFailed { .. }
        ));
        assert_eq!(
            resolve_flow_desk_state(true, RosterStatus::Ready, None, false, 0),
            FlowDeskState::ReadyEmpty
        );
        assert_eq!(
            resolve_flow_desk_state(true, RosterStatus::Ready, None, true, 0),
            FlowDeskState::NoMatch
        );
        assert_eq!(
            resolve_flow_desk_state(true, RosterStatus::Ready, None, true, 1),
            FlowDeskState::Ready
        );
    }
}

#[cfg(test)]
mod flow_desk_row_descriptor {
    use super::{flow_desk_flow_row_descriptor, mdflow_run_accepted_context, FlowDeskRowVerb};

    fn flow(interactive: bool, workflow: bool) -> crate::flows::model::FlowDescriptor {
        serde_json::from_value(serde_json::json!({
            "id": "project:truthful",
            "path": "/private/canary/flows/truthful.md",
            "source": "project",
            "name": "truthful",
            "description": "Truthful flow",
            "engine": "pi",
            "inputs": [],
            "isWorkflow": workflow,
            "interactive": interactive,
            "mtimeMs": 0
        }))
        .expect("flow descriptor")
    }

    #[test]
    fn flow_kinds_expose_only_performable_primary_and_secondary_verbs() {
        let conversational = flow_desk_flow_row_descriptor(&flow(false, false));
        assert_eq!(conversational.primary, FlowDeskRowVerb::Converse);
        assert_eq!(conversational.secondary, Some(FlowDeskRowVerb::RunOnce));

        let workflow = flow_desk_flow_row_descriptor(&flow(false, true));
        assert_eq!(workflow.primary, FlowDeskRowVerb::RunOnce);
        assert_eq!(workflow.secondary, None);

        let interactive = flow_desk_flow_row_descriptor(&flow(true, false));
        assert_eq!(interactive.primary, FlowDeskRowVerb::OpenInTerminal);
        assert_eq!(interactive.secondary, None);
    }

    #[test]
    fn fast_terminal_mdflow_phases_prove_context_acceptance() {
        use crate::flows::model::RunPhase;
        assert!(!mdflow_run_accepted_context(RunPhase::Starting));
        assert!(mdflow_run_accepted_context(RunPhase::Running));
        assert!(mdflow_run_accepted_context(RunPhase::Succeeded));
        assert!(mdflow_run_accepted_context(RunPhase::Cancelled));
        assert!(!mdflow_run_accepted_context(RunPhase::Failed));
        assert!(!mdflow_run_accepted_context(RunPhase::Cancelling));
    }
}

#[cfg(test)]
mod flow_session_return_route_model {
    use super::{flow_conversation_return_route_kind, FlowConversationReturnRoute};

    #[test]
    fn direct_route_has_an_explicit_non_desk_identity() {
        assert_eq!(
            flow_conversation_return_route_kind(&FlowConversationReturnRoute::Direct),
            "direct"
        );
    }
}

/// C-R1: the flow session is the single exhaustive key owner. WP7 deleted
/// ChatPrompt's key handling for transcript-only hosts, so this resolver must
/// cover every binding — and its Enter branch must reject modified Enter so
/// Shift+Enter / Cmd+Enter never silently submit.
#[cfg(test)]
mod flow_session_key_owner {
    use super::{resolve_flow_session_key_action, FlowSessionKeyAction};

    /// (key, platform, shift, turn_active, actions_open) → action.
    fn action(
        key: &str,
        platform: bool,
        shift: bool,
        turn_active: bool,
        actions_open: bool,
    ) -> FlowSessionKeyAction {
        resolve_flow_session_key_action(
            key,
            platform,
            shift,
            crate::components::conversation_actions::FlowConversationCommandFacts {
                response_in_progress: turn_active,
                selected_has_response: true,
                composer_has_text: true,
                runtime_attached: true,
                ..Default::default()
            },
            actions_open,
        )
    }

    #[test]
    fn shift_cmd_escape_has_no_destructive_owner() {
        assert_eq!(
            action("escape", true, true, false, false),
            FlowSessionKeyAction::Ignore
        );
        assert_eq!(
            action("escape", true, true, true, false),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn plain_escape_backgrounds() {
        assert_eq!(
            action("escape", false, false, false, false),
            FlowSessionKeyAction::Background
        );
        // Modified Escape has no hidden lifecycle owner. It must never
        // terminate; only plain Escape owns Background.
        assert_eq!(
            action("escape", true, false, false, false),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn escape_ignored_while_actions_open() {
        // The Actions popup owns plain Escape and consumes modified Escape
        // while it is open; neither route terminates the Flow runtime.
        assert_eq!(
            action("escape", false, false, false, true),
            FlowSessionKeyAction::Ignore
        );
        assert_eq!(
            action("escape", true, true, false, true),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn cmd_period_stops_only_while_turn_active() {
        assert_eq!(
            action(".", true, false, true, false),
            FlowSessionKeyAction::Stop
        );
        // No turn in flight ⇒ nothing to stop.
        assert_eq!(
            action(".", true, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        // Bare `.` (no Cmd) is composer input.
        assert_eq!(
            action(".", false, false, true, false),
            FlowSessionKeyAction::Ignore
        );
    }

    /// ⌘L is Agent Chat's new-conversation chord. Flow now answers it too.
    ///
    /// It resolves REGARDLESS of `turn_active` on purpose: the "is this
    /// allowed right now" rule lives in `start_fresh_flow_conversation`, which
    /// both the chord and the ⌘K action call. Duplicating the guard here would
    /// give the two entry points two chances to disagree — and the key must be
    /// consumed either way, or a refused ⌘L types an "l" into the composer.
    #[test]
    fn cmd_l_starts_a_new_conversation_whether_or_not_a_turn_is_running() {
        assert_eq!(
            action("l", true, false, false, false),
            FlowSessionKeyAction::NewConversation
        );
        assert_eq!(
            action("L", true, false, true, false),
            FlowSessionKeyAction::NewConversation
        );
    }

    #[test]
    fn cmd_l_is_not_a_new_conversation_when_shifted_or_bare_or_popup_open() {
        // ⇧⌘L belongs to other surfaces (Notes uses it); do not claim it.
        assert_eq!(
            action("l", true, true, false, false),
            FlowSessionKeyAction::Ignore
        );
        // Bare `l` is composer input — the everyday case, and the one a
        // greedy binding would break on every word containing an l.
        assert_eq!(
            action("l", false, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        // The popup owns the keyboard while it is up.
        assert_eq!(
            action("l", true, false, false, true),
            FlowSessionKeyAction::Ignore
        );
    }

    /// The regression this arm was added for: `flow_desk_session_copy_last_response`
    /// shipped with a `⇧⌘C` badge in the ⌘K list and no arm here, so the badge
    /// was decorative — the action worked by clicking and the chord did
    /// nothing. `resolve_flow_session_key_action` is documented as "the single
    /// exhaustive key owner", which is exactly why an advertised-but-unowned
    /// chord is invisible: nothing else is looking.
    #[test]
    fn shift_cmd_c_copies_the_last_response_like_agent_chat() {
        assert_eq!(
            action("c", true, true, false, false),
            FlowSessionKeyAction::CopyLastResponse
        );
        // Case-insensitive: a shifted `c` may arrive as "C".
        assert_eq!(
            action("C", true, true, false, false),
            FlowSessionKeyAction::CopyLastResponse
        );
        // Resolves mid-turn too. There is always something to copy from an
        // earlier turn, and the handler owns the empty case.
        assert_eq!(
            action("c", true, true, true, false),
            FlowSessionKeyAction::CopyLastResponse
        );
    }

    /// Unshifted ⌘C must stay the platform's copy-the-selection. Flow answers
    /// render through the shared selectable `TextView` now, so claiming plain
    /// ⌘C would silently replace "copy the paragraph I highlighted" with "copy
    /// the whole answer" — the user's selection would be ignored, not honored.
    #[test]
    fn plain_cmd_c_is_left_to_the_text_selection() {
        assert_eq!(
            action("c", true, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        // Bare `c` is composer input.
        assert_eq!(
            action("c", false, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        // ⇧C without Cmd is a capital C the user is typing.
        assert_eq!(
            action("c", false, true, false, false),
            FlowSessionKeyAction::Ignore
        );
        // The popup owns the keyboard while it is up, same as ⌘L and ⎋.
        assert_eq!(
            action("c", true, true, false, true),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn cmd_k_toggles_actions_regardless_of_state() {
        assert_eq!(
            action("k", true, false, false, false),
            FlowSessionKeyAction::ToggleActions
        );
        assert_eq!(
            action("K", true, false, true, true),
            FlowSessionKeyAction::ToggleActions
        );
        // Bare `k` is composer input.
        assert_eq!(
            action("k", false, false, false, false),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn only_unmodified_enter_submits() {
        assert_eq!(
            action("enter", false, false, false, false),
            FlowSessionKeyAction::Submit
        );
        assert_eq!(
            action("return", false, false, false, false),
            FlowSessionKeyAction::Submit
        );
        // Shift+Enter and Cmd+Enter MUST NOT submit (regression: the old
        // handler submitted on any Enter). They fall through to the composer.
        assert_eq!(
            action("enter", false, true, false, false),
            FlowSessionKeyAction::Ignore
        );
        assert_eq!(
            action("enter", true, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        assert_eq!(
            action("enter", true, true, false, false),
            FlowSessionKeyAction::Ignore
        );
    }

    #[test]
    fn ordinary_typed_keys_fall_through() {
        assert_eq!(
            action("a", false, false, false, false),
            FlowSessionKeyAction::Ignore
        );
        assert_eq!(
            action("1", false, false, true, false),
            FlowSessionKeyAction::Ignore
        );
    }
}

#[cfg(test)]
mod flow_session_footer_and_finalize {
    use super::{
        finalize_flow_session_turn, flow_session_footer_hints, flow_turn_display_assistant,
        resolve_flow_session_key_action, FlowSessionKeyAction, FlowTurnOutcome,
        FLOW_STOPPED_CAPTION,
    };
    use crate::flows::session::{ActiveTurn, PersistedTurnOutcome, SessionTurn};

    fn active_turn(assistant_acc: &str) -> ActiveTurn {
        ActiveTurn {
            run_id: None,
            message_id: "message".into(),
            assistant_acc: assistant_acc.to_string(),
            current_item_id: None,
            item_acc: String::new(),
            user_text: "hello".into(),
        }
    }

    /// Footer grammar (Oracle 2026-07-21 adjudication): idle is exactly
    /// `↵ Send · ⌘K Actions · Esc Background`; working is exactly
    /// `⌘. Stop · ⌘K Actions · Esc Background` — no Send while busy, no permanent
    /// Terminate.
    ///
    /// The working row used to omit Stop entirely, so the one key that cancels
    /// a runaway turn was the only live binding the footer never named.
    #[test]
    fn flow_session_footer_matches_idle_and_working_grammar() {
        let idle = flow_session_footer_hints(false);
        let idle: Vec<&str> = idle.iter().map(|h| h.as_ref()).collect();
        assert_eq!(idle, vec!["↵ Send", "⌘K Actions", "Esc Background"]);

        let working = flow_session_footer_hints(true);
        let working: Vec<&str> = working.iter().map(|h| h.as_ref()).collect();
        assert_eq!(working, vec!["⌘. Stop", "⌘K Actions", "Esc Background"]);
    }

    /// Every hint the working footer shows must name a key the flow session
    /// actually resolves. A hint whose key resolves to nothing (or to a
    /// different action than its label claims) is a lie the user only
    /// discovers mid-runaway-turn.
    #[test]
    fn every_working_footer_hint_names_a_key_the_session_really_binds() {
        let facts = crate::components::conversation_actions::FlowConversationCommandFacts {
            response_in_progress: true,
            composer_has_text: true,
            runtime_attached: true,
            ..Default::default()
        };
        assert_eq!(
            resolve_flow_session_key_action(".", true, false, facts, false),
            FlowSessionKeyAction::Stop,
            "the footer promises ⌘. Stop while working"
        );
        assert_eq!(
            resolve_flow_session_key_action("k", true, false, facts, false),
            FlowSessionKeyAction::ToggleActions,
            "the footer promises ⌘K Actions"
        );
        assert_eq!(
            resolve_flow_session_key_action("escape", false, false, facts, false),
            FlowSessionKeyAction::Background,
            "the footer promises Escape leaves the session"
        );
    }

    /// Up/Down must feel like shell history, because that is what a user's
    /// fingers already expect. Flow had no recall at all: the arrow
    /// interceptor had an arm for the desk but none for a live session, so
    /// tweaking one word of a long prompt meant retyping the whole thing.
    #[test]
    fn prompt_history_recalls_newest_first_and_clamps_at_the_oldest() {
        use crate::FlowPromptHistoryMove::*;

        // From the live draft, Up goes to the NEWEST entry (index 2 of 3),
        // not the oldest.
        assert_eq!(crate::flow_prompt_history_move(3, None, true), Recall(2));
        // Then older, one at a time.
        assert_eq!(crate::flow_prompt_history_move(3, Some(2), true), Recall(1));
        assert_eq!(crate::flow_prompt_history_move(3, Some(1), true), Recall(0));
        // At the oldest, Up stays put rather than wrapping to the newest —
        // wrapping makes a long history feel like it lost your place.
        assert_eq!(crate::flow_prompt_history_move(3, Some(0), true), Recall(0));
    }

    #[test]
    fn arrowing_back_down_returns_to_the_draft_that_was_being_typed() {
        use crate::FlowPromptHistoryMove::*;

        assert_eq!(
            crate::flow_prompt_history_move(3, Some(0), false),
            Recall(1)
        );
        assert_eq!(
            crate::flow_prompt_history_move(3, Some(1), false),
            Recall(2)
        );
        // Past the newest entry is the user's own unsent draft, not an empty
        // composer — recall has to be reversible without retyping.
        assert_eq!(
            crate::flow_prompt_history_move(3, Some(2), false),
            RestoreDraft
        );
        // Already on the draft: nothing newer to go to.
        assert_eq!(crate::flow_prompt_history_move(3, None, false), Ignore);
    }

    /// A brand-new session has nothing to recall, so arrows must fall through
    /// untouched rather than being swallowed by a handler with no history.
    #[test]
    fn an_empty_history_never_claims_the_arrow_keys() {
        assert_eq!(
            crate::flow_prompt_history_move(0, None, true),
            crate::FlowPromptHistoryMove::Ignore
        );
        assert_eq!(
            crate::flow_prompt_history_move(0, None, false),
            crate::FlowPromptHistoryMove::Ignore
        );
        assert_eq!(
            crate::flow_prompt_history_move(0, Some(4), true),
            crate::FlowPromptHistoryMove::Ignore
        );
    }

    /// Every index this returns is used to subscript the history, so an
    /// out-of-range answer would panic or recall the wrong prompt.
    #[test]
    fn every_recalled_index_is_in_range() {
        for len in 1..6usize {
            for current in std::iter::once(None).chain((0..len).map(Some)) {
                for is_up in [true, false] {
                    if let crate::FlowPromptHistoryMove::Recall(index) =
                        crate::flow_prompt_history_move(len, current, is_up)
                    {
                        assert!(
                            index < len,
                            "len={len} current={current:?} is_up={is_up} recalled {index}"
                        );
                    }
                }
            }
        }
    }

    /// WP-B3: a streamed flow turn (many deltas accumulated into
    /// `assistant_acc`, exactly as `append_flow_turn_text` builds it) finalizes
    /// to the EXACT concatenated assistant text, and the display projection
    /// round-trips as `assistant + live_suffix` — so the finalize suffix that
    /// `finish_flow_turn` routes through the child-commit helper is precisely
    /// the tail the live row still needs, with no duplication or loss.
    #[test]
    fn flow_real_stream_preserves_exact_final_text_and_suffix() {
        // Multi-byte graphemes and item breaks split across "deltas".
        let deltas = ["Deploying ", "the ", "café ", "→ 日本語 ", "🚀 done"];
        let mut acc = String::new();
        for delta in deltas {
            acc.push_str(delta);
        }
        let expected: String = deltas.concat();

        // Ok outcome: display == raw assistant, so the suffix is empty.
        let ok = finalize_flow_session_turn(active_turn(&acc), FlowTurnOutcome::Ok);
        assert_eq!(ok.turn.assistant, expected);
        assert_eq!(ok.turn.outcome, PersistedTurnOutcome::Ok);
        assert_eq!(ok.live_suffix, "");
        assert_eq!(
            flow_turn_display_assistant(&ok.turn),
            format!("{}{}", ok.turn.assistant, ok.live_suffix),
            "display must round-trip as assistant + suffix"
        );

        // Stopped outcome: raw text is preserved verbatim and the suffix is the
        // ONLY added tail — assistant + suffix reconstructs the display exactly.
        let stopped = finalize_flow_session_turn(active_turn(&acc), FlowTurnOutcome::Stopped);
        assert_eq!(stopped.turn.assistant, expected);
        assert_eq!(
            format!("{}{}", stopped.turn.assistant, stopped.live_suffix),
            flow_turn_display_assistant(&stopped.turn),
        );
    }

    /// WP-A3: finalization persists RAW assistant text + structured outcome;
    /// the caption is only the live display suffix, never stored content.
    #[test]
    fn finalize_stopped_turn_keeps_assistant_raw() {
        let finalized =
            finalize_flow_session_turn(active_turn("partial answer"), FlowTurnOutcome::Stopped);
        assert_eq!(finalized.turn.assistant, "partial answer");
        assert_eq!(finalized.turn.outcome, PersistedTurnOutcome::Stopped);
        assert_eq!(finalized.turn.failure, None);
        assert_eq!(finalized.live_suffix, format!("\n\n{FLOW_STOPPED_CAPTION}"));

        let empty = finalize_flow_session_turn(active_turn(""), FlowTurnOutcome::Stopped);
        assert_eq!(empty.turn.assistant, "");
        assert_eq!(empty.live_suffix, FLOW_STOPPED_CAPTION);
    }

    /// The display projection reuses an existing trailing paragraph break
    /// instead of stacking a second blank line after item boundaries.
    #[test]
    fn stopped_projection_reuses_existing_newlines() {
        let turn = |raw: &str| SessionTurn {
            user: "u".into(),
            assistant: raw.into(),
            outcome: PersistedTurnOutcome::Stopped,
            failure: None,
        };
        assert_eq!(
            flow_turn_display_assistant(&turn("body\n\n")),
            format!("body\n\n{FLOW_STOPPED_CAPTION}")
        );
        assert_eq!(
            flow_turn_display_assistant(&turn("body\n")),
            format!("body\n\n{FLOW_STOPPED_CAPTION}")
        );
        assert_eq!(
            flow_turn_display_assistant(&turn("body")),
            format!("body\n\n{FLOW_STOPPED_CAPTION}")
        );
        assert_eq!(flow_turn_display_assistant(&turn("")), FLOW_STOPPED_CAPTION);
    }

    /// Structured outcome — not content sniffing — decides decoration: model
    /// output that naturally ends with the caption text still gets the real
    /// caption appended, and an Ok turn is never decorated.
    #[test]
    fn projection_is_outcome_driven_not_content_sniffed() {
        let natural = SessionTurn {
            user: "u".into(),
            assistant: format!("The literal marker is {FLOW_STOPPED_CAPTION}"),
            outcome: PersistedTurnOutcome::Stopped,
            failure: None,
        };
        assert!(
            flow_turn_display_assistant(&natural).ends_with(&format!("\n\n{FLOW_STOPPED_CAPTION}"))
        );

        let ok = SessionTurn {
            user: "u".into(),
            assistant: "done".into(),
            outcome: PersistedTurnOutcome::Ok,
            failure: None,
        };
        assert_eq!(flow_turn_display_assistant(&ok), "done");
    }

    /// S09: a Failed turn persists the TYPED failure projection — safe
    /// summary copy plus stable code/category — and never the raw transport
    /// string; unicode raw text round-trips through the projection safely.
    #[test]
    fn finalize_failed_turn_persists_typed_failure() {
        let record = crate::ai::reliability::provider_failure(
            sk_protocol::ai_reliability::ProtocolComponent::Mdflow,
            "raw transport stderr that must never persist",
        );
        let expected_summary = record.primary_message().to_string();
        let finalized =
            finalize_flow_session_turn(active_turn("partial 🚀"), FlowTurnOutcome::Failed(record));
        assert_eq!(finalized.turn.assistant, "partial 🚀");
        assert_eq!(finalized.turn.outcome, PersistedTurnOutcome::Failed);
        assert_eq!(finalized.live_suffix, "");
        let failure = finalized.turn.failure.as_ref().expect("typed failure");
        assert_eq!(failure.safe_summary, expected_summary);
        assert!(
            !failure.safe_summary.contains("raw transport stderr"),
            "raw transport text must never enter persistence"
        );
        assert!(
            failure.diagnostic_fingerprint.is_some(),
            "redacted diagnostic fingerprint must survive for CopyDetails"
        );
    }

    /// WP-A3: an ordinary completion persists the accumulator verbatim with
    /// no display suffix; unicode stays prefix-safe.
    #[test]
    fn finalize_ok_turn_is_verbatim() {
        let finalized = finalize_flow_session_turn(active_turn("done ✅"), FlowTurnOutcome::Ok);
        assert_eq!(finalized.turn.assistant, "done ✅");
        assert_eq!(finalized.turn.outcome, PersistedTurnOutcome::Ok);
        assert_eq!(finalized.turn.failure, None);
        assert_eq!(finalized.live_suffix, "");

        let stopped_unicode =
            finalize_flow_session_turn(active_turn("emoji 🎯"), FlowTurnOutcome::Stopped);
        assert_eq!(
            flow_turn_display_assistant(&stopped_unicode.turn),
            format!("emoji 🎯{}", stopped_unicode.live_suffix),
            "display must equal raw + live suffix exactly"
        );
    }
}
