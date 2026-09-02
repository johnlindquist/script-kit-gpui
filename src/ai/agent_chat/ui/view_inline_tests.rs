#[cfg(test)]
mod footer_owner_tests {
    use super::{
        desired_footer_owner_for_plan, plan_footer_owner_transition, plan_native_footer_lifecycle,
        AgentChatAutomationProjection, AgentChatFooterOwner, AgentChatFooterPresentationState,
    };
    use crate::ai::agent_chat::ui::layout::{AgentChatFooterInputs, ResolvedAgentChatRenderPlan};
    use crate::ai::agent_chat::ui::ui_variant::AgentChatUiVariant;
    use crate::footer_popup::MainWindowFooterConfig;

    fn detached_native_state(surface_tag: &'static str) -> AgentChatFooterPresentationState {
        AgentChatFooterPresentationState {
            owner: AgentChatFooterOwner::Native,
            is_main_window: false,
            native_config: Some(MainWindowFooterConfig::new(surface_tag, Vec::new())),
            theme_revision: 0,
        }
    }

    fn external_state() -> AgentChatFooterPresentationState {
        AgentChatFooterPresentationState {
            owner: AgentChatFooterOwner::External,
            is_main_window: false,
            native_config: None,
            theme_revision: 0,
        }
    }

    /// BC-2: leaving a detached native footer tears the previous host down; the
    /// memo also means an IDENTICAL presentation runs no side-effects (the
    /// per-frame re-sync is gone), while a config-only change re-syncs in place.
    #[test]
    fn agent_chat_footer_owner_transition_closes_stale_native_popup() {
        let native = detached_native_state("agent_chat");

        // Detached Native → External: tear down the previous native host.
        let leaving = plan_native_footer_lifecycle(Some(&native), &external_state());
        assert!(!leaving.unchanged);
        assert!(
            leaving.tear_down_previous_native,
            "leaving a detached native footer must tear down the stale host",
        );
        assert!(!leaving.sync_next_native);

        // Identical presentation → fully memoized, no side-effects this frame.
        let unchanged = plan_native_footer_lifecycle(Some(&native), &native.clone());
        assert!(unchanged.unchanged);
        assert!(!unchanged.tear_down_previous_native);
        assert!(!unchanged.sync_next_native);

        // Config-only change while staying detached Native → re-sync in place,
        // never a spurious teardown.
        let reconfigured = detached_native_state("agent_chat_reconfigured");
        let resync = plan_native_footer_lifecycle(Some(&native), &reconfigured);
        assert!(!resync.unchanged);
        assert!(!resync.tear_down_previous_native);
        assert!(resync.sync_next_native, "a config change re-syncs the host");

        // First entry (no previous) into a detached native footer → sync only.
        let first = plan_native_footer_lifecycle(None, &native);
        assert!(!first.unchanged);
        assert!(!first.tear_down_previous_native);
        assert!(first.sync_next_native);
    }

    fn native_conversation_owner() -> AgentChatFooterOwner {
        // Main window where Agent Chat owns the native footer surface.
        let plan = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::Standard,
            false,
            false,
            false,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            },
        );
        desired_footer_owner_for_plan(plan)
    }

    fn inline_conversation_owner() -> AgentChatFooterOwner {
        // Main window where Agent Chat is NOT the active native surface.
        let plan = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::Standard,
            false,
            false,
            false,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: false,
            },
        );
        desired_footer_owner_for_plan(plan)
    }

    fn focused_text_body_owner() -> AgentChatFooterOwner {
        let plan = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::FocusedTextMini,
            false,
            false,
            true,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            },
        );
        desired_footer_owner_for_plan(plan)
    }

    fn setup_body_owner() -> AgentChatFooterOwner {
        let plan = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::Standard,
            true,
            false,
            false,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            },
        );
        desired_footer_owner_for_plan(plan)
    }

    #[test]
    fn agent_chat_footer_owner_transition_native_to_inline_clears_native() {
        let t = plan_footer_owner_transition(
            Some(AgentChatFooterOwner::Native),
            AgentChatFooterOwner::Inline,
        );
        assert_eq!(t.owner, AgentChatFooterOwner::Inline);
        assert!(
            t.clears_native_host,
            "Native→Inline must record an explicit native clear"
        );
        assert_eq!(t.reserved_bands, 1);
    }

    #[test]
    fn agent_chat_footer_owner_transition_native_to_external_clears_native() {
        let t = plan_footer_owner_transition(
            Some(AgentChatFooterOwner::Native),
            AgentChatFooterOwner::External,
        );
        assert_eq!(t.owner, AgentChatFooterOwner::External);
        assert!(
            t.clears_native_host,
            "Native→External must record an explicit native clear"
        );
        assert_eq!(t.reserved_bands, 0);
    }

    #[test]
    fn agent_chat_footer_owner_transition_inline_to_native_syncs_without_clear() {
        let t = plan_footer_owner_transition(
            Some(AgentChatFooterOwner::Inline),
            AgentChatFooterOwner::Native,
        );
        assert_eq!(t.owner, AgentChatFooterOwner::Native);
        assert!(
            !t.clears_native_host,
            "entering Native re-syncs the host, never clears it"
        );
        assert_eq!(t.reserved_bands, 1);
    }

    #[test]
    fn agent_chat_footer_owner_transition_external_to_inline_no_native_clear() {
        let t = plan_footer_owner_transition(
            Some(AgentChatFooterOwner::External),
            AgentChatFooterOwner::Inline,
        );
        assert_eq!(t.owner, AgentChatFooterOwner::Inline);
        assert!(!t.clears_native_host, "no native host existed to clear");
        assert_eq!(t.reserved_bands, 1);
    }

    #[test]
    fn agent_chat_footer_owner_transition_conversation_to_setup_clears_native() {
        // The conversation shell owns the native footer; switching to the setup
        // body reconciles to External and tears the native host down.
        let conversation = native_conversation_owner();
        let setup = setup_body_owner();
        assert_eq!(conversation, AgentChatFooterOwner::Native);
        assert_eq!(setup, AgentChatFooterOwner::External);
        let t = plan_footer_owner_transition(Some(conversation), setup);
        assert_eq!(t.owner, AgentChatFooterOwner::External);
        assert!(t.clears_native_host);
        assert_eq!(t.reserved_bands, 0);
    }

    #[test]
    fn agent_chat_footer_owner_transition_standard_to_focused_text_mini() {
        // Standard (inline rail) → FocusedTextMini body (External owner).
        let standard = inline_conversation_owner();
        let focused = focused_text_body_owner();
        assert_eq!(standard, AgentChatFooterOwner::Inline);
        assert_eq!(focused, AgentChatFooterOwner::External);
        let t = plan_footer_owner_transition(Some(standard), focused);
        assert_eq!(t.owner, AgentChatFooterOwner::External);
        // Inline→External did not own a native host, so no native clear.
        assert!(!t.clears_native_host);
        assert_eq!(t.reserved_bands, 0);
    }

    /// Over the entire previous×desired matrix, exactly one owner survives (the
    /// desired one), the native host is cleared ONLY when leaving Native, and
    /// reserved bands are 0 exactly for External.
    #[test]
    fn agent_chat_footer_owner_transition_matrix_has_one_owner() {
        let owners = [
            AgentChatFooterOwner::External,
            AgentChatFooterOwner::Native,
            AgentChatFooterOwner::Inline,
        ];
        for &desired in &owners {
            for previous in [None, Some(owners[0]), Some(owners[1]), Some(owners[2])] {
                let t = plan_footer_owner_transition(previous, desired);
                assert_eq!(t.owner, desired, "exactly one (desired) owner survives");
                assert_eq!(
                    t.clears_native_host,
                    previous == Some(AgentChatFooterOwner::Native)
                        && desired != AgentChatFooterOwner::Native,
                    "native clear only on Native→non-Native",
                );
                assert_eq!(
                    t.reserved_bands == 0,
                    desired == AgentChatFooterOwner::External,
                    "reserved bands are 0 exactly for External",
                );
            }
        }
    }

    /// The automation projection is a pure function of the plan: body kind,
    /// composer slot, transcript anchor, density, footer owner and reserved
    /// bands all round-trip from the resolved plan.
    #[test]
    fn agent_chat_footer_owner_transition_projection_reflects_plan() {
        let conversation = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::BottomDock,
            false,
            false,
            false,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            },
        );
        let projection = AgentChatAutomationProjection::from_plan(conversation);
        assert_eq!(projection.body_kind, "conversation");
        assert_eq!(projection.composer_slot, "bottom");
        assert_eq!(projection.transcript_anchor, "bottom");
        assert_eq!(projection.density, "compact");
        assert_eq!(projection.footer_owner, "native");
        assert_eq!(projection.reserved_footer_bands, 1);

        // A setup body reports the setup body kind and reserves no band.
        let setup = ResolvedAgentChatRenderPlan::resolve(
            AgentChatUiVariant::Standard,
            true,
            false,
            false,
            AgentChatFooterInputs {
                uses_external_footer_host: false,
                is_main_window: true,
                glass_in_window_footer: false,
                platform_native_detached_footer: true,
                main_active_surface_is_agent_chat: true,
            },
        );
        let setup_projection = AgentChatAutomationProjection::from_plan(setup);
        assert_eq!(setup_projection.body_kind, "initial-setup");
        assert_eq!(setup_projection.footer_owner, "external");
        assert_eq!(setup_projection.reserved_footer_bands, 0);
    }
}

#[cfg(test)]
mod composer_sizing_tests {
    use super::{
        agent_chat_composer_geometry, combined_agent_model_header_label,
        composer_visible_line_count, focused_text_mini_input_shell_geometry,
        AgentChatComposerTextStyle, AgentChatView, FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX,
    };
    use crate::ai::agent_chat::ui::layout::AgentChatComposerSlot;
    use crate::ai::agent_chat::ui::permission_broker::AgentChatApprovalPreviewKind;
    use crate::theme::{AppChromeColors, Theme};
    use crate::ui_foundation::hex_to_rgba_with_opacity;

    #[test]
    fn visual_line_count_grows_then_clamps_and_expanded_pins_maximum() {
        assert_eq!(composer_visible_line_count(0, false), 1);
        assert_eq!(composer_visible_line_count(1, false), 1);
        assert_eq!(composer_visible_line_count(3, false), 3);
        assert_eq!(composer_visible_line_count(6, false), 6);
        assert_eq!(composer_visible_line_count(9, false), 6);
        assert_eq!(composer_visible_line_count(1, true), 6);
    }

    #[test]
    fn composer_constructor_tracks_active_search_tokens_and_theme_ui_family() {
        let theme = Theme::dark_default();
        let def = crate::designs::current_main_menu_theme().def();
        let search = def.search;
        let horizontal =
            crate::components::main_view_chrome::main_view_input_horizontal_metrics(def, 480.0);
        let text_style = AgentChatComposerTextStyle::current(&theme);

        assert_eq!(search.font_size, 20.0);
        assert_eq!(search.font_weight.0, 430.0);
        assert_eq!(search.height, 26.0);
        assert_eq!(text_style.font_size, search.font_size);
        assert_eq!(text_style.font_weight, search.font_weight);
        assert_eq!(text_style.font_family, theme.get_fonts().ui_family);
        assert_eq!(text_style.line_height, search.height);
        assert_eq!(text_style.one_line_height, search.height);
        assert_eq!(text_style.shell_inset_x, horizontal.shell_x);
        assert_eq!(text_style.text_inset_left, horizontal.text_inset_left);
        assert_eq!(text_style.text_inset_right, horizontal.text_inset_right);
        assert_eq!(
            AgentChatView::composer_height_for_visible_lines(1, &text_style),
            search.height
        );
        assert_eq!(
            AgentChatView::composer_height_for_visible_lines(3, &text_style),
            search.height * 3.0
        );
    }
    #[test]
    fn automation_composer_geometry_matches_shared_renderer_slots() {
        let theme = Theme::dark_default();
        let def = crate::designs::current_main_menu_theme().def();
        let text_style = AgentChatComposerTextStyle::current(&theme);
        let composer_height = text_style.height_for_visible_lines(3);
        let horizontal =
            crate::components::main_view_chrome::main_view_input_horizontal_metrics(def, 480.0);
        let header_metrics = crate::components::main_view_chrome::main_view_header_metrics(
            def,
            Some(composer_height),
        );
        let header = agent_chat_composer_geometry(
            480.0,
            440.0,
            44.0,
            AgentChatComposerSlot::Header,
            composer_height,
        );
        assert_eq!(header.composer_x, horizontal.shell_x);
        assert_eq!(header.composer_y, header_metrics.input_y);
        assert_eq!(header.composer_width, horizontal.shell_width);
        assert_eq!(header.composer_height, composer_height);
        assert_eq!(header.message_top, header_metrics.header_height);
        assert_eq!(
            header.message_height,
            440.0 - header_metrics.header_height - 44.0
        );
        let bottom = agent_chat_composer_geometry(
            480.0,
            440.0,
            44.0,
            AgentChatComposerSlot::Bottom,
            composer_height,
        );
        assert_eq!(bottom.composer_x, horizontal.shell_x);
        assert_eq!(bottom.composer_width, horizontal.shell_width);
        assert_eq!(bottom.composer_y, 440.0 - 44.0 - composer_height);
        assert_eq!(bottom.message_top, 0.0);
        assert_eq!(bottom.message_height, bottom.composer_y);
    }

    #[test]
    fn focused_text_mini_centers_one_canonical_shell_in_its_compact_slot() {
        let theme = Theme::dark_default();
        let text_style = AgentChatComposerTextStyle::current(&theme);
        let shell = focused_text_mini_input_shell_geometry(
            750.0,
            FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX,
            44.0,
            &text_style,
        );
        assert_eq!(shell.x, text_style.shell_inset_x);
        assert_eq!(
            shell.y,
            FOCUSED_TEXT_MINI_FRAME_BORDER_WIDTH_PX + (44.0 - text_style.one_line_height) / 2.0
        );
        assert_eq!(shell.width, 750.0 - text_style.shell_inset_x * 2.0);
        assert_eq!(shell.height, text_style.one_line_height);
    }
    #[test]
    fn permission_execute_chrome_follows_theme_warning_and_text_tiers() {
        let mut theme = Theme::dark_default();
        theme.colors.ui.warning = 0x12_34_56;
        {
            let opacity = theme.opacity.as_mut().expect("default theme opacity");
            opacity.text_strong = 0.71;
            opacity.text_muted_alpha = 0.53;
        }
        let opacity = theme.get_opacity();

        let permission =
            AgentChatView::permission_preview_chrome(AgentChatApprovalPreviewKind::Execute, &theme);
        let chrome = AppChromeColors::from_theme(&theme);
        let expected_badge = chrome.semantic_chip_colors(&theme, theme.colors.ui.warning);

        assert_eq!(permission.badge, expected_badge);
        assert_eq!(
            permission.accent_rgba,
            hex_to_rgba_with_opacity(theme.colors.ui.warning, opacity.text_strong)
        );
        assert_eq!(permission.title_text_rgba, chrome.text_strong_rgba);
        assert_eq!(permission.subject_text_rgba, chrome.text_muted_rgba);
    }

    #[test]
    fn header_deduplicates_profile_when_model_falls_back_to_same_display_name() {
        assert_eq!(
            combined_agent_model_header_label("Agent Chat Kitchen Sink", "Agent Chat Kitchen Sink"),
            "Agent Chat Kitchen Sink"
        );
        assert_eq!(
            combined_agent_model_header_label("Codex", "GPT-5.6"),
            "Codex · GPT-5.6"
        );
    }
}

#[cfg(all(test, target_os = "macos"))]
mod semantic_epoch_tests {
    use super::*;
    use crate::ai::agent_chat::ui::capabilities::AgentChatSessionPolicy;
    use crate::computer_use::owned_render_capture::validate_current_frame_identity;
    use crate::protocol::AutomationTargetIdentitySnapshot;
    use gpui::AppContext as _;

    fn frame_authority(revision: u64) -> AutomationTargetIdentitySnapshot {
        AutomationTargetIdentitySnapshot {
            window_id: "detached-chat-test".into(),
            window_generation: Some(1),
            app_view_variant: "AgentChatView".into(),
            target_generation: 1,
            surface_generation: revision,
            data_generation: revision,
            presentation_revision: Some(1),
            theme_revision: Some(1),
            frame_generation: Some(7),
        }
    }

    #[gpui::test]
    fn detached_mutations_reject_prior_exact_frame_before_paint(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            let (thread, _) = crate::ai::agent_chat::ui::mock_fixture::create_mock_fixture_thread(
                "detached-epoch", AgentChatSessionPolicy::Full, cx,
            );
            let view = cx.new(|cx| AgentChatView::new(thread.clone(), cx));
            thread.update(cx, |thread, cx| thread.set_input("alpha", cx));
            let retained = frame_authority(view.read(cx).semantic_revision(cx));
            validate_current_frame_identity(&retained, &retained).unwrap();
            thread.update(cx, |thread, cx| thread.set_input("bravo", cx));
            let changed = frame_authority(view.read(cx).semantic_revision(cx));
            assert!(changed.data_generation > retained.data_generation);
            assert_eq!(changed.frame_generation, retained.frame_generation);
            assert_eq!(validate_current_frame_identity(&changed, &retained).unwrap_err().to_string(), "capture_frame_identity_stale");
            thread.update(cx, |thread, cx| thread.set_input("alpha", cx));
            let aba = frame_authority(view.read(cx).semantic_revision(cx));
            assert!(aba.data_generation > changed.data_generation);
            assert!(validate_current_frame_identity(&aba, &retained).is_err());
            assert_eq!(thread.read(cx).input.text(), "alpha");
            view.update(cx, |view, _| view.set_context_capture_pending(true));
            let child_state = frame_authority(view.read(cx).semantic_revision(cx));
            assert!(child_state.data_generation > aba.data_generation);
            assert!(validate_current_frame_identity(&child_state, &aba).is_err());
            assert_eq!(view.read(cx).semantic_revision(cx), child_state.data_generation);
            // This is the production exact comparator, not native publication:
            // no frame or observer has been advanced within this app update.
            validate_current_frame_identity(&child_state, &child_state).unwrap();
        });
    }

    #[gpui::test]
    fn transcript_child_toggle_aba_changes_view_authority_without_paint(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            let (thread, _) = crate::ai::agent_chat::ui::mock_fixture::create_mock_fixture_thread(
                "detached-child-epoch", AgentChatSessionPolicy::Full, cx,
            );
            let view = cx.new(|cx| AgentChatView::new(thread, cx));
            let transcript = cx.new(|cx| AgentChatTranscript::new(vec![
                AgentChatThreadMessage {
                    id: 1,
                    role: AgentChatThreadMessageRole::Thought,
                    body: "Reasoning".into(),
                    tool_call_id: None,
                    tool_meta: None,
                    attachments: Vec::new(),
                },
            ], cx));
            view.update(cx, |view, _| view.transcript = Some(transcript.clone()));
            let retained = frame_authority(view.read(cx).semantic_revision(cx));
            transcript.update(cx, |transcript, cx| transcript.toggle_collapsed(1, cx));
            let expanded = view.read(cx).semantic_revision(cx);
            assert!(expanded > retained.data_generation);
            transcript.update(cx, |transcript, cx| transcript.toggle_collapsed(1, cx));
            let aba = frame_authority(view.read(cx).semantic_revision(cx));
            assert!(aba.data_generation > expanded);
            assert!(validate_current_frame_identity(&aba, &retained).is_err());
        });
    }
}
