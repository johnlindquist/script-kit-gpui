const TRANSCRIPT_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/components/transcript.rs");
const VIEW_SOURCE: &str = include_str!("../src/ai/agent_chat/ui/view.rs");
const MAIN_VIEW_CHROME_SOURCE: &str = include_str!("../src/components/main_view_chrome.rs");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing start marker: {}", start));
    let source = &source[start_index..];
    let end_index = source
        .find(end)
        .unwrap_or_else(|| panic!("missing end marker after {}: {}", start, end));
    &source[..end_index]
}

#[test]
fn transcript_list_state_starts_with_existing_messages() {
    let body = source_between(TRANSCRIPT_SOURCE, "pub fn new(", "\n    pub fn list_state(");

    assert!(
        body.contains("let total = messages.len();"),
        "AgentChatTranscript::new must size the virtual list from existing messages"
    );
    assert!(
        body.contains("ListState::new(total, ListAlignment::Bottom"),
        "AgentChatTranscript::new must not mount an already-populated thread with a zero-row list"
    );
    assert!(
        !body.contains("ListState::new(0, ListAlignment::Bottom"),
        "zero-row transcript list initialization hides pre-existing Agent Chat messages"
    );
}

#[test]
fn streaming_activity_row_is_a_single_idempotent_tail_row() {
    // Decision (2026-06-10, supersedes the footer-only rule): while a turn is
    // streaming with no assistant text yet, the transcript renders one
    // synthetic "Thinking…" tail row so submit gives immediate visible
    // feedback. The churn-safety invariants that motivated the old rule are
    // kept: the setter must be idempotent (no reset/notify when unchanged)
    // and the list row count must only change through row_count().
    let setter_body = source_between(
        TRANSCRIPT_SOURCE,
        "pub fn set_show_activity_row(",
        "\n    pub fn toggle_collapsed(",
    );
    assert!(
        setter_body.contains("if self.show_activity_row == show")
            && setter_body.contains("return;"),
        "set_show_activity_row must early-return when the flag is unchanged to avoid reset/notify churn"
    );
    assert!(
        setter_body.contains("self.list_state.reset(self.row_count())"),
        "set_show_activity_row must resize the virtual list via row_count() so the tail row is reachable"
    );
    assert!(
        TRANSCRIPT_SOURCE.contains("fn render_activity_row(")
            && TRANSCRIPT_SOURCE.contains("ix == visible_indices.len()"),
        "the activity row must render as the single tail row after all message rows"
    );
    assert!(
        !TRANSCRIPT_SOURCE.contains("Working..."),
        "the transcript activity row must not duplicate the footer's Working... status text"
    );
}

#[test]
fn footer_snapshot_carries_streaming_status_next_to_model_name() {
    assert!(
        VIEW_SOURCE.contains("pub(crate) status_text: Option<&'static str>")
            && VIEW_SOURCE.contains("pub(crate) fn model_status_label(&self) -> String")
            && VIEW_SOURCE.contains("format!(\"{} · {}\", self.model_display, status)")
            && VIEW_SOURCE.contains("AgentChatThreadStatus::Streaming => Some(\"Working...\")"),
        "Agent Chat footer snapshot must carry status text for the footer model label"
    );
}

#[test]
fn transcript_render_does_not_reset_list_state_each_frame() {
    let body = source_between(
        TRANSCRIPT_SOURCE,
        "impl Render for AgentChatTranscript",
        "\n}",
    );

    assert!(
        !body.contains("self.list_state.reset("),
        "AgentChatTranscript render must not mutate the virtual list row count every frame"
    );
    assert!(
        body.contains(".relative()")
            && body.contains(".flex_1()")
            && body.contains(".overflow_hidden()"),
        "AgentChatTranscript render must preserve the virtual-list viewport wrapper"
    );
    assert!(
        body.contains(".size_full()")
            && body.contains(".with_sizing_behavior(ListSizingBehavior::Auto)")
            && body.contains(".vertical_scrollbar(&self.list_state)"),
        "AgentChatTranscript render must size the virtualized list and keep transcript scrolling wired"
    );
}

#[test]
fn main_view_main_slot_is_a_flex_column_viewport() {
    let body = source_between(
        MAIN_VIEW_CHROME_SOURCE,
        "pub(crate) fn render_main_view_main_slot(",
        "\n}\n\npub(crate) fn main_view_input_text_inset_left",
    );

    assert!(
        body.contains(".flex_1()")
            && body.contains(".min_h(px(0.))")
            && body.contains(".w_full()")
            && body.contains(".overflow_hidden()"),
        "MainViewMain must remain a bounded viewport"
    );
    assert!(
        body.contains(".flex()") && body.contains(".flex_col()"),
        "MainViewMain must be a flex column so Agent Chat transcript descendants receive real height"
    );

    let flex = body.find(".flex()").expect("missing flex");
    let child = body.find(".child(main)").expect("missing child(main)");
    assert!(
        flex < child,
        "MainViewMain must become a flex container before mounting the Agent Chat body"
    );
}

#[test]
fn agent_chat_middle_area_is_a_bounded_transcript_viewport() {
    let body = source_between(
        VIEW_SOURCE,
        "fn render_agent_chat_middle_area(",
        "\n    pub(crate) fn open_profile_picker(",
    );

    assert!(
        body.contains(".child(self.ensure_transcript(cx).into_any_element())"),
        "Agent Chat middle area must mount the transcript"
    );
    assert!(
        body.contains(".h_full()")
            && body.contains(".overflow_hidden()")
            && body.contains(".flex()")
            && body.contains(".flex_col()"),
        "Agent Chat middle area must provide a real flex viewport for the virtualized transcript"
    );
}

#[test]
fn transcript_message_sync_is_idempotent() {
    let helper_body = source_between(
        TRANSCRIPT_SOURCE,
        "fn messages_match_current(",
        "\n    pub fn set_messages(",
    );
    let setter_body = source_between(
        TRANSCRIPT_SOURCE,
        "pub fn set_messages(",
        "\n    pub fn set_show_activity_row(",
    );

    assert!(
        helper_body.contains("current.id == incoming.id")
            && helper_body.contains("current.role == incoming.role")
            && helper_body.contains("current.body == incoming.body")
            && helper_body.contains("current.tool_call_id == incoming.tool_call_id"),
        "AgentChatTranscript message sync must compare the rendered message signature"
    );
    assert!(
        setter_body.contains("if self.messages_match_current(&messages)")
            && setter_body.contains("return;"),
        "AgentChatTranscript::set_messages must avoid notify/reset churn when messages are unchanged"
    );
}

#[test]
fn transcript_heavy_markdown_preview_covers_link_dense_user_prompts() {
    let stats_body = source_between(
        TRANSCRIPT_SOURCE,
        "impl HeavyMarkdownStats",
        "\npub struct AgentChatTranscript",
    );
    let preview_body = source_between(
        TRANSCRIPT_SOURCE,
        "fn should_use_heavy_markdown_preview(",
        "\n    fn heavy_markdown_preview_text(",
    );

    assert!(
        TRANSCRIPT_SOURCE.contains("link_like_spans")
            && stats_body.contains("count_link_like_spans(trimmed)")
            && stats_body.contains("self.link_like_spans >= 8")
            && stats_body.contains("self.link_like_spans >= 14"),
        "link-dense Brain prompts must qualify for the heavy markdown preview before they become large by bytes/lines alone"
    );
    assert!(
        preview_body.contains("AgentChatThreadMessageRole::User")
            && preview_body.contains("AgentChatThreadMessageRole::Assistant"),
        "heavy markdown preview must cover user prompts as well as assistant responses"
    );
}

#[test]
fn agent_chat_mounts_transcript_from_existing_thread_messages() {
    let ensure_body = source_between(
        VIEW_SOURCE,
        "fn ensure_transcript(",
        "\n    fn confirm_setup_agent_selection(",
    );
    let middle_area_body = source_between(
        VIEW_SOURCE,
        "fn render_agent_chat_middle_area(",
        "\n    pub(crate) fn open_profile_picker(",
    );
    let render_body = source_between(
        VIEW_SOURCE,
        "impl Render for AgentChatView",
        "\n#[cfg(test)]",
    );

    assert!(
        ensure_body.contains("thread_ref.messages.clone()"),
        "Agent Chat must seed the transcript from already-available thread messages"
    );
    assert!(
        ensure_body.contains("AgentChatTranscript::new(messages, cx)"),
        "Agent Chat must pass existing messages into the transcript entity"
    );
    assert!(
        ensure_body.contains("transcript.set_messages(messages, cx)"),
        "Agent Chat must keep an existing transcript entity synced with live thread messages"
    );
    assert!(
        middle_area_body.contains(".child(self.ensure_transcript(cx).into_any_element())")
            && render_body.contains("self.render_agent_chat_middle_area("),
        "Agent Chat must mount the transcript through the middle-area render path even when assistant text already exists"
    );
}
