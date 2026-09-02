//! Provider-free application-library stories. Test windows use GPUI's test
//! platform; none of these stories proves native rendering or application startup.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::{App, AppContext as _, Context, Entity, IntoElement, Render, Window};
use sk_protocol::ai_reliability::*;

fn selection() -> AiSelectionState {
    let selected = AiModelSelection {
        provider_id: Some(ProviderId::from("fixture-provider")),
        model_id: Some(ModelId::from("fixture-model")),
        profile_id: Some(ProfileId::from("fixture-profile")),
    };
    AiSelectionState {
        requested: Some(selected.clone()),
        effective: Some(selected),
        origin: SelectionOrigin::ExplicitThisTurn,
        acknowledged_change: None,
    }
}

fn work() -> AiWorkSnapshot {
    AiWorkSnapshot {
        key: WorkKey::from("production-story-work"),
        transcript: PreservationReceipt::Preserved {
            fingerprint: Fingerprint::from("fixture-transcript"),
        },
        draft: PreservationReceipt::Restorable {
            fingerprint: Fingerprint::from("fixture-draft"),
        },
        attachments: PreservationReceipt::Preserved {
            fingerprint: Fingerprint::from("fixture-attachments"),
        },
        partial_output: PreservationReceipt::Restorable {
            fingerprint: Fingerprint::from("fixture-partial"),
        },
    }
}

fn submitted() -> AiOperationState {
    let ready = AiOperationState::ready(
        AiSurfaceIdentity::AgentChat {
            profile_id: ProfileId::from("fixture-profile"),
            provider_id: Some(ProviderId::from("fixture-provider")),
            model_id: Some(ModelId::from("fixture-model")),
            cwd_fingerprint: Fingerprint::from("fixture-cwd"),
        },
        selection(),
        work(),
        RetryPolicy {
            automatic_max: 0,
            manual_max: 1,
        },
    );
    let submitted = transition(
        ready,
        AiOperationEvent::SubmitRequested {
            request: TurnRequestRef::from("production-story-request"),
            work: work(),
            selection: selection(),
            risk: TurnRisk::ReadOnly,
        },
    )
    .expect("submit the preserved user turn");
    assert!(matches!(
        submitted.commands.as_slice(),
        [AiCommand::PersistWork(_), AiCommand::CheckCapabilities(_)]
    ));
    submitted.next
}

fn start(state: AiOperationState, turn: &str) -> (AiOperationState, CommandId) {
    let starting = transition(
        state,
        AiOperationEvent::CapabilityResolved(CapabilityDecision::Compatible),
    )
    .expect("compatible preflight");
    let command_id = match starting.commands.as_slice() {
        [AiCommand::PersistWork(saved), AiCommand::StartTurn(command)] => {
            assert_eq!(saved, &work());
            assert_eq!(
                command.request,
                TurnRequestRef::from("production-story-request")
            );
            assert_eq!(command.work_key, work().key);
            assert_eq!(command.selection, selection());
            command.command_id
        }
        commands => panic!("expected persist then start, got {commands:?}"),
    };
    let wrong = transition(
        starting.next.clone(),
        AiOperationEvent::RuntimeStarted {
            command_id: CommandId(command_id.0 + 1),
            turn: TurnRef::from("wrong-turn"),
        },
    )
    .expect_err("wrong start acknowledgement must be rejected");
    assert_eq!(wrong.reason, InvalidTransitionReason::CommandIdMismatch);
    let running = transition(
        starting.next,
        AiOperationEvent::RuntimeStarted {
            command_id,
            turn: TurnRef::from(turn),
        },
    )
    .expect("matching runtime acknowledgement");
    assert!(running.commands.is_empty());
    assert_eq!(running.next.phase.tag(), AiPhaseTag::Running);
    (running.next, command_id)
}

#[test]
fn production_story_launcher_filtering_policy() {
    use crate::launch_filter_policy::{
        filter_change_flips_list_structure as flips,
        menu_syntax_filter_only_escape_should_clear as clears,
    };

    for (before, after, expected) in [
        ("", "fruit", true),
        ("fr", "fruit", false),
        ("fruit", "", true),
        ("fruit", "@file:", true),
        ("@file:", "@file:readme", false),
        ("@", "/", true),
        ("has", "has:", true),
        ("has:", "has", true),
        ("  @file:", "@file:readme", false),
    ] {
        assert_eq!(flips(before, after), expected, "{before:?} -> {after:?}");
    }
    for (input, expected) in [
        (":", true),
        (":type:", true),
        ("type:script", true),
        ("type:script fruit", false),
        ("fruit picker", false),
        ("", false),
        ("   ", false),
    ] {
        let mode = crate::menu_syntax::MenuSyntaxMode::from_input(input);
        assert_eq!(clears(input, &mode), expected, "{input:?}");
    }
}

#[test]
fn production_story_prompt_action_selection() {
    use crate::actions::{Action, ActionCategory, ActionsDialog, ActionsDialogConfig};
    use crate::protocol::ElementType;
    use crate::windows::automation_surface_collector::collect_actions_dialog_elements;

    let callbacks = Arc::new(Mutex::new(Vec::<String>::new()));
    let recorded = Arc::clone(&callbacks);
    let callback: Arc<dyn Fn(String) + Send + Sync> = Arc::new(move |id| {
        recorded.lock().expect("callback recorder").push(id);
    });
    let test = gpui::TestAppContext::single();
    let dialog = test.update(|cx| {
        gpui_component::init(cx);
        cx.new(|cx| {
            ActionsDialog::from_actions_with_context(
                cx.focus_handle(),
                callback,
                vec![
                    Action::new(
                        "story-first",
                        "Duplicate",
                        None,
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Story actions"),
                    Action::new(
                        "story-second",
                        "Duplicate",
                        None,
                        ActionCategory::ScriptContext,
                    )
                    .with_section("Story actions"),
                ],
                None,
                None,
                Arc::new(crate::theme::Theme::default()),
                crate::designs::DesignVariant::Default,
                None,
                ActionsDialogConfig::default(),
            )
        })
    });
    test.update(|cx| {
        assert_eq!(
            dialog.read(cx).get_selected_action_id().as_deref(),
            Some("story-first")
        );
        assert_eq!(
            dialog.update(cx, |dialog, cx| dialog
                .select_action_by_id("story-second", cx)),
            Some("story-second".to_string())
        );
        let selected_index = dialog.read(cx).selected_index.expect("selected visual row");
        assert!(
            selected_index > 1,
            "section header must precede the two choices"
        );
        let snapshot = collect_actions_dialog_elements(&dialog, 100, cx);
        let selected = snapshot
            .elements
            .iter()
            .filter(|node| node.element_type == ElementType::Choice && node.selected == Some(true))
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].value.as_deref(), Some("story-second"));
        let semantic_id = selected[0].semantic_id.clone();
        assert_eq!(
            snapshot.selected_semantic_id.as_deref(),
            Some(semantic_id.as_str())
        );

        dialog.update(cx, |dialog, cx| {
            assert_eq!(
                dialog.select_action_by_id("story-first", cx).as_deref(),
                Some("story-first")
            );
            assert_eq!(
                dialog.select_action_by_semantic_id(&semantic_id, cx),
                Some(semantic_id.clone())
            );
            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("story-second")
            );
            assert_eq!(dialog.select_action_by_id("story-absent", cx), None);
            let mismatched_id = format!("choice:{selected_index}:story-first");
            assert_eq!(
                dialog.select_action_by_semantic_id(&mismatched_id, cx),
                None
            );
            assert_eq!(
                dialog.select_action_by_semantic_id("choice:0:story-second", cx),
                None
            );
            assert_eq!(
                dialog.select_action_by_semantic_id("choice:999:story-second", cx),
                None
            );
            assert_eq!(
                dialog.select_action_by_semantic_id("input:actions-search", cx),
                None
            );
            assert_eq!(
                dialog.get_selected_action_id().as_deref(),
                Some("story-second")
            );
        });
    });
    assert!(
        callbacks.lock().expect("callback recorder").is_empty(),
        "selection is not activation"
    );
}

struct StoryButton {
    focus: gpui::FocusHandle,
    clicks: Rc<Cell<usize>>,
    disabled: bool,
    loading: bool,
}

impl Render for StoryButton {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let clicks = self.clicks.clone();
        crate::components::Button::new(
            "production-story:button",
            "Continue",
            crate::components::ButtonColors::default(),
        )
        .focus_handle(self.focus.clone())
        .disabled(self.disabled)
        .loading(self.loading)
        .on_click(Box::new(
            move |_: &gpui::ClickEvent, _: &mut Window, _: &mut App| {
                clicks.set(clicks.get() + 1);
            },
        ))
    }
}

#[test]
fn production_story_prompt_button_dispatch() {
    let clicks = Rc::new(Cell::new(0));
    let recorded = clicks.clone();
    let mut test = gpui::TestAppContext::single();
    let window = test.update(|cx| {
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|cx| StoryButton {
                focus: cx.focus_handle(),
                clicks: recorded,
                disabled: false,
                loading: false,
            })
        })
        .expect("test-platform button window")
    });
    window
        .update(&mut test, |button, window, cx| {
            window.focus(&button.focus, cx)
        })
        .expect("focus test-platform button");

    for (disabled, loading, expected) in [
        (false, false, 2),
        (true, false, 2),
        (false, true, 2),
        (false, false, 4),
    ] {
        window
            .update(&mut test, |button, _, cx| {
                button.disabled = disabled;
                button.loading = loading;
                cx.notify();
            })
            .expect("update button interactivity");
        for key in ["enter", "space", "a"] {
            test.dispatch_keystroke(*window, gpui::Keystroke::parse(key).expect("story key"));
        }
        assert_eq!(
            clicks.get(),
            expected,
            "disabled={disabled}, loading={loading}"
        );
    }
}

#[test]
fn production_story_ai_failure_retry_recovery() {
    use crate::ai::reliability::{project_recovery, SurfaceRecoveryCapabilities};

    let (running, first_start) = start(submitted(), "production-story-turn-1");
    let failure = AiFailure::new(
        AiFailureKind::Connectivity(ConnectivityFailure::Timeout),
        RetrySafety::SameSelectionReadOnly,
    );
    let failed =
        transition(running, AiOperationEvent::Failed(failure.clone())).expect("typed failure");
    assert_eq!(failed.next.phase.tag(), AiPhaseTag::AwaitingRecovery);
    assert!(failed.commands.is_empty(), "no automatic provider retry");
    assert_eq!(failed.next.work, work());
    assert_eq!(failed.next.selection, selection());
    let capabilities = SurfaceRecoveryCapabilities::only([
        RecoveryActionKind::Retry,
        RecoveryActionKind::CheckAgain,
    ]);
    let card = project_recovery(&failed.next.identity, &failed.next, &capabilities)
        .expect("production recovery projection");
    assert_eq!(card.title.as_ref(), "AI request timed out");
    assert_eq!(
        card.actions
            .iter()
            .filter(|action| { action.semantic_id == "ai-recovery-retry" && action.enabled })
            .count(),
        1
    );
    let semantic_tree = crate::components::recovery_semantic_tree(&card);
    assert_eq!(
        semantic_tree
            .iter()
            .filter(|node| { node.semantic_id == "ai-recovery-retry" && node.enabled })
            .count(),
        1
    );

    let retrying = transition(
        failed.next,
        AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
    )
    .expect("manual retry");
    let backoff_id = match retrying.commands.as_slice() {
        [AiCommand::PersistWork(saved), AiCommand::ScheduleBackoff {
            command_id,
            attempt,
            ..
        }] => {
            assert_eq!(saved, &work());
            assert_eq!(*attempt, RetryAttempt::Manual(1));
            *command_id
        }
        commands => panic!("expected preserved manual retry, got {commands:?}"),
    };
    assert_eq!(retrying.next.retry.manual_used, 1);
    assert_eq!(retrying.next.selection, selection());
    let stale = transition(
        retrying.next.clone(),
        AiOperationEvent::BackoffElapsed {
            command_id: CommandId(0),
        },
    )
    .expect_err("stale callback must not start a turn");
    assert_eq!(stale.reason, InvalidTransitionReason::CommandIdMismatch);
    let preflight = transition(
        retrying.next,
        AiOperationEvent::BackoffElapsed {
            command_id: backoff_id,
        },
    )
    .expect("matching backoff callback");
    let (running_again, second_start) = start(preflight.next, "production-story-turn-2");
    assert_ne!(first_start, second_start);
    assert_eq!(running_again.work, work());
    assert_eq!(running_again.selection, selection());

    // The same failed retry cannot silently consume a second manual attempt.
    let exhausted = transition(running_again.clone(), AiOperationEvent::Failed(failure))
        .expect("second failure");
    let refused = transition(
        exhausted.next,
        AiOperationEvent::RecoverySelected(AiRecoveryAction::Retry),
    )
    .expect_err("manual retry budget exhausted");
    assert_eq!(
        refused.reason,
        InvalidTransitionReason::RetryBudgetExhausted
    );

    let succeeded = transition(
        running_again,
        AiOperationEvent::Completed(CompletionKind::Complete),
    )
    .expect("completion");
    assert_eq!(succeeded.next.phase.tag(), AiPhaseTag::Succeeded);
    assert_eq!(
        succeeded.outcome,
        Some(AiOutcome::Succeeded {
            completeness: CompletionKind::Complete
        })
    );
    assert!(succeeded.commands.is_empty());
    assert!(succeeded.next.pending.is_none());
    assert_eq!(succeeded.next.work, work());
    assert!(project_recovery(&succeeded.next.identity, &succeeded.next, &capabilities).is_none());
}

#[test]
fn production_story_ai_stop_cancel() {
    for partial in [
        PartialOutputState::None,
        PartialOutputState::Preserved {
            fingerprint: Fingerprint::from("fixture-partial"),
        },
    ] {
        let (running, _) = start(submitted(), "production-story-stop-turn");
        let stopping = transition(running, AiOperationEvent::StopRequested).expect("explicit Stop");
        assert_eq!(
            stopping.commands,
            vec![AiCommand::CancelTurn {
                turn: TurnRef::from("production-story-stop-turn")
            }]
        );
        assert_eq!(stopping.next.phase.tag(), AiPhaseTag::Cancelling);
        assert!(
            stopping.outcome.is_none(),
            "Stop waits for runtime acknowledgement"
        );
        assert!(
            transition(
                stopping.next.clone(),
                AiOperationEvent::RuntimeCancelled {
                    partial: partial.clone()
                }
            )
            .is_err(),
            "legacy cancellation cannot settle explicit Stop"
        );
        assert!(
            transition(
                stopping.next.clone(),
                AiOperationEvent::Completed(CompletionKind::Complete)
            )
            .is_err(),
            "late completion cannot override Stop"
        );
        let stopped = transition(
            stopping.next,
            AiOperationEvent::RuntimeStopped {
                partial: partial.clone(),
            },
        )
        .expect("matching Stop acknowledgement");
        assert_eq!(
            stopped.outcome,
            Some(AiOutcome::Cancelled {
                kind: CancellationKind::UserStopped,
                partial
            })
        );
        assert!(stopped.commands.is_empty());
        assert!(stopped.next.pending.is_none());
        assert_eq!(stopped.next.work, work());
        assert_eq!(stopped.next.selection, selection());
        assert!(
            crate::ai::reliability::project_recovery(
                &stopped.next.identity,
                &stopped.next,
                &crate::ai::reliability::SurfaceRecoveryCapabilities::only([]),
            )
            .is_none(),
            "cancellation is not a failure card"
        );
    }
    let (running, _) = start(submitted(), "production-story-cancel-turn");
    let cancelling = transition(running, AiOperationEvent::CancelRequested).expect("legacy cancel");
    assert!(transition(
        cancelling.next.clone(),
        AiOperationEvent::RuntimeStopped {
            partial: PartialOutputState::None
        }
    )
    .is_err());
    let cancelled = transition(
        cancelling.next,
        AiOperationEvent::RuntimeCancelled {
            partial: PartialOutputState::None,
        },
    )
    .expect("matching legacy cancellation");
    assert_eq!(
        cancelled.outcome,
        Some(AiOutcome::Cancelled {
            kind: CancellationKind::UserCancelled,
            partial: PartialOutputState::None,
        })
    );
}

struct StoryNotesEditor {
    editor: Entity<crate::components::notes_editor::NotesEditor>,
}

impl Render for StoryNotesEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.editor.read(cx).render_input(cx)
    }
}

#[test]
fn production_story_notes_edit_and_selection() {
    use crate::components::notes_editor::{NotesEditor, NotesEditorMarkdownConfig};
    use crate::notes::{Note, NoteId, NotesApp};

    let notes = vec![
        Note::with_content("First fixture"),
        Note::with_content("Second fixture"),
    ];
    let second = notes[1].id;
    assert_eq!(
        NotesApp::resolve_selected_note(Some(second), &notes).map(|(id, note)| (id, note.id)),
        Some((second, second))
    );
    assert!(NotesApp::resolve_selected_note(Some(NoteId::new()), &notes).is_none());
    assert!(NotesApp::resolve_selected_note(None, &notes).is_none());

    let mut test = gpui::TestAppContext::single();
    let window = test.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| {
                let (_, editor) = NotesEditor::new_markdown_pair(
                    window,
                    cx,
                    NotesEditorMarkdownConfig::new("- [ ] café\nsecond line"),
                );
                StoryNotesEditor { editor }
            })
        })
        .expect("test-platform Notes editor window")
    });
    window
        .update(&mut test, |host, window, cx| {
            host.editor.update(cx, |editor, cx| {
                let original = "- [ ] café\nsecond line";
                editor.set_selection(6, 11, window, cx);
                assert_eq!(editor.selection(cx), 6..11);
                assert_eq!(editor.content(cx), original);
                assert!(editor.toggle_task_marker_at(2..5, false, window, cx));
                assert_eq!(editor.content(cx), "- [x] café\nsecond line");
                assert_eq!(
                    editor.selection(cx),
                    6..11,
                    "checkbox mutation must preserve selected text"
                );
                for invalid in [2..5, 10..11, 0..100] {
                    assert!(!editor.toggle_task_marker_at(invalid, false, window, cx));
                    assert_eq!(editor.content(cx), "- [x] café\nsecond line");
                    assert_eq!(editor.selection(cx), 6..11);
                }
                assert!(editor.toggle_task_marker_at(2..5, true, window, cx));
                assert_eq!(editor.content(cx), original);
                assert_eq!(editor.selection(cx), 6..11);
            });
        })
        .expect("mutate real Notes editor");
}

#[test]
fn production_story_dictation_target_refusal() {
    use crate::dictation::DictationTarget;
    use crate::dictation::{
        resolve_delivery_target_request, resolve_dictation_target_label,
        DictationDeliveryTargetResolution as Resolution, DictationDeliveryTargetSource as Source,
        DictationWrongTargetReason as Reason, DictationWrongTargetRefusalDraft,
    };

    assert_eq!(
        resolve_delivery_target_request(
            Some("mainWindowFilter"),
            Some(DictationTarget::NotesEditor),
            7
        ),
        Resolution::Deliver {
            target: DictationTarget::MainWindowFilter,
            source: Source::ExplicitLabel
        }
    );
    assert_eq!(
        resolve_delivery_target_request(None, Some(DictationTarget::NotesEditor), 7),
        Resolution::Deliver {
            target: DictationTarget::NotesEditor,
            source: Source::ActiveSession
        }
    );
    assert_eq!(
        resolve_delivery_target_request(
            Some("definitely-missing"),
            Some(DictationTarget::MainWindowFilter),
            7
        ),
        Resolution::Refuse(DictationWrongTargetRefusalDraft {
            reason: Reason::UnknownTargetLabel,
            requested_target_label: Some("definitely-missing".to_string()),
            requested_target: None,
            delivery_generation_before: 7,
        })
    );
    assert_eq!(
        resolve_delivery_target_request(None, None, 7),
        Resolution::Refuse(DictationWrongTargetRefusalDraft {
            reason: Reason::TargetUnavailable,
            requested_target_label: None,
            requested_target: None,
            delivery_generation_before: 7,
        })
    );
    let migrated =
        resolve_dictation_target_label("aiChatComposer").expect("legacy target migration");
    assert_eq!(migrated.target, DictationTarget::TabAiHarness);
    assert!(migrated.migrated_legacy_ai_chat);
    assert!(
        !resolve_dictation_target_label("agentChat")
            .expect("current target")
            .migrated_legacy_ai_chat
    );
}

#[test]
fn production_story_conversation_portal_contract() {
    use crate::ai::agent_chat::ui::portal_contract::{
        apply_portal_replacement, clear_terminal_portal_state, decide_portal_open,
        exact_replacement_target_for_range, next_portal_state,
        AgentChatPortalOpenRefusal as Refusal, AgentChatPortalSessionEvent as Event,
        AgentChatPortalSessionState as State,
    };

    assert_eq!(
        decide_portal_open(false, true),
        Err(Refusal::UnsupportedByHost)
    );
    assert_eq!(
        decide_portal_open(true, false),
        Err(Refusal::MissingHostCallback)
    );
    assert_eq!(decide_portal_open(true, true), Ok(()));
    let staged = next_portal_state(State::Idle, Event::Stage).expect("stage");
    let active = next_portal_state(staged, Event::Activate).expect("activate");
    assert_eq!(active, State::Active);
    assert_eq!(next_portal_state(State::Idle, Event::Accept), None);
    assert_eq!(next_portal_state(active, Event::Stage), None);
    assert_eq!(clear_terminal_portal_state(active), active);
    for (event, terminal) in [
        (Event::Accept, State::Accepted),
        (Event::Cancel, State::Cancelled),
        (Event::Orphan, State::Orphaned),
    ] {
        let result = next_portal_state(active, event).expect("terminal portal event");
        assert_eq!(result, terminal);
        assert_eq!(next_portal_state(result, Event::Activate), None);
        assert_eq!(clear_terminal_portal_state(result), State::Idle);
    }

    let original = "é @file:old end";
    let target = exact_replacement_target_for_range(original, 2..11, original.chars().count());
    assert_eq!(
        apply_portal_replacement(original, &target, "@file:new"),
        ("é @file:new end".to_string(), 11, true)
    );
    // The user changed the token while the picker was open: do not overwrite it.
    let changed = "é @file:own end";
    assert_eq!(
        apply_portal_replacement(changed, &target, "@file:new"),
        ("é @file:own end@file:new".to_string(), 24, false)
    );
}
