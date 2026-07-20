use super::AgentChatView;
use crate::ai::agent_chat::ui::permission_broker::{
    AgentChatApprovalPreview, AgentChatApprovalRequest,
};
use crate::ai::agent_chat::ui::thread::{
    AgentChatAuthRecovery, AgentChatCallout, AgentChatCalloutSeverity, AgentChatThreadMessage,
    AgentChatThreadMessageRole, AgentChatThreadStatus,
};
use crate::ai::context_selector::types::{ContextSelectorRow, ContextSelectorRowKind};
use gpui::{Modifiers, SharedString};
use std::collections::HashMap;

fn attached_flow(label: &str) -> crate::ai::message_parts::AiContextPart {
    crate::ai::message_parts::AiContextPart::SkillFile {
        path: format!("/tmp/{}.md", label.trim_start_matches('-')),
        label: label.to_string(),
        skill_name: label.trim_start_matches('-').to_string(),
        owner_label: super::slash_and_skills::FLOW_OWNER_LABEL.to_string(),
        slash_name: label.trim_start_matches('-').to_string(),
    }
}

#[test]
fn attached_flow_token_highlights_with_passed_accent() {
    let accent = 0x12_34_56;
    let ranges = AgentChatView::attached_flow_token_highlight_ranges(
        "-agent-chat",
        &[attached_flow(" -agent-chat ")],
        accent,
    );

    assert_eq!(ranges.len(), 1);
    assert_eq!((ranges[0].start, ranges[0].end), (0, 11));
    assert_eq!(ranges[0].color, accent);
}

#[test]
fn attached_flow_token_without_attached_part_stays_plain() {
    assert!(AgentChatView::attached_flow_token_highlight_ranges("-agent-chat", &[], 7).is_empty());
}

#[test]
fn attached_flow_token_requires_whitespace_boundaries() {
    let attached = [attached_flow("-agent-chat")];

    assert!(AgentChatView::attached_flow_token_highlight_ranges(
        "prefix-agent-chat suffix",
        &attached,
        7,
    )
    .is_empty());
    assert!(
        AgentChatView::attached_flow_token_highlight_ranges("re-agent-chat", &attached, 7,)
            .is_empty()
    );
    assert!(
        AgentChatView::attached_flow_token_highlight_ranges("buy milk - urgent", &attached, 7,)
            .is_empty()
    );
}

#[test]
fn attached_flow_token_highlights_two_staged_flows() {
    let ranges = AgentChatView::attached_flow_token_highlight_ranges(
        "-agent-chat then -launcher",
        &[attached_flow("-agent-chat"), attached_flow("-launcher")],
        9,
    );

    assert_eq!(
        ranges
            .iter()
            .map(|range| (range.start, range.end, range.color))
            .collect::<Vec<_>>(),
        vec![(0, 11, 9), (17, 26, 9)]
    );
}

#[test]
fn inactive_transient_lanes_consume_zero_height() {
    assert_eq!(super::agent_chat_transient_lane_height(156.0, false), 0.0);
    assert_eq!(super::agent_chat_transient_lane_height(84.0, true), 84.0);
}

fn retryable_callout() -> AgentChatCallout {
    AgentChatCallout {
        severity: AgentChatCalloutSeverity::Error,
        title: "Turn failed".into(),
        detail: Some("connection lost".into()),
        raw_detail: Some("raw provider error".into()),
        can_retry: true,
        auth_recovery: Some(AgentChatAuthRecovery::AuthenticationRequired),
    }
}

#[test]
fn retryable_error_callout_surfaces_retry_in_footer_contract() {
    let callout = retryable_callout();
    let button = AgentChatView::retry_footer_button(AgentChatThreadStatus::Error, Some(&callout))
        .expect("retryable error callout should add a footer button");
    assert_eq!(button.action, crate::footer_popup::FooterAction::Retry);
    assert_eq!(button.key, "⌘⇧R");
    assert_eq!(button.label, "Retry");
    assert!(button.enabled);
    assert_eq!(
        AgentChatView::footer_button_element_id(button.action, 0),
        "agent_chat-callout-retry",
        "the moved retry footer button should retain the former callout automation id"
    );
    assert!(
        AgentChatView::retry_footer_button(AgentChatThreadStatus::Idle, Some(&callout)).is_none()
    );
}

#[test]
fn active_callout_render_model_is_passive_content_only() {
    let callout = retryable_callout();
    let model = AgentChatView::active_callout_render_model(&callout);

    assert_eq!(model.severity, AgentChatCalloutSeverity::Error);
    assert_eq!(model.title.as_ref(), "Turn failed");
    assert_eq!(model.detail.as_ref(), "connection lost");

    let action_ids = AgentChatView::active_callout_action_specs(Some(&callout))
        .into_iter()
        .map(|action| action.id)
        .collect::<Vec<_>>();
    assert_eq!(
        action_ids,
        vec![
            super::AGENT_CHAT_CALLOUT_SIGN_IN_ACTION_ID,
            super::AGENT_CHAT_CALLOUT_SWITCH_ACCOUNT_ACTION_ID,
            super::AGENT_CHAT_CALLOUT_COPY_ERROR_ACTION_ID,
        ],
        "callout interactions belong to the actions dialog, not the render model"
    );
    assert!(
        AgentChatView::active_callout_action_specs(None).is_empty(),
        "recovery rows must not appear without an active callout"
    );
}

fn cmd_modifiers() -> Modifiers {
    Modifiers {
        platform: true,
        ..Default::default()
    }
}

fn cmd_shift_modifiers() -> Modifiers {
    Modifiers {
        platform: true,
        shift: true,
        ..Default::default()
    }
}

#[test]
fn variation_tab_cycle_wraps_and_handles_empty() {
    // No cards → no selection to make.
    assert_eq!(AgentChatView::next_variation_index_wrapping(None, 0), None);
    // First Tab lands on the first card.
    assert_eq!(
        AgentChatView::next_variation_index_wrapping(None, 3),
        Some(0)
    );
    // Tab advances…
    assert_eq!(
        AgentChatView::next_variation_index_wrapping(Some(0), 3),
        Some(1)
    );
    // …and wraps from the last card back to the first.
    assert_eq!(
        AgentChatView::next_variation_index_wrapping(Some(2), 3),
        Some(0)
    );
    // Stale out-of-range selection resets to the first card.
    assert_eq!(
        AgentChatView::next_variation_index_wrapping(Some(7), 3),
        Some(0)
    );
}

#[test]
fn thread_summary_title_uses_first_line_truncated() {
    assert_eq!(
        AgentChatView::thread_summary_title("Refactor the parser\nwith details"),
        "Refactor the parser"
    );
    assert_eq!(AgentChatView::thread_summary_title("   \n"), "New Thread");
    let long = "x".repeat(80);
    let title = AgentChatView::thread_summary_title(&long);
    assert_eq!(title.chars().count(), 49, "48 chars + ellipsis");
    assert!(title.ends_with('…'));
}

#[test]
fn mention_picker_width_respects_window_gutters() {
    let width = AgentChatView::composer_picker_width_for_window(240.0);
    assert_eq!(
        width, 216.0,
        "picker width should shrink to fit within the window gutters"
    );
}

#[test]
fn mention_picker_width_caps_at_design_width() {
    let width = AgentChatView::composer_picker_width_for_window(1200.0);
    assert_eq!(
        width,
        AgentChatView::AGENT_CHAT_COMPOSER_PICKER_WIDTH,
        "wide windows should keep the canonical picker width"
    );
}

#[test]
fn mention_picker_left_clamps_to_visible_right_edge() {
    let left = AgentChatView::clamp_composer_picker_left(640.0, 320.0, 800.0);
    assert_eq!(
        left, 468.0,
        "picker should shift left so its right edge stays onscreen"
    );
}

#[test]
fn mention_picker_left_never_moves_past_left_padding() {
    let left = AgentChatView::clamp_composer_picker_left(-30.0, 320.0, 800.0);
    assert_eq!(
        left,
        AgentChatView::AGENT_CHAT_INPUT_PADDING_X,
        "picker should stay aligned to the input gutter when the anchor is too far left"
    );
}

#[test]
fn caret_after_replacement_tracks_inserted_token_not_end_of_composer() {
    let range = 6..10;
    let replacement = "@snapshot ";
    assert_eq!(
        AgentChatView::caret_after_replacement(&range, replacement),
        16,
        "caret should land immediately after the accepted token"
    );
}

#[test]
fn replace_text_in_char_range_preserves_surrounding_text() {
    let updated = AgentChatView::replace_text_in_char_range("hello @con", 6..10, "@snapshot ");
    assert_eq!(updated, "hello @snapshot ");
}

#[test]
fn text_in_char_range_extracts_original_trigger_token() {
    let original = AgentChatView::text_in_char_range("review @fi later", 7..10);
    assert_eq!(original, "@fi");
}

#[test]
fn hint_prefix_replacement_preserves_deliberate_trailing_space() {
    let (updated, cursor) =
        AgentChatView::replace_active_trigger_or_insert_at_cursor("/he", 3, "/help ");
    assert_eq!(updated, "/help ");
    assert_eq!(
        cursor, 6,
        "cursor should land after the preserved trailing space"
    );
}

#[test]
fn cwd_footer_prefix_insert_opens_cwd_sigil_at_cursor() {
    let (updated, cursor) =
        AgentChatView::replace_active_trigger_or_insert_at_cursor("review files", 6, ">");
    assert_eq!(updated, "review > files");
    assert_eq!(cursor, 8);
}

#[test]
fn composer_is_active_requires_focus_and_no_actions_window() {
    assert!(AgentChatView::composer_is_active(true, true, false));
    assert!(!AgentChatView::composer_is_active(true, false, false));
    assert!(!AgentChatView::composer_is_active(false, true, false));
    assert!(!AgentChatView::composer_is_active(true, true, true));
}

#[test]
fn agent_chat_spine_accepts_colon_list_filter_projection() {
    let projection =
        crate::spine::input_projection::project_text_at_char_cursor(":type:script", 12);
    let Some(kind) = projection
        .projection
        .as_ref()
        .map(|projection| &projection.active_segment_kind)
    else {
        panic!("expected ':' to produce an active Spine projection");
    };

    assert!(matches!(
        kind,
        crate::spine::SpineSegmentKind::ListFilter { .. }
    ));
    assert!(
        crate::spine::input_projection::projection_owns_prompt_builder_list(
            projection.projection.as_ref(),
            &projection.parse,
        )
    );
    assert!(
        AgentChatView::agent_chat_spine_segment_kind_has_context_projection(kind),
        "Agent Chat should not filter ':' out before rendering shared Spine list sections"
    );
}

#[test]
fn profile_spine_acceptance_targets_profile_callback_and_clears_trigger() {
    let action = crate::spine::list::SpineListAction::ResolveSegment {
        segment_index: 3,
        segment_byte_range: 7..26,
        replacement: SharedString::from("|nn29-switch-target"),
        resolution_id: SharedString::from("nn29-switch-target"),
        resolution_label: SharedString::from("NN29 Switch Target"),
        resolution_source: SharedString::from("profile"),
        trailing_space: true,
    };

    let effect = AgentChatView::profile_spine_acceptance_effect(&action)
        .expect("profile ResolveSegment must route through profile selection acceptance");

    assert_eq!(effect.profile_id, "nn29-switch-target");
    assert_eq!(effect.segment_index, 3);
    assert_eq!(effect.segment_byte_range, 7..26);
    assert_eq!(effect.replacement, "");
    assert!(!effect.trailing_space);
}

#[test]
fn profile_spine_acceptance_preserves_preceding_draft_exactly() {
    const DRAFT: &str = "NN29 profile accept draft";
    let draft_with_trigger = format!("{DRAFT} |text");
    let trigger_start = draft_with_trigger
        .rfind('|')
        .expect("fixture must contain the profile trigger");
    let acceptance_range = AgentChatView::profile_spine_acceptance_range(
        &draft_with_trigger,
        trigger_start..draft_with_trigger.len(),
    );
    let mut after_acceptance = draft_with_trigger;
    after_acceptance.replace_range(acceptance_range, "");

    assert_eq!(
        after_acceptance, DRAFT,
        "acceptance must strip only the profile trigger and its separator"
    );
    assert_eq!(
        AgentChatView::profile_spine_trigger_input(DRAFT),
        format!("{DRAFT} |"),
        "opening the profile Spine must append its trigger without replacing the draft"
    );
}

#[test]
fn permission_request_matches_tool_message_by_tool_call_id() {
    let (reply_tx, _reply_rx) = async_channel::bounded(1);
    let request = AgentChatApprovalRequest {
        id: 1,
        title: "Agent Chat permission request".into(),
        body: String::new(),
        preview: Some(AgentChatApprovalPreview::new("write_text_file", "tc-123")),
        options: vec![],
        reply_tx,
    };
    let msg = AgentChatThreadMessage {
        id: 9,
        role: AgentChatThreadMessageRole::Tool,
        body: "Write file\nrunning".into(),
        tool_call_id: Some("tc-123".to_string()),
        tool_meta: None,
        attachments: Vec::new(),
    };

    assert!(AgentChatView::permission_request_matches_message(
        &msg, &request
    ));
}

#[test]
fn telemetry_item_id_redacts_local_paths() {
    let file_item = ContextSelectorRow {
        id: SharedString::from("file:/tmp/secrets.txt"),
        label: SharedString::from("secrets.txt"),
        description: SharedString::from("/tmp/secrets.txt"),
        meta: SharedString::from("@file:/tmp/secrets.txt"),
        kind: ContextSelectorRowKind::File(std::path::PathBuf::from("/tmp/secrets.txt")),
        score: 100,
        label_highlight_indices: Vec::new(),
        meta_highlight_indices: Vec::new(),
    };
    let folder_item = ContextSelectorRow {
        id: SharedString::from("folder:/Users/john/Documents"),
        label: SharedString::from("Documents"),
        description: SharedString::from("/Users/john/Documents"),
        meta: SharedString::from("@file:/Users/john/Documents"),
        kind: ContextSelectorRowKind::Folder(std::path::PathBuf::from("/Users/john/Documents")),
        score: 100,
        label_highlight_indices: Vec::new(),
        meta_highlight_indices: Vec::new(),
    };

    assert_eq!(
        AgentChatView::telemetry_item_id(&file_item),
        "file:secrets.txt"
    );
    assert_eq!(
        AgentChatView::telemetry_item_id(&folder_item),
        "folder:Documents"
    );
}

#[test]
fn focused_inline_token_prefers_preview_for_resolved_builtin_mention() {
    let text = "Review @clipboard now";
    let cursor = "Review @clipboard".chars().count();

    assert!(AgentChatView::focused_inline_token_prefers_preview(
        text,
        cursor,
        &HashMap::new(),
    ));
}

#[test]
fn focused_inline_token_prefers_preview_for_typed_portal_token() {
    let text = "Review @note:\"Daily Standup\" soon";
    let cursor = "Review @note:\"Daily Standup\"".chars().count();

    assert!(AgentChatView::focused_inline_token_prefers_preview(
        text,
        cursor,
        &HashMap::new(),
    ));
}

#[test]
fn focused_inline_token_prefers_preview_ignores_in_progress_query() {
    let text = "Review @clip";
    let cursor = text.chars().count();

    assert!(!AgentChatView::focused_inline_token_prefers_preview(
        text,
        cursor,
        &HashMap::new(),
    ));
}

#[test]
fn reopen_focused_mention_shortcut_accepts_cmd_period_and_cmd_shift_o() {
    assert!(AgentChatView::is_reopen_focused_mention_shortcut(
        "period",
        &cmd_modifiers(),
    ));
    assert!(AgentChatView::is_reopen_focused_mention_shortcut(
        "o",
        &cmd_shift_modifiers(),
    ));
    assert!(!AgentChatView::is_reopen_focused_mention_shortcut(
        "o",
        &cmd_modifiers(),
    ));
}

#[test]
fn portal_target_from_inline_token_supports_dictation_portal_tokens() {
    use crate::ai::context_selector::types::ContextPortalKind;

    assert_eq!(
        crate::ai::agent_chat::ui::portal_contract::portal_target_from_inline_token("@dictation"),
        Some((ContextPortalKind::DictationHistory, String::new()))
    );

    assert_eq!(
        crate::ai::agent_chat::ui::portal_contract::portal_target_from_inline_token(
            "@dictation:entry-123",
        ),
        Some((ContextPortalKind::DictationHistory, "entry-123".to_string()))
    );
}

#[test]
fn picker_portal_query_clears_in_progress_dictation_picker_text() {
    use crate::ai::context_selector::types::ContextPortalKind;

    assert_eq!(
        crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
            ContextPortalKind::DictationHistory,
            "di",
        ),
        ""
    );
}

#[test]
fn picker_portal_query_preserves_non_dictation_portal_text() {
    use crate::ai::context_selector::types::ContextPortalKind;

    assert_eq!(
        crate::ai::agent_chat::ui::portal_contract::picker_portal_query(
            ContextPortalKind::BrowserHistory,
            "bro"
        ),
        "bro"
    );
}

// ── ScriptReadyReceipt parsing tests ──

#[test]
fn parse_script_ready_receipt_valid() {
    let text = "Some output\nSCRIPT_READY path=/foo/bar.ts validated=true";
    let receipt = super::parse_script_ready_receipt(text).unwrap();
    assert_eq!(receipt.path, std::path::PathBuf::from("/foo/bar.ts"));
    assert!(receipt.validated);
}

#[test]
fn parse_script_ready_receipt_not_validated() {
    let text = "SCRIPT_READY path=/foo/bar.ts validated=false";
    let receipt = super::parse_script_ready_receipt(text).unwrap();
    assert_eq!(receipt.path, std::path::PathBuf::from("/foo/bar.ts"));
    assert!(!receipt.validated);
}

#[test]
fn parse_script_ready_receipt_no_match() {
    let text = "Some random output\nNo receipt here.";
    assert!(super::parse_script_ready_receipt(text).is_none());
}

#[test]
fn parse_script_ready_receipt_missing_path() {
    let text = "SCRIPT_READY validated=true";
    assert!(super::parse_script_ready_receipt(text).is_none());
}

#[test]
fn parse_script_ready_receipt_uses_last_occurrence() {
    let text = "SCRIPT_READY path=/old.ts validated=true\nMore text\nSCRIPT_READY path=/new.ts validated=true";
    let receipt = super::parse_script_ready_receipt(text).unwrap();
    assert_eq!(receipt.path, std::path::PathBuf::from("/new.ts"));
}

#[test]
fn parse_script_ready_receipt_with_home_tilde() {
    let text = "Validation passed.\nSCRIPT_READY path=~/.scriptkit/plugins/main/scripts/clipboard-cleanup.ts validated=true";
    let receipt = super::parse_script_ready_receipt(text).unwrap();
    assert_eq!(
        receipt.path,
        std::path::PathBuf::from("~/.scriptkit/plugins/main/scripts/clipboard-cleanup.ts")
    );
    assert!(receipt.validated);
}

// ── Focused-text variation state tests ──────────────────────────

use crate::ai::focused_text::FocusedTextPromptAngle;

#[test]
fn variation_streaming_starts_with_streaming_status() {
    let state = super::FocusedTextVariationState::streaming(FocusedTextPromptAngle::Conservative);
    assert_eq!(state.status, super::FocusedTextVariationStatus::Streaming);
    assert!(state.text.is_empty());
    assert!(state.error.is_none());
}

#[test]
fn variation_status_state_ids_are_distinct() {
    let ids = [
        super::FocusedTextVariationStatus::Idle.state_id(),
        super::FocusedTextVariationStatus::Streaming.state_id(),
        super::FocusedTextVariationStatus::Complete.state_id(),
        super::FocusedTextVariationStatus::Error.state_id(),
    ];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "state_ids must be unique");
        }
    }
}

#[test]
fn mini_phase_state_ids_are_distinct() {
    let ids = [
        super::FocusedTextMiniPhase::InputOnly.state_id(),
        super::FocusedTextMiniPhase::Loading.state_id(),
        super::FocusedTextMiniPhase::Streaming.state_id(),
        super::FocusedTextMiniPhase::Result.state_id(),
        super::FocusedTextMiniPhase::Error.state_id(),
    ];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "phase state_ids must be unique");
        }
    }
}

#[test]
fn focused_text_mini_layout_budget_reserves_native_footer_without_overlap() {
    let footer_height = crate::components::footer_chrome::current_main_menu_footer_height();
    let budget = super::focused_text_mini_layout_budget(152.0, false, footer_height);

    assert_eq!(budget.content_height + budget.footer_height, 152.0);
    assert_eq!(
        budget.input_height,
        crate::window_resize::focused_text_mini_input_height()
    );
    assert_eq!(
        budget.result_y + budget.result_height,
        budget.content_height
    );
    assert!(budget.result_height > 0.0);
}

#[test]
fn focused_text_mini_input_only_budget_never_invents_canonical_header_space() {
    let input_height = crate::window_resize::focused_text_mini_input_height();
    let budget = super::focused_text_mini_layout_budget(input_height, false, 0.0);

    assert_eq!(budget.input_height, input_height);
    assert_eq!(budget.scope_height, 0.0);
    assert_eq!(budget.result_height, 0.0);
}

#[test]
fn variation_angles_returns_three_distinct_angles() {
    let angles = AgentChatView::focused_text_variation_angles();
    assert_eq!(angles.len(), 3);
    assert_eq!(angles[0].id(), "conservative");
    assert_eq!(angles[1].id(), "balanced");
    assert_eq!(angles[2].id(), "creative");
}

#[test]
fn variation_snapshot_preserves_state() {
    let mut state = super::FocusedTextVariationState::streaming(FocusedTextPromptAngle::Balanced);
    state.text = "Hello world".to_string();
    state.status = super::FocusedTextVariationStatus::Complete;
    let snapshot = state.snapshot(1, true);
    assert_eq!(snapshot.text, "Hello world");
    assert_eq!(snapshot.status, super::FocusedTextVariationStatus::Complete);
    assert_eq!(snapshot.angle_id, "balanced");
    assert!(snapshot.selected);
}

#[test]
fn focused_text_context_status_user_messages_cover_all_known_codes() {
    let codes_and_expected = [
        ("accessibilityPermissionRequired", "Accessibility"),
        ("secureField", "secure field"),
        ("unsupportedTarget", "Unable to grab text"),
        ("staleSession", "session expired"),
        ("platform", "system error"),
    ];
    for (code, substring) in codes_and_expected {
        let status = super::FocusedTextContextStatus::CaptureFailed { reason_code: code };
        let msg = status.user_message().unwrap_or("");
        assert!(
            msg.contains(substring),
            "code {code:?} message should contain {substring:?}, got: {msg:?}"
        );
    }
}

#[test]
fn unknown_capture_reason_code_has_generic_message() {
    let status = super::FocusedTextContextStatus::CaptureFailed {
        reason_code: "unknown_future_code",
    };
    let msg = status.user_message().unwrap_or("");
    assert!(msg.contains("Unable to grab text"));
}
