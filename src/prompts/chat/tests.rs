use super::*;
#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use std::collections::HashMap;

    use crate::protocol::ChatPromptMessage;

    use super::{
        next_chat_scroll_follow_state, next_reveal_boundary, resolve_chat_input_key_action,
        resolve_setup_card_key, should_ignore_stream_reveal_update,
        should_show_script_generation_actions, ChatInputKeyAction, ChatScrollDirection,
        ScriptGenerationAction, SetupCardAction,
    };

    const CHAT_RENDER_CORE_SOURCE: &str = include_str!("render_core.rs");
    const CHAT_RENDER_INPUT_SOURCE: &str = include_str!("render_input.rs");
    const CHAT_RENDER_TURNS_SOURCE: &str = include_str!("render_turns.rs");

    use super::{resolve_chat_render_plan, ChatPromptHostMode, ChatTranscriptAlignment};
    use crate::prompts::chat::types::ChatBodyKind;

    /// Locks the chrome + key-handler composition for each host mode: a
    /// Standalone host owns header/input/footer/keys (header suppressed only
    /// in mini chrome), while a TranscriptOnly host owns none of it — the
    /// external host is the single lifecycle/key owner.
    #[test]
    fn chat_prompt_host_mode() {
        // Standalone, full chrome (transcript body): header + input + footer,
        // and the prompt owns its Enter/Escape handlers.
        let full =
            resolve_chat_render_plan(ChatPromptHostMode::Standalone { mini: false }, false, false);
        assert_eq!(full.body, ChatBodyKind::Transcript);
        assert!(full.render_header, "standalone full renders its header");
        assert!(full.render_input, "standalone full renders its composer");
        assert!(full.render_footer, "standalone full renders its footer");
        assert!(
            full.install_key_handlers,
            "standalone host owns its key handlers"
        );
        assert!(full.owns_focus, "standalone host owns first-render focus");
        assert!(
            full.owns_input_lifecycle,
            "standalone host owns its input lifecycle"
        );

        // Standalone mini: borderless chrome suppresses the header, but the
        // composer, footer, and key handlers all remain.
        let mini =
            resolve_chat_render_plan(ChatPromptHostMode::Standalone { mini: true }, false, false);
        assert!(!mini.render_header, "mini chrome has no header");
        assert!(mini.render_input, "mini still renders its composer");
        assert!(mini.render_footer, "mini still renders its footer");
        assert!(mini.install_key_handlers, "mini host owns its key handlers");

        // Standalone setup body: header + card only (no composer, no footer),
        // key handlers still installed for setup-card navigation.
        let setup =
            resolve_chat_render_plan(ChatPromptHostMode::Standalone { mini: false }, true, false);
        assert_eq!(setup.body, ChatBodyKind::Setup);
        assert!(setup.render_header);
        assert!(!setup.render_input);
        assert!(!setup.render_footer);
        assert!(setup.install_key_handlers);

        // TranscriptOnly: nothing local, no keys — the host owns everything.
        for alignment in [
            ChatTranscriptAlignment::Top,
            ChatTranscriptAlignment::Bottom,
        ] {
            let hosted = resolve_chat_render_plan(
                ChatPromptHostMode::TranscriptOnly { alignment },
                false,
                false,
            );
            assert!(!hosted.render_header);
            assert!(!hosted.render_input);
            assert!(!hosted.render_footer);
            assert!(
                !hosted.install_key_handlers,
                "transcript-only host installs no key handlers"
            );
            assert!(
                !hosted.owns_focus,
                "transcript-only host never grabs first-render focus"
            );
            assert!(
                !hosted.owns_input_lifecycle,
                "transcript-only host owns no input lifecycle"
            );
        }
    }

    /// C-R2: a transcript-only host owns NO input lifecycle in ANY body state,
    /// so a hidden hosted ChatPrompt can never auto-submit, start a cursor
    /// blink, or process an initial response. Contrast: a standalone host owns
    /// the lifecycle in every state.
    #[test]
    fn chat_render_plan_lifecycle_ownership_by_host() {
        let hosted = ChatPromptHostMode::TranscriptOnly {
            alignment: ChatTranscriptAlignment::Top,
        };
        for (needs_setup, loading) in [(true, false), (false, true), (false, false)] {
            let plan = resolve_chat_render_plan(hosted, needs_setup, loading);
            assert!(
                !plan.owns_focus && !plan.owns_input_lifecycle,
                "transcript-only host owns no focus/lifecycle (setup={needs_setup}, loading={loading})"
            );
        }
        for (needs_setup, loading) in [(true, false), (false, true), (false, false)] {
            let plan = resolve_chat_render_plan(
                ChatPromptHostMode::Standalone { mini: false },
                needs_setup,
                loading,
            );
            assert!(
                plan.owns_focus && plan.owns_input_lifecycle,
                "standalone host owns focus/lifecycle (setup={needs_setup}, loading={loading})"
            );
        }
    }

    /// Regression lock for the bug WP7 fixes: a transcript-only host must
    /// suppress ALL local chrome and key handlers in EVERY body state —
    /// including the setup and loading early returns, which previously
    /// composed the local header even under an external host.
    #[test]
    fn chat_prompt_external_host_suppresses_local_chrome_in_all_states() {
        let mode = ChatPromptHostMode::TranscriptOnly {
            alignment: ChatTranscriptAlignment::Top,
        };
        // (needs_setup, loading_providers) → every reachable body state.
        for (needs_setup, loading, expected_body) in [
            (true, false, ChatBodyKind::Setup),
            (false, true, ChatBodyKind::Loading),
            (false, false, ChatBodyKind::Transcript),
        ] {
            let plan = resolve_chat_render_plan(mode, needs_setup, loading);
            assert_eq!(
                plan.body, expected_body,
                "body kind resolves before any early return"
            );
            assert!(
                !plan.render_header,
                "no local header in {expected_body:?} under an external host"
            );
            assert!(
                !plan.render_input,
                "no local composer in {expected_body:?} under an external host"
            );
            assert!(
                !plan.render_footer,
                "no local footer in {expected_body:?} under an external host"
            );
            assert!(
                !plan.install_key_handlers,
                "no local key handlers in {expected_body:?} under an external host"
            );
        }

        // Contrast: a standalone host DOES compose local chrome in the same
        // states, so the suppression above is genuinely host-driven.
        let standalone =
            resolve_chat_render_plan(ChatPromptHostMode::Standalone { mini: false }, true, false);
        assert!(standalone.render_header);
        assert!(standalone.install_key_handlers);
    }

    #[test]
    fn resolve_setup_card_key_cycles_focus_for_tab_and_arrows() {
        assert_eq!(
            resolve_setup_card_key("tab", false, 0),
            (1, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("Tab", false, 1),
            (0, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("tab", true, 0),
            (1, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("tab", true, 1),
            (0, SetupCardAction::None, true)
        );

        assert_eq!(
            resolve_setup_card_key("up", false, 0),
            (1, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("ArrowUp", false, 1),
            (0, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("down", false, 0),
            (1, SetupCardAction::None, true)
        );
        assert_eq!(
            resolve_setup_card_key("arrowdown", false, 1),
            (0, SetupCardAction::None, true)
        );
    }

    #[test]
    fn resolve_setup_card_key_activates_buttons_and_escape() {
        assert_eq!(
            resolve_setup_card_key("enter", false, 0),
            (0, SetupCardAction::ActivateConfigure, false)
        );
        assert_eq!(
            resolve_setup_card_key("Return", false, 1),
            (1, SetupCardAction::ActivateClaudeCode, false)
        );
        assert_eq!(
            resolve_setup_card_key(" ", false, 0),
            (0, SetupCardAction::ActivateConfigure, false)
        );
        assert_eq!(
            resolve_setup_card_key("escape", false, 1),
            (1, SetupCardAction::Escape, false)
        );
    }

    #[test]
    fn resolve_setup_card_key_ignores_unhandled_keys() {
        assert_eq!(
            resolve_setup_card_key("x", false, 1),
            (1, SetupCardAction::None, false)
        );
    }

    #[test]
    fn chat_layout_renderers_use_shared_spacing_and_translucent_surfaces() {
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains(".px(px(CHAT_LAYOUT_PADDING_X))"),
            "Render core should use shared horizontal padding constants"
        );
        assert!(
            CHAT_RENDER_CORE_SOURCE.contains("render_simple_hint_strip(")
                && CHAT_RENDER_CORE_SOURCE.contains("universal_prompt_hints()"),
            "Mini footer should delegate to the shared universal hint strip"
        );
        assert!(
            CHAT_RENDER_INPUT_SOURCE.contains("AppChromeColors::from_theme("),
            "Input surface should use AppChromeColors for background tokens"
        );
        assert!(
            CHAT_RENDER_TURNS_SOURCE.contains("CHAT_LAYOUT_CARD_PADDING_X")
                && CHAT_RENDER_TURNS_SOURCE.contains("CHAT_LAYOUT_CARD_PADDING_Y"),
            "Turn renderer should use shared card padding constants"
        );
    }

    #[test]
    fn resolve_chat_input_key_action_routes_enter_variants() {
        assert_eq!(
            resolve_chat_input_key_action("enter", false, false),
            ChatInputKeyAction::Submit
        );
        assert_eq!(
            resolve_chat_input_key_action("return", false, true),
            ChatInputKeyAction::InsertNewline
        );
        assert_eq!(
            resolve_chat_input_key_action("enter", true, false),
            ChatInputKeyAction::ContinueInChat
        );
        assert_eq!(
            resolve_chat_input_key_action("enter", true, true),
            ChatInputKeyAction::ContinueInChat
        );
    }

    #[test]
    fn resolve_chat_input_key_action_routes_shortcuts_and_fallback() {
        assert_eq!(
            resolve_chat_input_key_action("escape", false, false),
            ChatInputKeyAction::Escape
        );
        assert_eq!(
            resolve_chat_input_key_action(".", true, false),
            ChatInputKeyAction::StopStreaming
        );
        assert_eq!(
            resolve_chat_input_key_action("k", true, false),
            ChatInputKeyAction::ToggleActions
        );
        assert_eq!(
            resolve_chat_input_key_action("c", true, false),
            ChatInputKeyAction::CopyLastResponse
        );
        assert_eq!(
            resolve_chat_input_key_action("backspace", true, false),
            ChatInputKeyAction::ClearConversation
        );
        assert_eq!(
            resolve_chat_input_key_action("v", true, false),
            ChatInputKeyAction::Paste
        );
        assert_eq!(
            resolve_chat_input_key_action("backspace", false, false),
            ChatInputKeyAction::DelegateToInput
        );
        assert_eq!(
            resolve_chat_input_key_action("x", true, false),
            ChatInputKeyAction::Ignore
        );
        assert_eq!(
            resolve_chat_input_key_action("a", false, false),
            ChatInputKeyAction::DelegateToInput
        );
    }

    #[test]
    fn should_ignore_stream_reveal_update_when_stream_stopped_or_replaced() {
        assert!(
            should_ignore_stream_reveal_update(None, "stream-a"),
            "Stopped streams should ignore further reveal updates"
        );
        assert!(
            should_ignore_stream_reveal_update(Some("stream-b"), "stream-a"),
            "Replaced streams should ignore stale reveal updates"
        );
        assert!(
            !should_ignore_stream_reveal_update(Some("stream-a"), "stream-a"),
            "Active stream should continue receiving reveal updates"
        );
    }

    #[test]
    fn should_show_script_generation_actions_only_when_draft_is_ready() {
        assert!(
            should_show_script_generation_actions(true, false, true),
            "Script actions should show only when generation mode is on, not streaming, and a draft exists"
        );
        assert!(
            !should_show_script_generation_actions(false, false, true),
            "Script actions should stay hidden when script generation mode is disabled"
        );
        assert!(
            !should_show_script_generation_actions(true, true, true),
            "Script actions should stay hidden while streaming is in progress"
        );
        assert!(
            !should_show_script_generation_actions(true, false, false),
            "Script actions should stay hidden when there is no draft response yet"
        );
    }

    #[test]
    fn script_generation_action_should_run_after_save_only_for_run_variants() {
        assert!(
            !ScriptGenerationAction::Save.should_run_after_save(),
            "Save should not run the script"
        );
        assert!(
            ScriptGenerationAction::Run.should_run_after_save(),
            "Run should run after saving"
        );
        assert!(
            ScriptGenerationAction::SaveAndRun.should_run_after_save(),
            "SaveAndRun should run after saving"
        );
    }

    #[test]
    fn assistant_response_markdown_source_wraps_plain_script_in_script_generation_mode() {
        let response = r#"// Name: Example
// Description: Example script
import "@scriptkit/sdk";

await div("Hello");
"#;

        let normalized = super::types::assistant_response_markdown_source(true, response);
        assert_eq!(
            normalized.as_ref(),
            r#"```typescript
// Name: Example
// Description: Example script
import "@scriptkit/sdk";

await div("Hello");
```"#
        );
    }

    #[test]
    fn assistant_response_markdown_source_keeps_existing_fence_unchanged() {
        let response = r#"```typescript
await div("Hello");
```"#;

        let normalized = super::types::assistant_response_markdown_source(true, response);
        assert_eq!(normalized.as_ref(), response);
    }

    #[test]
    fn assistant_response_markdown_source_keeps_plain_text_when_not_script_generation() {
        let response = r#"// Name: Example
await div("Hello");"#;

        let normalized = super::types::assistant_response_markdown_source(false, response);
        assert_eq!(normalized.as_ref(), response);
    }

    // --- next_reveal_boundary tests ---

    #[test]
    fn reveal_boundary_empty_remaining() {
        assert_eq!(next_reveal_boundary("hello", 5), None);
        assert_eq!(next_reveal_boundary("", 0), None);
    }

    #[test]
    fn reveal_boundary_reveals_through_newline() {
        let text = "first line\nsecond line\n";
        assert_eq!(next_reveal_boundary(text, 0), Some(11)); // "first line\n"
        assert_eq!(next_reveal_boundary(text, 11), Some(23)); // "second line\n"
    }

    #[test]
    fn reveal_boundary_word_by_word_without_newline() {
        let text = "hello world foo";
        // "hello " → advances past word + whitespace to start of "world"
        assert_eq!(next_reveal_boundary(text, 0), Some(6));
        assert_eq!(next_reveal_boundary(text, 6), Some(12)); // "world "
                                                             // "foo" — partial word, no trailing whitespace
        assert_eq!(next_reveal_boundary(text, 12), None);
    }

    #[test]
    fn reveal_boundary_partial_word_waits() {
        assert_eq!(next_reveal_boundary("hel", 0), None);
        assert_eq!(next_reveal_boundary("- T", 2), None); // "T" partial
    }

    #[test]
    fn reveal_boundary_newline_takes_priority_over_words() {
        let text = "hello world\nfoo";
        // Should reveal through newline, not stop at word boundary
        assert_eq!(next_reveal_boundary(text, 0), Some(12)); // "hello world\n"
    }

    #[test]
    fn reveal_boundary_markdown_list_lines() {
        let text = "- First item\n- Second item\n- Third\n";
        let mut offset = 0;
        let mut lines = vec![];
        while let Some(new_offset) = next_reveal_boundary(text, offset) {
            lines.push(&text[offset..new_offset]);
            offset = new_offset;
        }
        assert_eq!(
            lines,
            vec!["- First item\n", "- Second item\n", "- Third\n"]
        );
    }

    #[test]
    fn reveal_boundary_utf8_safe() {
        let text = "héllo wörld\n";
        assert_eq!(next_reveal_boundary(text, 0), Some(text.len()));
    }

    /// Simulate the full reveal of a markdown string and verify the final
    /// result matches the original. This catches cases where progressive
    /// reveal could produce a different final string.
    #[test]
    fn progressive_reveal_produces_complete_content() {
        let content = "Sure! Here's a list:\n\n\
            **Things to do:**\n\
            - Read a good book\n\
            - Watch your favorite movies or TV shows\n\
            - Try a new recipe or bake something delicious\n\
            - Work on a puzzle\n\n\
            Would you like me to create a list on a different topic?\n";

        let mut offset = 0;
        let mut revealed = String::new();
        let mut boundary_count = 0usize;

        while let Some(new_offset) = next_reveal_boundary(content, offset) {
            assert!(
                new_offset > offset,
                "Reveal boundary must always advance. offset={offset}, new_offset={new_offset}"
            );
            revealed.push_str(&content[offset..new_offset]);
            offset = new_offset;
            boundary_count += 1;
        }

        // Simulate the final "flush remainder" pass done when streaming finishes.
        revealed.push_str(&content[offset..]);

        assert!(
            boundary_count > 1,
            "Multi-line content should reveal progressively before final flush"
        );
        assert_eq!(revealed, content);
    }

    /// Verify that reveal never skips content — each boundary advances
    /// monotonically and covers the full string.
    #[test]
    fn reveal_offsets_are_monotonically_increasing() {
        let content = "- First\n- Second\n- Third item with longer text\n\nParagraph after.\n";
        let mut offset = 0;
        let mut prev = 0;
        let mut reconstructed = String::new();
        let mut boundary_count = 0usize;
        while let Some(new_offset) = next_reveal_boundary(content, offset) {
            assert!(
                new_offset > prev,
                "Offset did not advance: prev={}, new={}",
                prev,
                new_offset
            );
            assert!(
                content.is_char_boundary(new_offset),
                "Offset {} must be on a UTF-8 char boundary",
                new_offset
            );
            reconstructed.push_str(&content[offset..new_offset]);
            prev = new_offset;
            offset = new_offset;
            boundary_count += 1;
        }
        reconstructed.push_str(&content[offset..]);
        assert!(
            boundary_count > 0,
            "Expected at least one progressive boundary for newline-delimited input"
        );
        assert!(
            reconstructed == content,
            "Reconstructed content must match original without gaps or duplication"
        );
    }

    #[test]
    fn build_conversation_turns_pairs_user_assistant_messages() {
        let messages = vec![
            ChatPromptMessage::user("First user").with_id("u1"),
            ChatPromptMessage::assistant("First assistant").with_id("a1"),
            ChatPromptMessage::assistant("Standalone assistant").with_id("a2"),
            ChatPromptMessage::user("Second user").with_id("u2"),
        ];

        let turns = super::build_conversation_turns(&messages, &HashMap::new());
        assert_eq!(turns.len(), 3);

        assert_eq!(turns[0].user_prompt, "First user");
        assert_eq!(
            turns[0].assistant_response.as_deref(),
            Some("First assistant")
        );

        assert!(turns[1].user_prompt.is_empty());
        assert_eq!(
            turns[1].assistant_response.as_deref(),
            Some("Standalone assistant")
        );

        assert_eq!(turns[2].user_prompt, "Second user");
        assert!(turns[2].assistant_response.is_none());
    }

    #[test]
    fn chat_scroll_follow_state_disables_follow_on_upward_scroll() {
        assert!(
            next_chat_scroll_follow_state(false, ChatScrollDirection::Up, false),
            "Scrolling upward should mark the user as manually scrolled up"
        );
    }

    #[test]
    fn chat_scroll_follow_state_keeps_manual_mode_when_scrolling_down_above_bottom() {
        assert!(
            next_chat_scroll_follow_state(true, ChatScrollDirection::Down, false),
            "Scrolling down away from bottom should keep manual mode enabled"
        );
    }

    #[test]
    fn chat_scroll_follow_state_reenables_follow_when_scrolling_down_at_bottom() {
        assert!(
            !next_chat_scroll_follow_state(true, ChatScrollDirection::Down, true),
            "Reaching the bottom while scrolling down should re-enable auto-follow"
        );
    }

    #[test]
    fn chat_scroll_follow_state_preserves_follow_state_for_non_scrolling_events() {
        assert!(
            next_chat_scroll_follow_state(true, ChatScrollDirection::None, false),
            "No directional input should preserve manual mode"
        );
        assert!(
            !next_chat_scroll_follow_state(false, ChatScrollDirection::None, false),
            "No directional input should preserve follow mode"
        );
    }

    #[test]
    fn chat_scroll_offset_bottom_check_tolerates_subpixel_shortfall() {
        use super::{scroll_offset_is_at_bottom, CHAT_SCROLL_BOTTOM_TOLERANCE_PX};
        // Trackpad momentum stopping a fraction of a pixel short of the exact
        // max offset must still read as "at the bottom" (2026-07-11 report:
        // the Jump to latest pill stayed visible at the visual bottom).
        assert!(scroll_offset_is_at_bottom(
            999.6,
            1000.0,
            CHAT_SCROLL_BOTTOM_TOLERANCE_PX
        ));
        // Overshoot past max (padding skew) is also the bottom.
        assert!(scroll_offset_is_at_bottom(
            1016.0,
            1000.0,
            CHAT_SCROLL_BOTTOM_TOLERANCE_PX
        ));
        // A reading position well above the bottom is NOT the bottom.
        assert!(!scroll_offset_is_at_bottom(
            900.0,
            1000.0,
            CHAT_SCROLL_BOTTOM_TOLERANCE_PX
        ));
    }

    #[test]
    fn test_resolve_chat_input_key_action_maps_cmd_down_and_end_to_jump_to_latest() {
        assert_eq!(
            resolve_chat_input_key_action("down", true, false),
            ChatInputKeyAction::JumpToLatest
        );
        assert_eq!(
            resolve_chat_input_key_action("arrowdown", true, false),
            ChatInputKeyAction::JumpToLatest
        );
        assert_eq!(
            resolve_chat_input_key_action("end", false, false),
            ChatInputKeyAction::JumpToLatest
        );
        assert_eq!(
            resolve_chat_input_key_action("End", false, false),
            ChatInputKeyAction::JumpToLatest
        );
    }
}

/// C-R2: rendering a real `ChatPrompt` entity under a transcript-only host
/// must run NONE of the local input lifecycle — no auto-submit, no cursor-blink
/// start — regardless of builder order, and the internal input must never grab
/// focus. Contrast against a standalone host, which owns the full lifecycle.
#[cfg(test)]
mod chat_prompt_host_mode_lifecycle {
    use super::super::{
        ChatPrompt, ChatPromptHostMode, ChatSubmitCallback, ChatTranscriptAlignment,
    };
    use crate::protocol::ChatPromptMessage;
    use crate::theme;
    use gpui::{prelude::*, px};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const TRANSCRIPT_ONLY: ChatPromptHostMode = ChatPromptHostMode::TranscriptOnly {
        alignment: ChatTranscriptAlignment::Top,
    };

    fn window_options() -> gpui::WindowOptions {
        let mut options = gpui::WindowOptions::default();
        options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
            gpui::point(px(0.0), px(0.0)),
            gpui::size(px(480.0), px(320.0)),
        )));
        options
    }

    /// Both builder orders — `with_host_mode(...).with_mini_mode(true)` and
    /// `with_mini_mode(true).with_host_mode(...)` — must stay TranscriptOnly.
    /// (Before C-R2, `with_mini_mode` called `set_host_mode(Standalone)`, so the
    /// second order silently converted the host back to Standalone.)
    #[gpui::test]
    fn both_builder_orders_stay_transcript_only(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);
        let (host_first, mini_first) = cx.update(|cx| {
            let host_first = ChatPrompt::new(
                "order-a".to_string(),
                None,
                Vec::new(),
                None,
                None,
                cx.focus_handle(),
                Arc::new(|_, _| {}) as ChatSubmitCallback,
                Arc::new(theme::Theme::default()),
            )
            .with_host_mode(TRANSCRIPT_ONLY)
            .with_mini_mode(true)
            .host_mode();
            let mini_first = ChatPrompt::new(
                "order-b".to_string(),
                None,
                Vec::new(),
                None,
                None,
                cx.focus_handle(),
                Arc::new(|_, _| {}) as ChatSubmitCallback,
                Arc::new(theme::Theme::default()),
            )
            .with_mini_mode(true)
            .with_host_mode(TRANSCRIPT_ONLY)
            .host_mode();
            (host_first, mini_first)
        });
        assert!(
            host_first.is_transcript_only(),
            "host_mode-then-mini must stay TranscriptOnly"
        );
        assert!(
            mini_first.is_transcript_only(),
            "mini-then-host_mode must stay TranscriptOnly (C-R2 builder-order bug)"
        );
    }

    /// WP-B3: a Flow history replay restores an entire conversation in ONE
    /// bulk pass. `restore_messages` extends the message vector and rebuilds the
    /// turn cache exactly once, producing the same coherent turn list a
    /// per-message replay would — without the per-message rebuild amplification.
    #[gpui::test]
    fn flow_history_restore_bulk_rebuilds_once(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);
        let window = cx.update(|cx| {
            cx.open_window(window_options(), |_, cx| {
                let focus_handle = cx.focus_handle();
                cx.new(|_| {
                    ChatPrompt::new(
                        "flow-restore".to_string(),
                        None,
                        Vec::new(),
                        None,
                        None,
                        focus_handle,
                        Arc::new(|_, _| {}) as ChatSubmitCallback,
                        Arc::new(theme::Theme::default()),
                    )
                    .with_host_mode(TRANSCRIPT_ONLY)
                })
            })
            .expect("flow-restore chat window opens")
        });
        cx.run_until_parked();

        window
            .update(cx, |prompt, _window, cx| {
                let restored = vec![
                    ChatPromptMessage::user("first ask"),
                    ChatPromptMessage::assistant("first answer"),
                    ChatPromptMessage::user("second ask"),
                    ChatPromptMessage::assistant("second answer"),
                    ChatPromptMessage::user("third ask"),
                    ChatPromptMessage::assistant("third answer"),
                ];
                prompt.restore_messages(restored, cx);
                // One coherent rebuild: three user/assistant pairs → three turns,
                // cache no longer dirty, and each turn carries its exact text.
                assert!(!prompt.conversation_turns_dirty);
                assert_eq!(prompt.conversation_turns_cache.len(), 3);
                assert_eq!(prompt.conversation_turns_cache[0].user_prompt, "first ask");
                assert_eq!(
                    prompt.conversation_turns_cache[2]
                        .assistant_response
                        .as_deref(),
                    Some("third answer"),
                );
            })
            .expect("flow-restore chat window updates");
    }

    /// A transcript-only host with a pending submit and non-empty input must NOT
    /// fire the submit callback, must NOT start a cursor blink, and must NOT
    /// focus its internal input across a real render pass.
    #[gpui::test]
    fn transcript_only_render_runs_no_input_lifecycle(cx: &mut gpui::TestAppContext) {
        cx.update(gpui_component::init);
        let submits = Arc::new(AtomicUsize::new(0));
        let submits_cb = submits.clone();
        let window = cx.update(|cx| {
            cx.open_window(window_options(), |_, cx| {
                let focus_handle = cx.focus_handle();
                cx.new(|_| {
                    let on_submit: ChatSubmitCallback = Arc::new(move |_id, _text| {
                        submits_cb.fetch_add(1, Ordering::SeqCst);
                    });
                    let mut prompt = ChatPrompt::new(
                        "hosted".to_string(),
                        None,
                        Vec::new(),
                        None,
                        None,
                        focus_handle,
                        on_submit,
                        Arc::new(theme::Theme::default()),
                    )
                    .with_pending_submit(true)
                    .with_host_mode(TRANSCRIPT_ONLY);
                    prompt.input.set_text("hello".to_string());
                    prompt
                })
            })
            .expect("hosted chat window opens")
        });
        cx.run_until_parked();

        assert_eq!(
            submits.load(Ordering::SeqCst),
            0,
            "transcript-only host must never auto-submit"
        );
        window
            .update(cx, |chat, window, _cx| {
                assert!(
                    chat.pending_submit(),
                    "pending submit stays queued (host owns submission)"
                );
                assert!(
                    !chat.cursor_blink_started,
                    "transcript-only host must not start its cursor blink"
                );
                assert!(
                    !chat.focus_handle.is_focused(window),
                    "transcript-only host must not grab focus"
                );
            })
            .expect("hosted chat window updates");
    }

    // Contrast: a standalone host DOES own the lifecycle. A full real-entity
    // render is NOT exercised here — a standalone chat paints the native
    // main-window footer slot (`render_main_window_footer_slot_for_prompt_surface`),
    // which requires a real platform window handle the headless gpui test window
    // panics on ("Test Windows are not backed by a real platform window"). The
    // transcript-only host suppresses that footer, which is exactly why the
    // hosted case above renders headlessly. Standalone lifecycle ownership is
    // locked instead by `chat_render_plan_lifecycle_ownership_by_host`
    // (owns_focus && owns_input_lifecycle for every Standalone body state) — the
    // same plan the render pass gates on.
}

/// Test-only public access to `next_reveal_boundary` for cross-module tests.
#[cfg(test)]
pub(crate) mod chat_tests {
    pub fn next_reveal_boundary_pub(text: &str, offset: usize) -> Option<usize> {
        super::next_reveal_boundary(text, offset)
    }
}
