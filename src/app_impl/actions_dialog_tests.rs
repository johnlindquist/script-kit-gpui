#[cfg(test)]
mod close_actions_popup_regression_tests {
    use std::fs;

    #[test]
    fn test_close_actions_popup_invokes_on_close_before_clearing_dialog_state() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let on_close_pos = close_fn
            .find("on_close(cx);")
            .expect("close_actions_popup must invoke on_close callback");
        let clear_dialog_pos = close_fn
            .find("self.actions_dialog = None;")
            .expect("close_actions_popup must clear actions_dialog state");

        assert!(
            on_close_pos < clear_dialog_pos,
            "close_actions_popup must invoke on_close before clearing actions_dialog state"
        );
    }

    #[test]
    fn test_close_actions_popup_resyncs_filter_input_after_clearing_dialog_state() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let clear_dialog_pos = close_fn
            .find("self.mark_actions_popup_closed();")
            .expect("close_actions_popup must clear actions_dialog state");
        let resync_pos = close_fn
            .find("self.resync_filter_input_after_actions_if_needed(window, cx);")
            .expect("close_actions_popup must resync canonical filter input state");

        assert!(
            clear_dialog_pos < resync_pos,
            "close_actions_popup must resync filter input after clearing actions dialog state"
        );
    }

    #[test]
    fn test_close_actions_popup_notifies_after_focus_restore_paths() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let fallback_focus_pos = close_fn
            .find("window.focus(&self.focus_handle, cx);")
            .expect("close_actions_popup must keep fallback root focus");
        let notify_pos = close_fn
            .find("cx.notify();")
            .expect("close_actions_popup must notify after closing popup");

        assert!(
            fallback_focus_pos < notify_pos,
            "close_actions_popup must notify after focus restore paths complete"
        );
    }

    #[test]
    fn test_close_actions_popup_restores_host_focus_before_apply_pending_focus() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let host_restore_pos = close_fn
            .find("self.request_focus_restore_for_actions_host(host);")
            .expect("close_actions_popup must request host-specific focus restore");
        let apply_pending_pos = close_fn
            .find("self.apply_pending_focus(window, cx)")
            .expect("close_actions_popup must apply pending focus after host restore");

        assert!(
            host_restore_pos < apply_pending_pos,
            "close_actions_popup should request host-specific focus before applying pending focus"
        );
    }

    #[test]
    fn test_close_actions_popup_defers_focus_apply_from_actions_window() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let close_fn_start = source
            .find("pub(crate) fn close_actions_popup")
            .expect("close_actions_popup function not found");
        let close_fn = &source[close_fn_start..];

        let detect_pos = close_fn
            .find("let closing_from_actions_window = crate::actions::is_actions_window(window);")
            .expect("close_actions_popup must detect actions-window-originated closes");
        let activate_pos = close_fn
            .find("crate::platform::activate_main_window();")
            .expect("actions-window-originated close must activate the main window");
        let host_restore_pos = close_fn
            .find("self.request_focus_restore_for_actions_host(host);")
            .expect("host focus restore request missing");
        // The first `if closing_from_actions_window` is the early main-window
        // activation guard; the deferred-apply branch is the one after the
        // host focus restore, so search from there.
        let deferred_branch_pos = close_fn[host_restore_pos..]
            .find("if closing_from_actions_window")
            .map(|pos| host_restore_pos + pos)
            .expect("actions-window-originated close must skip child-window focus apply");
        let apply_pending_pos = close_fn
            .find("self.apply_pending_focus(window, cx)")
            .expect("non-actions-window close path must still apply focus immediately");

        assert!(
            detect_pos < activate_pos && activate_pos < host_restore_pos,
            "close_actions_popup should activate the main window before queuing host focus restore"
        );
        assert!(
            host_restore_pos < deferred_branch_pos && deferred_branch_pos < apply_pending_pos,
            "actions-window closes must defer focus application instead of focusing the popup window"
        );
    }

    #[test]
    fn day_page_command_bar_keys_are_not_swallowed_by_missing_shared_dialog() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");
        let helper_start = source
            .find("fn route_key_to_actions_dialog")
            .expect("route_key_to_actions_dialog function not found");
        let helper_fn = &source[helper_start..];

        assert!(
            helper_fn.contains("let Some(ref dialog) = self.actions_dialog else")
                && helper_fn.contains("return ActionsRoute::NotHandled;"),
            "route_key_to_actions_dialog must not swallow keys when a child CommandBar owns the detached actions window"
        );
    }
}

#[cfg(test)]
mod actions_host_focus_restore_tests {
    use super::*;
    use crate::focus_coordinator::FocusRequest;

    fn restore(host: ActionsDialogHost, surface: SurfaceKind) -> FocusRequest {
        ScriptListApp::focus_restore_request_for_actions_host(host, surface)
    }

    /// Closing Cmd+K over a flow session must hand focus back to the shared
    /// MAIN input — the flow composer IS the main filter input
    /// (`current_view_uses_shared_filter_input`). A `term_prompt()` restore
    /// here silently no-ops (FlowSessionView has no terminal entity) and
    /// strands focus, which is exactly the regression this locks out.
    #[test]
    fn flow_session_actions_restore_main_input_focus() {
        assert_eq!(
            restore(ActionsDialogHost::FlowDesk, SurfaceKind::FlowSession),
            FocusRequest::main_filter(),
        );
        assert_eq!(
            restore(ActionsDialogHost::FlowDesk, SurfaceKind::FlowUx),
            FocusRequest::main_filter(),
        );
    }

    /// Day Page Cmd+K close must restore focus to the Day editor, not the
    /// main filter.
    #[test]
    fn day_page_mainlist_actions_restore_editor_focus() {
        assert_eq!(
            restore(ActionsDialogHost::MainList, SurfaceKind::DayPage),
            FocusRequest::editor_prompt(),
        );
        assert_eq!(
            restore(ActionsDialogHost::MainList, SurfaceKind::ScriptList),
            FocusRequest::main_filter(),
        );
    }

    /// Every prompt host restores focus to its own input when the dialog
    /// closes; list-like hosts restore the shared main filter.
    #[test]
    fn prompt_hosts_restore_their_own_inputs() {
        let cases = [
            (ActionsDialogHost::ArgPrompt, FocusRequest::arg_prompt()),
            (ActionsDialogHost::ChatPrompt, FocusRequest::chat_prompt()),
            (ActionsDialogHost::AgentChat, FocusRequest::agent_chat()),
            (
                ActionsDialogHost::EditorPrompt,
                FocusRequest::editor_prompt(),
            ),
            (
                ActionsDialogHost::TemplatePrompt,
                FocusRequest::template_prompt(),
            ),
            (ActionsDialogHost::FormPrompt, FocusRequest::form_prompt()),
            (ActionsDialogHost::DivPrompt, FocusRequest::div_prompt()),
            (ActionsDialogHost::TermPrompt, FocusRequest::term_prompt()),
            (ActionsDialogHost::WebcamPrompt, FocusRequest::div_prompt()),
            (ActionsDialogHost::FileSearch, FocusRequest::main_filter()),
            (
                ActionsDialogHost::ClipboardHistory,
                FocusRequest::main_filter(),
            ),
            (ActionsDialogHost::EmojiPicker, FocusRequest::main_filter()),
            (ActionsDialogHost::AppLauncher, FocusRequest::main_filter()),
        ];

        for (host, expected) in cases {
            assert_eq!(
                restore(host, SurfaceKind::ScriptList),
                expected,
                "host {host:?} must restore its own input focus",
            );
        }
    }
}

#[cfg(test)]
mod actions_host_mapping_tests {
    use super::*;

    #[test]
    fn actions_host_for_view_maps_in_scope_surfaces() {
        let cases = vec![
            (AppView::ScriptList, Some(ActionsDialogHost::MainList)),
            (
                AppView::AppLauncherView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::AppLauncher),
            ),
            (
                AppView::ThemeChooserView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::ThemeChooser),
            ),
            (
                AppView::SettingsView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::BuiltinList),
            ),
            (
                AppView::AgentChatHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::AgentChatHistory),
            ),
            (
                AppView::ClipboardHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::ClipboardHistory),
            ),
            (
                AppView::BrowserHistoryView {
                    filter: String::new(),
                    selected_index: 0,
                },
                Some(ActionsDialogHost::BuiltinList),
            ),
            (
                AppView::EmojiPickerView {
                    filter: String::new(),
                    selected_index: 0,
                    selected_category: None,
                },
                Some(ActionsDialogHost::EmojiPicker),
            ),
            (
                AppView::MiniPrompt {
                    id: String::new(),
                    placeholder: String::new(),
                    choices: Vec::new(),
                },
                None,
            ),
        ];

        for (view, expected) in cases {
            assert_eq!(ScriptListApp::actions_host_for_view(&view), expected);
        }
    }

    #[test]
    fn live_actions_host_for_view_excludes_generic_builtin_list_views() {
        let browser_history = AppView::BrowserHistoryView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&browser_history),
            None
        );

        let settings = AppView::SettingsView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(ScriptListApp::live_actions_host_for_view(&settings), None);

        let theme_chooser = AppView::ThemeChooserView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&theme_chooser),
            Some(ActionsDialogHost::ThemeChooser)
        );

        let current_app_commands = AppView::CurrentAppCommandsView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&current_app_commands),
            None
        );

        let process_manager = AppView::ProcessManagerView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&process_manager),
            None
        );
    }

    #[test]
    fn live_actions_host_for_view_keeps_selection_specific_hosts() {
        let file_search = AppView::FileSearchView {
            query: String::new(),
            selected_index: 0,
            presentation: FileSearchPresentation::Full,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&file_search),
            Some(ActionsDialogHost::FileSearch)
        );

        let clipboard = AppView::ClipboardHistoryView {
            filter: String::new(),
            selected_index: 0,
        };
        assert_eq!(
            ScriptListApp::live_actions_host_for_view(&clipboard),
            Some(ActionsDialogHost::ClipboardHistory)
        );
    }
}

#[cfg(test)]
mod actions_dialog_wiring_regression_tests {
    use std::fs;

    use super::menu_syntax_displayed_shortcut_should_consume;

    #[test]
    fn render_script_list_routes_popup_keys_before_generic_cmd_shortcuts() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        let route_pos = source
            .find("this.route_key_to_actions_dialog(")
            .expect("render_script_list must use the shared actions router");
        let cmd_pos = source
            .find("if has_cmd")
            .expect("render_script_list cmd shortcut block not found");

        assert!(
            route_pos < cmd_pos,
            "render_script_list must route popup keys before generic Cmd shortcuts"
        );
    }

    #[test]
    fn menu_syntax_displayed_shortcut_only_consumes_enter() {
        assert!(menu_syntax_displayed_shortcut_should_consume("enter"));

        for shortcut in ["s", "t", "backspace", "escape", "cmd-k"] {
            assert!(
                !menu_syntax_displayed_shortcut_should_consume(shortcut),
                "menu-syntax main-list ownership must not swallow {shortcut}; the focused input or normal key handlers still own editing keys"
            );
        }
    }

    #[test]
    fn route_key_to_actions_dialog_notifies_after_arrow_navigation() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        let route_start = source
            .find("pub(crate) fn route_key_to_actions_dialog")
            .expect("route_key_to_actions_dialog not found");
        let route_fn = &source[route_start..];

        let up_start = route_fn
            .find("if is_key_up(key)")
            .expect("up branch missing");
        let down_start = route_fn
            .find("if is_key_down(key)")
            .expect("down branch missing");
        let jump_start = route_fn
            .find("let is_home = key.eq_ignore_ascii_case(\"home\")")
            .expect("jump-key section missing");

        assert!(
            route_fn[up_start..down_start].contains("crate::actions::notify_actions_window(cx);"),
            "up branch must notify the actions window"
        );
        assert!(
            route_fn[down_start..jump_start].contains("crate::actions::notify_actions_window(cx);"),
            "down branch must notify the actions window"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_handles_cmd_k_close() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source.contains("key.eq_ignore_ascii_case(\"k\")")
                && source.contains("self.close_actions_popup(host, window, cx);"),
            "shared actions router should close the popup on Cmd+K"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_keeps_detached_window_routable() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source
                .contains("!self.show_actions_popup && !crate::actions::is_actions_window_open()"),
            "shared actions router must keep routing keys while the detached actions window is open"
        );
    }

    #[test]
    fn route_key_to_actions_dialog_preserves_return_origin_for_explicit_agent_chat_handoff() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("Failed to read src/app_impl/actions_dialog.rs");

        assert!(
            source.contains(
                "self.open_tab_ai_agent_chat_with_explicit_target_preserving_return(target, cx);"
            ),
            "shared actions Cmd+Enter handoff should seed Agent Chat return origin before opening Agent Chat"
        );
    }

    #[test]
    fn render_script_list_has_no_duplicate_popup_handler() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        // The old inline popup handler used this pattern - it should be gone
        assert!(
            !source.contains("if this.show_actions_popup {"),
            "render_script_list must not contain a duplicate inline popup key handler"
        );
    }
}

#[cfg(test)]
mod agent_chat_spine_dispatch_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct DispatchTestConnection;

    impl crate::ai::agent_chat::runtime::AgentChatConnection for DispatchTestConnection {
        fn start_turn(
            &self,
            _request: crate::ai::agent_chat::runtime::AgentChatTurnRequest,
        ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
        {
            Err(crate::ai::reliability::AiAdapterError::from_record(
                crate::ai::reliability::provider_failure(
                    sk_protocol::ai_reliability::ProtocolComponent::Provider,
                    "OF-35b dispatch test must not submit",
                ),
            ))
        }

        fn cancel_turn(
            &self,
            _ui_thread_id: String,
        ) -> crate::ai::reliability::AiAdapterResult<()> {
            Ok(())
        }

        fn prepare_session(
            &self,
            _ui_thread_id: String,
            _cwd: std::path::PathBuf,
        ) -> crate::ai::reliability::AiAdapterResult<crate::ai::agent_chat::events::AgentChatEventRx>
        {
            let (_tx, rx) = async_channel::bounded(1);
            Ok(rx)
        }
    }

    /// OF-35b: bare Enter from the main-window actions interceptor must reach
    /// the embedded Agent Chat Spine before the inactive-main-list shortcut
    /// bypass. This exercises the real dispatcher with a live AgentChatView;
    /// it must accept the selected profile without submitting the transcript.
    #[gpui::test]
    fn embedded_agent_chat_profile_spine_enter_reaches_existing_acceptance_path(
        cx: &mut gpui::TestAppContext,
    ) {
        let catalog = crate::flows::catalog::flow_catalog();
        catalog.set_notify_hook(|_, _| {});
        catalog.prime_ready_for_test(&crate::flows::resolve_flow_cwd(None));

        let app_slot = Arc::new(Mutex::new(None));
        let app_slot_for_window = Arc::clone(&app_slot);
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                let mut config = crate::config::Config::default();
                let mut built_ins = config.get_builtins();
                built_ins.app_launcher = false;
                config.built_ins = Some(built_ins);
                let app = cx.new(|cx| ScriptListApp::new(config, false, window, cx));
                *app_slot_for_window.lock().expect("store OF-35b test app") = Some(app);
                cx.new(|_| crate::MainMenuSelectionTestHost)
            })
            .expect("open OF-35b dispatch test window")
        });
        let app = app_slot
            .lock()
            .expect("read OF-35b test app")
            .take()
            .expect("OF-35b test app initialized");

        let selected_profiles = Arc::new(Mutex::new(Vec::<String>::new()));
        let selected_profiles_for_callback = Arc::clone(&selected_profiles);
        let (handled, before, after) = cx.update(|cx| {
            window
                .update(cx, |_host, window, cx| {
                    app.update(cx, |app, cx| {
                    let (_broker, permission_rx) =
                        crate::ai::agent_chat::ui::AgentChatPermissionBroker::new();
                    let thread = cx.new(|cx| {
                        crate::ai::agent_chat::ui::AgentChatThread::new(
                            Arc::new(DispatchTestConnection),
                            permission_rx,
                            crate::ai::agent_chat::ui::AgentChatThreadInit {
                                ui_thread_id: "of35b-dispatch-test".to_string(),
                                cwd: std::env::temp_dir().join("of35b-dispatch-test"),
                                initial_input: None,
                                initial_context_parts: Vec::new(),
                                display_name: "OF-35b Test".into(),
                                profile_id: crate::ai::agent_chat::profiles::BUILTIN_GENERAL_PROFILE_ID
                                    .to_string(),
                                profile_display_name: Some("OF-35b Test".into()),
                                profile_icon_name: None,
                                selected_agent: None,
                                available_agents: Vec::new(),
                                launch_requirements:
                                    crate::ai::agent_chat::ui::AgentChatLaunchRequirements::default(),
                                available_models: Vec::new(),
                                selected_model_id: None,
                                session_policy:
                                    crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy::Full,
                            },
                            cx,
                        )
                    });
                    thread.update(cx, |thread, cx| {
                        thread.mark_context_bootstrap_ready(cx);
                        thread
                            .apply_test_fixture(
                                "assistantText",
                                Some("OF-35b user message".to_string()),
                                Some("OF-35b assistant message".to_string()),
                                None,
                                cx,
                            )
                            .expect("seed OF-35b transcript");
                    });
                    let entity = cx.new(|cx| {
                        crate::ai::agent_chat::ui::AgentChatView::new(thread, cx)
                    });
                    app.current_view = AppView::AgentChatView {
                        entity: entity.clone(),
                    };
                    entity.update(cx, |chat, cx| {
                        let thread = chat.thread().expect("fixture must have a live thread");
                        let mut draft = thread.read(cx).draft_snapshot();
                        draft.pending_context_items = vec![
                            crate::ai::staged_context::StagedContextItem::pending(
                                crate::ai::message_parts::AiContextPart::SkillFile {
                                    path: "/tmp/of35b-context.md".to_string(),
                                    label: "OF-35b context".to_string(),
                                    skill_name: "of35b-context".to_string(),
                                    owner_label: "test".to_string(),
                                    slash_name: "of35b-context".to_string(),
                                },
                                crate::ai::staged_context::ContextProvenance::UserMention,
                                crate::ai::staged_context::ContextRole::Supplemental,
                            ),
                        ];
                        thread.update(cx, |thread, cx| {
                            thread.restore_draft_snapshot(draft, cx);
                        });
                        chat.set_on_profile_selected(move |profile_id, _cx| {
                            selected_profiles_for_callback
                                .lock()
                                .expect("record selected profile")
                                .push(profile_id);
                        });
                        // The GPUI test platform has no native display handle;
                        // opening the same profile trigger without popup geometry
                        // produces the identical Spine projection used by dispatch.
                        chat.open_profile_trigger_picker(cx);
                        chat.set_input("|text".to_string(), cx);
                        chat.refresh_agent_chat_spine_from_composer(cx);
                    });

                    let before = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    let spine = before
                        .spine
                        .as_ref()
                        .expect("exact profile query must project through Spine");
                    assert!(spine.owns_list);
                    assert_eq!(spine.active_segment_kind, "profile");
                    assert_eq!(spine.row_count, 1);
                    assert_eq!(spine.selectable_row_count, 1);
                    assert_eq!(spine.selected_index, 0);
                    assert_eq!(before.message_count, 2);
                    assert_eq!(before.context_chip_count, 1);

                    let handled = app.try_execute_main_list_action_shortcut_from_display(
                        "enter",
                        &gpui::Modifiers::default(),
                        window,
                        cx,
                    );
                    let after = entity.read(cx).collect_agent_chat_state_snapshot(cx);
                    (handled, before, after)
                    })
                })
                .expect("update OF-35b dispatch test window")
        });
        cx.run_until_parked();

        assert!(handled, "the interceptor must consume Spine profile Enter");
        assert_eq!(
            selected_profiles
                .lock()
                .expect("read selected profiles")
                .as_slice(),
            ["text"]
        );
        assert_eq!(after.input_text, "", "profile trigger must clear exactly");
        assert!(
            !after.spine.as_ref().is_some_and(|spine| spine.owns_list),
            "accepted profile must release Spine list ownership"
        );
        assert_eq!(after.message_count, before.message_count, "must not submit");
        assert_eq!(
            after.context_chip_count, before.context_chip_count,
            "profile acceptance must preserve staged context"
        );
    }
}

#[cfg(test)]
mod modal_backdrop_policy_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn backdrop_test_flow_meta(id: u64) -> crate::flows::session::FlowSessionMeta {
        crate::flows::session::FlowSessionMeta {
            id,
            flow_id: "project:backdrop-test".into(),
            flow_name: "flow-backdrop-test".into(),
            friendly_name: "Backdrop Test".into(),
            origin: "Project".into(),
            origin_kind: crate::flows::session::FlowOriginKind::Project,
            engine: "codex".into(),
            model: None,
            model_source: crate::flows::session::FlowModelSource::Unavailable,
            flow_path: "/tmp/flow-backdrop-test.md".into(),
            flow_mtime_ms: 0,
            cwd: "/tmp".into(),
            transport: crate::flows::session::SessionTransport::CodexThread,
            state: crate::flows::session::SessionState::NeedsYou,
            started_at: std::time::Instant::now(),
            last_activity: std::time::SystemTime::UNIX_EPOCH
                + std::time::Duration::from_secs(1_000),
            active_thread_id: crate::flows::session::FlowSessionMeta::new_thread_id(),
            active_thread_created_at: chrono::Utc::now().to_rfc3339(),
            active_parent_thread_id: None,
            turns: vec![],
            archived_threads: vec![],
            transcript_selection: crate::flows::session::FlowTranscriptSelection::Active,
            inherited_turn_count: 0,
            active_draft: String::new(),
            draft_generation: 0,
            runtime_generation: 0,
            persistence_revision: 0,
            active_turn: None,
            thread_ready: true,
            needs_rethread: false,
            pending_runtime_termination: false,
            reliability: crate::flows::session::FlowReliability::new(
                "project:backdrop-test",
                "/tmp/flow-backdrop-test.md",
                "codex",
            ),
        }
    }

    /// Oracle step 8 policy lock (deferred by the plan until the step-6
    /// store existed): a modal-backdrop click DISMISSES THE MODAL ONLY.
    /// The topmost modal owns and consumes the click; it must never
    /// background, touch, remove, or otherwise mutate the underlying AI
    /// session, and it must not change the surface underneath. Ruling:
    /// user submission 98cab5e5-…641, confirmed verbatim by Oracle consult
    /// `floating-capsule-entry-material`.
    ///
    /// This drives `close_actions_popup` — the exact method the backdrop
    /// click handler routes to (`src/render_prompts/arg/helpers.rs`) — so
    /// the assertion covers the real dismissal path, not a re-derived one.
    #[gpui::test]
    fn backdrop_dismissal_never_mutates_backgrounded_sessions(cx: &mut gpui::TestAppContext) {
        let catalog = crate::flows::catalog::flow_catalog();
        catalog.set_notify_hook(|_, _| {});
        catalog.prime_ready_for_test(&crate::flows::resolve_flow_cwd(None));

        let app_slot = Arc::new(Mutex::new(None));
        let app_slot_for_window = Arc::clone(&app_slot);
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                let mut config = crate::config::Config::default();
                let mut built_ins = config.get_builtins();
                built_ins.app_launcher = false;
                config.built_ins = Some(built_ins);
                let app = cx.new(|cx| ScriptListApp::new(config, false, window, cx));
                *app_slot_for_window.lock().expect("store backdrop test app") = Some(app);
                cx.new(|_| crate::MainMenuSelectionTestHost)
            })
            .expect("open backdrop policy test window")
        });
        let app = app_slot
            .lock()
            .expect("read backdrop test app")
            .take()
            .expect("backdrop test app initialized");

        let (store_before, view_before) = cx.update(|cx| {
            app.update(cx, |app, cx| {
                // One backgrounded flow session in the canonical store.
                let focus_handle = cx.focus_handle();
                let entity = cx.new(|_| {
                    crate::prompts::ChatPrompt::new(
                        "flow-session-11".to_string(),
                        None,
                        vec![],
                        None,
                        None,
                        focus_handle,
                        Some(Arc::new(|_| Ok(())) as crate::prompts::ChatSubmitCallback),
                        Arc::new(crate::theme::Theme::default()),
                    )
                });
                app.conversations
                    .flow_sessions
                    .push((backdrop_test_flow_meta(11), entity));

                // Open a minimal actions dialog — the modal under test.
                let theme_arc = Arc::clone(&app.theme);
                let dialog = cx.new(move |cx| {
                    let focus_handle = cx.focus_handle();
                    ActionsDialog::with_config(
                        focus_handle,
                        Arc::new(|_action_id| {}),
                        vec![],
                        theme_arc,
                        crate::actions::ActionsDialogConfig::default(),
                    )
                });
                app.actions_dialog = Some(dialog);
                app.mark_actions_popup_opening();

                (
                    app.conversations.snapshot(),
                    format!("{:?}", std::mem::discriminant(&app.current_view)),
                )
            })
        });

        // The backdrop click: exactly what the shield's on_click listener
        // does (arg/helpers.rs backdrop_click -> close_actions_popup).
        cx.update(|cx| {
            window
                .update(cx, |_host, window, cx| {
                    app.update(cx, |app, cx| {
                        app.close_actions_popup(ActionsDialogHost::MainList, window, cx);
                    });
                })
                .expect("drive backdrop dismissal");
        });

        cx.update(|cx| {
            app.update(cx, |app, _cx| {
                // DismissModal happened…
                assert!(
                    app.actions_dialog.is_none(),
                    "backdrop click must dismiss the modal"
                );
                // …and ONLY DismissModal: the store is byte-identical (same
                // count, ids, liveness, turnInFlight, and last_activity —
                // dismissal is NOT semantic session activity), and the
                // surface underneath did not change or background.
                assert_eq!(
                    app.conversations.snapshot(),
                    store_before,
                    "modal dismissal must not mutate any backgrounded session"
                );
                assert_eq!(
                    format!("{:?}", std::mem::discriminant(&app.current_view)),
                    view_before,
                    "modal dismissal must not change the underlying surface"
                );
            })
        });
    }
}
