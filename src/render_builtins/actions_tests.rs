#[cfg(test)]
mod dictation_history_action_model_tests {
    use super::ScriptListApp;

    #[test]
    fn standalone_and_portal_actions_advertise_only_performable_verbs() {
        let standalone = ScriptListApp::dictation_history_actions_for_dialog(false);
        let standalone_ids = standalone
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>();
        assert!(standalone_ids.contains(&"dictation_history_paste"));
        assert!(standalone_ids.contains(&"dictation_history_add_to_agent_chat"));
        assert!(!standalone
            .iter()
            .any(|action| { action.title.contains("Ask") || action.title.contains("Send") }));

        let portal = ScriptListApp::dictation_history_actions_for_dialog(true);
        let portal_ids = portal
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>();
        assert!(!portal_ids.contains(&"dictation_history_paste"));
        assert!(!portal_ids.contains(&"dictation_history_add_to_agent_chat"));
        assert!(portal_ids.contains(&"dictation_history_copy"));
        assert!(portal_ids.contains(&"dictation_history_delete"));
    }
}

#[cfg(test)]
mod on_close_reentrancy_tests {
    use std::fs;

    #[test]
    fn test_render_builtins_actions_clipboard_popup_uses_mini_menu_contract() {
        let source = fs::read_to_string("src/render_builtins/actions.rs")
            .expect("Failed to read src/render_builtins/actions.rs");

        let clipboard_fn = source
            .split("fn toggle_clipboard_actions(")
            .nth(1)
            .expect("missing toggle_clipboard_actions");

        assert!(
            clipboard_fn.contains("dialog.set_config(crate::actions::ActionsDialogConfig {"),
            "clipboard actions should set an explicit ActionsDialogConfig"
        );
        assert!(
            clipboard_fn.contains("search_position: crate::actions::SearchPosition::Top"),
            "clipboard actions should place search at the top"
        );
        assert!(
            clipboard_fn.contains("section_style: crate::actions::SectionStyle::Headers"),
            "clipboard actions should use section headers"
        );
        assert!(
            clipboard_fn.contains("anchor: crate::actions::AnchorPosition::Top"),
            "clipboard actions should anchor to the top"
        );
        assert!(
            clipboard_fn.contains("show_icons: true"),
            "clipboard actions should show icons"
        );
        assert!(
            clipboard_fn.contains("show_context_header: false"),
            "clipboard actions should hide the context header"
        );
        assert!(
            clipboard_fn.contains("crate::actions::WindowPosition::TopCenter"),
            "clipboard actions should open in the top-center mini-menu position"
        );
        assert!(
            !clipboard_fn.contains("crate::actions::WindowPosition::BottomRight"),
            "clipboard actions should not open in the bottom-right position"
        );
    }
}

#[cfg(test)]
mod flow_desk_create_discoverability_tests {
    //! Actions-menu leg of the discoverability bar (paired with
    //! tests/launcher_discoverability_contract.rs): every Flow Desk ⌘K
    //! subject must surface the create-flow affordance, and surfacing it
    //! must not degrade the menu (ids stay unique, Danger stays last).

    use super::{FlowDeskSubject, ScriptListApp};
    use crate::flows::model::{FlowDescriptor, FlowSource};

    fn sample_flow() -> FlowDescriptor {
        FlowDescriptor {
            id: "project:flow-gmail".to_string(),
            path: "/test/flows/flow-gmail.md".to_string(),
            source: FlowSource::Project,
            name: "flow-gmail".to_string(),
            description: Some("Triage email".to_string()),
            engine: "codex".to_string(),
            engine_source: None,
            inputs: Vec::new(),
            is_workflow: false,
            interactive: false,
            mtime_ms: 0,
            origin: Some("repo flows/".to_string()),
            wrapper_command: None,
        }
    }

    fn session_subject(working: bool) -> FlowDeskSubject {
        FlowDeskSubject::Session {
            id: 7,
            facts: crate::components::conversation_actions::FlowConversationCommandFacts {
                response_in_progress: working,
                viewing_archive: false,
                has_archives: false,
                selected_has_response: true,
                composer_has_text: true,
                hidden_draft_exists: false,
                runtime_attached: true,
            },
            archives: Vec::new(),
            open_required: true,
        }
    }

    fn already_open_session_subject(working: bool) -> FlowDeskSubject {
        let mut subject = session_subject(working);
        if let FlowDeskSubject::Session { open_required, .. } = &mut subject {
            *open_required = false;
        }
        subject
    }

    fn subjects() -> Vec<(&'static str, FlowDeskSubject)> {
        vec![
            ("flow", FlowDeskSubject::Flow(sample_flow())),
            ("session", session_subject(false)),
            ("create", FlowDeskSubject::Create),
        ]
    }

    #[test]
    fn every_flow_desk_subject_surfaces_the_create_affordance() {
        for (label, subject) in subjects() {
            let actions = ScriptListApp::flow_desk_actions_for_dialog(&subject);
            assert!(
                actions.iter().any(|action| action.id == "flow_desk_create"),
                "the {label} subject must surface the flow_desk_create action"
            );
        }
    }

    #[test]
    fn action_ids_stay_unique_per_subject() {
        for (label, subject) in subjects() {
            let actions = ScriptListApp::flow_desk_actions_for_dialog(&subject);
            let mut ids: Vec<&str> = actions.iter().map(|action| action.id.as_str()).collect();
            let before = ids.len();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(
                before,
                ids.len(),
                "the {label} subject repeats an action id"
            );
        }
    }

    #[test]
    fn danger_actions_stay_last_for_sessions() {
        let actions = ScriptListApp::flow_desk_actions_for_dialog(&session_subject(false));
        let first_danger = actions
            .iter()
            .position(|action| action.section.as_deref() == Some("Danger"))
            .expect("session subject must keep its Danger section");
        assert!(
            actions[first_danger..]
                .iter()
                .all(|action| action.section.as_deref() == Some("Danger")),
            "Danger must remain the trailing section — new verbs go before it"
        );
    }

    /// Who is supposed to honor a shortcut badge printed in the session ⌘K
    /// menu.
    enum ChordOwner {
        /// `resolve_flow_session_key_action` answers it. Checkable right here.
        SessionKeyResolver(super::FlowSessionKeyAction),
    }

    /// Parse a menu badge (`⇧⌘C`) into the key + modifiers a keystroke
    /// delivers. Deliberately strict: an unknown glyph panics rather than
    /// being skipped, because silently ignoring a modifier would let this
    /// whole test pass a chord it never really checked.
    fn parse_chord(badge: &str) -> (String, bool, bool) {
        let (mut platform, mut shift) = (false, false);
        let mut key = String::new();
        for ch in badge.chars() {
            match ch {
                '\u{2318}' => platform = true,
                '\u{21e7}' => shift = true,
                '\u{2325}' | '\u{2303}' => panic!("{badge}: option/control chords are unmodelled"),
                '\u{238b}' => key.push_str("escape"),
                '\u{21b5}' => key.push_str("enter"),
                other => key.push(other.to_ascii_lowercase()),
            }
        }
        assert!(!key.is_empty(), "{badge} has modifiers but no key");
        (key, platform, shift)
    }

    /// The regression lock for the 2026-07-25 finding.
    ///
    /// `flow_desk_session_copy_last_response` shipped advertising `⇧⌘C` while
    /// `resolve_flow_session_key_action` — documented as the single exhaustive
    /// key owner — had no arm for it. The action worked when clicked, so every
    /// test passed; the chord did nothing, and nothing anywhere related the
    /// badge to the binding. A user who reads a shortcut once and then uses it
    /// forever gets silence, and concludes the feature is broken.
    ///
    /// This test is the missing relation. Every shortcut-bearing session
    /// action must name an owner, and the resolver-owned ones are actually
    /// pressed. Adding a badge without declaring an owner fails here, which
    /// is exactly the step that got skipped.
    #[test]
    fn every_advertised_session_shortcut_has_a_declared_owner() {
        use super::{resolve_flow_session_key_action, FlowSessionKeyAction};

        let owners: &[(&str, ChordOwner)] = &[
            (
                "flow_desk_session_stop",
                ChordOwner::SessionKeyResolver(FlowSessionKeyAction::Stop),
            ),
            (
                "flow_desk_session_background",
                ChordOwner::SessionKeyResolver(FlowSessionKeyAction::Background),
            ),
            (
                "flow_desk_session_new_conversation",
                ChordOwner::SessionKeyResolver(FlowSessionKeyAction::NewConversation),
            ),
            (
                "flow_desk_session_copy_last_response",
                ChordOwner::SessionKeyResolver(FlowSessionKeyAction::CopyLastResponse),
            ),
        ];

        // Idle and working sessions expose different enabled sets, so both are
        // swept. Terminate is deliberately absent here because it has no chord.
        for working in [false, true] {
            let subject = already_open_session_subject(working);
            let facts = match &subject {
                FlowDeskSubject::Session { facts, .. } => *facts,
                _ => unreachable!(),
            };
            let actions = ScriptListApp::flow_desk_actions_for_dialog(&subject);

            for action in actions.iter() {
                if action.disabled_reason().is_some() {
                    continue;
                }
                let Some(badge) = action.shortcut.as_deref() else {
                    continue;
                };
                let owner = owners
                    .iter()
                    .find(|(id, _)| *id == action.id)
                    .map(|(_, owner)| owner)
                    .unwrap_or_else(|| {
                        panic!(
                            "{} advertises the shortcut {badge:?} but no owner is declared. \
                             Add an arm to resolve_flow_session_key_action (and list it here), \
                             or name the window-level interceptor that answers it. An \
                             advertised chord nobody honors is worse than no chord: the user \
                             stops looking for the menu item.",
                            action.id
                        )
                    });

                let (key, platform, shift) = parse_chord(badge);
                let resolved = resolve_flow_session_key_action(&key, platform, shift, facts, false);

                let ChordOwner::SessionKeyResolver(expected) = owner;
                assert_eq!(
                    resolved, *expected,
                    "{} advertises {badge:?}; pressing it (working={working}) must resolve \
                     to {expected:?}, not {resolved:?}",
                    action.id
                );
            }
        }
    }

    /// Copying the last answer is the most common thing a user does with a
    /// finished turn, and Flow had no way to do it — only "Copy Transcript",
    /// which forced the user to paste everything and hand-delete the rest.
    ///
    /// Flow now mirrors Agent Chat exactly: same title, same shortcut, same
    /// section. Asserting the literals here is the point — a differently
    /// named or differently bound twin is precisely the drift this workstream
    /// exists to stop.
    #[test]
    fn flow_sessions_copy_the_last_response_the_same_way_agent_chat_does() {
        let actions = ScriptListApp::flow_desk_actions_for_dialog(&session_subject(false));
        let copy = actions
            .iter()
            .find(|action| action.id == "flow_desk_session_copy_last_response")
            .expect("a flow session must offer Copy Last Response");

        assert_eq!(copy.title, "Copy Last Response");
        assert_eq!(copy.shortcut.as_deref(), Some("\u{21e7}\u{2318}C"));
        assert_eq!(copy.section.as_deref(), Some("Response"));

        let transcript = actions
            .iter()
            .find(|action| action.id == "flow_desk_session_copy_transcript")
            .expect("Copy Transcript must survive alongside it");
        assert_eq!(
            transcript.section.as_deref(),
            Some("Response"),
            "both copy verbs belong to one section, so they are found together"
        );
    }

    /// Agent Chat has had "New Conversation" on ⌘L; Flow's only way to start
    /// over was Terminate, which permanently destroys the conversation. A user
    /// who wanted a clean slate had to pick the destructive verb.
    ///
    /// The literals are asserted deliberately: this action exists to be the
    /// SAME action on both surfaces, so a renamed or rebound twin is exactly
    /// the drift being prevented. The expected values are read from Agent
    /// Chat's own builder rather than typed twice, so a change on that side
    /// fails here instead of quietly splitting the two again.
    #[test]
    fn flow_sessions_start_a_new_conversation_the_same_way_agent_chat_does() {
        let agent_chat = crate::actions::get_agent_chat_actions();
        let reference = agent_chat
            .iter()
            .find(|action| action.id == "agent_chat_new_conversation")
            .expect("Agent Chat owns the reference New Conversation action");

        let actions = ScriptListApp::flow_desk_actions_for_dialog(&session_subject(false));
        let flow = actions
            .iter()
            .find(|action| action.id == "flow_desk_session_new_conversation")
            .expect("an idle flow session must offer New Conversation");

        assert_eq!(flow.title, reference.title);
        assert_eq!(flow.shortcut, reference.shortcut);
        assert_eq!(flow.section, reference.section);
        assert_eq!(flow.icon, reference.icon);
    }

    /// A temporarily unavailable command remains visible with a typed reason,
    /// so the Actions menu explains the stable command vocabulary without
    /// allowing a running turn to be orphaned.
    #[test]
    fn new_conversation_is_visible_but_disabled_while_a_turn_is_running() {
        let working = ScriptListApp::flow_desk_actions_for_dialog(&session_subject(true));
        let new_conversation = working
            .iter()
            .find(|action| action.id == "flow_desk_session_new_conversation")
            .expect("a working session must keep New Conversation discoverable");
        assert_eq!(
            new_conversation.disabled_reason(),
            Some("Stop the current response first.")
        );
        // Everything else stays put — disabling one verb must not quietly
        // strip the session's other affordances.
        for expected in [
            "flow_desk_session_open",
            "flow_desk_session_stop",
            "flow_desk_session_terminate",
        ] {
            assert!(
                working.iter().any(|action| action.id == expected),
                "{expected} must survive while the session is working"
            );
        }
    }
}
